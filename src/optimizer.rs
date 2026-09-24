//! Production optimization algorithms for the Aniimo optimizer.
//!
//! This module contains the core optimization logic that calculates
//! production efficiencies and finds the best production paths to
//! achieve currency goals.

use std::collections::{HashMap, HashSet};

use microlp::{ComparisonOp, OptimizationDirection, Problem};

use crate::models::{
    EnergyItemEfficiency, EnvironmentAssignment, FacilityPlacement, GoalResult, PlanProduct, PlanStep,
    PlanStepStatus, ProductionPlan, FacilityCounts, ModuleLevels, ProductionEfficiency, ProductionItem,
    ProductionPath, ProductionStep, SeedRequirement,
};

/// Calculates the optimal allocation of facilities to minimize production time
/// when producing multiple different materials.
/// 
/// Given:
/// - `materials`: Vec of (material_name, batches_needed, time_per_batch)
/// - `total_facilities`: Total number of facilities available
/// 
/// Returns: Vec of (material_name, batches_needed, optimal_facilities_to_allocate)
/// 
/// Uses binary search on the answer for O(M * sqrt(B) * log(M * sqrt(B))) complexity,
/// where M = number of materials, B = max batches.
fn calculate_optimal_facility_allocation(
    materials: &[(String, u32, f64)],
    total_facilities: u32,
) -> Vec<(String, u32, u32)> {
    if materials.is_empty() {
        return vec![];
    }
    
    if total_facilities == 0 {
        return materials.iter()
            .map(|(name, batches, _)| (name.clone(), *batches, 0))
            .collect();
    }
    
    if materials.len() == 1 {
        // Single material gets all facilities
        return vec![(materials[0].0.clone(), materials[0].1, total_facilities)];
    }
    
    // Filter to only materials that need production (batches > 0 and time > 0)
    let active_materials: Vec<(usize, u32, f64)> = materials.iter()
        .enumerate()
        .filter(|(_, (_, batches, time))| *batches > 0 && *time > 0.0)
        .map(|(i, (_, batches, time))| (i, *batches, *time))
        .collect();
    
    if active_materials.is_empty() {
        // No active materials - just return with 0 allocations
        return materials.iter()
            .map(|(name, batches, _)| (name.clone(), *batches, 0))
            .collect();
    }
    
    // Check if we have enough facilities (at least 1 per active material)
    if (active_materials.len() as u32) > total_facilities {
        // Not enough - distribute proportionally
        return distribute_proportionally(materials, total_facilities);
    }
    
    // Collect all possible completion times using sqrt decomposition
    // For ceil(b/k) where k goes from 1 to b, there are only O(sqrt(b)) distinct values
    let mut candidate_times: Vec<f64> = Vec::new();
    for (_, batches, time) in &active_materials {
        if *batches == 0 || *time <= 0.0 {
            continue;
        }
        let mut k = 1u32;
        while k <= *batches {
            let rounds = (*batches).div_ceil(k); // ceil(batches / k)
            candidate_times.push(rounds as f64 * time);
            // Jump to next k that gives a different ceil value
            if rounds > 1 {
                k = *batches / (rounds - 1);
                if k * (rounds - 1) < *batches {
                    k += 1;
                }
            } else {
                break;
            }
        }
        // Also add the case where we use all facilities for this material
        if total_facilities > 0 {
            candidate_times.push((*batches).div_ceil(total_facilities) as f64 * time);
        }
    }
    
    // Handle empty candidates
    if candidate_times.is_empty() {
        return materials.iter()
            .enumerate()
            .map(|(i, (name, batches, _))| {
                let alloc = if i < active_materials.len() { 1 } else { 0 };
                (name.clone(), *batches, alloc)
            })
            .collect();
    }
    
    // Sort and deduplicate
    candidate_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    candidate_times.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    
    // Binary search for minimum feasible time
    let optimal_time = binary_search_min_time(&candidate_times, &active_materials, total_facilities);
    
    // Calculate the allocation for this optimal time
    calculate_allocation_for_time(materials, &active_materials, optimal_time, total_facilities)
}

/// Binary search to find the minimum feasible completion time
fn binary_search_min_time(
    candidate_times: &[f64],
    active_materials: &[(usize, u32, f64)],
    total_facilities: u32,
) -> f64 {
    if candidate_times.is_empty() {
        return 0.0;
    }
    
    let mut lo = 0;
    let mut hi = candidate_times.len();
    
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if is_time_feasible(candidate_times[mid], active_materials, total_facilities) {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    
    if lo < candidate_times.len() {
        candidate_times[lo]
    } else {
        // Fallback - use the largest candidate time
        candidate_times.last().copied().unwrap_or(0.0)
    }
}

/// Check if a target completion time is achievable with the given facilities
fn is_time_feasible(
    target_time: f64,
    active_materials: &[(usize, u32, f64)],
    total_facilities: u32,
) -> bool {
    let mut facilities_needed = 0u32;
    
    for (_, batches, time) in active_materials {
        if *time <= 0.0 {
            continue;
        }
        
        let max_rounds = (target_time / time).floor() as u32;
        if max_rounds == 0 {
            return false; // Can't complete even one round in time
        }
        
        // Minimum facilities needed: ceil(batches / max_rounds)
        let min_facilities = (*batches).div_ceil(max_rounds);
        facilities_needed = facilities_needed.saturating_add(min_facilities);
        
        if facilities_needed > total_facilities {
            return false;
        }
    }
    
    true
}

/// Calculate the actual facility allocation for a given target time
fn calculate_allocation_for_time(
    materials: &[(String, u32, f64)],
    active_materials: &[(usize, u32, f64)],
    target_time: f64,
    total_facilities: u32,
) -> Vec<(String, u32, u32)> {
    let mut result: Vec<(String, u32, u32)> = materials.iter()
        .map(|(name, batches, _)| (name.clone(), *batches, 0))
        .collect();
    
    // Calculate minimum facilities needed for each active material
    let mut allocations: Vec<(usize, u32)> = Vec::new();
    let mut total_min = 0u32;
    
    for (idx, batches, time) in active_materials {
        if *time <= 0.0 {
            allocations.push((*idx, 1));
            total_min += 1;
            continue;
        }
        
        let max_rounds = (target_time / time).floor() as u32;
        let min_facilities = if max_rounds == 0 {
            *batches // Need one facility per batch (shouldn't happen if time is feasible)
        } else {
            (*batches).div_ceil(max_rounds)
        };
        
        allocations.push((*idx, min_facilities));
        total_min += min_facilities;
    }
    
    // Distribute remaining facilities to reduce time further where possible
    let mut remaining = total_facilities.saturating_sub(total_min);
    
    // Assign minimum allocations first
    for (idx, min_fac) in &allocations {
        result[*idx].2 = *min_fac;
    }
    
    // Distribute remaining facilities greedily - give to the material that benefits most
    while remaining > 0 {
        let mut best_improvement = 0.0f64;
        let mut best_idx = None;
        
        for (idx, batches, time) in active_materials {
            let current_facilities = result[*idx].2;
            if current_facilities == 0 {
                continue;
            }
            
            let current_rounds = (*batches).div_ceil(current_facilities);
            let new_rounds = (*batches + current_facilities) / (current_facilities + 1);
            
            if new_rounds < current_rounds {
                let improvement = (current_rounds - new_rounds) as f64 * time;
                if improvement > best_improvement {
                    best_improvement = improvement;
                    best_idx = Some(*idx);
                }
            }
        }
        
        if let Some(idx) = best_idx {
            result[idx].2 += 1;
            remaining -= 1;
        } else {
            // No improvement possible, distribute to first active material
            if let Some((idx, _, _)) = active_materials.first() {
                result[*idx].2 += remaining;
            }
            break;
        }
    }
    
    result
}

/// Distribute facilities proportionally when there aren't enough
fn distribute_proportionally(
    materials: &[(String, u32, f64)],
    total_facilities: u32,
) -> Vec<(String, u32, u32)> {
    let total_batches: u32 = materials.iter().map(|(_, b, _)| b).sum();
    
    if total_batches == 0 {
        return materials.iter()
            .map(|(name, batches, _)| (name.clone(), *batches, 0))
            .collect();
    }
    
    let mut result: Vec<(String, u32, u32)> = Vec::with_capacity(materials.len());
    let mut remaining = total_facilities;
    
    for (i, (name, batches, _)) in materials.iter().enumerate() {
        let alloc = if i == materials.len() - 1 {
            remaining
        } else if *batches > 0 {
            let frac = (*batches as f64 / total_batches as f64 * total_facilities as f64).round() as u32;
            frac.min(remaining).max(1)
        } else {
            0
        };
        result.push((name.clone(), *batches, alloc));
        remaining = remaining.saturating_sub(alloc);
    }
    
    result
}

/// Result of calculating production requirements for an item
#[derive(Debug, Clone)]
struct ProductionRequirements {
    /// Total time to produce the item (including all dependencies)
    total_time: f64,
    /// Total energy consumed (including all dependencies)
    total_energy: Option<f64>,
    /// Total cost of raw materials
    total_cost: f64,
    /// Names of all raw materials in the chain
    raw_names: Vec<String>,
    /// Primary facility for the base raw material
    primary_facility: Option<String>,
    /// All facilities used in this production chain (for conflict detection)
    all_facilities: HashSet<String>,
    /// Intermediate processing steps: (item_name, facility, amount_per_parent_batch)
    intermediate_steps: Vec<(String, String, u32)>,
    /// Whether this production chain is valid
    is_valid: bool,
}

/// Resolves a raw material name to the actual item that would supply it, preferring the quick_
/// variant when it exists and is usable; same substitution rule used everywhere else raw
/// material names are resolved. Keys the item map by owned `String` since that's what
/// `calculate_efficiencies` and the resource-demand functions below use.
fn resolve_raw_material<'a>(
    name: &str,
    item_map: &HashMap<String, &'a ProductionItem>,
    facility_counts: &FacilityCounts,
    module_levels: &ModuleLevels,
) -> Option<&'a ProductionItem> {
    let high_speed_name = format!("quick_{}", name);
    if let Some(&hs) = item_map.get(&high_speed_name) {
        let can_use = hs
            .module_requirement
            .as_ref()
            .is_none_or(|(m, l)| module_levels.can_use(m, *l));
        let can_produce = facility_counts.can_produce(&hs.facility, hs.facility_level);
        if can_use && can_produce {
            return Some(hs);
        }
    }
    item_map.get(name).copied()
}

/// Walks `item`'s full ingredient tree, accumulating; per FACILITY touched anywhere in the
/// tree, including `item`'s own facility; total *utilization*: batches/sec of whatever runs
/// there, weighted by that item's own `production_time`, required per one batch/sec of the
/// tree's root. Utilization (a dimensionless "fraction of one facility's continuous operation")
/// is what makes sharing correct in every shape it comes in, because it's always additive:
///
/// - The same item needed via two different branches; soy_sauce_tofu needs both soy_sauce
///   (Bouncy Brew Keg) and tofu (Carousel Mill), and both independently need soybean from the
///   same Farmland. Computing each branch's rate in isolation (the old approach) let each assume
///   it alone could draw all 20 Farmland's worth of soybean, silently doubling the effective
///   rate. Here both branches' soybean utilization lands in the same `Farmland` entry and sums.
/// - Two DIFFERENT items hosted at the same facility for the same chain; e.g. Claw Game Cooker
///   both turning sugarcane into rock_candy AND assembling rock_candy+tofu into tofu_cake, or
///   Woodland growing both coconut and lemon for a soap recipe. These aren't the same item, but
///   they're still time-sharing one facility's capacity, so their utilization sums too.
///
/// Keying by facility (not item name) is what makes both cases fall out of one formula instead
/// of needing separate handling.
fn accumulate_demand<'a>(
    item: &'a ProductionItem,
    ratio: f64,
    item_map: &HashMap<String, &'a ProductionItem>,
    facility_counts: &FacilityCounts,
    module_levels: &ModuleLevels,
    demand: &mut HashMap<(&'a str, &'a str), f64>,
    depth: u32,
) {
    if depth > 8 {
        return; // guard against unexpected circular references
    }
    // Keyed by (facility, item) rather than facility alone; see `ProductionEfficiency`'s
    // `facility_demand` doc comment for why per-item granularity matters (a facility can grow/host
    // several distinct items for one chain, each needing its own separate accounting rather than
    // one blurred-together total). The HashMap key itself does the "same item visited via multiple
    // branches" dedup-and-sum that an explicit `Vec` + manual scan used to do.
    *demand.entry((item.facility.as_str(), item.name.as_str())).or_insert(0.0) += ratio * item.production_time;
    let Some(ref raw_mats) = item.raw_materials else {
        return;
    };
    let required_amounts = item.required_amount.as_deref().unwrap_or(&[]);
    for (i, raw_mat) in raw_mats.iter().enumerate() {
        let Some(resolved) = resolve_raw_material(raw_mat, item_map, facility_counts, module_levels)
        else {
            continue;
        };
        let required_per_batch = required_amounts.get(i).copied().unwrap_or(1) as f64;
        if required_per_batch <= 0.0 {
            continue;
        }
        let sub_ratio = ratio * required_per_batch / resolved.yield_amount as f64;
        accumulate_demand(
            resolved,
            sub_ratio,
            item_map,
            facility_counts,
            module_levels,
            demand,
            depth + 1,
        );
    }
}

/// Convenience wrapper around [`accumulate_demand`] that returns the completed demand map for
/// `item`'s whole ingredient tree, rooted at `item` itself (ratio 1.0).
fn compute_resource_demand<'a>(
    item: &'a ProductionItem,
    item_map: &HashMap<String, &'a ProductionItem>,
    facility_counts: &FacilityCounts,
    module_levels: &ModuleLevels,
) -> HashMap<(&'a str, &'a str), f64> {
    let mut demand = HashMap::new();
    accumulate_demand(item, 1.0, item_map, facility_counts, module_levels, &mut demand, 0);
    demand
}

/// The true bottleneck-limited batches/sec achievable at the root of a resource-demand map: the
/// minimum, over every (facility, required-level threshold) touched anywhere in the tree, of
/// however many owned tiers meet that threshold (see [`FacilityCounts::capacity_at_level`])
/// divided by the accumulated utilization from items requiring at least that level (see
/// [`accumulate_demand`]); same threshold technique as `solve_facility_allocation`'s own
/// capacity constraints (see that function's doc comment for why per-threshold, not just
/// per-facility, is the exact bound once a facility can own tiers at different levels). A
/// facility owned at a single level (the common case) has only one threshold and this reduces to
/// the familiar `facility_count / production_time / required_per_batch`; a facility touched by
/// multiple items (whether the same item via different branches, or different items time-sharing
/// it) gets their utilization summed at each threshold rather than compared individually.
fn batch_rate_bound(
    demand: &HashMap<(&str, &str), f64>,
    facility_counts: &FacilityCounts,
    item_map: &HashMap<String, &ProductionItem>,
) -> f64 {
    let item_facility_level = |item_name: &str| item_map.get(item_name).map(|i| i.facility_level).unwrap_or(1);
    let mut facility_thresholds: HashMap<&str, HashSet<u32>> = HashMap::new();
    for &(facility, item) in demand.keys() {
        facility_thresholds.entry(facility).or_default().insert(item_facility_level(item));
    }
    // Sorted for the same determinism reason as `solve_facility_allocation`'s identical pattern
    // (see that function's doc comment): `HashMap`/`HashSet` iteration order is randomized
    // per-process, and floating-point addition isn't perfectly order-independent, so summing
    // `utilization` in a different order across runs could shift this bound by a rounding hair,
    // enough in a near-tied LP to flip which of two equally-profitable candidates wins.
    let mut sorted_facilities: Vec<&str> = facility_thresholds.keys().copied().collect();
    sorted_facilities.sort_unstable();
    let mut bound = f64::INFINITY;
    for facility in sorted_facilities {
        let mut thresholds: Vec<u32> = facility_thresholds[facility].iter().copied().collect();
        thresholds.sort_unstable();
        for threshold in thresholds {
            let utilization: f64 = demand
                .iter()
                .filter(|&(&(f, item), _)| f == facility && item_facility_level(item) >= threshold)
                .map(|(_, &u)| u)
                .sum();
            if utilization <= 0.0 {
                continue;
            }
            let capacity = facility_counts.capacity_at_level(facility, threshold) as f64;
            bound = bound.min(capacity / utilization);
        }
    }
    bound
}

/// Recursively calculates production requirements for an item.
///
/// This handles both simple raw materials and processed items that may
/// require other processed items as ingredients (e.g., caramel_nut_chips requires nuts).
fn calculate_item_requirements(
    item_name: &str,
    required_amount: f64,
    item_map: &HashMap<String, &ProductionItem>,
    facility_counts: &FacilityCounts,
    module_levels: &ModuleLevels,
    visited: &mut HashSet<String>, // Prevent infinite recursion
) -> ProductionRequirements {
    // Check for circular dependencies
    if visited.contains(item_name) {
        return ProductionRequirements {
            total_time: 0.0,
            total_energy: None,
            total_cost: 0.0,
            raw_names: vec![],
            primary_facility: None,
            all_facilities: HashSet::new(),
            intermediate_steps: vec![],
            is_valid: false,
        };
    }
    
    // Try to find the best variant of this item (check the quick_ variant first, e.g.
    // quick_wheat for wheat)
    let high_speed_name = format!("quick_{}", item_name);
    let (item, actual_name) = {
        // Check if quick_ variant exists and is usable
        if let Some(hs_item) = item_map.get(&high_speed_name) {
            // Verify we can use the quick_ variant (check module requirements)
            let can_use_hs = if let Some((ref module_name, required_level)) = hs_item.module_requirement {
                module_levels.can_use(module_name, required_level)
            } else {
                true
            };
            // Also check facility requirements
            let can_produce_hs = facility_counts.can_produce(&hs_item.facility, hs_item.facility_level);

            if can_use_hs && can_produce_hs {
                // Use quick_ variant - it produces more in same/less time
                (*hs_item, high_speed_name.as_str())
            } else if let Some(base_item) = item_map.get(item_name) {
                // Fall back to base variant
                (*base_item, item_name)
            } else {
                return ProductionRequirements {
                    total_time: 0.0,
                    total_energy: None,
                    total_cost: 0.0,
                    raw_names: vec![],
                    primary_facility: None,
                    all_facilities: HashSet::new(),
                    intermediate_steps: vec![],
                    is_valid: false,
                };
            }
        } else if let Some(base_item) = item_map.get(item_name) {
            // No high_speed variant, use base
            (*base_item, item_name)
        } else {
            return ProductionRequirements {
                total_time: 0.0,
                total_energy: None,
                total_cost: 0.0,
                raw_names: vec![],
                primary_facility: None,
                all_facilities: HashSet::new(),
                intermediate_steps: vec![],
                is_valid: false,
            };
        }
    };
    
    // Check if facility can produce this item
    if !facility_counts.can_produce(&item.facility, item.facility_level) {
        return ProductionRequirements {
            total_time: 0.0,
            total_energy: None,
            total_cost: 0.0,
            raw_names: vec![],
            primary_facility: None,
            all_facilities: HashSet::new(),
            intermediate_steps: vec![],
            is_valid: false,
        };
    }
    
    // Check module requirements
    if let Some((ref module_name, required_level)) = item.module_requirement {
        if !module_levels.can_use(module_name, required_level) {
            return ProductionRequirements {
                total_time: 0.0,
                total_energy: None,
                total_cost: 0.0,
                raw_names: vec![],
                primary_facility: None,
                all_facilities: HashSet::new(),
                intermediate_steps: vec![],
                is_valid: false,
            };
        }
    }
    
    visited.insert(actual_name.to_string());
    
    let result = if let Some(ref raw_mats) = item.raw_materials {
        // This is a processed item - recursively calculate requirements for each ingredient
        let required_amounts = item.required_amount.as_deref().unwrap_or(&[]);
        
        let mut max_ingredient_time = 0.0;
        let mut total_ingredient_energy: Option<f64> = None;
        let mut total_ingredient_cost = 0.0;
        let mut all_raw_names: Vec<String> = Vec::new();
        let mut primary_facility: Option<String> = None;
        let mut all_facilities: HashSet<String> = HashSet::new();
        let mut intermediate_steps: Vec<(String, String, u32)> = Vec::new();
        
        // Add THIS item's processing facility
        all_facilities.insert(item.facility.clone());
        
        // Calculate how many batches of this processed item we need
        let batches_needed = (required_amount / item.yield_amount as f64).ceil();
        
        for (i, raw_mat) in raw_mats.iter().enumerate() {
            let ingredient_required_per_batch = required_amounts.get(i).copied().unwrap_or(1);
            let ingredient_required = ingredient_required_per_batch as f64 * batches_needed;
            
            let ingredient_reqs = calculate_item_requirements(
                raw_mat,
                ingredient_required,
                item_map,
                facility_counts,
                module_levels,
                visited,
            );
            
            if !ingredient_reqs.is_valid {
                visited.remove(actual_name);
                return ProductionRequirements {
                    total_time: 0.0,
                    total_energy: None,
                    total_cost: 0.0,
                    raw_names: vec![],
                    primary_facility: None,
                    all_facilities: HashSet::new(),
                    intermediate_steps: vec![],
                    is_valid: false,
                };
            }
            
            // Ingredients can be gathered in parallel, so take max time
            if ingredient_reqs.total_time > max_ingredient_time {
                max_ingredient_time = ingredient_reqs.total_time;
            }
            
            // Energy and cost are additive
            if let Some(e) = ingredient_reqs.total_energy {
                total_ingredient_energy = Some(total_ingredient_energy.unwrap_or(0.0) + e);
            }
            total_ingredient_cost += ingredient_reqs.total_cost;
            
            all_raw_names.extend(ingredient_reqs.raw_names);
            if primary_facility.is_none() {
                primary_facility = ingredient_reqs.primary_facility;
            }
            
            // Merge all facilities from ingredients
            all_facilities.extend(ingredient_reqs.all_facilities);
            
            // Propagate intermediate steps from ingredients
            intermediate_steps.extend(ingredient_reqs.intermediate_steps);
            
            // If this ingredient is itself a processed item, add it as an intermediate step
            // Check by looking up the item and seeing if it has raw_materials
            let high_speed_mat = format!("quick_{}", raw_mat);
            let mat_item = item_map.get(&high_speed_mat)
                .filter(|hs| {
                    let can_use = if let Some((ref m, l)) = hs.module_requirement {
                        module_levels.can_use(m, l)
                    } else { true };
                    can_use && facility_counts.can_produce(&hs.facility, hs.facility_level)
                })
                .or_else(|| item_map.get(raw_mat.as_str()));
            
            if let Some(mat) = mat_item {
                if mat.raw_materials.is_some() {
                    // This is a processed intermediate - add it as a step
                    intermediate_steps.push((
                        mat.name.clone(),
                        mat.facility.clone(),
                        ingredient_required_per_batch,
                    ));
                }
            }
        }
        
        // Add processing time for this item; only units at a tier meeting `item.facility_level`
        // can actually run this recipe (see `FacilityCounts::capacity_at_level`).
        let processing_facility_count =
            facility_counts.capacity_at_level(&item.facility, item.facility_level) as f64;
        let processing_time = item.production_time * (batches_needed / processing_facility_count).ceil();
        
        // Add processing energy
        let total_energy = match (total_ingredient_energy, item.energy) {
            (Some(ie), Some(pe)) => Some(ie + pe * batches_needed),
            (Some(ie), None) => Some(ie),
            (None, Some(pe)) => Some(pe * batches_needed),
            (None, None) => None,
        };
        
        ProductionRequirements {
            total_time: max_ingredient_time + processing_time,
            total_energy,
            total_cost: total_ingredient_cost,
            raw_names: all_raw_names,
            primary_facility,
            all_facilities,
            intermediate_steps,
            is_valid: true,
        }
    } else {
        // This is a base raw material; only units at a tier meeting `item.facility_level` can
        // actually run this recipe (see `FacilityCounts::capacity_at_level`).
        let facility_count = facility_counts.capacity_at_level(&item.facility, item.facility_level) as f64;
        let batches_needed = (required_amount / item.yield_amount as f64).ceil();

        // Calculate time with parallel facilities
        let time_per_batch = item.production_time;
        let parallel_batches = (batches_needed / facility_count).ceil();
        
        let total_time = time_per_batch * parallel_batches;
        let total_energy = item.energy.map(|e| e * batches_needed);
        let total_cost = item.cost.unwrap_or(0.0) * batches_needed;
        
        let mut all_facilities = HashSet::new();
        all_facilities.insert(item.facility.clone());
        
        ProductionRequirements {
            total_time,
            total_energy,
            total_cost,
            raw_names: vec![actual_name.to_string()],
            primary_facility: Some(item.facility.clone()),
            all_facilities,
            intermediate_steps: vec![],
            is_valid: true,
        }
    };
    
    visited.remove(actual_name);
    result
}

/// Maps a byproduct-target pseudo-currency name to the resource it names, or `None` if `target`
/// is an ordinary sellable currency (`"coins"`). Wood Blocks/Mineral Sand only
/// ever come as a side effect of growing/mining (`ProductionItem::byproduct`), never as something
/// directly sold, so they don't correspond to any `item.sell_currency`; targeting one means
/// "maximize how much of this resource Woodland/Mine produces," not "maximize profit."
pub fn byproduct_resource_name(target: &str) -> Option<&'static str> {
    match target {
        "wood_blocks" => Some("Wood Blocks"),
        "mineral_sand" => Some("Mineral Sand"),
        _ => None,
    }
}

/// Calculates efficiency metrics for all production items.
///
/// This function evaluates each production item based on:
/// - Profit per second (time efficiency)
/// - Profit per energy unit (energy efficiency)
/// - Total production time including raw material gathering
/// - Parallel production capability based on facility counts
///
/// # Arguments
///
/// * `items` - All available production items
/// * `target_currency` - What to optimize for: a sellable currency (`"coins"`) or
///   a byproduct pseudo-currency (`"wood_blocks"`/`"mineral_sand"`; see
///   [`byproduct_resource_name`])
/// * `facility_counts` - Configuration for each facility (count and level)
/// * `module_levels` - Configuration for each item upgrade module level
///
/// # Returns
///
/// A vector of [`ProductionEfficiency`] structs for all valid production options.
///
/// # Filtering
///
/// Items are filtered out if:
/// - Their facility level exceeds the specific facility's level
/// - They require a module level that isn't met
/// - They don't produce the target (its `sell_currency` for a currency target, or its
///   `byproduct` resource for a byproduct target; see [`byproduct_resource_name`])
/// - Their required raw materials aren't available at the raw material facility's level
///
/// A byproduct target only ever matches raw grower items (every processed item's `byproduct` is
/// always `None`), so the processed-item recursive branch below is simply never reached for one.
///
/// # Example
///
/// ```no_run
/// use aniimax::optimizer::calculate_efficiencies;
/// use aniimax::models::{FacilityCounts, ModuleLevels};
/// use aniimax::data::load_all_data;
/// use std::path::Path;
///
/// let items = load_all_data(Path::new("data")).unwrap();
/// let counts = FacilityCounts::from_pairs(&[
///     ("Farmland", 4, 3),        // 4 farmlands at level 3
///     ("Woodland", 1, 2),        // 1 woodland at level 2
///     ("Mine", 1, 1),            // 1 mine at level 1
///     ("Carousel Mill", 2, 2),   // 2 carousel mills at level 2
///     ("Jukebox Dryer", 1, 1),
///     ("Crafting Table", 1, 1),
///     ("Nimbus Bed", 1, 1),      // 1 nimbus bed (Wool/Petals)
/// ]);
/// let modules = ModuleLevels::default();
///
/// let efficiencies = calculate_efficiencies(&items, "coins", &counts, &modules);
/// ```
pub fn calculate_efficiencies(
    items: &[ProductionItem],
    target_currency: &str,
    facility_counts: &FacilityCounts,
    module_levels: &ModuleLevels,
) -> Vec<ProductionEfficiency> {
    let item_map: HashMap<String, &ProductionItem> =
        items.iter().map(|i| (i.name.clone(), i)).collect();

    let mut efficiencies = Vec::new();

    for item in items {
        // Filter by facility level (check if this facility can produce this item)
        if !facility_counts.can_produce(&item.facility, item.facility_level) {
            continue;
        }

        // Filter by module requirement
        if let Some((ref module_name, required_level)) = item.module_requirement {
            if !module_levels.can_use(module_name, required_level) {
                continue;
            }
        }

        // Filter by target: a byproduct target matches the item's byproduct resource instead of
        // its sell_currency (byproducts aren't sold, so sell_currency is irrelevant to them).
        match byproduct_resource_name(target_currency) {
            Some(resource) => {
                if item.byproduct.as_ref().map(|(r, _)| r.as_str()) != Some(resource) {
                    continue;
                }
            }
            None => {
                if item.sell_currency != target_currency {
                    continue;
                }
            }
        }

        let (total_time, steady_state_time, total_energy, raw_cost, requires_raw, raw_facility, all_facilities, intermediate_steps, raw_material_details, facility_demand) =
            if let Some(ref raw_mats) = item.raw_materials {
                // This is a processed item - use recursive calculation to handle nested dependencies
                let required_amounts = item.required_amount.as_deref().unwrap_or(&[]);
                
                // Track totals across all raw materials
                let mut max_ingredient_time = 0.0;
                let mut total_ingredient_energy: Option<f64> = None;
                let mut total_ingredient_cost = 0.0;
                let mut all_raw_names: Vec<String> = Vec::new();
                let mut primary_facility: Option<String> = None;
                let mut all_facilities_collected: HashSet<String> = HashSet::new();
                let mut all_intermediate_steps: Vec<(String, String, u32)> = Vec::new();
                // (name, amount_per_batch, time_per_batch, facility) - includes facility for filtering
                let mut raw_material_details_collected: Vec<(String, u32, f64, String)> = Vec::new();
                let mut skip_item = false;

                // Add THIS item's processing facility
                all_facilities_collected.insert(item.facility.clone());

                for (i, raw_mat) in raw_mats.iter().enumerate() {
                    let required = required_amounts.get(i).copied().unwrap_or(1);
                    
                    let mut visited = HashSet::new();
                    let reqs = calculate_item_requirements(
                        raw_mat,
                        required as f64,
                        &item_map,
                        facility_counts,
                        module_levels,
                        &mut visited,
                    );
                    
                    if !reqs.is_valid {
                        skip_item = true;
                        break;
                    }
                    
                    // Ingredients can be gathered in parallel, so take max time
                    if reqs.total_time > max_ingredient_time {
                        max_ingredient_time = reqs.total_time;
                    }
                    
                    // Energy and cost are additive
                    if let Some(e) = reqs.total_energy {
                        total_ingredient_energy = Some(total_ingredient_energy.unwrap_or(0.0) + e);
                    }
                    total_ingredient_cost += reqs.total_cost;
                    
                    all_raw_names.extend(reqs.raw_names);
                    if primary_facility.is_none() {
                        primary_facility = reqs.primary_facility;
                    }
                    
                    // Merge all facilities from ingredients
                    all_facilities_collected.extend(reqs.all_facilities);
                    
                    // Collect intermediate steps from recursive requirements
                    all_intermediate_steps.extend(reqs.intermediate_steps);
                    
                    // If this raw_mat is itself a processed item, add it as an intermediate step
                    let high_speed_name = format!("quick_{}", raw_mat);
                    let mat_item = item_map.get(&high_speed_name)
                        .filter(|hs| {
                            let can_use = if let Some((ref m, l)) = hs.module_requirement {
                                module_levels.can_use(m, l)
                            } else { true };
                            can_use && facility_counts.can_produce(&hs.facility, hs.facility_level)
                        })
                        .or_else(|| item_map.get(raw_mat.as_str()));
                    
                    if let Some(mat) = mat_item {
                        if mat.raw_materials.is_some() {
                            // This is a processed intermediate
                            all_intermediate_steps.push((
                                mat.name.clone(),
                                mat.facility.clone(),
                                required,
                            ));
                        }
                    }
                }

                if skip_item {
                    continue;
                }

                let processing_facility_count =
                    facility_counts.capacity_at_level(&item.facility, item.facility_level) as f64;
                let processing_time_per_mill = item.production_time; // Time for 1 mill to process 1 batch
                let processing_time = processing_time_per_mill / processing_facility_count;

                // For steady-state production, we need to find the bottleneck between:
                // 1. Raw material production rate
                // 2. Processing rate
                //
                // Processing rate: processing_facility_count batches per processing_time_per_mill seconds
                // Raw material rate: depends on how fast we can gather ingredients
                //
                // max_ingredient_time is the time to gather materials for 1 processed batch
                // But with more farms, we gather materials faster than needed for 1 batch
                // 
                // For super_wheatmeal example:
                // - 1 batch needs 120 wheat
                // - With 20 farms, we gather 300 wheat per 90 seconds
                // - That's 300/120 = 2.5 batches worth per 90 seconds
                // - Processing can do 5 batches per 60 seconds = 7.5 batches per 90 seconds
                // - Bottleneck is gathering: 2.5 batches per 90 seconds
                //
                // The issue is max_ingredient_time is calculated assuming we stop after gathering 120 wheat.
                // We need to calculate the raw material RATE instead.
                //
                // For now, let's calculate: what's the rate at which farms can supply materials?
                // rate = (yield per batch × facility_count) / batch_time / required_per_processed_batch
                // This gives us "processed batches worth of materials per second"
                
                // Collect raw material details for the same-facility multi-material allocation
                // feature (e.g. dried_flowers' lavender+rose both from Farmland). The actual
                // bottleneck-limited rate is computed separately below, via the whole-tree
                // resource-demand walk, not per-ingredient here, since two ingredients can
                // independently need the same deeper raw material (see `accumulate_demand`'s doc
                // comment), which a per-ingredient loop can't detect.
                for (i, raw_mat) in raw_mats.iter().enumerate() {
                    let required_per_batch = required_amounts.get(i).copied().unwrap_or(1) as f64;
                    let Some(raw) =
                        resolve_raw_material(raw_mat, &item_map, facility_counts, module_levels)
                    else {
                        continue;
                    };

                    // Collect raw material details for optimal allocation calculation
                    // Only include materials from the primary facility (for allocation to make sense)
                    // (name, amount_per_batch, time_per_batch, facility)
                    raw_material_details_collected.push((
                        raw.name.clone(),
                        required_per_batch as u32,
                        raw.production_time,
                        raw.facility.clone(),
                    ));
                }

                // True bottleneck-limited rate for this item: walks the whole ingredient tree and
                // takes the minimum capacity/demand ratio over every facility touched anywhere in
                // it (including this item's own processing facility, and any raw material shared
                // across multiple branches); see `compute_resource_demand`/`batch_rate_bound`.
                let demand = compute_resource_demand(item, &item_map, facility_counts, module_levels);
                let batches_per_second = batch_rate_bound(&demand, facility_counts, &item_map);
                let facility_demand: Vec<(String, String, f64)> = demand
                    .into_iter()
                    .map(|((facility, item_name), utilization)| {
                        (facility.to_string(), item_name.to_string(), utilization)
                    })
                    .collect();

                let steady_state_time = if batches_per_second > 0.0 && batches_per_second.is_finite() {
                    1.0 / batches_per_second
                } else {
                    f64::INFINITY 
                };
                
                // Total time for a single batch (used for display) is still sequential
                let total_time = max_ingredient_time + processing_time;
                let total_energy = match (total_ingredient_energy, item.energy) {
                    (Some(ie), Some(pe)) => Some(ie + pe),
                    (Some(ie), None) => Some(ie),
                    (None, Some(pe)) => Some(pe),
                    (None, None) => None,
                };

                // Deduplicate raw names while preserving order
                let unique_raw_names: Vec<String> = {
                    let mut seen = HashSet::new();
                    all_raw_names.into_iter().filter(|n| seen.insert(n.clone())).collect()
                };
                
                // Only keep raw_material_details if we have multiple materials FROM THE SAME FACILITY
                // (allocation only makes sense when splitting facilities of the same type)
                let raw_details = if raw_material_details_collected.len() > 1 {
                    // Check if all materials come from the same facility
                    let first_facility = &raw_material_details_collected[0].3;
                    let all_same_facility = raw_material_details_collected.iter()
                        .all(|(_, _, _, facility)| facility == first_facility);
                    
                    if all_same_facility {
                        // Convert to (name, amount, time) format - drop facility field
                        Some(raw_material_details_collected.into_iter()
                            .map(|(name, amt, time, _)| (name, amt, time))
                            .collect())
                    } else {
                        // Materials come from different facilities, allocation doesn't apply
                        None
                    }
                } else {
                    None
                };

                (
                    total_time,
                    steady_state_time, // Pass steady_state_time for efficiency calculation
                    total_energy,
                    total_ingredient_cost,
                    Some(unique_raw_names.join("+")),
                    primary_facility,
                    all_facilities_collected,
                    all_intermediate_steps,
                    raw_details,
                    facility_demand,
                )
            } else {
                // This is a raw material - direct production. Only units at a tier meeting
                // `item.facility_level` can actually run this recipe.
                let facility_count = facility_counts.capacity_at_level(&item.facility, item.facility_level) as f64;
                let time_per_batch = item.production_time;

                // For display purposes, time_per_unit is how long to produce one unit
                let effective_time_per_yield =
                    time_per_batch / (item.yield_amount as f64 * facility_count);
                // For raw materials, steady-state time equals batch time / facility count
                let steady_state_time = time_per_batch / facility_count;
                // Energy per batch (not per unit) to match units_needed which counts batches
                let energy_per_batch = item.energy;
                let cost_per_batch = item.cost.unwrap_or(0.0);
                
                // Raw materials use just their own facility
                let mut raw_all_facilities = HashSet::new();
                raw_all_facilities.insert(item.facility.clone());

                (effective_time_per_yield, steady_state_time, energy_per_batch, cost_per_batch, None, None, raw_all_facilities, vec![], None, vec![(item.facility.clone(), item.name.clone(), item.production_time)])
            };

        // The LP objective value for one batch: net profit (sell revenue minus ingredient cost)
        // for a currency target, or just the byproduct amount for a byproduct target; there's no
        // currency involved there, maximizing raw output IS the goal, not profit. (The filter
        // above already guarantees `item.byproduct` matches when targeting one.)
        let batch_value = match byproduct_resource_name(target_currency) {
            Some(_) => item.byproduct.as_ref().map(|(_, amount)| *amount as f64).unwrap_or(0.0),
            None => item.sell_value * item.yield_amount as f64 - raw_cost,
        };

        // For efficiency comparison, use steady-state time (bottleneck)
        let profit_per_second = if steady_state_time > 0.0 {
            batch_value / steady_state_time
        } else {
            0.0
        };
        let profit_per_energy = total_energy.map(|e| if e > 0.0 { batch_value / e } else { 0.0 });

        // For time optimization, use batch-based profit_per_second directly
        // (facility parallelism is already factored into batch_time)
        let effective_profit_per_second = profit_per_second;

        // Startup time is the time to produce the first batch (before steady-state begins)
        // This equals total_time for the first unit/batch
        let startup_time = total_time;

        efficiencies.push(ProductionEfficiency {
            item: item.clone(),
            batch_value,
            profit_per_second,
            profit_per_energy,
            total_time_per_unit: total_time,
            total_energy_per_unit: total_energy,
            requires_raw,
            raw_cost,
            raw_facility,
            all_facilities,
            intermediate_steps,
            startup_time,
            effective_profit_per_second,
            raw_material_details,
            facility_demand,
        });
    }

    efficiencies
}

/// Finds the optimal production path to achieve a target currency amount.
///
/// This function selects the most efficient production option based on
/// the optimization mode (time or energy) and calculates the complete
/// production path including raw material gathering.
///
/// # Arguments
///
/// * `efficiencies` - Pre-calculated efficiency metrics for all items
/// * `target_amount` - Target amount of currency to produce
/// * `optimize_energy` - If true, optimize for energy efficiency; otherwise optimize for time
/// * `energy_cost_per_min` - Cost of energy per minute (used when optimizing for time)
/// * `facility_counts` - Configuration for each facility (count and level)
///
/// # Returns
///
/// An `Option<ProductionPath>` containing the optimal path, or `None` if no valid path exists.
///
/// # Optimization Modes
///
/// - **Time optimization** (default): Maximizes profit per second, considering energy costs
/// - **Energy optimization**: Maximizes profit per energy unit consumed
///
/// # Example
///
/// ```no_run
/// use aniimax::optimizer::{calculate_efficiencies, find_best_production_path};
/// use aniimax::models::{FacilityCounts, ModuleLevels};
/// use aniimax::data::load_all_data;
/// use std::path::Path;
///
/// let items = load_all_data(Path::new("data")).unwrap();
/// let counts = FacilityCounts::from_pairs(&[
///     ("Farmland", 4, 3),        // 4 farmlands at level 3
///     ("Woodland", 1, 2),        // 1 woodland at level 2
///     ("Mine", 1, 1),            // 1 mine at level 1
///     ("Carousel Mill", 2, 2),   // 2 carousel mills at level 2
///     ("Jukebox Dryer", 1, 1),
///     ("Crafting Table", 1, 1),
///     ("Nimbus Bed", 1, 1),      // 1 nimbus bed (Wool/Petals)
/// ]);
/// let modules = ModuleLevels::default();
///
/// let efficiencies = calculate_efficiencies(&items, "coins", &counts, &modules);
/// let path = find_best_production_path(&efficiencies, 5000.0, false, 0.0, &counts);
/// ```
pub fn find_best_production_path(
    efficiencies: &[ProductionEfficiency],
    target_amount: f64,
    optimize_energy: bool,
    energy_cost_per_min: f64,
    facility_counts: &FacilityCounts,
) -> Option<ProductionPath> {
    if efficiencies.is_empty() {
        return None;
    }

    // Sort by efficiency metric
    let mut sorted = efficiencies.to_vec();
    if optimize_energy {
        sorted.sort_by(|a, b| {
            let a_eff = a.profit_per_energy.unwrap_or(0.0);
            let b_eff = b.profit_per_energy.unwrap_or(0.0);
            b_eff
                .partial_cmp(&a_eff)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    } else {
        // When optimizing for time, use effective profit per second (considers parallelization)
        sorted.sort_by(|a, b| {
            let a_energy_cost = a.total_energy_per_unit.unwrap_or(0.0) * energy_cost_per_min / 60.0;
            let b_energy_cost = b.total_energy_per_unit.unwrap_or(0.0) * energy_cost_per_min / 60.0;
            let a_net =
                a.effective_profit_per_second - (a_energy_cost / a.total_time_per_unit.max(1.0));
            let b_net =
                b.effective_profit_per_second - (b_energy_cost / b.total_time_per_unit.max(1.0));
            b_net
                .partial_cmp(&a_net)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    // Get the best option
    let best = &sorted[0];

    // Calculate how many units we need to produce
    let profit_per_unit = best.item.sell_value * best.item.yield_amount as f64 - best.raw_cost;
    let units_needed = (target_amount / profit_per_unit).ceil() as u32;

    let mut steps = Vec::new();

    // Get facility count for the main production
    let main_facility_count = facility_counts.get_count(&best.item.facility);

    // Add raw material step if needed
    if let Some(ref raw_name) = best.requires_raw {
        // Sum all required amounts for display purposes
        let raw_amount_needed = best.item.required_amount
            .as_ref()
            .map(|amounts| amounts.iter().sum::<u32>())
            .unwrap_or(1) * units_needed;
        let raw_facility = best.raw_facility.as_deref().unwrap_or("Unknown");
        let raw_facility_count = facility_counts.get_count(raw_facility);
        
        // Calculate optimal facility allocation for multi-material production
        let facility_allocation = if let Some(ref details) = best.raw_material_details {
            // details is Vec<(name, amount_per_batch, time_per_batch)>
            // We need to scale amounts by units_needed and calculate optimal facility split
            let materials_for_allocation: Vec<(String, u32, f64)> = details.iter()
                .map(|(name, amt_per_batch, time)| {
                    (name.clone(), amt_per_batch * units_needed, *time)
                })
                .collect();
            
            let allocation = calculate_optimal_facility_allocation(&materials_for_allocation, raw_facility_count);
            if allocation.len() > 1 {
                Some(allocation)
            } else {
                None
            }
        } else {
            None
        };
        
        steps.push(ProductionStep {
            item_name: raw_name.clone(),
            facility: format!("{} (x{})", raw_facility, raw_facility_count),
            quantity: raw_amount_needed,
            time: 0.0, // Time is included in total
            energy: None,
            profit_contribution: 0.0,
            chain_id: None,
            facility_allocation,
        });
        
        // Add intermediate processing steps (e.g., nuts for caramel_nut_chips)
        for (int_name, int_facility, int_amount_per_batch) in &best.intermediate_steps {
            let int_qty = int_amount_per_batch * units_needed;
            steps.push(ProductionStep {
                item_name: int_name.clone(),
                facility: format!("{} (x{})", int_facility, facility_counts.get_count(int_facility)),
                quantity: int_qty,
                time: 0.0,
                energy: None,
                profit_contribution: 0.0,
                chain_id: None,
                facility_allocation: None,
            });
        }
    }

    // Add production step
    steps.push(ProductionStep {
        item_name: best.item.name.clone(),
        facility: format!("{} (x{})", best.item.facility, main_facility_count),
        quantity: units_needed,
        time: best.total_time_per_unit * units_needed as f64,
        energy: best
            .total_energy_per_unit
            .map(|e| e * units_needed as f64),
        profit_contribution: profit_per_unit * units_needed as f64,
        chain_id: None,
        facility_allocation: None,
    });

    // Calculate actual time with parallelization
    // For processed items, use the steady-state calculation:
    //   time = units_needed * profit_per_unit / effective_profit_per_second
    // For raw materials, use direct batch calculation
    let total_time = if best.requires_raw.is_some() {
        // For processed items, effective_profit_per_second already accounts for bottleneck
        // time = profit_needed / profit_per_second
        units_needed as f64 * profit_per_unit / best.effective_profit_per_second
    } else {
        // For raw materials, units_needed is already the number of batches
        best.item.production_time * (units_needed as f64 / main_facility_count as f64).ceil()
    };

    let total_energy = best
        .total_energy_per_unit
        .map(|e| e * units_needed as f64);

    Some(ProductionPath {
        steps,
        total_time: total_time + best.startup_time, // Include startup delay
        startup_time: best.startup_time,
        total_energy,
        total_profit: profit_per_unit * units_needed as f64,
        currency: best.item.sell_currency.clone(),
        items_produced: units_needed * best.item.yield_amount,
        is_energy_self_sufficient: false,
        energy_items_produced: None,
        energy_item_name: None,
    })
}

/// Finds the optimal production path using cross-facility parallelization.
///
/// This function finds all production chains that can run simultaneously without
/// sharing any facilities. For example:
/// - Farmland → Carousel Mill (super_wheatmeal)
/// - Woodland → Crafting Table (wood_sculpture)  
/// - Nimbus Bed (wool)
///
/// All running in parallel since they use different facilities.
///
/// # Algorithm
///
/// Uses a greedy approach:
/// 1. Sort all items by profit per second
/// 2. Select the best item
/// 3. Find the next best item that doesn't share any facilities with selected items
/// 4. Repeat until no more non-conflicting items can be added
///
/// # Arguments
///
/// * `efficiencies` - Pre-calculated efficiency metrics for all items
/// * `target_amount` - Target amount of currency to produce
/// * `facility_counts` - Configuration for each facility (count and level)
///
/// # Returns
///
/// An `Option<ProductionPath>` containing the optimal parallel path, or `None` if no valid path exists.
pub fn find_parallel_production_path(
    efficiencies: &[ProductionEfficiency],
    target_amount: f64,
    facility_counts: &FacilityCounts,
) -> Option<ProductionPath> {
    if efficiencies.is_empty() {
        return None;
    }

    // Helper to get all facilities used by an item (including intermediate processing)
    fn get_facilities_used(eff: &ProductionEfficiency) -> HashSet<String> {
        // Use the pre-computed all_facilities set which tracks the entire chain
        eff.all_facilities.clone()
    }

    // Sort efficiencies by profit per second (descending)
    let mut sorted_effs: Vec<&ProductionEfficiency> = efficiencies.iter().collect();
    sorted_effs.sort_by(|a, b| {
        b.effective_profit_per_second
            .partial_cmp(&a.effective_profit_per_second)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Greedily select non-conflicting items
    let mut selected_items: Vec<&ProductionEfficiency> = Vec::new();
    let mut occupied_facilities: HashSet<String> = HashSet::new();

    for eff in &sorted_effs {
        // Skip items with no profit
        if eff.effective_profit_per_second <= 0.0 {
            continue;
        }
        
        // Skip items from facilities with 0 count
        if facility_counts.get_count(&eff.item.facility) == 0 {
            continue;
        }
        if let Some(ref raw_fac) = eff.raw_facility {
            if facility_counts.get_count(raw_fac) == 0 {
                continue;
            }
        }

        let facilities_needed = get_facilities_used(eff);
        
        // Check if any facility is already occupied
        let has_conflict = facilities_needed.iter().any(|f| occupied_facilities.contains(f));
        
        if !has_conflict {
            // Add this item to selected list
            selected_items.push(eff);
            occupied_facilities.extend(facilities_needed);
        }
    }

    // Need at least 2 items for parallel mode to be useful
    if selected_items.len() <= 1 {
        return None;
    }

    // Calculate combined profit rate
    let combined_profit_per_second: f64 = selected_items
        .iter()
        .map(|eff| eff.effective_profit_per_second)
        .sum();

    // Calculate startup time: max first-batch time across all parallel chains
    // This is the time before steady-state production begins
    let startup_time: f64 = selected_items
        .iter()
        .map(|eff| eff.startup_time)
        .fold(0.0, f64::max);

    // Calculate time needed (steady-state only, startup added separately)
    let theoretical_time = target_amount / combined_profit_per_second;

    // Build production steps
    let mut steps = Vec::new();
    let mut total_profit = 0.0;
    let mut total_energy: Option<f64> = None;
    let mut total_items = 0u32;

    for (current_chain_id, eff) in (0_u32..).zip(selected_items.iter()) {
        let profit_per_batch = eff.item.sell_value * eff.item.yield_amount as f64 - eff.raw_cost;
        
        // Calculate batches based on steady-state time
        let batches = if eff.requires_raw.is_some() {
            // Processed item: use steady-state calculation
            (theoretical_time * eff.effective_profit_per_second / profit_per_batch).ceil() as u32
        } else {
            // Raw item
            let facility_count = facility_counts.get_count(&eff.item.facility) as f64;
            let time_per_effective_batch = eff.item.production_time / facility_count;
            (theoretical_time / time_per_effective_batch).ceil() as u32
        };

        if batches == 0 {
            continue;
        }

        let step_profit = profit_per_batch * batches as f64;
        total_profit += step_profit;

        // Calculate actual time for this step
        let step_time = if eff.requires_raw.is_some() {
            // For processed items, time = batches * steady_state_time_per_batch
            batches as f64 * (profit_per_batch / eff.effective_profit_per_second)
        } else {
            let facility_count = facility_counts.get_count(&eff.item.facility) as f64;
            eff.item.production_time * (batches as f64 / facility_count).ceil()
        };

        if let Some(energy) = eff.total_energy_per_unit {
            let step_energy = energy * batches as f64;
            total_energy = Some(total_energy.unwrap_or(0.0) + step_energy);
        }

        total_items += batches * eff.item.yield_amount;

        // For processed items, show the full production chain
        if let Some(ref requires) = eff.requires_raw {
            // Step 1: Raw materials
            let raw_facility = eff.raw_facility.as_ref().unwrap_or(&eff.item.facility);
            let raw_qty = if let Some(ref amounts) = eff.item.required_amount {
                amounts.iter().sum::<u32>() * batches
            } else {
                batches
            };
            
            // Calculate optimal facility allocation for multi-material production
            let raw_facility_count = facility_counts.get_count(raw_facility);
            let facility_allocation = if let Some(ref details) = eff.raw_material_details {
                let materials_for_allocation: Vec<(String, u32, f64)> = details.iter()
                    .map(|(name, amt_per_batch, time)| {
                        (name.clone(), amt_per_batch * batches, *time)
                    })
                    .collect();
                
                let allocation = calculate_optimal_facility_allocation(&materials_for_allocation, raw_facility_count);
                if allocation.len() > 1 {
                    Some(allocation)
                } else {
                    None
                }
            } else {
                None
            };
            
            steps.push(ProductionStep {
                item_name: requires.clone(),
                facility: format!("{} (x{})", raw_facility, facility_counts.get_count(raw_facility)),
                quantity: raw_qty,
                time: step_time,
                energy: None,
                profit_contribution: 0.0,
                chain_id: Some(current_chain_id),
                facility_allocation,
            });
            
            // Step 2: Intermediate processing steps (e.g., nuts for caramel_nut_chips)
            for (int_name, int_facility, int_amount_per_batch) in &eff.intermediate_steps {
                let int_qty = int_amount_per_batch * batches;
                steps.push(ProductionStep {
                    item_name: int_name.clone(),
                    facility: format!("{} (x{})", int_facility, facility_counts.get_count(int_facility)),
                    quantity: int_qty,
                    time: step_time,
                    energy: None,
                    profit_contribution: 0.0,
                    chain_id: Some(current_chain_id),
                    facility_allocation: None,
                });
            }
        }

        // Step 3 (or 1 for raw items): Final product
        steps.push(ProductionStep {
            item_name: eff.item.name.clone(),
            facility: format!("{} (x{})", eff.item.facility, facility_counts.get_count(&eff.item.facility)),
            quantity: batches,
            time: step_time,
            energy: eff.total_energy_per_unit.map(|e| e * batches as f64),
            profit_contribution: step_profit,
            chain_id: Some(current_chain_id),
            facility_allocation: None,
        });
    }

    // Make sure we meet target by iteratively increasing if needed
    while total_profit < target_amount {
        // Find the step with highest profit/sec and add one batch
        let best_step_idx = steps
            .iter()
            .enumerate()
            .filter(|(_, s)| s.profit_contribution > 0.0)
            .max_by(|(_, a), (_, b)| {
                let a_rate = a.profit_contribution / a.time;
                let b_rate = b.profit_contribution / b.time;
                a_rate.partial_cmp(&b_rate).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i);

        if let Some(idx) = best_step_idx {
            let step = &mut steps[idx];
            let profit_per_batch = step.profit_contribution / step.quantity as f64;
            step.quantity += 1;
            step.profit_contribution += profit_per_batch;
            total_profit += profit_per_batch;
        } else {
            break;
        }
    }

    // Recalculate actual total time (longest step since they run in parallel)
    let actual_total_time = steps.iter().map(|s| s.time).fold(0.0, f64::max);

    // Only return if we have multiple independent productions
    let production_count = steps.iter().filter(|s| s.profit_contribution > 0.0).count();
    if production_count <= 1 {
        return None;
    }

    Some(ProductionPath {
        steps,
        total_time: actual_total_time + startup_time, // Include startup delay
        startup_time,
        total_energy,
        total_profit,
        currency: selected_items[0].item.sell_currency.clone(),
        items_produced: total_items,
        is_energy_self_sufficient: false,
        energy_items_produced: None,
        energy_item_name: None,
    })
}

/// Calculates efficiency metrics for items that can be consumed for energy.
///
/// Only items with a non-None energy field can be consumed for energy.
/// This is used for energy self-sufficient mode.
pub fn calculate_energy_efficiencies(
    items: &[ProductionItem],
    facility_counts: &FacilityCounts,
    module_levels: &ModuleLevels,
) -> Vec<EnergyItemEfficiency> {
    let mut efficiencies = Vec::new();

    for item in items {
        // Only raw materials (no raw_materials field) can be efficiently produced for energy
        // Processed items require raw materials which have opportunity cost
        if item.raw_materials.is_some() {
            continue;
        }

        // Must have energy value to be consumable
        let energy_per_batch = match item.energy {
            Some(e) if e > 0.0 => e,
            _ => continue,
        };

        // Filter by facility level
        if !facility_counts.can_produce(&item.facility, item.facility_level) {
            continue;
        }

        // Filter by module requirement
        if let Some((ref module_name, required_level)) = item.module_requirement {
            if !module_levels.can_use(module_name, required_level) {
                continue;
            }
        }

        let facility_count = facility_counts.get_count(&item.facility) as f64;
        let time_per_batch = item.production_time / facility_count;
        let energy_per_second = energy_per_batch / time_per_batch;
        let cost_per_batch = item.cost.unwrap_or(0.0);

        efficiencies.push(EnergyItemEfficiency {
            item: item.clone(),
            energy_per_second,
            time_per_batch,
            energy_per_batch,
            cost_per_batch,
        });
    }

    // Sort by energy per second (best first)
    efficiencies.sort_by(|a, b| {
        b.energy_per_second
            .partial_cmp(&a.energy_per_second)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    efficiencies
}

/// Finds the optimal production path with energy self-sufficiency.
///
/// This function calculates a production plan where:
/// - Some facilities produce items for profit (to sell)
/// - Some facilities produce items for energy (to consume)
/// - Total energy from consumed items >= energy consumed during production
///
/// # Arguments
///
/// * `profit_efficiencies` - Efficiency metrics for profit items
/// * `energy_efficiencies` - Efficiency metrics for energy items
/// * `target_amount` - Target profit to achieve
/// * `energy_cost_per_min` - Energy consumed per minute of production
/// * `facility_counts` - Configuration for each facility
///
/// # Returns
///
/// An `Option<ProductionPath>` with the optimal self-sufficient plan.
pub fn find_self_sufficient_path(
    profit_efficiencies: &[ProductionEfficiency],
    energy_efficiencies: &[EnergyItemEfficiency],
    target_amount: f64,
    energy_cost_per_min: f64,
    facility_counts: &FacilityCounts,
) -> Option<ProductionPath> {
    if profit_efficiencies.is_empty() {
        return None;
    }

    // If no energy cost, just use the simple path
    if energy_cost_per_min <= 0.0 {
        return find_best_production_path(
            profit_efficiencies,
            target_amount,
            false,
            0.0,
            facility_counts,
        );
    }

    // If no energy items available, can't be self-sufficient
    if energy_efficiencies.is_empty() {
        return None;
    }

    // Sort profit items by profit per second
    let mut sorted_profit = profit_efficiencies.to_vec();
    sorted_profit.sort_by(|a, b| {
        b.effective_profit_per_second
            .partial_cmp(&a.effective_profit_per_second)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let best_profit = &sorted_profit[0];
    let best_energy = &energy_efficiencies[0]; // Already sorted

    // Calculate profit per batch for the profit item
    let profit_per_batch = best_profit.item.sell_value * best_profit.item.yield_amount as f64
        - best_profit.raw_cost;

    // Get facility counts
    let profit_facility_count = facility_counts.get_count(&best_profit.item.facility) as f64;
    let energy_facility_count = facility_counts.get_count(&best_energy.item.facility) as f64;

    // Energy rate (per second)
    let energy_rate = energy_cost_per_min / 60.0;

    // Energy production rate from the best energy item (with all facilities)
    let energy_production_rate = best_energy.energy_per_second * energy_facility_count;

    // Check if we can even be self-sufficient
    // We need: energy_production_rate > energy_rate (otherwise we can never catch up)
    if energy_production_rate <= energy_rate {
        // Can't be self-sufficient with current setup
        return None;
    }

    // Calculate the optimal split
    // Let T_profit = time producing profit items
    // Let T_energy = time producing energy items
    // Let T_total = T_profit + T_energy
    //
    // Constraints:
    // 1. Profit >= target: profit_rate * T_profit >= target
    // 2. Energy balance: energy_produced >= energy_consumed
    //    best_energy.energy_per_second * energy_facility_count * T_energy >= energy_rate * T_total
    //
    // From constraint 2:
    // E * T_energy >= R * (T_profit + T_energy)
    // E * T_energy >= R * T_profit + R * T_energy
    // T_energy * (E - R) >= R * T_profit
    // T_energy >= T_profit * R / (E - R)

    // Calculate time needed for profit production
    let batches_for_profit = (target_amount / profit_per_batch).ceil();

    // Time to produce profit items (with parallelization)
    let time_for_profit = if best_profit.requires_raw.is_some() {
        best_profit.total_time_per_unit * batches_for_profit / profit_facility_count
    } else {
        best_profit.item.production_time * (batches_for_profit / profit_facility_count).ceil()
    };

    // Calculate energy batches needed using the formula:
    // Energy needed = (T_profit + T_energy) * R
    // Energy produced = B * E  (where B = batches, E = energy per batch)
    // T_energy = production_time * ceil(B / facility_count)
    //
    // For self-sufficiency: B * E >= (T_profit + T_energy) * R
    // 
    // Let's solve iteratively since ceiling makes it non-linear
    let production_time_per_batch = best_energy.item.production_time;
    
    // Start with minimum batches and increase until we have enough energy
    let mut energy_batches = 1u32;
    loop {
        let rounds = (energy_batches as f64 / energy_facility_count).ceil();
        let actual_energy_time = production_time_per_batch * rounds;
        let total_time = time_for_profit + actual_energy_time;
        let energy_needed = total_time * energy_rate;
        let energy_produced = energy_batches as f64 * best_energy.energy_per_batch;
        
        if energy_produced >= energy_needed {
            break;
        }
        
        energy_batches += 1;
        
        // Safety check to prevent infinite loop
        if energy_batches > 10000 {
            return None;
        }
    }

    // Calculate actual times with the determined batch counts
    let energy_rounds = (energy_batches as f64 / energy_facility_count).ceil();
    let actual_energy_production_time = production_time_per_batch * energy_rounds;
    let total_time = time_for_profit + actual_energy_production_time;
    let total_energy_needed = total_time * energy_rate;

    // Build the production steps
    let mut steps = Vec::new();

    // Add energy production step
    steps.push(ProductionStep {
        item_name: format!("{} (for energy)", best_energy.item.name),
        facility: format!(
            "{} (x{})",
            best_energy.item.facility,
            facility_counts.get_count(&best_energy.item.facility)
        ),
        quantity: energy_batches,
        time: actual_energy_production_time,
        energy: Some(energy_batches as f64 * best_energy.energy_per_batch),
        profit_contribution: -(energy_batches as f64 * best_energy.cost_per_batch), // Cost of seeds
        chain_id: None,
        facility_allocation: None,
    });

    // Add raw material step for profit item if needed
    if let Some(ref raw_name) = best_profit.requires_raw {
        let raw_amount_needed = best_profit.item.required_amount
            .as_ref()
            .map(|amounts| amounts.iter().sum::<u32>())
            .unwrap_or(1) * batches_for_profit as u32;
        let raw_facility = best_profit.raw_facility.as_deref().unwrap_or("Unknown");
        let raw_facility_count = facility_counts.get_count(raw_facility);
        
        // Calculate optimal facility allocation for multi-material production
        let facility_allocation = if let Some(ref details) = best_profit.raw_material_details {
            let materials_for_allocation: Vec<(String, u32, f64)> = details.iter()
                .map(|(name, amt_per_batch, time)| {
                    (name.clone(), amt_per_batch * batches_for_profit as u32, *time)
                })
                .collect();
            
            let allocation = calculate_optimal_facility_allocation(&materials_for_allocation, raw_facility_count);
            if allocation.len() > 1 {
                Some(allocation)
            } else {
                None
            }
        } else {
            None
        };
        
        steps.push(ProductionStep {
            item_name: raw_name.clone(),
            facility: format!("{} (x{})", raw_facility, raw_facility_count),
            quantity: raw_amount_needed,
            time: 0.0,
            energy: None,
            profit_contribution: 0.0,
            chain_id: None,
            facility_allocation,
        });
        
        // Add intermediate processing steps (e.g., nuts for caramel_nut_chips)
        for (int_name, int_facility, int_amount_per_batch) in &best_profit.intermediate_steps {
            let int_qty = int_amount_per_batch * batches_for_profit as u32;
            steps.push(ProductionStep {
                item_name: int_name.clone(),
                facility: format!("{} (x{})", int_facility, facility_counts.get_count(int_facility)),
                quantity: int_qty,
                time: 0.0,
                energy: None,
                profit_contribution: 0.0,
                chain_id: None,
                facility_allocation: None,
            });
        }
    }

    // Add profit production step
    steps.push(ProductionStep {
        item_name: format!("{} (for profit)", best_profit.item.name),
        facility: format!(
            "{} (x{})",
            best_profit.item.facility,
            facility_counts.get_count(&best_profit.item.facility)
        ),
        quantity: batches_for_profit as u32,
        time: time_for_profit,
        energy: None,
        profit_contribution: profit_per_batch * batches_for_profit,
        chain_id: None,
        facility_allocation: None,
    });

    // Calculate actual profit (minus seed costs for energy items)
    let energy_seed_cost = energy_batches as f64 * best_energy.cost_per_batch;
    let gross_profit = profit_per_batch * batches_for_profit;
    let net_profit = gross_profit - energy_seed_cost;
    
    // For energy self-sufficient mode, startup time is the longer of the two chains
    let startup_time = best_profit.startup_time.max(best_energy.item.production_time);

    Some(ProductionPath {
        steps,
        total_time: total_time + startup_time,
        startup_time,
        total_energy: Some(total_energy_needed),
        total_profit: net_profit,
        currency: best_profit.item.sell_currency.clone(),
        items_produced: batches_for_profit as u32 * best_profit.item.yield_amount,
        is_energy_self_sufficient: true,
        energy_items_produced: Some(energy_batches * best_energy.item.yield_amount),
        energy_item_name: Some(best_energy.item.name.clone()),
    })
}

/// Time (seconds) until the very first batch of `name` is ready; the growing/gathering lead
/// time before any output (and thus any coin income) exists at all, as opposed to the
/// steady-state throughput rate used everywhere else in this module.
///
/// For a raw material this is just `production_time` (every owned copy of the facility starts
/// at t=0 and finishes its first batch together, regardless of facility count; more facilities
/// mean more *batches per completion*, not a faster *first* completion). For a processed item
/// it's the slowest ingredient's own lead time, plus this item's own processing time on top
/// (ingredients are gathered in parallel with each other, but processing can't start on an
/// ingredient before that ingredient exists). Recurses for multi-level chains (e.g. nuts, itself
/// processed, used as an ingredient in caramel_nut_chips).
///
/// Deliberately ignores facility count (already folded into
/// `ProductionEfficiency::startup_time`, which this does NOT reuse; that field divides
/// processing time by facility count, which is right for steady-state throughput but wrong for
/// "time until the first batch exists", the thing this function needs).
pub(crate) fn item_lead_time(name: &str, item_map: &HashMap<&str, &ProductionItem>, depth: u32) -> f64 {
    if depth > 8 {
        return 0.0; // guard against unexpected circular references
    }
    let Some(item) = item_map.get(name) else {
        return 0.0;
    };
    match &item.raw_materials {
        None => item.production_time,
        Some(raw_mats) => {
            let max_ingredient_lead = raw_mats
                .iter()
                .map(|m| item_lead_time(m, item_map, depth + 1))
                .fold(0.0_f64, f64::max);
            max_ingredient_lead + item.production_time
        }
    }
}

/// Finds the item in `root`'s ingredient tree (possibly `root` itself) that directly consumes
/// `target`. A recipe ingredient `X` also matches a `target` of `quick_X`, since the quick variant
/// is what gets grown in its place. Returns `None` if `target` isn't anywhere in the tree.
///
/// Used for the plan's "Used for ..." text: each row names only its next step (quick_wheat says
/// "wheatmeal", not "premium_bread"), and the wheatmeal row names premium_bread in turn.
fn direct_consumer(root: &str, target: &str, item_map: &HashMap<&str, &ProductionItem>, depth: u32) -> Option<String> {
    if depth > 8 {
        return None; // guard against unexpected circular references
    }
    let raw_mats = item_map.get(root)?.raw_materials.as_ref()?;
    if raw_mats.iter().any(|m| m == target || target.strip_prefix("quick_") == Some(m.as_str())) {
        return Some(root.to_string());
    }
    raw_mats.iter().find_map(|m| direct_consumer(m, target, item_map, depth + 1))
}

/// Solves for the provably-optimal simultaneous allocation of every owned facility's capacity
/// across every candidate item; replacing the old greedy-plus-leftover-patches approach (three
/// separate mechanisms accumulated over this project's history, each added to fix one more
/// scenario the previous ones missed) with a linear program: maximize total coins/sec subject to
/// no facility's capacity being oversubscribed.
///
/// `eff.facility_demand` (see `compute_resource_demand`) already gives exactly the constraint
/// coefficients needed: for item `i` and facility `f`, how much of `f`'s batch capacity one
/// batch/sec of `i` consumes. That's the entire LP; one variable per item (its batches/sec,
/// coefficient = net profit per batch, ≥ 0), one constraint per owned facility (sum of
/// utilization × rate across every item touching it ≤ that facility's count). A facility ending
/// up split between multiple items falls out of the solution naturally whenever that's optimal;
/// no separate "leftover" bookkeeping needed, and no dependency on the order items happen to be
/// considered in, since every variable is solved for simultaneously.
///
/// Environment buildings and the temperature mode(s) each can be configured to provide. A
/// building's mode is an optimizer decision (see `solve_facility_allocation`), not a player
/// input; enter how many you own, the solver picks the profit-maximizing mode/layout. No two
/// building types ever share a mode name, so `mode` alone always uniquely identifies which
/// building type produced it.
const ENVIRONMENT_BUILDINGS: &[(&str, &[&str])] = &[
    ("Heat Furnace", &["Warm", "Scorching"]),
    ("Cooling Unit", &["Cool", "Freeze"]),
    ("Sunlamp", &["Adequate"]),
];

/// For every environment-gated `(facility_type, mode)` combination with at least one candidate
/// item, its per-plot value: `MAX` over such items of `batch_value / utilization`; "profit per
/// plot if a plot there were fully dedicated to this item's chain." Used to prioritize the
/// environment packing solve (`crate::coverage::solve_building_packing`); see that function's
/// doc comment for why this is computed from each item's own static economics rather than from a
/// completed rate solve (avoiding a chicken-and-egg bootstrap: an item needing an environment has
/// rate 0 until coverage exists, so weighting by rate would mean nothing gated ever gets covered).
fn compute_coverage_weights(
    item_map: &HashMap<&str, &ProductionItem>,
    effs: &[ProductionEfficiency],
) -> HashMap<(&'static str, &'static str), f64> {
    let mut weights: HashMap<(&'static str, &'static str), f64> = HashMap::new();
    for eff in effs {
        if eff.batch_value <= 0.0 {
            continue;
        }
        for (facility, item_name, utilization) in &eff.facility_demand {
            if *utilization <= 0.0 {
                continue;
            }
            let Some(&(facility_type, _)) =
                crate::coverage::ENVIRONMENT_GATED_FACILITIES.iter().find(|(f, _)| f == facility)
            else {
                continue;
            };
            let Some(env) = item_map.get(item_name.as_str()).and_then(|i| i.environment.as_deref()) else {
                continue;
            };
            let Some(mode) =
                ENVIRONMENT_BUILDINGS.iter().flat_map(|(_, modes)| modes.iter()).find(|&&m| m == env).copied()
            else {
                continue;
            };
            let per_plot = eff.batch_value / utilization;
            let entry = weights.entry((facility_type, mode)).or_insert(0.0);
            if per_plot > *entry {
                *entry = per_plot;
            }
        }
    }
    weights
}

/// Solves the environment-coverage packing for every owned environment building type, using
/// `weights` (see `compute_coverage_weights`) to prioritize which facility types get coverage
/// when several share one building. Returns `(mode_counts, placements, layouts)`; `mode_counts`
/// keyed by `(building, mode)`, `placements`/`layouts` keyed by `mode`; one independent (and
/// fast) `crate::coverage::solve_building_packing` call per building type, never mixed with the
/// continuous item-rate LP in the same `Problem` (see `solve_facility_allocation`'s doc comment
/// for why that combination hangs in practice).
///
/// Results are memoized per thread (see [`PACKING_CACHE`]): the exclusion passes in
/// `find_production_plan` re-solve the same coverage weights many times over (excluding one recipe
/// rarely changes any facility type's per-plot weight), and each packing solve is by far the most
/// expensive step of a trial.
fn solve_environment_coverage(
    weights: &HashMap<(&'static str, &'static str), f64>,
    facility_counts: &FacilityCounts,
) -> EnvironmentCoverage {
    let key = packing_cache_key(weights, facility_counts);
    if let Some(hit) = PACKING_CACHE.with(|cache| cache.borrow().get(&key).cloned()) {
        return hit;
    }
    let solved = solve_environment_coverage_uncached(weights, facility_counts);
    PACKING_CACHE.with(|cache| cache.borrow_mut().insert(key, solved.clone()));
    solved
}

/// `(mode_counts, placements, layouts)`; see [`solve_environment_coverage`].
type EnvironmentCoverage = (
    HashMap<(&'static str, &'static str), u32>,
    HashMap<&'static str, Vec<(crate::coverage::Placement, u32)>>,
    HashMap<&'static str, Vec<Vec<crate::coverage::Placement>>>,
);

/// Everything a packing solve depends on: each `(facility type, mode)` weight (as exact bits, so
/// only truly identical inputs share an entry) and the owned counts of every environment building
/// and environment-gated facility type.
type PackingCacheKey = (Vec<(&'static str, &'static str, u64)>, Vec<u32>);

thread_local! {
    /// Memoized [`solve_environment_coverage`] results. Cleared at the start of every
    /// `find_production_plan_with_progress` call so it never grows past one plan's worth of
    /// distinct inputs.
    static PACKING_CACHE: std::cell::RefCell<HashMap<PackingCacheKey, EnvironmentCoverage>> =
        std::cell::RefCell::new(HashMap::new());
}

fn packing_cache_key(
    weights: &HashMap<(&'static str, &'static str), f64>,
    facility_counts: &FacilityCounts,
) -> PackingCacheKey {
    let mut weight_bits: Vec<(&'static str, &'static str, u64)> =
        weights.iter().map(|(&(facility, mode), w)| (facility, mode, w.to_bits())).collect();
    weight_bits.sort_unstable();
    let owned: Vec<u32> = ENVIRONMENT_BUILDINGS
        .iter()
        .map(|&(building, _)| facility_counts.get_count(building))
        .chain(
            crate::coverage::ENVIRONMENT_GATED_FACILITIES
                .iter()
                .map(|&(facility, _)| facility_counts.get_count(facility)),
        )
        .collect();
    (weight_bits, owned)
}

fn solve_environment_coverage_uncached(
    weights: &HashMap<(&'static str, &'static str), f64>,
    facility_counts: &FacilityCounts,
) -> EnvironmentCoverage {
    let mut mode_counts = HashMap::new();
    let mut placements: HashMap<&'static str, Vec<(crate::coverage::Placement, u32)>> = HashMap::new();
    let mut layouts: HashMap<&'static str, Vec<Vec<crate::coverage::Placement>>> = HashMap::new();

    // How many of each environment-gated facility type the player actually owns; caps the
    // packing's placement usage so it can't invent more covered plots than physically exist (see
    // `solve_building_packing`'s doc comment).
    let facility_owned: Vec<(&'static str, u32)> = crate::coverage::ENVIRONMENT_GATED_FACILITIES
        .iter()
        .map(|&(facility_type, _)| (facility_type, facility_counts.get_count(facility_type)))
        .collect();

    for &(building, modes) in ENVIRONMENT_BUILDINGS {
        let owned = facility_counts.get_count(building);
        if owned == 0 {
            continue;
        }
        // Sorted (rather than left in `weights`' `HashMap` iteration order, which is randomized
        // per-process) for two reasons: determinism; the same facility counts should always
        // produce the same plan, not a different one on every run/restart; and because this
        // order becomes each candidate's variable-creation order in
        // `crate::coverage::solve_one_building_layout`'s binary ILP, which measurably affects how
        // fast its branch & bound converges.
        let mut weighted: Vec<(&'static str, &'static str, f64)> = weights
            .iter()
            .filter(|((_, mode), _)| modes.contains(mode))
            .map(|(&(facility_type, mode), &w)| (facility_type, mode, w))
            .collect();
        weighted.sort_by(|a, b| {
            b.2.partial_cmp(&a.2)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(b.0))
                .then_with(|| a.1.cmp(b.1))
        });
        if weighted.is_empty() {
            continue;
        }
        let (mc, pl, lo) =
            crate::coverage::solve_building_packing(building, modes, &weighted, owned, &facility_owned);
        mode_counts.extend(mc);
        for (mode, placed) in pl {
            placements.entry(mode).or_default().extend(placed);
        }
        for (mode, mode_layouts) in lo {
            layouts.entry(mode).or_default().extend(mode_layouts);
        }
    }

    (mode_counts, placements, layouts)
}

/// Returns batches/sec for every item whose solved rate is above a negligible threshold, keyed by
/// item name.
///
/// # Environment coverage: real 2D geometric packing (`crate::coverage`)
///
/// Farmland/Woodland/Starfall Hammock/Tidewhisper Sandcastle/Floral Windmill/Dewy House crops
/// can need a growing environment (`ProductionItem::environment`, e.g. "Cool", "Adequate") that
/// only exists where an owned environment building (Heat Furnace/Cooling Unit/Sunlamp) covers it.
/// A building's coverage is real 2D area, not a small set of fixed presets, so how many of each
/// facility type can share one building's coverage is computed via exact integer optimization
/// (`crate::coverage::solve_building_packing`) rather than picked from a lookup table.
///
/// That packing solve is **not** part of this function's `Problem`; it's solved separately,
/// upfront, by `solve_environment_coverage`, and its result (`coverage_bounds`) is passed in here
/// as a fixed capacity per `(facility_type, mode)` pair. A single combined mixed-integer problem
/// (continuous item rates + integer coverage packing, jointly optimized) hangs in practice: a
/// scenario with only ~50 continuous variables plus ~120 integer variables never returns in 20+
/// seconds of `microlp` branch & bound, while the packing alone solves in low milliseconds and
/// this plain continuous LP is always fast on its own. Decoupling them; packing computed once
/// from each item's static economics (see `compute_coverage_weights`), then fed into this plain LP
/// as a fixed bound; trades perfectly joint optimality for tractability, the same kind of
/// accepted approximation `find_production_plan`'s own stranded-chain exclusion loop already uses
/// elsewhere. `byproduct_floors` is `[(byproduct_resource_name, minimum_rate)]`; e.g.
/// `[("Wood Blocks", 12.3)]`; used by the "prioritize Wood Blocks/Mineral Sand" toggle (see
/// `find_production_plan`'s doc comment) to force the LP to hit at least that total byproduct
/// rate, guaranteeing it before letting profit maximization use whatever facility capacity is
/// left over. Pass an empty slice for the normal (profit-only) case.
///
/// Memoized per thread (see [`ALLOCATION_CACHE`]): the exclusion passes in
/// `find_production_plan` solve the same candidate set against the same coverage many times.
fn solve_facility_allocation<'a>(
    item_map: &HashMap<&str, &ProductionItem>,
    effs: &'a [ProductionEfficiency],
    facility_counts: &FacilityCounts,
    coverage_bounds: &HashMap<(String, String), u32>,
    byproduct_floors: &[(&str, f64)],
) -> HashMap<&'a str, f64> {
    let key = allocation_cache_key(effs, coverage_bounds, byproduct_floors);
    if let Some(rates) = ALLOCATION_CACHE.with(|cache| cache.borrow().get(&key).cloned()) {
        let by_name: HashMap<&str, &'a str> = effs.iter().map(|e| (e.item.name.as_str(), e.item.name.as_str())).collect();
        return rates.iter().filter_map(|(name, rate)| by_name.get(name.as_str()).map(|&n| (n, *rate))).collect();
    }
    let solved = solve_facility_allocation_uncached(item_map, effs, facility_counts, coverage_bounds, byproduct_floors);
    let rates: Vec<(String, f64)> = solved.iter().map(|(&name, &rate)| (name.to_string(), rate)).collect();
    ALLOCATION_CACHE.with(|cache| cache.borrow_mut().insert(key, rates));
    solved
}

/// Everything one facility-allocation LP depends on within a single plan: each candidate's name
/// and per-batch value (the byproduct-floor pre-solve prices the same items by byproduct instead
/// of coins, so the name alone isn't enough), the coverage bounds, and the byproduct floors. Item
/// data and facility counts are fixed for the length of one `find_production_plan` call, which is
/// as long as the cache lives.
type AllocationCacheKey = (Vec<(String, u64)>, Vec<(String, String, u32)>, Vec<(String, u64)>);

thread_local! {
    /// Memoized [`solve_facility_allocation`] results, as `(item name, rate)` pairs. Cleared at the
    /// start of every `find_production_plan_with_progress` call.
    static ALLOCATION_CACHE: std::cell::RefCell<HashMap<AllocationCacheKey, Vec<(String, f64)>>> =
        std::cell::RefCell::new(HashMap::new());
}

fn allocation_cache_key(
    effs: &[ProductionEfficiency],
    coverage_bounds: &HashMap<(String, String), u32>,
    byproduct_floors: &[(&str, f64)],
) -> AllocationCacheKey {
    let mut candidates: Vec<(String, u64)> =
        effs.iter().map(|e| (e.item.name.clone(), e.batch_value.to_bits())).collect();
    candidates.sort_unstable();
    let mut bounds: Vec<(String, String, u32)> =
        coverage_bounds.iter().map(|((f, m), &n)| (f.clone(), m.clone(), n)).collect();
    bounds.sort_unstable();
    let floors: Vec<(String, u64)> = byproduct_floors.iter().map(|(r, f)| (r.to_string(), f.to_bits())).collect();
    (candidates, bounds, floors)
}

fn solve_facility_allocation_uncached<'a>(
    item_map: &HashMap<&str, &ProductionItem>,
    effs: &'a [ProductionEfficiency],
    facility_counts: &FacilityCounts,
    coverage_bounds: &HashMap<(String, String), u32>,
    byproduct_floors: &[(&str, f64)],
) -> HashMap<&'a str, f64> {
    let mut problem = Problem::new(OptimizationDirection::Maximize);

    // One column per candidate item with positive net profit; anything else can never help the
    // objective (its coefficient would be ≤ 0), so excluding it up front keeps the problem small
    // without changing the optimal solution.
    let mut item_vars: Vec<(&'a ProductionEfficiency, microlp::Variable)> = Vec::new();
    for eff in effs {
        if facility_counts.get_count(&eff.item.facility) == 0 {
            continue;
        }
        let net_profit_per_batch = eff.batch_value;
        if net_profit_per_batch <= 0.0 {
            continue;
        }
        let var = problem.add_var(net_profit_per_batch, (0.0, f64::INFINITY));
        item_vars.push((eff, var));
    }

    // One row per (owned facility, required-level threshold) touched by at least one candidate:
    // total utilization from items requiring AT LEAST that level can't exceed however many owned
    // tiers meet that bar (see `FacilityCounts::capacity_at_level`); a higher-level tier can
    // always run a lower-level recipe too (an upgrade never takes capability away), so eligible-
    // unit sets NEST as the threshold rises (level>=5 ⊆ level>=4 ⊆ ... ⊆ level>=1). One
    // constraint per distinct threshold that actually appears is the exact LP formulation for
    // that nesting (a totally-ordered/"laminar" family of capacity constraints; both necessary,
    // since items requiring level D can only ever use tiers meeting it, and sufficient, since a
    // nested-demand transportation problem's feasibility is exactly characterized by these
    // threshold cuts), not an approximation. A facility owned at a single level (the
    // overwhelmingly common case) has only one threshold and this reduces to exactly one
    // constraint. A single chain can host MULTIPLE distinct items at the same
    // facility (e.g. caramel_nut_chips needs walnut, chestnut, AND maple_syrup all from Woodland)
    //; `facility_demand` has one entry per item, so this sums every matching entry rather than
    // taking just the first (which would silently undercount the facility's true total
    // utilization for that chain).
    let item_facility_level = |item_name: &str| item_map.get(item_name).map(|i| i.facility_level).unwrap_or(1);
    let mut facility_thresholds: HashMap<&str, HashSet<u32>> = HashMap::new();
    for (eff, _) in &item_vars {
        for (facility, item_name, _) in &eff.facility_demand {
            facility_thresholds.entry(facility.as_str()).or_default().insert(item_facility_level(item_name));
        }
    }
    // Sorted (rather than left in `HashMap`/`HashSet` iteration order, which is randomized
    // per-process) so the exact same input always produces the exact same plan: constraints get
    // added to `problem` in a fixed order regardless of which run/browser-tab/WASM instance is
    // solving it. Otherwise, whenever the true optimum has TIES (e.g. pine and a fallback crop
    // both fully using Woodland's capacity at equal profit), the simplex method's tie-breaking
    // can depend on constraint insertion order, so the same facilities could non-deterministically
    // solve to a different-but-equally-optimal choice from one page load to the next.
    let mut sorted_facilities: Vec<&str> = facility_thresholds.keys().copied().collect();
    sorted_facilities.sort_unstable();
    for facility in sorted_facilities {
        let mut thresholds: Vec<u32> = facility_thresholds[facility].iter().copied().collect();
        thresholds.sort_unstable();
        for threshold in thresholds {
            // No lower bound skip here on purpose: a facility with zero owned tiers meeting this
            // threshold must still get a constraint (capacity 0), not no constraint at all;
            // skipping it here would let the LP treat "you don't own this" as "unlimited supply
            // of it" instead of "none available".
            let capacity = facility_counts.capacity_at_level(facility, threshold) as f64;
            let terms: Vec<(microlp::Variable, f64)> = item_vars
                .iter()
                .filter_map(|(eff, var)| {
                    let total_utilization: f64 = eff
                        .facility_demand
                        .iter()
                        .filter(|(f, item_name, _)| f == facility && item_facility_level(item_name) >= threshold)
                        .map(|(_, _, utilization)| utilization)
                        .sum();
                    (total_utilization > 0.0).then_some((*var, total_utilization))
                })
                .collect();
            if terms.is_empty() {
                continue;
            }
            problem.add_constraint(&terms, ComparisonOp::Le, capacity);
        }
    }

    // Coverage-link constraints: for every (facility-type, environment) pair with at least one
    // candidate item needing it, total utilization from items needing that environment there
    // can't exceed the fixed coverage bound already computed for it (see this function's doc
    // comment); the LP's own capacity constraint above already bounds an item's utilization by
    // its facility's plot count; this adds a second, potentially tighter bound from environment
    // coverage alone. Each `facility_demand` entry now names its own specific item, so this reads
    // that item's own environment directly rather than assuming "whichever item happened to be
    // first" applies to everything a chain hosts at that facility; a single chain can draw
    // multiple items needing DIFFERENT (or no) environment from the same facility type (e.g.
    // walnut needs Cool, chestnut needs none, both feed caramel_nut_chips via Woodland).
    for &(facility_type, _) in crate::coverage::ENVIRONMENT_GATED_FACILITIES {
        let mut environments_needed: HashSet<&str> = HashSet::new();
        for (eff, _) in &item_vars {
            for (facility, item_name, _) in &eff.facility_demand {
                if facility != facility_type {
                    continue;
                }
                if let Some(env) = item_map.get(item_name.as_str()).and_then(|item| item.environment.as_deref()) {
                    environments_needed.insert(env);
                }
            }
        }

        // Sorted for the same determinism reason noted on the capacity-constraint loop above:
        // `HashSet` iteration order is randomized per-process, so adding these constraints in a
        // different order across runs/page-loads could flip which of two equally-optimal
        // environment-gated crops (e.g. pine vs. a fallback) the simplex method lands on for
        // otherwise byte-identical input.
        let mut sorted_envs: Vec<&str> = environments_needed.into_iter().collect();
        sorted_envs.sort_unstable();
        for env in sorted_envs {
            let terms: Vec<(microlp::Variable, f64)> = item_vars
                .iter()
                .filter_map(|(eff, var)| {
                    let total_utilization: f64 = eff
                        .facility_demand
                        .iter()
                        .filter(|(facility, item_name, _)| {
                            facility == facility_type
                                && item_map.get(item_name.as_str()).and_then(|item| item.environment.as_deref())
                                    == Some(env)
                        })
                        .map(|(_, _, utilization)| utilization)
                        .sum();
                    (total_utilization > 0.0).then_some((*var, total_utilization))
                })
                .collect();
            if terms.is_empty() {
                continue;
            }
            let bound =
                coverage_bounds.get(&(facility_type.to_string(), env.to_string())).copied().unwrap_or(0) as f64;
            problem.add_constraint(&terms, ComparisonOp::Le, bound);
        }
    }

    // Hard byproduct floors ("prioritize Wood Blocks/Mineral Sand"): for each `(resource,
    // min_rate)` pair, require the TOTAL byproduct output across every candidate touching that
    // resource to be at least `min_rate`, guaranteeing whatever rate was already found achievable
    // when that byproduct was solved as its own target; combined with the facility's own
    // capacity constraint above, this forces the LP into (a mix that achieves) that same maximum,
    // while still letting it pick whichever specific mix is most profitable among ties. A
    // candidate's own contribution per unit of its rate is, for each `facility_demand` entry whose
    // item has a matching byproduct: `(utilization / that item's production_time) * byproduct
    // amount`; i.e. batches/sec of that specific item per one batch/sec of the candidate, times
    // how much byproduct each batch yields.
    for &(resource, floor) in byproduct_floors {
        if floor <= 0.0 {
            continue;
        }
        let terms: Vec<(microlp::Variable, f64)> = item_vars
            .iter()
            .filter_map(|(eff, var)| {
                let coefficient: f64 = eff
                    .facility_demand
                    .iter()
                    .filter_map(|(_, item_name, utilization)| {
                        let item = item_map.get(item_name.as_str())?;
                        let Some((ref res, amount)) = item.byproduct else { return None };
                        (res == resource).then(|| (utilization / item.production_time) * amount as f64)
                    })
                    .sum();
                (coefficient > 0.0).then_some((*var, coefficient))
            })
            .collect();
        if terms.is_empty() {
            continue; // nothing can contribute to this resource, nothing to constrain
        }
        // The floor is normally the exact maximum a separate solve reached, so a hair of slack
        // keeps floating-point noise from turning "exactly the maximum" into "infeasible".
        problem.add_constraint(&terms, ComparisonOp::Ge, floor * (1.0 - 1e-6));
    }

    // Every variable is bounded by at least its own facility's constraint (facility_demand always
    // includes the item's own facility; see `accumulate_demand`), and every constraint's RHS is
    // a non-negative facility count, so this is always feasible and bounded without floors. A
    // byproduct floor this candidate set can't reach (e.g. after an exclusion pass removed the
    // item that carried it) falls back to the unfloored solve: prioritizing byproducts must never
    // turn a feasible plan into no plan at all. Degrade to "nothing selected" rather than panic if
    // even that fails.
    let Ok(solution) = problem.solve() else {
        if byproduct_floors.is_empty() {
            return HashMap::new();
        }
        return solve_facility_allocation(item_map, effs, facility_counts, coverage_bounds, &[]);
    };

    item_vars
        .into_iter()
        .filter_map(|(eff, var)| {
            let rate = solution[var];
            if rate > 1e-9 {
                Some((eff.item.name.as_str(), rate))
            } else {
                None
            }
        })
        .collect()
}

/// Converts fractional shares of a pool's capacity into whole counts, using the largest-remainder
/// method (the same apportionment technique used to allocate parliament seats) so the rounded
/// counts land as close as possible to the true fractions while still being integers; a player
/// assigns a whole plot to a crop, not a percentage of one, so "78% funnels into X" isn't
/// actionable the way "16 Farmland grow X" is. `fractions` need not sum to 1.0.
///
/// `round_up` controls how the TOTAL used is rounded, and the two modes matter for genuinely
/// different reasons:
/// - `false` (grower plots, and a coverage pool's chain shares): round to nearest. Any shortfall
///   between true continuous demand and `total` is genuinely idle capacity, not a rounding
///   artifact; `total` here is a real, already-integer scarce resource (owned plot count, or an
///   already-rounded coverage pool), so there's nothing to gain by rounding up past what the
///   fractions actually add up to.
/// - `true` (an environment BUILDING's continuous share; see `build_environment_assignment`):
///   round the total UP instead, capped at `total`. Configuring one more owned-but-otherwise-idle
///   building unit than the continuous LP strictly computed costs nothing (the unit is already
///   owned), but rounding it DOWN would silently under-cover what the continuous LP assumed was
///   available, corrupting every downstream rate for the item(s) relying on that coverage.
fn apportion_counts(fractions: &[f64], total: u32, round_up: bool) -> Vec<u32> {
    let total_f = total as f64;
    let ideal: Vec<f64> = fractions.iter().map(|f| f * total_f).collect();
    let mut counts: Vec<u32> = ideal.iter().map(|v| v.floor() as u32).collect();
    let sum = ideal.iter().sum::<f64>();
    let target = if round_up {
        (sum.ceil() as u32).min(total)
    } else {
        sum.round() as u32
    };
    let base: u32 = counts.iter().sum();
    let remaining = target.saturating_sub(base) as usize;

    let mut order: Vec<usize> = (0..ideal.len()).collect();
    order.sort_by(|&a, &b| {
        let remainder_a = ideal[a] - counts[a] as f64;
        let remainder_b = ideal[b] - counts[b] as f64;
        remainder_b.partial_cmp(&remainder_a).unwrap_or(std::cmp::Ordering::Equal)
    });
    for &i in order.iter().take(remaining) {
        counts[i] += 1;
    }
    counts
}

/// A GROWER facility (Farmland, Woodland, Mine, ...) has every plot committed to one crop
/// for its whole cycle; it structurally never hosts a processed item (see the data loaders in
/// `data.rs`: `load_farmland`/`load_woodland`/`load_workload_raw_material`/`load_nimbus_bed`
/// always set `raw_materials: None`; `load_processing_*` always set it to `Some`). A PROCESSOR
/// facility (Claw Game Cooker, Carousel Mill, ...) has no such constraint; it processes whatever
/// ingredients are ready, so genuinely cycling between recipes in whatever proportion their
/// inputs allow is real, achievable behavior, not something that needs rounding.
fn is_grower_facility(items: &[ProductionItem], name: &str) -> bool {
    !items.iter().any(|it| it.facility == name && it.raw_materials.is_some())
}

/// Whole units of every gatherer after rounding the continuous solve (see
/// `build_grower_assignment`).
struct GrowerAssignment {
    /// Whole units growing or mining each item, keyed by `(facility, item)`.
    units: std::collections::BTreeMap<(String, String), u32>,
    /// Units' worth of each item each chain can draw on, keyed by `(facility, chain, item)`: its
    /// continuous demand, scaled down when the item's whole units can't cover every chain using it.
    by_chain: HashMap<(String, String, String), f64>,
}

/// Apportions every grower facility's continuous LP shares into whole units (see
/// `apportion_counts`), one facility at a time. A unit grows or mines one item, and any chain can
/// use any unit's harvest of that item, so shares are pooled by item first: two chains each wanting
/// half a Well's water share one Well rather than rounding each half separately (which would hand
/// the Well to one of them and strand the other). Keyed by owned `String`s rather than borrowing
/// from `allocation`/`eff_by_name`, since this is called against *trial* candidate sets that may
/// get discarded (see `find_production_plan`'s exclusion loops) as well as the final settled one.
///
/// One chain can draw several distinct items from the same facility (e.g. caramel_nut_chips needs
/// walnut, chestnut, AND maple_syrup, all grown on Woodland); each item is its own share of the
/// facility's units.
///
/// `environment_assignment` (see `build_environment_assignment`, which must be computed BEFORE
/// this) caps each environment-gated item's demand on the grower pool at what it can actually use
/// once environment coverage is rounded to whole buildings; without this, an item whose
/// continuous rate assumed more coverage than rounds down to (e.g. the continuous LP finding 1.17
/// Cooling Units enough, which only rounds to 1 whole unit) would keep reserving MORE grower
/// plots than it can truly use, silently starving whatever plots should fall back to a
/// non-gated/differently-gated crop instead. Capping the demand here; rather than only in
/// `final_rate_for` afterwards; lets the freed-up plots flow to that fallback the same way
/// competing chains already share a grower facility, instead of just vanishing.
fn build_grower_assignment(
    items: &[ProductionItem],
    allocation: &HashMap<&str, f64>,
    eff_by_name: &HashMap<&str, &ProductionEfficiency>,
    facility_counts: &FacilityCounts,
    environment_assignment: &HashMap<(String, String, String), u32>,
) -> GrowerAssignment {
    let mut grower_shares: HashMap<&str, Vec<(&str, &str, f64)>> = HashMap::new();
    for (&name, &rate) in allocation {
        let eff = eff_by_name[name];
        for (facility, item_name, utilization) in &eff.facility_demand {
            if !is_grower_facility(items, facility) {
                continue;
            }
            let capacity = facility_counts.get_count(facility) as f64;
            if capacity <= 0.0 {
                continue;
            }
            let mut fraction = utilization * rate / capacity;
            if let Some(&env_count) =
                environment_assignment.get(&(facility.clone(), name.to_string(), item_name.clone()))
            {
                fraction = fraction.min(env_count as f64 / capacity);
            }
            if fraction <= 0.0 {
                continue;
            }
            grower_shares.entry(facility.as_str()).or_default().push((name, item_name.as_str(), fraction));
        }
    }

    let mut assignment = GrowerAssignment { units: std::collections::BTreeMap::new(), by_chain: HashMap::new() };
    for (&facility, shares) in &grower_shares {
        // Per-item totals, sorted so rounding ties go the same way every run, not by `HashMap`
        // order.
        let mut item_shares: std::collections::BTreeMap<&str, f64> = std::collections::BTreeMap::new();
        for &(_, item_name, fraction) in shares {
            *item_shares.entry(item_name).or_default() += fraction;
        }
        let fractions: Vec<f64> = item_shares.values().copied().collect();
        let capacity = facility_counts.get_count(facility);
        let counts = apportion_counts(&fractions, capacity, false);
        let mut item_units: HashMap<&str, (u32, f64)> = HashMap::new();
        for ((&item_name, &fraction), &count) in item_shares.iter().zip(&counts) {
            item_units.insert(item_name, (count, fraction * capacity as f64));
            if count > 0 {
                assignment.units.insert((facility.to_string(), item_name.to_string()), count);
            }
        }
        for &(chain_name, item_name, fraction) in shares {
            let (count, demand) = item_units[item_name];
            let wanted = fraction * capacity as f64;
            let scale = if demand > 0.0 { (count as f64 / demand).min(1.0) } else { 0.0 };
            assignment
                .by_chain
                .insert((facility.to_string(), chain_name.to_string(), item_name.to_string()), wanted * scale);
        }
    }
    assignment
}

/// Caps an item's continuous LP rate by what its grower facilities can ACTUALLY supply once
/// rounded to whole units (see `build_grower_assignment`), AND by whatever environment coverage
/// (see `build_environment_assignment`) it's actually been assigned; a Farmland/Woodland crop
/// needing a growing environment is bound by both its raw plot assignment and its environment
/// assignment independently, since they're two separate whole-unit-rounded resources. This is the
/// only correction needed: the continuous rate already correctly accounts for fair sharing at
/// every PROCESSOR facility (that's what solving the LP jointly rather than greedily buys us), so
/// redoing that math independently per item here; instead of taking the min against the
/// untouched continuous rate; would silently reintroduce a shared-resource double-counting bug
/// (e.g. Claw Game Cooker's three-way split would let each item assume exclusive access to it
/// again).
fn final_rate_for(
    items: &[ProductionItem],
    item_map: &HashMap<&str, &ProductionItem>,
    eff: &ProductionEfficiency,
    continuous_rate: f64,
    grower_assignment: &GrowerAssignment,
    environment_assignment: &HashMap<(String, String, String), u32>,
) -> f64 {
    eff.facility_demand
        .iter()
        .filter(|(f, _, _)| is_grower_facility(items, f))
        .fold(continuous_rate, |bound, (facility, item_name, utilization)| {
            if *utilization <= 0.0 {
                return bound;
            }
            // Each entry now names its own specific item, so this chain's rate is bounded
            // independently by EVERY grower item it draws from (the min across all of them),
            // exactly what's needed for a chain that requires multiple distinct crops from the
            // same or different facilities (e.g. caramel_nut_chips needs walnut, chestnut, AND
            // maple_syrup; if any one of those is short, the whole chain is bottlenecked by it).
            let assigned = grower_assignment
                .by_chain
                .get(&(facility.clone(), eff.item.name.clone(), item_name.clone()))
                .copied()
                .unwrap_or(0.0);
            let mut bound = bound.min(assigned / utilization);

            // Every grower facility whose hosted crop needs an environment is capacity-gated now
            // (see `crate::coverage`); so this simplifies to just checking the crop itself.
            let needs_environment = item_map.get(item_name.as_str()).is_some_and(|item| item.environment.is_some());
            if needs_environment {
                let env_assigned = environment_assignment
                    .get(&(facility.clone(), eff.item.name.clone(), item_name.clone()))
                    .copied()
                    .unwrap_or(0);
                bound = bound.min(env_assigned as f64 / utilization);
            }
            bound
        })
}

/// Sums every candidate's actual, POST-ROUNDING contribution to the plan's total value:
/// `final_rate_for(...) * batch_value` for each entry with a positive final rate. Used by
/// `find_production_plan`'s marginal-chain exclusion check to compare whole-plan totals between
/// candidate sets, since the continuous LP's own objective value can overstate what's truly
/// achievable once grower/environment rounding is applied (see `final_rate_for`'s doc comment);
/// the rounded total is the only number that reflects what a player can actually execute.
fn total_final_value(
    items: &[ProductionItem],
    item_map: &HashMap<&str, &ProductionItem>,
    allocation: &HashMap<&str, f64>,
    eff_by_name: &HashMap<&str, &ProductionEfficiency>,
    grower_assignment: &GrowerAssignment,
    environment_assignment: &HashMap<(String, String, String), u32>,
) -> f64 {
    allocation
        .iter()
        .map(|(&name, &rate)| {
            let eff = eff_by_name[name];
            let final_rate = final_rate_for(items, item_map, eff, rate, grower_assignment, environment_assignment);
            if final_rate > 0.0 {
                eff.batch_value * final_rate
            } else {
                0.0
            }
        })
        .sum()
}

/// Apportions each (facility-type, environment) coverage pool's continuous LP demand into
/// authoritative whole-plot counts per contributing chain; mirrors `build_grower_assignment`
/// exactly (same largest-remainder `apportion_counts` call), just pooled by the coverage an
/// environment building provides instead of by raw facility ownership. Unlike the old
/// preset-based version, the pool's `total` here is already an EXACT integer straight from
/// `solve_facility_allocation`'s packing placements; no building-rounding step needed first
/// (see `crate::coverage`'s doc comment: the whole environment side of that solve is integer).
///
/// A single chain's *share* of a coverage pool can still exceed the pool's `total` in rare cases
/// (e.g. several competing chains' continuous demand summing to more than the integer solve
/// actually allotted that combination, a residual-imprecision case analogous to the one noted on
/// `build_grower_assignment`); `apportion_counts` assumes its input fractions are genuine
/// proportions of the whole (summing to at most 1), so a share above 1 would otherwise pass
/// straight through unclamped and hand out more coverage than physically exists. Normalizing by
/// the demand sum whenever it exceeds 1 (below) is the actual correctness guarantee.
///
/// Returns `environment_assignment`, keyed identically to `grower_assignment`
/// (`(facility_type, chain_name, item_name) -> assigned whole plots`) for `final_rate_for`'s
/// capping. The item is part of the key for the same reason as `build_grower_assignment`: one
/// chain can draw several distinct items from the same facility type, needing DIFFERENT (or no)
/// environment coverage each; e.g. caramel_nut_chips draws walnut (needs Cool), chestnut (needs
/// none), and maple_syrup (needs Warm), all from Woodland.
fn build_environment_assignment(
    item_map: &HashMap<&str, &ProductionItem>,
    allocation: &HashMap<&str, f64>,
    eff_by_name: &HashMap<&str, &ProductionEfficiency>,
    placements: &HashMap<&'static str, Vec<(crate::coverage::Placement, u32)>>,
) -> HashMap<(String, String, String), u32> {
    // Coverage actually available per (facility_type, environment) pool; summed directly from
    // the exact integer placement counts.
    let mut coverage: HashMap<(&str, &str), u32> = HashMap::new();
    for (&mode, placed) in placements {
        for (p, count) in placed {
            *coverage.entry((p.facility.as_str(), mode)).or_insert(0) += count;
        }
    }

    // Each chain's continuous fractional share of whichever (facility_type, environment) pool it
    // draws from; mirrors `build_grower_assignment`'s loop structure exactly, now per (chain,
    // item) share rather than per chain, since each `facility_demand` entry names its own item.
    let mut pool_shares: HashMap<(&str, &str), Vec<(&str, &str, f64)>> = HashMap::new();
    for (&name, &rate) in allocation {
        let eff = eff_by_name[name];
        for (facility, item_name, utilization) in &eff.facility_demand {
            let facility_type = facility.as_str();
            let Some(env) = item_map.get(item_name.as_str()).and_then(|item| item.environment.as_deref()) else {
                continue;
            };
            let Some(&pool_total) = coverage.get(&(facility_type, env)) else {
                continue;
            };
            if pool_total == 0 {
                continue;
            }
            let fraction = utilization * rate / pool_total as f64;
            if fraction <= 0.0 {
                continue;
            }
            pool_shares.entry((facility_type, env)).or_default().push((name, item_name.as_str(), fraction));
        }
    }

    let mut environment_assignment: HashMap<(String, String, String), u32> = HashMap::new();
    for (&(facility_type, env), shares) in &mut pool_shares {
        // Sorted so rounding ties go the same way every run, not by `HashMap` order.
        shares.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        let fractions: Vec<f64> = shares.iter().map(|(_, _, f)| *f).collect();
        let total = coverage[&(facility_type, env)];
        let demand_sum: f64 = fractions.iter().sum();
        let normalizer = demand_sum.max(1.0);
        let normalized: Vec<f64> = fractions.iter().map(|f| f / normalizer).collect();
        let counts = apportion_counts(&normalized, total, false);
        for (&(chain_name, item_name, _), &count) in shares.iter().zip(&counts) {
            environment_assignment.insert(
                (facility_type.to_string(), chain_name.to_string(), item_name.to_string()),
                count,
            );
        }
    }

    environment_assignment
}

/// How many whole units `units` of capacity takes (a unit is set to one recipe and left running,
/// so any use at all takes a whole one).
fn whole_units(units: f64) -> u32 {
    if units <= 1e-9 {
        0
    } else {
        (units - 1e-9).ceil() as u32
    }
}

/// The whole processor units each hosted item gets from a settled plan, keyed by `(facility,
/// item)`: every contributor rounded up to whole units, most valuable first, never past what's
/// owned.
fn dedicated_processor_units(
    usage: &HashMap<&str, Vec<(&ProductionEfficiency, &str, f64, f64)>>,
    facility_counts: &FacilityCounts,
) -> HashMap<(String, String), u32> {
    let mut units: HashMap<(String, String), u32> = HashMap::new();
    for (&facility, contributors) in usage {
        let mut contributors = contributors.clone();
        contributors.sort_by(|a, b| {
            b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.1.cmp(b.1))
        });
        let mut remaining = facility_counts.get_count(facility);
        for (_, item_name, units_needed, _) in contributors {
            let give = whole_units(units_needed).min(remaining);
            remaining -= give;
            if give > 0 {
                *units.entry((facility.to_string(), item_name.to_string())).or_default() += give;
            }
        }
    }
    units
}

/// Whole units of each gatherer (Farmland, Mine, Well, ...) growing or mining each item, keyed by
/// `(facility, item)`, from `grower_assignment`. An environment-gated item keeps no more units than
/// it has coverage for. Units the rounding left idle grow the facility's most valuable item to sell
/// directly, if it has one that needs no environment.
fn pool_grower_units(
    items: &[ProductionItem],
    effs: &[ProductionEfficiency],
    facility_counts: &FacilityCounts,
    item_map: &HashMap<&str, &ProductionItem>,
    grower_assignment: &GrowerAssignment,
    environment_assignment: &HashMap<(String, String, String), u32>,
) -> std::collections::BTreeMap<(String, String), u32> {
    let mut units = grower_assignment.units.clone();
    let mut covered: HashMap<(&str, &str), u32> = HashMap::new();
    for ((facility, _, item_name), &count) in environment_assignment {
        *covered.entry((facility.as_str(), item_name.as_str())).or_default() += count;
    }
    for ((facility, item_name), count) in units.iter_mut() {
        if item_map.get(item_name.as_str()).is_some_and(|item| item.environment.is_some()) {
            *count = (*count).min(covered.get(&(facility.as_str(), item_name.as_str())).copied().unwrap_or(0));
        }
    }

    let mut facilities: Vec<&str> = items.iter().map(|i| i.facility.as_str()).collect::<HashSet<&str>>().into_iter().collect();
    facilities.sort_unstable();
    for facility in facilities {
        let owned = facility_counts.get_count(facility);
        if owned == 0 || !is_grower_facility(items, facility) {
            continue;
        }
        let used: u32 = units.iter().filter(|((f, _), _)| f == facility).map(|(_, c)| c).sum();
        let idle = owned.saturating_sub(used);
        if idle == 0 {
            continue;
        }
        let per_unit_value = |eff: &ProductionEfficiency| -> Option<f64> {
            let [(f, item_name, utilization)] = eff.facility_demand.as_slice() else { return None };
            let sells_itself = f == facility && *item_name == eff.item.name && eff.item.facility == facility;
            let usable = eff.item.environment.is_none()
                && eff.item.raw_materials.is_none()
                && facility_counts.capacity_at_level(facility, eff.item.facility_level) >= owned;
            (sells_itself && usable && *utilization > 0.0 && eff.batch_value > 0.0)
                .then(|| eff.batch_value / utilization)
        };
        let best = effs
            .iter()
            .filter_map(|eff| per_unit_value(eff).map(|value| (eff, value)))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal).then_with(|| b.0.item.name.cmp(&a.0.item.name)));
        if let Some((eff, _)) = best {
            *units.entry((facility.to_string(), eff.item.name.clone())).or_default() += idle;
        }
    }
    units.retain(|_, count| *count > 0);
    units
}

/// Re-solves every rate within the plan's whole units. The continuous solve and its rounding
/// decide how many units of each gatherer produce each item and which processor units run which
/// recipe; but a unit rounded up (or shared by two chains) produces its full output, not the
/// fraction the continuous solve needed. This maximizes the same objective as
/// `solve_facility_allocation` over every candidate, with each `(facility, item)` capped at its
/// whole units, so that output goes to whichever chains can use it or sells directly.
///
/// Spare processor units (owned but running nothing) can take new recipes, as long as every new
/// recipe still gets whole units; a chain that would crowd them is dropped and the rest re-solved.
/// The settled plan's own rates always fit these caps, so this never does worse than it.
fn fill_whole_units<'a>(
    items: &[ProductionItem],
    item_map: &HashMap<&str, &ProductionItem>,
    effs: &'a [ProductionEfficiency],
    facility_counts: &FacilityCounts,
    grower_units: &std::collections::BTreeMap<(String, String), u32>,
    processor_units: &HashMap<(String, String), u32>,
) -> HashMap<&'a str, f64> {
    use std::collections::BTreeMap;
    let item_level = |name: &str| item_map.get(name).map_or(1, |item| item.facility_level);
    let mut spare: BTreeMap<&str, u32> = BTreeMap::new();
    for item in items {
        let facility = item.facility.as_str();
        if spare.contains_key(facility) || is_grower_facility(items, facility) {
            continue;
        }
        let claimed: u32 = processor_units.iter().filter(|((f, _), _)| f == facility).map(|(_, c)| c).sum();
        spare.insert(facility, facility_counts.get_count(facility).saturating_sub(claimed));
    }
    let is_new_host = |facility: &str, item_name: &str| {
        !is_grower_facility(items, facility) && !processor_units.contains_key(&(facility.to_string(), item_name.to_string()))
    };

    let solve = |banned: &HashSet<&str>, allow_new: bool| -> Option<Vec<(&'a ProductionEfficiency, f64)>> {
        let mut problem = Problem::new(OptimizationDirection::Maximize);
        let mut vars: Vec<(&'a ProductionEfficiency, microlp::Variable)> = Vec::new();
        let mut capped: BTreeMap<(&str, &str), Vec<(microlp::Variable, f64)>> = BTreeMap::new();
        let mut new_hosts: BTreeMap<&str, Vec<(microlp::Variable, f64)>> = BTreeMap::new();
        'effs: for eff in effs {
            if eff.batch_value <= 0.0
                || banned.contains(eff.item.name.as_str())
                || facility_counts.get_count(&eff.item.facility) == 0
            {
                continue;
            }
            for (facility, item_name, utilization) in &eff.facility_demand {
                if *utilization <= 0.0 {
                    continue;
                }
                let servable = if is_grower_facility(items, facility) {
                    grower_units.contains_key(&(facility.clone(), item_name.clone()))
                } else if is_new_host(facility, item_name) {
                    allow_new
                        && spare.get(facility.as_str()).copied().unwrap_or(0) > 0
                        && facility_counts.capacity_at_level(facility, item_level(item_name))
                            >= facility_counts.get_count(facility)
                } else {
                    true
                };
                if !servable {
                    continue 'effs;
                }
            }
            let var = problem.add_var(eff.batch_value, (0.0, f64::INFINITY));
            vars.push((eff, var));
            for (facility, item_name, utilization) in &eff.facility_demand {
                if *utilization <= 0.0 {
                    continue;
                }
                // A chain can list the same row twice (two items made at one spare processor);
                // the solver needs one term per variable, so those add up.
                let terms = if is_new_host(facility, item_name) {
                    new_hosts.entry(facility.as_str()).or_default()
                } else {
                    capped.entry((facility.as_str(), item_name.as_str())).or_default()
                };
                match terms.last_mut() {
                    Some((last, coefficient)) if *last == var => *coefficient += utilization,
                    _ => terms.push((var, *utilization)),
                }
            }
        }
        for ((facility, item_name), terms) in &capped {
            let key = (facility.to_string(), item_name.to_string());
            let cap = grower_units.get(&key).or_else(|| processor_units.get(&key)).copied().unwrap_or(0);
            problem.add_constraint(terms, ComparisonOp::Le, cap as f64);
        }
        for (facility, terms) in &new_hosts {
            problem.add_constraint(terms, ComparisonOp::Le, spare.get(facility).copied().unwrap_or(0) as f64);
        }
        let solution = problem.solve().ok()?;
        Some(vars.into_iter().map(|(eff, var)| (eff, solution[var])).filter(|(_, rate)| *rate > 1e-9).collect())
    };

    let to_map = |rates: Vec<(&'a ProductionEfficiency, f64)>| -> HashMap<&'a str, f64> {
        rates.into_iter().map(|(eff, rate)| (eff.item.name.as_str(), rate)).collect()
    };
    let mut banned: HashSet<&str> = HashSet::new();
    for _ in 0..32 {
        let Some(rates) = solve(&banned, true) else { break };
        // Whole units each new recipe on a spare processor needs.
        let mut new_units: BTreeMap<(&str, &str), f64> = BTreeMap::new();
        for (eff, rate) in &rates {
            for (facility, item_name, utilization) in &eff.facility_demand {
                if *utilization > 0.0 && is_new_host(facility, item_name) {
                    *new_units.entry((facility.as_str(), item_name.as_str())).or_default() += utilization * rate;
                }
            }
        }
        let mut needed: BTreeMap<&str, u32> = BTreeMap::new();
        for ((facility, _), units) in &new_units {
            *needed.entry(facility).or_default() += whole_units(*units);
        }
        let crowded: Vec<&str> = needed
            .iter()
            .filter(|(facility, n)| **n > spare.get(**facility).copied().unwrap_or(0))
            .map(|(facility, _)| *facility)
            .collect();
        if crowded.is_empty() {
            return to_map(rates);
        }
        let victim = rates
            .iter()
            .filter(|(eff, _)| {
                eff.facility_demand.iter().any(|(f, i, u)| *u > 0.0 && crowded.contains(&f.as_str()) && is_new_host(f, i))
            })
            .min_by(|a, b| {
                (a.0.batch_value * a.1)
                    .partial_cmp(&(b.0.batch_value * b.1))
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.0.item.name.cmp(&b.0.item.name))
            })
            .map(|(eff, _)| eff.item.name.as_str());
        match victim {
            Some(name) => {
                banned.insert(name);
            }
            None => break,
        }
    }
    solve(&HashSet::new(), false).map(to_map).unwrap_or_default()
}

/// Processor facility -> each `(chain, hosted item, units of capacity needed, rate_per_second)`
/// under the plan's final rates; see `build_processor_usage` for what "units needed" means.
fn processor_usage_from_rates<'a>(
    items: &[ProductionItem],
    rates: &HashMap<&'a str, f64>,
    eff_by_name: &HashMap<&'a str, &'a ProductionEfficiency>,
    facility_counts: &FacilityCounts,
) -> HashMap<&'a str, Vec<(&'a ProductionEfficiency, &'a str, f64, f64)>> {
    let mut usage: HashMap<&str, Vec<(&ProductionEfficiency, &str, f64, f64)>> = HashMap::new();
    for (&name, &rate) in rates {
        let eff = eff_by_name[name];
        let rate_per_second = eff.batch_value * rate;
        for (facility, item_name, utilization) in &eff.facility_demand {
            if is_grower_facility(items, facility) || facility_counts.get_count(facility) == 0 {
                continue;
            }
            let units_needed = utilization * rate;
            if units_needed < 1e-6 {
                continue;
            }
            usage.entry(facility.as_str()).or_default().push((eff, item_name.as_str(), units_needed, rate_per_second));
        }
    }
    usage
}

/// For every PROCESSOR facility touched by any candidate item with a positive final rate, the
/// `(item_name, units of capacity needed, rate_per_second)` for each item hosted there; reused
/// both to detect "genuine contention" (more distinct dedicated jobs wanting a facility than it
/// has units; see `find_production_plan`'s exclusion loop) and to build the final `coin_items`
/// facility-plan rows, so this logic isn't duplicated.
///
/// One entry PER DISTINCT ITEM a chain hosts at this facility, not one combined entry per chain;
/// a player sets each physical unit to run ONE recipe and leaves it running, never cycling a
/// single unit between recipes, so a chain needing an intermediate made at the same facility type
/// as its own final item (e.g. wool_fabric also needing woolen_yarn made at Joy Wheel Loom first)
/// genuinely needs a separate dedicated unit for each hop, not one unit time-sharing both.
///
/// The "units of capacity needed" figure is `utilization * rate` directly, not normalized against
/// owned capacity; see the comment at its computation below for why dividing by capacity there
/// would be wrong for anything needing more than one whole unit.
fn build_processor_usage<'a>(
    items: &[ProductionItem],
    item_map: &HashMap<&str, &ProductionItem>,
    allocation: &HashMap<&'a str, f64>,
    eff_by_name: &HashMap<&'a str, &'a ProductionEfficiency>,
    facility_counts: &FacilityCounts,
    grower_assignment: &GrowerAssignment,
    environment_assignment: &HashMap<(String, String, String), u32>,
) -> HashMap<&'a str, Vec<(&'a ProductionEfficiency, &'a str, f64, f64)>> {
    let mut usage: HashMap<&str, Vec<(&ProductionEfficiency, &str, f64, f64)>> = HashMap::new();
    for (&name, &continuous_rate) in allocation {
        let eff = eff_by_name[name];
        let rate = final_rate_for(items, item_map, eff, continuous_rate, grower_assignment, environment_assignment);
        if rate <= 0.0 {
            continue;
        }
        let rate_per_second = eff.batch_value * rate;
        for (facility, item_name, utilization) in &eff.facility_demand {
            if is_grower_facility(items, facility) {
                continue;
            }
            let capacity = facility_counts.get_count(facility) as f64;
            if capacity <= 0.0 {
                continue;
            }
            // `utilization * rate` is already, on its own, "how many whole facility units this
            // item's rate needs" (utilization is seconds of this facility's time per one batch of
            // `eff`'s own rate, so multiplying by a batches/sec rate cancels to a dimensionless
            // unit-count); see `accumulate_demand`'s doc comment and the identical quantity used
            // directly in `solve_facility_allocation`'s own capacity constraint. Do not divide by
            // `capacity` here: that would turn this into a fraction of total owned capacity, which
            // is silently wrong for anything needing more than one whole unit (e.g. one item alone
            // dominating 5 of 5 owned Crafting Tables would come out as a fraction of 1.0, ceiling
            // to 1 dedicated unit instead of the true 5).
            let units_needed = utilization * rate;
            if units_needed < 0.001 {
                continue; // negligible, not worth reporting
            }
            usage.entry(facility.as_str()).or_default().push((eff, item_name.as_str(), units_needed, rate_per_second));
        }
    }
    usage
}

/// The true maximum byproduct rate achievable for `byproduct_currency` (`"wood_blocks"` or
/// `"mineral_sand"`) GIVEN `coverage_bounds`, computed by running the same
/// efficiencies → facility-allocation LP pipeline `find_production_plan` itself uses, with that
/// byproduct as the sole target and no floor of its own. `coverage_bounds` is the REAL solve's own
/// environment-coverage allocation (see `find_production_plan_with_progress`'s doc comment on why
/// this can't solve its own, byproduct-priced coverage independently): a byproduct-yielding item
/// competing for a shared environment building (e.g. Woodland's pine, needing "Adequate") gets
/// however much of that building the real, currency-priced candidates actually leave it, not
/// however much it could claim if every byproduct-yielding item had that coverage all to itself.
/// Used by `find_production_plan`'s `prioritize_byproducts` to compute the floor it then forces
/// the real (profit-targeting) solve to meet; a floor computed any more optimistically than what
/// the real solve's own coverage allocation can actually support would force an unreachable
/// target, making the whole plan spuriously infeasible.
fn max_achievable_byproduct_rate(
    items: &[ProductionItem],
    byproduct_currency: &str,
    facility_counts: &FacilityCounts,
    module_levels: &ModuleLevels,
    coverage_bounds: &HashMap<(String, String), u32>,
) -> f64 {
    let item_map: HashMap<&str, &ProductionItem> = items.iter().map(|i| (i.name.as_str(), i)).collect();
    let effs = calculate_efficiencies(items, byproduct_currency, facility_counts, module_levels);
    let allocation = solve_facility_allocation(&item_map, &effs, facility_counts, coverage_bounds, &[]);
    effs.iter().filter_map(|eff| allocation.get(eff.item.name.as_str()).map(|&rate| eff.batch_value * rate)).sum()
}

/// Runs the environment-coverage-pricing + packing + two-pass-refinement + stranded/marginal
/// exclusion pipeline against one candidate set, returning `None` only if NOTHING is profitable
/// anywhere (genuinely infeasible). Factored out of `find_production_plan` so its own
/// environment-coverage-CHOICE exclusion pass (see that function's doc comment) can re-run this
/// whole pipeline against a candidate set with one more chain excluded, to test whether ceding an
/// environment-gated chain's building coverage to a competing chain does better overall; the
/// SAME "try excluding, keep it only if the real total improves" pattern already used inside this
/// pipeline for stranded/contended chains, just one level up (coverage-CHOICE rather than
/// facility-plot rounding).
/// Forwards `trial_count`'s new value to `on_progress`, if one was supplied; called after every
/// trial solve throughout `find_production_plan_with_progress`'s pipeline, so a caller (currently
/// only `wasm.rs`'s `find_plan`, for driving a real progress bar) can observe genuine solve
/// progress as it happens rather than a value simulated independently of the actual computation.
/// `&dyn Fn` (not `FnMut`) specifically so this reference can be freely reused across the many
/// sequential call sites below without fighting the borrow checker over exclusive access; the
/// callback itself doesn't need to mutate anything on the Rust side, only forward the count to
/// whatever JS callback wraps it.
fn report_progress(trial_count: u32, on_progress: Option<&dyn Fn(u32)>) {
    if let Some(cb) = on_progress {
        cb(trial_count);
    }
}

fn solve_environment_and_facility_allocation(
    items: &[ProductionItem],
    item_map: &HashMap<&str, &ProductionItem>,
    effs: &[ProductionEfficiency],
    facility_counts: &FacilityCounts,
    byproduct_floors: &[(&str, f64)],
    trial_count: &mut u32,
    on_progress: Option<&dyn Fn(u32)>,
) -> Option<(
    HashMap<(&'static str, &'static str), u32>,
    HashMap<&'static str, Vec<(crate::coverage::Placement, u32)>>,
    HashMap<&'static str, Vec<Vec<crate::coverage::Placement>>>,
    HashMap<(String, String), u32>,
    Vec<ProductionEfficiency>,
)> {
    // Environment coverage (see `solve_facility_allocation`'s doc comment for why this is solved
    // separately from, rather than jointly with, the continuous item-rate LP below) is priced by
    // `compute_coverage_weights`'s own doc comment as "profit per plot IF fully dedicated"; a
    // deliberately static, decoupled estimate to avoid a chicken-and-egg bootstrap. That estimate
    // can be badly wrong for a deep, multi-hop chain whose OWN other ingredients are themselves
    // bottlenecked: a chain needing several other constrained facilities can price its share of a
    // scarce coverage pool higher than a genuinely simpler, more valuable chain, even though the
    // deep chain never actually gets produced once every other constraint is accounted for;
    // reserving that coverage for it is pure waste, starving the simpler chain for nothing. Two
    // passes: solve once with the naive (possibly-optimistic) weights, see which chains
    // ACTUALLY end up producing something once the real joint solve (including grower/environment
    // rounding) settles, then recompute weights using ONLY those and re-solve once more; a
    // phantom chain that never survives the real solve can no longer skew coverage priority away
    // from something that does. Capped at exactly one refinement (not an open-ended fixed point)
    // to bound the added cost to roughly double, the same kind of tractability trade
    // `MAX_MARGINAL_EXCLUSIONS` below already makes.
    let mut coverage_weights = compute_coverage_weights(item_map, effs);
    let mut refined_once = false;
    let (mode_counts, placements, layouts, coverage_bounds, candidates) = loop {
        let (mode_counts, placements, layouts) = solve_environment_coverage(&coverage_weights, facility_counts);
        let mut coverage_bounds: HashMap<(String, String), u32> = HashMap::new();
        for (&mode, placed) in &placements {
            for (p, count) in placed {
                *coverage_bounds.entry((p.facility.clone(), mode.to_string())).or_insert(0) += count;
            }
        }

        // Solve for the provably-optimal simultaneous allocation of every owned facility's
        // capacity across every candidate item; see `solve_facility_allocation`'s doc comment
        // for why this replaces the old greedy-plus-leftover-patches approach entirely.
        //
        // A chain needing TWO OR MORE different grower facilities (e.g. maple_candy_star needs
        // both Woodland's maple_syrup and Starfall Hammock's star) can get "stranded": each
        // grower is apportioned independently (`build_grower_assignment`), so it's possible for
        // one of them to round that chain's share all the way down to zero (its capacity going
        // instead to something more valuable) while the OTHER grower still shows a whole-unit
        // assignment to a chain that can now never actually produce anything. Detect that and
        // re-solve with the dead chain excluded, so the LP finds the stranded facility's
        // genuinely useful alternative instead of recommending a dedication to nothing; repeats
        // until stable (each pass excludes at least one more item, and the candidate set is
        // small, so this converges quickly).
        let mut excluded: HashSet<String> = HashSet::new();
        // Bounds the marginal-chain exclusion pass below (see its own comment) to at most this
        // many exclusions total; each one re-solves the whole candidate set, so this caps that
        // pass's worst-case added cost to a fixed multiple of one solve rather than letting it
        // scale with candidate count. A plan this deep into marginal, single-plot-level
        // improvements is already very close to optimal; past this point, trade the last sliver
        // of precision for a bounded runtime, the same kind of tractability trade the rest of
        // this function already makes.
        const MAX_MARGINAL_EXCLUSIONS: u32 = 20;
        let mut marginal_exclusions = 0u32;
        let candidates: Vec<ProductionEfficiency> = 'exclusion: loop {
            let trial: Vec<ProductionEfficiency> =
                effs.iter().filter(|e| !excluded.contains(&e.item.name)).cloned().collect();
            let trial_allocation =
                solve_facility_allocation(item_map, &trial, facility_counts, &coverage_bounds, byproduct_floors);
            *trial_count += 1;
            report_progress(*trial_count, on_progress);
            if trial_allocation.is_empty() {
                // Nothing profitable available anywhere; genuinely infeasible.
                return None;
            }
            let trial_eff_by_name: HashMap<&str, &ProductionEfficiency> =
                trial.iter().map(|e| (e.item.name.as_str(), e)).collect();
            let trial_environment =
                build_environment_assignment(item_map, &trial_allocation, &trial_eff_by_name, &placements);
            let trial_growers = build_grower_assignment(
                items,
                &trial_allocation,
                &trial_eff_by_name,
                facility_counts,
                &trial_environment,
            );
            let stranded: Vec<String> = trial_allocation
                .iter()
                .filter(|&(&name, &rate)| {
                    let eff = trial_eff_by_name[name];
                    final_rate_for(items, item_map, eff, rate, &trial_growers, &trial_environment) <= 0.0
                })
                .map(|(&name, _)| name.to_string())
                .collect();

            // A processor facility can only ever be "set and left" on ONE recipe at a time; the
            // continuous LP relaxation's assumption that it can be fractionally time-shared
            // between several recipes isn't something a player can actually execute. When more
            // distinct recipes want a facility than it has units, keep the `owned` most
            // profitable ones (ranked by their own rate_per_second; every contributor needs
            // exactly one dedicated unit regardless of its fraction, see
            // `build_processor_usage`'s doc comment, so ranking by economic value directly
            // answers "which recipe is worth the unit") and exclude the rest, re-solving so the
            // LP finds their genuinely-usable alternative instead of recommending an
            // unexecutable fractional split.
            let trial_processor_usage = build_processor_usage(
                items,
                item_map,
                &trial_allocation,
                &trial_eff_by_name,
                facility_counts,
                &trial_growers,
                &trial_environment,
            );
            let mut contention_losers: Vec<String> = Vec::new();
            for (&facility, contributors) in &trial_processor_usage {
                let owned = facility_counts.get_count(facility) as usize;
                // Each DISTINCT item a chain hosts here needs its own dedicated unit (see
                // `build_processor_usage`'s doc comment); a two-hop chain (e.g. wool_fabric also
                // needing woolen_yarn made here first) needs two whole units, not one shared
                // fractionally. Group by chain, sum each chain's own hop count, and greedily keep
                // the most profitable chains (by rate_per_second, identical across all of one
                // chain's own hops) until owned capacity runs out; excluding a losing chain
                // WHOLLY (never just one of its hops) once it can't fit.
                let mut hops_per_chain: HashMap<&str, (usize, f64)> = HashMap::new();
                for &(eff, _item_name, _fraction, rate_per_second) in contributors {
                    let entry = hops_per_chain.entry(eff.item.name.as_str()).or_insert((0, rate_per_second));
                    entry.0 += 1;
                }
                let mut sorted: Vec<(&str, usize, f64)> =
                    hops_per_chain.into_iter().map(|(name, (hops, rate))| (name, hops, rate)).collect();
                // Name breaks ties so equal-rate chains don't fall back on `HashMap` order, which
                // Rust randomizes per run and would make the same input give different plans.
                sorted.sort_by(|a, b| {
                    b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.0.cmp(b.0))
                });
                let mut remaining = owned;
                for (chain_name, hops, _rate) in sorted {
                    if hops <= remaining {
                        remaining -= hops;
                    } else {
                        contention_losers.push(chain_name.to_string());
                    }
                }
            }

            if !stranded.is_empty() || !contention_losers.is_empty() {
                excluded.extend(stranded);
                excluded.extend(contention_losers);
                continue;
            }

            if marginal_exclusions >= MAX_MARGINAL_EXCLUSIONS {
                break 'exclusion trial;
            }

            // No fully-stranded or contended chains left; but independent per-facility rounding
            // (`build_grower_assignment`) only prevents a chain's rate from rounding all the way
            // to ZERO; it doesn't ask whether keeping that chain's rounded plots is actually the
            // more profitable use of them. A chain can survive with a small, real-but-worse
            // foothold on a shared grower facility instead of being properly out-competed by
            // whatever else could be done with those same plots. This shows up in two different
            // shapes: (a) two SEPARATE chains sharing one facility, e.g. a small quick_lemon share
            // for premium_lemon_incense surviving rounding while worth less than giving those
            // Woodland plots to pine/cedarwood_incense instead; (b) a SINGLE chain spanning
            // multiple items at one facility, e.g. caramel_nut_chips's walnut+chestnut+maple_syrup
            // split of Woodland surviving rounding while worth less than dropping the whole chain
            // and selling walnut alone. Case (b) means the right test isn't "does this facility
            // have more than one CHAIN on it" (that misses a single chain using several items
            // there) but "does it have more than one DISTINCT ITEM on it"; any chain touching
            // such a facility can possibly be improved by dropping it, since a chain with the
            // facility's sole item has nothing to lose it to. Sorted for determinism; `HashSet`
            // iteration order is randomized per-process, and picking between candidates based on
            // that would make the same input produce different plans across runs.
            let current_total = total_final_value(
                items,
                item_map,
                &trial_allocation,
                &trial_eff_by_name,
                &trial_growers,
                &trial_environment,
            );
            let mut items_per_facility: HashMap<&str, HashSet<&str>> = HashMap::new();
            for ((facility, _chain_name, item_name), &count) in &trial_growers.by_chain {
                if count <= 1e-9 {
                    continue;
                }
                items_per_facility.entry(facility.as_str()).or_default().insert(item_name.as_str());
            }
            let contested_facilities: HashSet<&str> =
                items_per_facility.into_iter().filter(|(_, items)| items.len() > 1).map(|(f, _)| f).collect();
            let mut sharing_candidates: Vec<&str> = trial_growers
                .by_chain
                .iter()
                .filter(|((facility, _, _), &count)| count > 1e-9 && contested_facilities.contains(facility.as_str()))
                .map(|((_, chain_name, _), _)| chain_name.as_str())
                .collect::<HashSet<&str>>()
                .into_iter()
                .collect();
            sharing_candidates.sort_unstable();

            let mut improved = false;
            for candidate in sharing_candidates {
                let mut test_excluded = excluded.clone();
                test_excluded.insert(candidate.to_string());
                let test_trial: Vec<ProductionEfficiency> =
                    effs.iter().filter(|e| !test_excluded.contains(&e.item.name)).cloned().collect();
                let test_allocation = solve_facility_allocation(
                    item_map,
                    &test_trial,
                    facility_counts,
                    &coverage_bounds,
                    byproduct_floors,
                );
                if test_allocation.is_empty() {
                    continue;
                }
                let test_eff_by_name: HashMap<&str, &ProductionEfficiency> =
                    test_trial.iter().map(|e| (e.item.name.as_str(), e)).collect();
                let test_environment =
                    build_environment_assignment(item_map, &test_allocation, &test_eff_by_name, &placements);
                let test_growers = build_grower_assignment(
                    items,
                    &test_allocation,
                    &test_eff_by_name,
                    facility_counts,
                    &test_environment,
                );
                let test_total = total_final_value(
                    items,
                    item_map,
                    &test_allocation,
                    &test_eff_by_name,
                    &test_growers,
                    &test_environment,
                );
                if test_total > current_total + 1e-9 {
                    excluded.insert(candidate.to_string());
                    marginal_exclusions += 1;
                    improved = true;
                    break;
                }
            }
            if improved {
                continue;
            }

            break 'exclusion trial;
        };

        if refined_once {
            break (mode_counts, placements, layouts, coverage_bounds, candidates);
        }

        // Which of the settled candidates actually end up producing something, once grower and
        // environment rounding are both applied; the same "true, rounded outcome" check
        // `total_final_value` uses, not the raw continuous LP rate.
        let pass_allocation =
            solve_facility_allocation(item_map, &candidates, facility_counts, &coverage_bounds, byproduct_floors);
        let pass_eff_by_name: HashMap<&str, &ProductionEfficiency> =
            candidates.iter().map(|e| (e.item.name.as_str(), e)).collect();
        let pass_environment =
            build_environment_assignment(item_map, &pass_allocation, &pass_eff_by_name, &placements);
        let pass_growers =
            build_grower_assignment(items, &pass_allocation, &pass_eff_by_name, facility_counts, &pass_environment);
        let producing: Vec<ProductionEfficiency> = candidates
            .iter()
            .filter(|eff| {
                let rate = pass_allocation.get(eff.item.name.as_str()).copied().unwrap_or(0.0);
                final_rate_for(items, item_map, eff, rate, &pass_growers, &pass_environment) > 0.0
            })
            .cloned()
            .collect();

        let refined_weights = compute_coverage_weights(item_map, &producing);
        refined_once = true;
        if refined_weights == coverage_weights {
            // Nothing would change; skip the redundant second pass.
            break (mode_counts, placements, layouts, coverage_bounds, candidates);
        }
        coverage_weights = refined_weights;
    };

    Some((mode_counts, placements, layouts, coverage_bounds, candidates))
}

/// Re-solves the whole environment-coverage-and-allocation pipeline with `to_exclude` added to
/// `excluded`, returning the resulting plan's real total value alongside the state needed to keep
/// it if it wins; `None` only if excluding everything in `to_exclude` makes the candidate set
/// infeasible (nothing profitable anywhere). Factored out of `find_production_plan`'s
/// environment-coverage-CHOICE exclusion loop so it can try excluding more than one candidate at
/// once (see that loop's own doc comment for why a single exclusion at a time isn't always
/// enough).
#[allow(clippy::too_many_arguments)]
fn try_environment_exclusion_set(
    items: &[ProductionItem],
    item_map: &HashMap<&str, &ProductionItem>,
    effs: &[ProductionEfficiency],
    facility_counts: &FacilityCounts,
    byproduct_floors: &[(&str, f64)],
    trial_count: &mut u32,
    on_progress: Option<&dyn Fn(u32)>,
    excluded: &HashSet<String>,
    to_exclude: &[&str],
) -> Option<(
    f64,
    HashMap<(&'static str, &'static str), u32>,
    HashMap<&'static str, Vec<(crate::coverage::Placement, u32)>>,
    HashMap<&'static str, Vec<Vec<crate::coverage::Placement>>>,
    HashMap<(String, String), u32>,
    Vec<ProductionEfficiency>,
)> {
    let mut test_excluded = excluded.clone();
    for &name in to_exclude {
        test_excluded.insert(name.to_string());
    }
    let test_effs: Vec<ProductionEfficiency> =
        effs.iter().filter(|e| !test_excluded.contains(&e.item.name)).cloned().collect();
    let (test_mode_counts, test_placements, test_layouts, test_coverage_bounds, test_candidates) =
        solve_environment_and_facility_allocation(
            items,
            item_map,
            &test_effs,
            facility_counts,
            byproduct_floors,
            trial_count,
            on_progress,
        )?;
    let test_allocation = solve_facility_allocation(
        item_map,
        &test_candidates,
        facility_counts,
        &test_coverage_bounds,
        byproduct_floors,
    );
    *trial_count += 1;
    report_progress(*trial_count, on_progress);
    let test_eff_by_name: HashMap<&str, &ProductionEfficiency> =
        test_candidates.iter().map(|e| (e.item.name.as_str(), e)).collect();
    let test_environment =
        build_environment_assignment(item_map, &test_allocation, &test_eff_by_name, &test_placements);
    let test_growers =
        build_grower_assignment(items, &test_allocation, &test_eff_by_name, facility_counts, &test_environment);
    let test_total = total_final_value(
        items,
        item_map,
        &test_allocation,
        &test_eff_by_name,
        &test_growers,
        &test_environment,
    );
    Some((test_total, test_mode_counts, test_placements, test_layouts, test_coverage_bounds, test_candidates))
}

/// Solves for the provably-optimal simultaneous use of every owned facility for one target,
/// a currency (`"coins"`) or byproduct pseudo-currency (`"wood_blocks"`/
/// `"mineral_sand"`; see `byproduct_resource_name`), matching `calculate_efficiencies`'
/// `target_currency`. Target-independent: no goal amount is needed to know the best achievable
/// rate and facility plan. Pass the result to `time_to_reach_goal` to find out how long a
/// specific goal takes.
///
/// `prioritize_byproducts`, when `true` and `currency` is itself a real currency (not already a
/// byproduct target; prioritizing would be a no-op there, the whole plan already IS byproduct
/// maximization), forces the solve to hit the true maximum achievable Wood Blocks AND Mineral Sand
/// rate first (see `max_achievable_byproduct_rate`), then optimizes `currency` profit with
/// whatever facility capacity is left over; a hard, real trade of profit for guaranteed maximum
/// byproduct output, not a soft weighting. This is a real in-game constraint some players face
/// (Wood Blocks/Mineral Sand are needed elsewhere and can't just be bought), so it's opt-in per
/// call rather than a fixed behavior.
pub fn find_production_plan(
    items: &[ProductionItem],
    currency: &str,
    facility_counts: &FacilityCounts,
    module_levels: &ModuleLevels,
    prioritize_byproducts: bool,
) -> Option<ProductionPlan> {
    find_production_plan_with_progress(items, currency, facility_counts, module_levels, prioritize_byproducts, None)
}

/// Same as [`find_production_plan`], with an optional `on_progress` callback invoked (via
/// `report_progress`) after every trial solve throughout the whole pipeline; factored out as its
/// own function, rather than adding the parameter directly to `find_production_plan`, so every
/// existing call site (the large majority: every test, plus any future non-wasm caller) is
/// unaffected and keeps calling the simple, progress-less version. Currently only `wasm.rs`'s
/// `find_plan` calls this directly, to drive a real progress bar from genuine solve progress
/// instead of a value simulated independently of the actual computation.
pub fn find_production_plan_with_progress(
    items: &[ProductionItem],
    currency: &str,
    facility_counts: &FacilityCounts,
    module_levels: &ModuleLevels,
    prioritize_byproducts: bool,
    on_progress: Option<&dyn Fn(u32)>,
) -> Option<ProductionPlan> {
    PACKING_CACHE.with(|cache| cache.borrow_mut().clear());
    ALLOCATION_CACHE.with(|cache| cache.borrow_mut().clear());
    let item_map: HashMap<&str, &ProductionItem> =
        items.iter().map(|i| (i.name.as_str(), i)).collect();

    let effs = calculate_efficiencies(items, currency, facility_counts, module_levels);

    // See `find_production_plan`'s own doc comment on `prioritize_byproducts`: computed once
    // upfront (each is its own full sub-solve) and reused as a fixed floor for every
    // `solve_facility_allocation` call below, the same way `coverage_bounds` is.
    //
    // Each byproduct's own maximum is computed against `currency`'s OWN environment-coverage
    // allocation (below), not a coverage pre-solve priced from the byproduct alone. A
    // byproduct-yielding item can ALSO be environment-gated (e.g. Woodland's pine, needing
    // "Adequate"), and if that coverage were priced purely by byproduct value, a byproduct-only
    // sub-solve would happily hand it every scrap of a shared building's coverage; the REAL solve
    // below prices that same coverage by `currency` value instead, and a genuinely more profitable
    // `currency` candidate elsewhere (e.g. Farmland's grape, also wanting "Adequate") can win most
    // of it, leaving the byproduct-yielding item far less than the isolated sub-solve assumed.
    // Computing the floor against the coverage IT WILL ACTUALLY GET, rather than the coverage it
    // could claim in isolation, is what keeps this floor honest: forcing a floor no real solve can
    // reach makes the whole plan spuriously infeasible, exactly the failure this avoids.
    let byproduct_floors: Vec<(&str, f64)> = if prioritize_byproducts && byproduct_resource_name(currency).is_none() {
        let coverage_weights = compute_coverage_weights(&item_map, &effs);
        let (_, placements, _) = solve_environment_coverage(&coverage_weights, facility_counts);
        let mut coverage_bounds: HashMap<(String, String), u32> = HashMap::new();
        for (&mode, placed) in &placements {
            for (p, count) in placed {
                *coverage_bounds.entry((p.facility.clone(), mode.to_string())).or_insert(0) += count;
            }
        }
        [("wood_blocks", "Wood Blocks"), ("mineral_sand", "Mineral Sand")]
            .into_iter()
            .map(|(target, resource)| {
                (
                    resource,
                    max_achievable_byproduct_rate(items, target, facility_counts, module_levels, &coverage_bounds),
                )
            })
            .collect()
    } else {
        Vec::new()
    };

    let mut trial_count: u32 = 0;
    let (mut mode_counts, mut placements, mut layouts, mut coverage_bounds, mut candidates) =
        solve_environment_and_facility_allocation(
            items,
            &item_map,
            &effs,
            facility_counts,
            &byproduct_floors,
            &mut trial_count,
            on_progress,
        )?;

    // Environment-coverage-CHOICE exclusion: `solve_environment_and_facility_allocation` already
    // fixes a phantom-coverage bug (a chain whose price LOOKED good but never actually produces
    // anything hogging a building's coverage), but that's not the only way static per-plot pricing
    // can mislead the packing pre-solve. Two chains can BOTH genuinely produce something and still
    // be a bad joint choice: a chain whose environment is covered by a scarce building type can
    // outbid a competing chain for it, even when that competing chain would do better sharing a
    // more abundant building type's larger pool with something else. Since coverage is priced from
    // each chain's OWN static economics with no idea how many buildings of each competing
    // environment exist, it can't see this trade-off coming; the only way to know is to actually
    // try the alternative and compare real totals, the same "try excluding, keep it only if it
    // truly helps" pattern `solve_environment_and_facility_allocation`'s own marginal-exclusion
    // pass already uses one level down (for facility-plot rounding rather than environment-coverage
    // choice). A shared PROCESSOR facility (not just an environment building) can hide the exact
    // same kind of mistake: `solve_environment_and_facility_allocation`'s own processor-contention
    // resolution is a greedy, rate_per_second-ranked choice with no visibility into whether the
    // chain it favors is starving a different chain sharing that same processor, so this pass's
    // candidate search also includes any chain currently occupying a processor more than one
    // candidate could use, not just chains touching a contested environment mode; and every round
    // tries EVERY single candidate AND EVERY PAIR, keeping whichever shows the best total overall
    // -- not just whichever single candidate sorts first, and NOT preferring a single exclusion
    // just because it found SOME improvement. That second point used to be a real bug: a single
    // exclusion that shows a small real improvement got applied and locked in immediately
    // (`environment_excluded` is permanent once a round applies to it), which permanently
    // prevented ever trying that SAME candidate paired with another one from a clean baseline --
    // even when the pair would have unlocked a dramatically bigger improvement the single-only
    // choice was quietly blocking. See
    // `test_environment_coverage_choice_finds_a_real_improvement_even_when_incomplete`'s doc
    // comment for a concrete, confirmed-by-hand case of exactly this. This still isn't an
    // exhaustive search over every possible COMBINATION of exclusions (that would require trying
    // every subset of every shared-resource group, and each trial reruns the full packing
    // pipeline, making a truly exhaustive search too slow for a live tool on some scenarios) --
    // just singles and pairs, repeated across up to `MAX_ENVIRONMENT_EXCLUSIONS` rounds (each
    // round's pair search gets its own fresh wall-clock budget, not one shared across rounds,
    // since an earlier round's search must never starve a later round that needs its own pair
    // search to find a genuinely different improvement against the new baseline).
    //
    // Each trial here reruns that ENTIRE pipeline (packing ILP included), unlike the cheap LP-only
    // marginal-exclusion pass inside it, so this pass is bounded by total OWNED environment-
    // building count rather than wall-clock time: each owned building widens that type's
    // placement-variable integer range in `solve_building_packing`'s ILP (see its own doc
    // comment), so more owned buildings means a larger branch & bound regardless of system load;
    // a deterministic proxy that stays correct under CPU contention, unlike timing the solve
    // directly.
    let total_environment_buildings: u32 = ENVIRONMENT_BUILDINGS
        .iter()
        .map(|&(building, _)| facility_counts.get_count(building))
        .sum();
    const MAX_ENVIRONMENT_BUILDINGS_FOR_EXCLUSION_PASS: u32 = 12;

    let mut environment_excluded: HashSet<String> = HashSet::new();
    // Bounds this pass the same way `MAX_MARGINAL_EXCLUSIONS` bounds the one inside
    // `solve_environment_and_facility_allocation`; each exclusion re-runs that entire pipeline
    // (itself not free), so this caps the added cost to a small fixed multiple rather than letting
    // it scale with how many environment-gated chains are in play.
    const MAX_ENVIRONMENT_EXCLUSIONS: u32 = 3;
    for _ in 0..MAX_ENVIRONMENT_EXCLUSIONS {
        if total_environment_buildings > MAX_ENVIRONMENT_BUILDINGS_FOR_EXCLUSION_PASS {
            break;
        }
        let allocation =
            solve_facility_allocation(&item_map, &candidates, facility_counts, &coverage_bounds, &byproduct_floors);
        trial_count += 1;
        report_progress(trial_count, on_progress);
        let eff_by_name: HashMap<&str, &ProductionEfficiency> =
            candidates.iter().map(|e| (e.item.name.as_str(), e)).collect();
        let environment_assignment = build_environment_assignment(&item_map, &allocation, &eff_by_name, &placements);
        let grower_assignment =
            build_grower_assignment(items, &allocation, &eff_by_name, facility_counts, &environment_assignment);
        let current_total =
            total_final_value(items, &item_map, &allocation, &eff_by_name, &grower_assignment, &environment_assignment);

        // Only a chain that's (a) currently actually producing something (excluding a chain
        // that's already at zero can't possibly free up coverage anyone else can use) and (b)
        // needs an environment mode that's ALSO needed by some OTHER chain among ALL original
        // candidates (no genuine alternative claimant, no possible improvement from excluding it)
        // is worth the cost of a full trial re-solve. Keyed by MODE alone, not `(facility_type,
        // mode)`: every mode maps to exactly one physical building type (see
        // `ENVIRONMENT_BUILDINGS`), so a Farmland chain and a Woodland chain both needing
        // "Adequate" genuinely compete for the same Sunlamp's coverage even though they're
        // different facility types; keying by the pair would treat them as separate, uncontested
        // pools and miss exactly the trade-off this pass exists to catch. Sorted for determinism,
        // same reason as every other exclusion pass in this function.
        let mut environment_modes: HashMap<&str, HashSet<&str>> = HashMap::new();
        for eff in &effs {
            for (facility, item_name, _) in &eff.facility_demand {
                if !crate::coverage::ENVIRONMENT_GATED_FACILITIES.iter().any(|(f, _)| f == facility) {
                    continue;
                }
                if let Some(env) = item_map.get(item_name.as_str()).and_then(|i| i.environment.as_deref()) {
                    environment_modes.entry(eff.item.name.as_str()).or_default().insert(env);
                }
            }
        }
        let contested_modes: HashSet<&str> = {
            let mut counts: HashMap<&str, HashSet<&str>> = HashMap::new();
            for (&chain, modes) in &environment_modes {
                for &mode in modes {
                    counts.entry(mode).or_default().insert(chain);
                }
            }
            counts.into_iter().filter(|(_, chains)| chains.len() > 1).map(|(mode, _)| mode).collect()
        };
        // One representative per contested mode, not every chain touching one; a full trial
        // re-solve here reruns the entire coverage-packing pipeline (not cheap, unlike the LP-only
        // marginal-exclusion pass above), so this bounds the trial count by how many distinct
        // SCARCE coverage pools are actually contested rather than by how many chains happen to
        // touch one, which would retry every touching chain even when a scenario has no actual
        // coverage-choice improvement to find. Prefer a currently-producing candidate (excluding
        // it directly frees real capacity someone else could use); but a mode can ALSO end up with
        // its coverage claimed by a chain that produces nothing at all once every other constraint
        // (e.g. a downstream processor another chain has since monopolized) settles, so fall back
        // to any not-yet-excluded candidate touching the mode rather than silently dropping it: a
        // zero-producing claimant can still be pinning the packing's static per-plot pricing toward
        // a facility type that never converts that coverage into anything, starving a genuinely
        // better alternative of the chance to be repriced at all.
        // Owned `String`s, not `&str` borrowed from `candidates`: the exclusion trials below
        // reassign `candidates` on success, which a borrow tied to the pre-trial `candidates`
        // couldn't survive across.
        let mut trial_candidate_set: HashSet<String> = HashSet::new();
        for &mode in &contested_modes {
            let touches_mode = |eff: &&ProductionEfficiency| {
                !environment_excluded.contains(&eff.item.name)
                    && environment_modes.get(eff.item.name.as_str()).is_some_and(|modes| modes.contains(mode))
            };
            // EVERY currently-producing candidate touching this mode, not just the first one
            // found: two different crops can both be producing off the SAME contested mode at
            // once (e.g. one covered by a Cooling Unit, the other by a Sunlamp both giving
            // "Cool"), and only trying whichever happens to be first in `candidates`'s iteration
            // order can miss the one actually worth excluding (confirmed by hand: `quick_rose`
            // and `walnut` were simultaneously producing for the same contested mode, but
            // iteration order meant only `walnut` ever got a trial, silently hiding `quick_rose`
            // -- the one whose exclusion is needed for the real improvement documented on
            // `test_environment_coverage_choice_finds_a_real_improvement_even_when_incomplete`).
            let producing: Vec<&String> = candidates
                .iter()
                .filter(touches_mode)
                .filter(|eff| {
                    let rate = allocation.get(eff.item.name.as_str()).copied().unwrap_or(0.0);
                    final_rate_for(items, &item_map, eff, rate, &grower_assignment, &environment_assignment) > 0.0
                })
                .map(|eff| &eff.item.name)
                .collect();
            if producing.is_empty() {
                // A mode can ALSO end up with its coverage claimed by a chain that produces
                // nothing at all once every other constraint settles (see this pass's own doc
                // comment above); fall back to any not-yet-excluded candidate touching the mode
                // rather than silently dropping it.
                if let Some(eff) = candidates.iter().filter(touches_mode).min_by_key(|eff| &eff.item.name) {
                    trial_candidate_set.insert(eff.item.name.clone());
                }
            } else {
                for name in producing {
                    trial_candidate_set.insert(name.clone());
                }
            }
        }

        // A shared PROCESSOR facility can hide the same joint suboptimality a contested
        // environment mode can: `solve_environment_and_facility_allocation`'s own marginal-
        // exclusion pass resolves processor contention greedily, by raw rate_per_second, with no
        // way to know that the chain it favors might be starving a DIFFERENT chain sharing the
        // same processor. Every chain CURRENTLY occupying a processor more than one distinct
        // candidate (across the full, unexcluded `effs`) could possibly use is added here so its
        // solo exclusion gets a trial below, the same as a contested-mode candidate; the RAW
        // continuous LP rate, not `final_rate_for`'s grower-rounded one, since a chain occupying a
        // processor's dedicated unit (see `build_processor_usage`'s doc comment: every contributor
        // needs exactly one whole unit regardless of its own fraction) is still "using" that
        // processor even when its own rate is too small for grower-plot rounding to show any whole
        // output.
        let mut processor_chains: HashMap<&str, HashSet<&str>> = HashMap::new();
        for eff in &effs {
            for (facility, _item_name, utilization) in &eff.facility_demand {
                if *utilization <= 0.0 || is_grower_facility(items, facility) {
                    continue;
                }
                processor_chains.entry(facility.as_str()).or_default().insert(eff.item.name.as_str());
            }
        }
        let contested_processors: HashSet<&str> =
            processor_chains.into_iter().filter(|(_, chains)| chains.len() > 1).map(|(f, _)| f).collect();
        // One group per contested processor: every not-yet-excluded, currently-producing
        // candidate using it. Kept separately from `trial_candidate_set` (below) so it can ALSO
        // drive a dedicated, targeted exhaustive search over just that processor's own small
        // candidate set (see the block right after this loop) -- catching "two candidates sharing
        // one processor both need excluding together" directly and deterministically, rather than
        // relying on the general single-candidate-plus-pairs search to stumble onto the exact
        // right pair (it doesn't reliably: alphabetical tie-breaking among several EQUALLY-scored
        // single exclusions can pick a candidate unrelated to the real fix, and once picked, that
        // choice is permanent for the rest of this round, which can quietly prevent the two real
        // candidates from ever being tried together at all in the same round).
        // Sorted before iterating: `HashSet` iteration order is randomized per-process, and this
        // loop's insertion order becomes `processor_groups`' order, which later determines
        // tie-breaking (via a stable sort) among same-size groups in the dedicated exhaustive
        // search below -- without sorting first, which of two equally-sized, equally-improving
        // groups gets tried (and thus which winning combination gets applied, when they lead to
        // different downstream totals) could differ from run to run of the exact same input.
        let mut contested_processors_sorted: Vec<&str> = contested_processors.into_iter().collect();
        contested_processors_sorted.sort_unstable();
        let mut processor_groups: Vec<Vec<String>> = Vec::new();
        for facility in contested_processors_sorted {
            let group: Vec<String> = candidates
                .iter()
                .filter(|eff| !environment_excluded.contains(&eff.item.name))
                .filter(|eff| eff.facility_demand.iter().any(|(f, _, u)| f.as_str() == facility && *u > 0.0))
                .filter(|eff| allocation.get(eff.item.name.as_str()).copied().unwrap_or(0.0) > 0.0)
                .map(|eff| eff.item.name.clone())
                .collect();
            for name in &group {
                trial_candidate_set.insert(name.clone());
            }
            processor_groups.push(group);
        }
        processor_groups.sort_unstable();
        processor_groups.dedup();

        // A shared RAW INGREDIENT can hide a joint suboptimality that neither the contested-mode
        // nor the contested-processor grouping above can see: two different chains can EACH grow
        // their OWN copy of the SAME environment-gated raw item internally (e.g. two different
        // incense recipes each growing their own `quick_rose`), competing for that raw item's
        // Farmland/environment capacity even though they're never simultaneously the one thing
        // with positive LP rate -- the solver picks whichever ONE of them is locally best each
        // solve, so the contested-processor grouping above (which only includes candidates
        // CURRENTLY producing) never sees them together, and the contested-mode grouping only
        // includes a candidate once it's actually producing too. A candidate can still be
        // "pinning" the coverage-packing pre-solve's static per-plot pricing toward that raw item
        // even while its OWN current rate is zero (see this pass's own doc comment on phantom
        // coverage), so this group is NOT filtered by current production status at all -- every
        // not-yet-excluded chain that needs the ingredient is included, and the exhaustive search
        // below decides which (if any) combination is worth excluding. Confirmed by hand: this is
        // exactly the pattern behind `{premium_rose_incense, rose_incense}` both needing exclusion
        // together (see `test_environment_coverage_choice_finds_a_real_improvement_even_when_incomplete`).
        let mut raw_item_chains: HashMap<&str, HashSet<&str>> = HashMap::new();
        for eff in &effs {
            for (facility, item_name, _) in &eff.facility_demand {
                if !crate::coverage::ENVIRONMENT_GATED_FACILITIES.iter().any(|(f, _)| f == facility) {
                    continue;
                }
                if item_map.get(item_name.as_str()).and_then(|i| i.environment.as_deref()).is_some() {
                    raw_item_chains.entry(item_name.as_str()).or_default().insert(eff.item.name.as_str());
                }
            }
        }
        let mut ingredient_groups: Vec<Vec<String>> = Vec::new();
        for (_, chains) in raw_item_chains.into_iter().filter(|(_, chains)| chains.len() > 1) {
            let mut group: Vec<String> = chains
                .into_iter()
                .filter(|&name| !environment_excluded.contains(name))
                .map(String::from)
                .collect();
            group.sort_unstable();
            ingredient_groups.push(group);
        }
        ingredient_groups.sort_unstable();
        ingredient_groups.dedup();

        let mut trial_candidates: Vec<String> = trial_candidate_set.into_iter().collect();
        trial_candidates.sort_unstable();

        type ExclusionOutputs = (
            HashMap<(&'static str, &'static str), u32>,
            HashMap<&'static str, Vec<(crate::coverage::Placement, u32)>>,
            HashMap<&'static str, Vec<Vec<crate::coverage::Placement>>>,
            HashMap<(String, String), u32>,
            Vec<ProductionEfficiency>,
        );
        let mut best: Option<(f64, Vec<String>, ExclusionOutputs)> = None;

        // Dedicated exhaustive search over each contested processor's or shared ingredient's OWN
        // small candidate set, tried FIRST and directly: every non-empty subset of that one
        // group's candidates (bounded, since a real group here is a handful of recipes sharing a
        // couple of owned processor units or one raw ingredient, not hundreds -- capped
        // defensively regardless). This is what reliably catches "two candidates need excluding
        // together" (confirmed by hand: `{premium_rose_incense, rose_incense}`, both internally
        // growing their own `quick_rose`, together unlocking 46.64230303030303 coins/sec,
        // comfortably above the Farmland=28 baseline of 45.71963636363636); the general
        // single-candidate-plus-pairs search below can miss this exact pair even though it tries
        // pairs too, because several candidates can tie at the SAME improved total, and
        // alphabetical tie-breaking among them can permanently pick an unrelated one for the rest
        // of the round, in effect burying the two candidates that actually needed to be excluded
        // together before they ever get tried as a pair.
        // Kept small (unlike the small handful a processor group tends to have, an ingredient
        // group can be as large as however many recipes happen to share one raw item) and bounded
        // with a wall-clock deadline: 2^6 - 1 = 63 subsets is already a meaningfully complete
        // search for the common case, and the deadline is the real backstop against a
        // pathological input with several large groups each costing a full pipeline re-solve per
        // subset. Groups are tried smallest-first so a cheap, common 2-3-candidate group (the
        // usual case) always gets its full exhaustive search before a rare large one can eat the
        // whole budget.
        const MAX_PROCESSOR_GROUP_SIZE: usize = 6;
        // Total subset-evaluation budget for this whole phase, generous enough to fully cover
        // every group scenario actually observed (confirmed by hand: a 16-group case needing
        // 160-208 total subsets across successive rounds) with real margin to spare, while still
        // bounding worst-case work for a pathological input with many large groups.
        const MAX_DEDICATED_SEARCH_EVALS: u32 = 400;
        let mut sized_groups: Vec<&Vec<String>> = processor_groups
            .iter()
            .chain(ingredient_groups.iter())
            .filter(|group| group.len() >= 2 && group.len() <= MAX_PROCESSOR_GROUP_SIZE)
            .collect();
        sized_groups.sort_by_key(|group| group.len());
        // Decides which groups get searched BEFORE any solving starts, purely from each group's
        // own size (a fixed, precomputable quantity, not live wall-clock time): keep taking
        // groups smallest-first (as already sorted) until the next one would push the running
        // total past budget, then stop. For the same candidate set this always keeps the exact
        // same groups in the exact same order, regardless of machine speed or load -- unlike a
        // wall-clock deadline, whose cutoff point (and therefore which groups get searched at
        // all) depends on how fast the machine happens to be running at that moment. A real
        // pathological input (many large groups) still gets bounded, by dropping the largest,
        // costliest groups entirely rather than partially exploring whichever one the clock ran
        // out on.
        let mut budget_remaining = MAX_DEDICATED_SEARCH_EVALS;
        let mut bounded_groups: Vec<&Vec<String>> = Vec::new();
        for group in sized_groups {
            let cost = (1u32 << group.len()) - 1;
            if cost > budget_remaining {
                break;
            }
            budget_remaining -= cost;
            bounded_groups.push(group);
        }
        // A second cap, on LP solves rather than subsets: each subset re-runs the whole pipeline,
        // whose own cost grows with how many recipes are in play, so at late-game RV levels the
        // subset budget alone let one round spend ~17,000 LP solves (tens of seconds). Counting
        // solves keeps it deterministic (no wall clock); since groups run smallest-first and
        // subsets fewest-exclusions-first, what the cap cuts is the largest, least likely
        // combinations. Measured on RV 9-20 setups: 3-7x faster, within ~1% of the uncapped plan.
        const MAX_DEDICATED_SEARCH_TRIALS: u32 = 3000;
        let dedicated_search_start = trial_count;
        'groups: for group in bounded_groups {
            let n = group.len();
            let mut masks: Vec<u32> = (1..(1u32 << n)).collect();
            masks.sort_by_key(|mask| mask.count_ones());
            for mask in masks {
                if trial_count - dedicated_search_start >= MAX_DEDICATED_SEARCH_TRIALS {
                    break 'groups;
                }
                let to_exclude: Vec<&str> =
                    (0..n).filter(|&i| mask & (1 << i) != 0).map(|i| group[i].as_str()).collect();
                let Some((
                    test_total,
                    test_mode_counts,
                    test_placements,
                    test_layouts,
                    test_coverage_bounds,
                    test_candidates,
                )) = try_environment_exclusion_set(
                    items,
                    &item_map,
                    &effs,
                    facility_counts,
                    &byproduct_floors,
                    &mut trial_count,
                    on_progress,
                    &environment_excluded,
                    &to_exclude,
                ) else {
                    continue;
                };
                if test_total > current_total + 1e-9
                    && best.as_ref().is_none_or(|(best_total, ..)| test_total > *best_total)
                {
                    best = Some((
                        test_total,
                        to_exclude.into_iter().map(String::from).collect(),
                        (test_mode_counts, test_placements, test_layouts, test_coverage_bounds, test_candidates),
                    ));
                }
            }
        }

        // Tries every single candidate AND every pair this round, keeping whichever shows the
        // BEST total overall -- not just whichever single candidate sorts first, and NOT
        // preferring singles over pairs just because a single already found SOME improvement.
        // Compared against `best` above (already possibly set by the dedicated per-processor
        // search), so this can still win if a single candidate or a cross-group pair beats
        // whatever the per-processor search found.
        for candidate_name in &trial_candidates {
            let Some((test_total, test_mode_counts, test_placements, test_layouts, test_coverage_bounds, test_candidates)) =
                try_environment_exclusion_set(
                    items,
                    &item_map,
                    &effs,
                    facility_counts,
                    &byproduct_floors,
                    &mut trial_count,
                    on_progress,
                    &environment_excluded,
                    &[candidate_name.as_str()],
                )
            else {
                continue;
            };
            if test_total > current_total + 1e-9 && best.as_ref().is_none_or(|(best_total, ..)| test_total > *best_total)
            {
                best = Some((
                    test_total,
                    vec![candidate_name.clone()],
                    (test_mode_counts, test_placements, test_layouts, test_coverage_bounds, test_candidates),
                ));
            }
        }
        // Each pair trial costs as much as a full single-candidate trial, so trying every
        // combination scales as O(n^2) in `trial_candidates.len()`; this stays a small handful of
        // candidates in every scenario seen so far. Bounded the same way as the dedicated-group
        // search above: a fixed, precomputable total (here, how many candidates to include before
        // C(k,2) would exceed budget) decided before any solving starts, not a live wall-clock
        // deadline -- so the same input always tries the exact same pairs regardless of machine
        // speed. `MAX_PAIR_SEARCH_EVALS` is comfortably above any n^2 seen so far (n=10 -> 45
        // pairs) while still bounding a pathological input with many contested candidates.
        const MAX_PAIR_SEARCH_EVALS: usize = 2000;
        let mut pair_candidate_count = trial_candidates.len();
        while pair_candidate_count > 1 && pair_candidate_count * (pair_candidate_count - 1) / 2 > MAX_PAIR_SEARCH_EVALS {
            pair_candidate_count -= 1;
        }
        if pair_candidate_count > 1 {
            for i in 0..pair_candidate_count {
                for j in (i + 1)..pair_candidate_count {
                    let pair = [trial_candidates[i].as_str(), trial_candidates[j].as_str()];
                    let Some((
                        test_total,
                        test_mode_counts,
                        test_placements,
                        test_layouts,
                        test_coverage_bounds,
                        test_candidates,
                    )) = try_environment_exclusion_set(
                        items,
                        &item_map,
                        &effs,
                        facility_counts,
                        &byproduct_floors,
                        &mut trial_count,
                        on_progress,
                        &environment_excluded,
                        &pair,
                    ) else {
                        continue;
                    };
                    if test_total > current_total + 1e-9
                        && best.as_ref().is_none_or(|(best_total, ..)| test_total > *best_total)
                    {
                        best = Some((
                            test_total,
                            vec![pair[0].to_string(), pair[1].to_string()],
                            (test_mode_counts, test_placements, test_layouts, test_coverage_bounds, test_candidates),
                        ));
                    }
                }
            }
        }
        let mut improved = false;
        if let Some((_, names, (test_mode_counts, test_placements, test_layouts, test_coverage_bounds, test_candidates))) =
            best
        {
            for name in names {
                environment_excluded.insert(name);
            }
            mode_counts = test_mode_counts;
            placements = test_placements;
            layouts = test_layouts;
            coverage_bounds = test_coverage_bounds;
            candidates = test_candidates;
            improved = true;
        }
        if !improved {
            break;
        }
    }

    // `candidates` has now settled (no stranded chains, and no environment-coverage exclusion left
    // to try); solve once more against it so `allocation`/`eff_by_name`/`grower_assignment` all
    // borrow from a value that lives for the rest of the function, instead of threading trial
    // values out through the borrow checker. Cheap: this problem size solves in well under a
    // millisecond. `mode_counts` and `placements` (environment coverage) were already solved above,
    // reused as-is.
    let allocation =
        solve_facility_allocation(&item_map, &candidates, facility_counts, &coverage_bounds, &byproduct_floors);
    trial_count += 1;
    report_progress(trial_count, on_progress);
    let eff_by_name: HashMap<&str, &ProductionEfficiency> =
        candidates.iter().map(|e| (e.item.name.as_str(), e)).collect();
    let environment_assignment = build_environment_assignment(&item_map, &allocation, &eff_by_name, &placements);
    let grower_assignment = build_grower_assignment(
        items,
        &allocation,
        &eff_by_name,
        facility_counts,
        &environment_assignment,
    );
    let is_grower = |name: &str| is_grower_facility(items, name);

    // Final fill (see `fill_whole_units`): the rounding above settles how many whole units of each
    // gatherer grow or mine which item and which processor units run which recipe; this re-solves
    // every rate within those whole units, so their full output gets used or sold instead of just
    // the fraction the continuous solve happened to need.
    let processor_units = dedicated_processor_units(
        &build_processor_usage(
            items,
            &item_map,
            &allocation,
            &eff_by_name,
            facility_counts,
            &grower_assignment,
            &environment_assignment,
        ),
        facility_counts,
    );
    let grower_units =
        pool_grower_units(items, &effs, facility_counts, &item_map, &grower_assignment, &environment_assignment);
    let final_rates = fill_whole_units(items, &item_map, &effs, facility_counts, &grower_units, &processor_units);
    trial_count += 1;
    report_progress(trial_count, on_progress);
    let fill_by_name: HashMap<&str, &ProductionEfficiency> = effs.iter().map(|e| (e.item.name.as_str(), e)).collect();

    // One income stream per item the plan produces.
    let mut income_streams: Vec<PlanProduct> = Vec::new();
    for (&name, &rate) in &final_rates {
        let eff = fill_by_name[name];
        // The frontend recomputes each row's "worth" as floor(total_units) * sell_value, so
        // sell_value must mean "value earned per ITEM unit" in both modes: the coin price for a
        // currency target, or the byproduct amount per item unit (batch_value / yield_amount,
        // since batch_value is already per-batch) for a byproduct target.
        let sell_value = match byproduct_resource_name(currency) {
            Some(_) => eff.batch_value / eff.item.yield_amount as f64,
            None => eff.item.sell_value,
        };
        income_streams.push(PlanProduct {
            item_name: eff.item.name.clone(),
            facility: eff.item.facility.clone(),
            sell_value,
            rate_per_second: eff.batch_value * rate,
            units_per_second: rate * eff.item.yield_amount as f64,
            lead_time: item_lead_time(&eff.item.name, &item_map, 0),
            total_units: 0.0,
            total_value: 0.0,
        });
    }
    income_streams.sort_by(|a, b| a.item_name.cmp(&b.item_name));

    // Processor facility -> each (chain, hosted item, units of capacity needed, rate_per_second).
    let facility_usage = processor_usage_from_rates(items, &final_rates, &fill_by_name, facility_counts);

    let rate_per_second = income_streams.iter().map(|p| p.rate_per_second).sum();

    // Every distinct facility the user owns (count > 0), so the result can report on ALL owned
    // facilities; including ones with nothing profitable to produce right now, not just the
    // productive ones.
    let mut facility_names: Vec<&str> = items
        .iter()
        .map(|i| i.facility.as_str())
        .collect::<HashSet<&str>>()
        .into_iter()
        .filter(|name| facility_counts.get_count(name) > 0)
        .collect();
    facility_names.sort_unstable();

    // Wood Blocks/Mineral Sand produced as a side effect; purely informational (see doc comment
    // on `ProductionPlan::byproduct_rates`). A byproduct comes from growing or mining, so it's
    // credited from the whole units assigned to each item, whether or not every harvest ends up
    // used. Kept as a rate + lead time here; `time_to_reach_goal` turns these into totals once a
    // plan's duration is known.
    //
    // Skipped entirely when `currency` itself targets a byproduct: every candidate in that mode
    // already produces exactly that resource, so it's already the plan's PRIMARY income stream
    // above; crediting it again here would double-count it.
    let mut byproduct_rates: Vec<(String, f64, f64)> = Vec::new();
    if byproduct_resource_name(currency).is_none() {
        for ((facility, item_name), &count) in &grower_units {
            let Some(&item) = item_map.get(item_name.as_str()) else { continue };
            let Some((ref resource, amount)) = item.byproduct else { continue };
            let rate = count as f64 * amount as f64 / item.production_time;
            if count == 0 || rate <= 0.0 || facility_counts.get_count(facility) == 0 {
                continue;
            }
            let lead = item_lead_time(&item.name, &item_map, 0);
            byproduct_rates.push((resource.clone(), rate, lead));
        }
    }

    // "Sells directly", "Used for X", "Used for X, Y; the rest sells directly": what an item made
    // at `facility` goes to, across every chain drawing on it.
    let uses_of = |facility: &str, item_name: &str| -> Option<String> {
        let mut sells = false;
        let mut uses: Vec<String> = Vec::new();
        let mut names: Vec<&&str> = final_rates.keys().collect();
        names.sort_unstable();
        for &&chain in &names {
            let eff = fill_by_name[chain];
            if !eff.facility_demand.iter().any(|(f, i, u)| f == facility && i == item_name && *u > 0.0) {
                continue;
            }
            if chain == item_name && eff.item.facility == facility {
                sells = true;
            } else {
                let consumer =
                    direct_consumer(chain, item_name, &item_map, 0).unwrap_or_else(|| chain.to_string());
                if !uses.contains(&consumer) {
                    uses.push(consumer);
                }
            }
        }
        match (uses.is_empty(), sells) {
            (true, true) => Some("Sells directly".to_string()),
            (true, false) => None,
            (false, false) => Some(format!("Used for {}", uses.join(", "))),
            (false, true) => Some(format!("Used for {}; the rest sells directly", uses.join(", "))),
        }
    };

    let coin_items: Vec<PlanStep> = facility_names
        .iter()
        .flat_map(|&name| -> Vec<PlanStep> {
            if is_grower(name) {
                // One row per item grown or mined here, from the whole units assigned to it; every
                // chain drawing on that item shares the same units.
                let total_owned = facility_counts.get_count(name);
                let mut assigned: Vec<(&str, u32)> = grower_units
                    .iter()
                    .filter(|((facility, _), &count)| facility.as_str() == name && count > 0)
                    .map(|((_, item_name), &count)| (item_name.as_str(), count))
                    .collect();
                assigned.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

                if assigned.is_empty() {
                    return vec![PlanStep {
                        item_name: None,
                        facility: name.to_string(),
                        facility_count: total_owned,
                        status: PlanStepStatus::NothingAvailable,
                        reason: "Nothing it can make helps this plan".to_string(),
                        is_grower: true,
                        cycle_time: None,
                        environment: None,
                        busy_units: None,
                    }];
                }

                let mut idle = total_owned.saturating_sub(assigned.iter().map(|(_, c)| c).sum());
                let mut steps: Vec<PlanStep> = Vec::new();
                for (item_name, count) in assigned {
                    let Some(reason) = uses_of(name, item_name) else {
                        idle += count;
                        continue;
                    };
                    let item = item_map.get(item_name);
                    steps.push(PlanStep {
                        item_name: Some(crate::models::base_item_name(item_name).to_string()),
                        facility: name.to_string(),
                        facility_count: count,
                        status: PlanStepStatus::Producing,
                        reason,
                        is_grower: true,
                        cycle_time: item.map(|item| item.production_time),
                        environment: item.and_then(|item| item.environment.clone()),
                        busy_units: None,
                    });
                }
                if idle > 0 {
                    steps.push(PlanStep {
                        item_name: None,
                        facility: name.to_string(),
                        facility_count: idle,
                        status: PlanStepStatus::Idle,
                        reason: "No further profitable use found".to_string(),
                        is_grower: true,
                        cycle_time: None,
                        environment: None,
                        busy_units: None,
                    });
                }
                return steps;
            }

            // Processor facility: a player sets each unit to one recipe and leaves it running, so
            // every chain needing the same hosted item shares the units making it. One row per
            // hosted item, most valuable first; `fill_whole_units` already kept the whole units
            // this needs within what's owned.
            let mut hosted: Vec<(&str, f64, f64)> = Vec::new();
            for (_, item_name, units_needed, rate) in facility_usage.get(name).cloned().unwrap_or_default() {
                match hosted.iter_mut().find(|(i, _, _)| *i == item_name) {
                    Some(entry) => {
                        entry.1 += units_needed;
                        entry.2 += rate;
                    }
                    None => hosted.push((item_name, units_needed, rate)),
                }
            }
            hosted.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.0.cmp(b.0)));

            if hosted.is_empty() {
                return vec![PlanStep {
                    item_name: None,
                    facility: name.to_string(),
                    facility_count: facility_counts.get_count(name),
                    status: PlanStepStatus::NothingAvailable,
                    reason: "Nothing it can make helps this plan".to_string(),
                    is_grower: false,
                    cycle_time: None,
                    environment: None,
                    busy_units: None,
                }];
            }

            let owned = facility_counts.get_count(name);
            let mut remaining = owned;
            let mut steps: Vec<PlanStep> = Vec::new();
            for (item_name, units_needed, _) in hosted {
                let count = (whole_units(units_needed)).min(remaining);
                remaining -= count;
                steps.push(PlanStep {
                    item_name: Some(crate::models::base_item_name(item_name).to_string()),
                    facility: name.to_string(),
                    facility_count: count,
                    status: PlanStepStatus::Producing,
                    reason: uses_of(name, item_name).unwrap_or_else(|| "Sells directly".to_string()),
                    is_grower: false,
                    cycle_time: None,
                    environment: None,
                    busy_units: Some(units_needed.min(count as f64)),
                });
            }
            if remaining > 0 {
                steps.push(PlanStep {
                    item_name: None,
                    facility: name.to_string(),
                    facility_count: remaining,
                    status: PlanStepStatus::Idle,
                    reason: "No further profitable use found".to_string(),
                    is_grower: false,
                    cycle_time: None,
                    environment: None,
                    busy_units: None,
                });
            }
            steps
        })
        .collect();

    // Build the display-facing per-building breakdown directly from what `solve_environment_coverage`
    // already solved; `layouts` (one entry per assigned building instance, per mode) comes
    // straight out of `crate::coverage::solve_building_packing`'s own sequential per-building
    // assignment, so there's no separate re-solve needed here anymore (unlike the earlier
    // aggregate-then-decompose design).
    let environment_assignments: Vec<EnvironmentAssignment> = mode_counts
        .iter()
        .filter(|(_, &units)| units > 0)
        .map(|(&(building, mode), &units)| {
            let placed = placements.get(mode).cloned().unwrap_or_default();
            let mut covered: Vec<(String, u32)> = Vec::new();
            for (p, count) in &placed {
                match covered.iter_mut().find(|(name, _)| name == &p.facility) {
                    Some((_, total)) => *total += count,
                    None => covered.push((p.facility.clone(), *count)),
                }
            }
            let building_layouts: Vec<Vec<FacilityPlacement>> = layouts
                .get(mode)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|building_layout| {
                    building_layout
                        .into_iter()
                        .map(|p| FacilityPlacement { facility: p.facility, x: p.x, y: p.y, size: p.size })
                        .collect()
                })
                .collect();
            EnvironmentAssignment {
                building: building.to_string(),
                mode: mode.to_string(),
                units,
                covered,
                layouts: building_layouts,
                partner: None,
                zone: None,
                pair_modes: None,
            }
        })
        .collect();

    Some(ProductionPlan {
        currency: currency.to_string(),
        rate_per_second,
        income_streams,
        coin_items,
        byproduct_rates,
        environment_assignments,
        candidates_evaluated: effs.len() as u32,
        trial_solves: trial_count,
    })
}

/// Turns a [`ProductionPlan`] plus a goal amount into a concrete time-to-target, using the plan's
/// already-computed rates; no facility-allocation re-solve, so this is cheap enough to call on
/// every keystroke of a goal-amount input. Returns `None` only in the pathological case where the
/// binary search exceeds ~317 years without reaching `target` (treated as genuinely infeasible;
/// shouldn't happen for a `plan` returned by `find_production_plan`, whose `income_streams` always
/// has at least one item with positive `rate_per_second` when `Some` is returned).
pub fn time_to_reach_goal(plan: &ProductionPlan, target: f64, current: f64) -> Option<GoalResult> {
    let delta = (target - current).max(0.0);

    if delta <= 0.0 {
        return Some(GoalResult {
            total_time: 0.0,
            amount_produced: 0.0,
            products: vec![],
            byproducts: vec![],
            seed_requirements: vec![],
        });
    }

    // Amount accumulated by time `t`: each income stream contributes nothing until its own lead
    // time has passed, then its steady-state rate after that. Monotonically non-decreasing in
    // `t`, so the minimal `t` reaching `delta` can be found by binary search.
    let amount_at = |t: f64| -> f64 {
        plan.income_streams
            .iter()
            .map(|p| p.rate_per_second * (t - p.lead_time).max(0.0))
            .sum()
    };

    let mut hi = 3600.0_f64;
    while amount_at(hi) < delta {
        hi *= 2.0;
        if hi > 1e10 {
            // ~317 years; treat as genuinely infeasible.
            return None;
        }
    }
    let mut lo = 0.0_f64;
    for _ in 0..100 {
        let mid = (lo + hi) / 2.0;
        if amount_at(mid) >= delta {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Some(production_over(plan, hi))
}

/// What `plan` makes in `total_time` seconds: each income stream from its own lead time on, the
/// byproducts, and the seeds planted. The breakdown behind a coin goal, and behind a goal for
/// anything else once its time is known.
pub fn production_over(plan: &ProductionPlan, total_time: f64) -> GoalResult {
    let amount_produced: f64 =
        plan.income_streams.iter().map(|p| p.rate_per_second * (total_time - p.lead_time).max(0.0)).sum();

    // Fill in each income stream's actual contribution over the plan's duration, and drop any
    // that never got past their own lead time (they were claimed, but the target was reached
    // before they produced anything); reported per-facility in `plan.coin_items`, but not worth
    // a row in this item-level breakdown. Sorted by total worth, most valuable first.
    let mut products: Vec<PlanProduct> = plan
        .income_streams
        .iter()
        .cloned()
        .map(|mut p| {
            let active_time = (total_time - p.lead_time).max(0.0);
            p.total_value = p.rate_per_second * active_time;
            p.total_units = p.units_per_second * active_time;
            p
        })
        .filter(|p| p.total_value > 0.0)
        .collect();
    products.sort_by(|a, b| {
        b.total_value
            .partial_cmp(&a.total_value)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut byproduct_totals: HashMap<&str, f64> = HashMap::new();
    for (resource, rate, lead) in &plan.byproduct_rates {
        let active = (total_time - lead).max(0.0);
        *byproduct_totals.entry(resource.as_str()).or_insert(0.0) += rate * active;
    }
    let mut byproducts: Vec<(String, f64)> = byproduct_totals
        .into_iter()
        .filter(|(_, amount)| *amount > 0.0)
        .map(|(resource, amount)| (resource.to_string(), amount))
        .collect();
    byproducts.sort_by(|a, b| a.0.cmp(&b.0));

    // How many times each crop actually being produced needs to be replanted over the whole
    // duration; one seed per planting, ceiling (not floor) because a seed is already spent
    // starting a cycle that might still be in progress when `total_time` is reached, even though
    // that cycle's output isn't counted as a completed unit yet (see `SeedRequirement`'s doc
    // comment). Seeds only exist for Farmland and Woodland plots; Mine is mined (no
    // seed), and the Aniimo-dispatch facilities (Nimbus Bed, Floral Windmill, Starfall Hammock,
    // Tidewhisper Sandcastle, Dewy House) are harvested via family dispatch, not planted either.
    // Processor rows are skipped too: they aren't planted, so they never need seeds.
    let mut seed_requirements: Vec<SeedRequirement> = plan
        .coin_items
        .iter()
        .filter(|s| {
            (s.facility == "Farmland" || s.facility == "Woodland")
                && s.status == PlanStepStatus::Producing
        })
        .filter_map(|s| {
            let cycle_time = s.cycle_time?;
            if cycle_time <= 0.0 {
                return None;
            }
            let seeds_per_plot = (total_time / cycle_time).ceil() as u64;
            if seeds_per_plot == 0 {
                return None;
            }
            Some(SeedRequirement {
                facility: s.facility.clone(),
                item_name: s.item_name.clone().unwrap_or_default(),
                facility_count: s.facility_count,
                seeds_per_plot,
                total_seeds: seeds_per_plot * s.facility_count as u64,
            })
        })
        .collect();
    seed_requirements.sort_by_key(|r| std::cmp::Reverse(r.total_seeds));

    GoalResult {
        total_time,
        amount_produced,
        products,
        byproducts,
        seed_requirements,
    }
}
