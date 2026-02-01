//! Production optimization algorithms for the Aniimo optimizer.
//!
//! This module contains the core optimization logic that calculates
//! production efficiencies and finds the best production paths to
//! achieve currency goals.

use std::collections::{HashMap, HashSet};

use crate::models::{
    EnergyItemEfficiency, FacilityCounts, ModuleLevels, ProductionEfficiency, ProductionItem, ProductionPath,
    ProductionStep,
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
            let rounds = (*batches + k - 1) / k; // ceil(batches / k)
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
            candidate_times.push(((*batches + total_facilities - 1) / total_facilities) as f64 * time);
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
        let min_facilities = (*batches + max_rounds - 1) / max_rounds;
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
            (*batches + max_rounds - 1) / max_rounds
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
            
            let current_rounds = (*batches + current_facilities - 1) / current_facilities;
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
    fertilizer_time_per_unit: f64,
    nimbus_bed_count: f64,
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
    
    // Try to find the best variant of this item (check high_speed_ variant first)
    let high_speed_name = format!("high_speed_{}", item_name);
    let (item, actual_name) = {
        // Check if high_speed variant exists and is usable
        if let Some(hs_item) = item_map.get(&high_speed_name) {
            // Verify we can use the high_speed variant (check module requirements)
            let can_use_hs = if let Some((ref module_name, required_level)) = hs_item.module_requirement {
                module_levels.can_use(module_name, required_level)
            } else {
                true
            };
            // Also check facility requirements
            let can_produce_hs = facility_counts.can_produce(&hs_item.facility, hs_item.facility_level);
            
            if can_use_hs && can_produce_hs {
                // Use high_speed variant - it produces more in same/less time
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
    
    // Check fertilizer requirements
    if item.requires_fertilizer && nimbus_bed_count == 0.0 {
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
    
    visited.insert(actual_name.to_string());
    
    let result = if let Some(ref raw_mats) = item.raw_materials {
        // This is a processed item - recursively calculate requirements for each ingredient
        let required_amounts = item.required_amount.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
        
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
                fertilizer_time_per_unit,
                nimbus_bed_count,
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
            let high_speed_mat = format!("high_speed_{}", raw_mat);
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
        
        // Add processing time for this item
        let processing_facility_count = facility_counts.get_count(&item.facility) as f64;
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
        // This is a base raw material
        let facility_count = facility_counts.get_count(&item.facility) as f64;
        let batches_needed = (required_amount / item.yield_amount as f64).ceil();
        
        // Calculate time with parallel facilities
        let time_per_batch = item.production_time;
        let parallel_batches = (batches_needed / facility_count).ceil();
        
        // Add fertilizer time if required
        let fertilizer_time = if item.requires_fertilizer {
            fertilizer_time_per_unit * batches_needed
        } else {
            0.0
        };
        
        let total_time = time_per_batch * parallel_batches + fertilizer_time;
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
/// * `target_currency` - The currency to optimize for ("coins" or "coupons")
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
/// - They don't produce the target currency
/// - Their required raw materials aren't available at the raw material facility's level
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
/// let counts = FacilityCounts {
///     farmland: (4, 3),        // 4 farmlands at level 3
///     woodland: (1, 2),        // 1 woodland at level 2
///     mineral_pile: (1, 1),    // 1 mineral pile at level 1
///     carousel_mill: (2, 2),   // 2 carousel mills at level 2
///     jukebox_dryer: (1, 1),
///     crafting_table: (1, 1),
///     dance_pad_polisher: (1, 1),
///     aniipod_maker: (1, 1),
///     nimbus_bed: (1, 1),      // 1 nimbus bed (for fertilizer)
/// };
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

    // Find fertilizer item for calculating fertilizer production time
    let fertilizer_item = item_map.get("fertilizer");
    let nimbus_bed_count = facility_counts.get_count("Nimbus Bed") as f64;

    // Calculate time to produce one fertilizer (if Nimbus Bed is available)
    // Fertilizer: 30 yield per 1800s, so each fertilizer takes 60s to produce
    let fertilizer_time_per_unit = fertilizer_item
        .map(|f| f.production_time / (f.yield_amount as f64 * nimbus_bed_count.max(1.0)))
        .unwrap_or(0.0);

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

        // Filter by target currency
        if item.sell_currency != target_currency {
            continue;
        }

        // Filter out items that require fertilizer if no Nimbus Bed is available
        if item.requires_fertilizer && nimbus_bed_count == 0.0 {
            continue;
        }

        let (total_time, steady_state_time, total_energy, raw_cost, requires_raw, raw_facility, all_facilities, intermediate_steps, raw_material_details) =
            if let Some(ref raw_mats) = item.raw_materials {
                // This is a processed item - use recursive calculation to handle nested dependencies
                let required_amounts = item.required_amount.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
                
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
                        fertilizer_time_per_unit,
                        nimbus_bed_count,
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
                    let high_speed_name = format!("high_speed_{}", raw_mat);
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

                let processing_facility_count = facility_counts.get_count(&item.facility) as f64;
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
                
                // Calculate gathering rate: processed_batches_worth per second
                // We assume all raw materials come from the same facility type for simplicity
                // (This is true for most recipes like super_wheatmeal)
                let mut gathering_batches_per_second = f64::INFINITY;
                
                for (i, raw_mat) in raw_mats.iter().enumerate() {
                    let required_per_batch = required_amounts.get(i).copied().unwrap_or(1) as f64;
                    
                    // Find the raw material item to get its production rate
                    let high_speed_name = format!("high_speed_{}", raw_mat);
                    let raw_item = if let Some(hs) = item_map.get(&high_speed_name) {
                        if let Some((ref module_name, required_level)) = hs.module_requirement {
                            if module_levels.can_use(module_name, required_level) { Some(*hs) } else { item_map.get(raw_mat.as_str()).copied() }
                        } else { Some(*hs) }
                    } else {
                        item_map.get(raw_mat.as_str()).copied()
                    };
                    
                    if let Some(raw) = raw_item {
                        let raw_facility_count = facility_counts.get_count(&raw.facility) as f64;
                        // Units produced per second by all facilities for this raw material
                        let units_per_second = (raw.yield_amount as f64 * raw_facility_count) / raw.production_time;
                        // Convert to "processed batches worth" per second
                        let batches_worth_per_second = units_per_second / required_per_batch;
                        gathering_batches_per_second = gathering_batches_per_second.min(batches_worth_per_second);
                        
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
                }
                
                // Processing rate: processed batches per second
                let processing_batches_per_second = processing_facility_count / processing_time_per_mill;
                
                // Steady-state rate is the minimum (bottleneck)
                let batches_per_second = gathering_batches_per_second.min(processing_batches_per_second);
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
                )
            } else {
                // This is a raw material - direct production
                let facility_count = facility_counts.get_count(&item.facility) as f64;
                let time_per_batch = item.production_time;
                
                // Add fertilizer time if required (1 fertilizer per batch)
                let fertilizer_time = if item.requires_fertilizer {
                    fertilizer_time_per_unit
                } else {
                    0.0
                };
                
                // For display purposes, time_per_unit is how long to produce one unit
                let effective_time_per_yield =
                    (time_per_batch + fertilizer_time) / (item.yield_amount as f64 * facility_count);
                // For raw materials, steady-state time equals batch time / facility count
                let steady_state_time = (time_per_batch + fertilizer_time) / facility_count;
                // Energy per batch (not per unit) to match units_needed which counts batches
                let energy_per_batch = item.energy;
                let cost_per_batch = item.cost.unwrap_or(0.0);
                
                // Raw materials use just their own facility
                let mut raw_all_facilities = HashSet::new();
                raw_all_facilities.insert(item.facility.clone());

                (effective_time_per_yield, steady_state_time, energy_per_batch, cost_per_batch, None, None, raw_all_facilities, vec![], None)
            };

        let net_profit = item.sell_value * item.yield_amount as f64 - raw_cost;
        
        // For efficiency comparison, use steady-state time (bottleneck)
        let profit_per_second = if steady_state_time > 0.0 {
            net_profit / steady_state_time
        } else {
            0.0
        };
        let profit_per_energy = total_energy.map(|e| if e > 0.0 { net_profit / e } else { 0.0 });

        // For time optimization, use batch-based profit_per_second directly
        // (facility parallelism is already factored into batch_time)
        let effective_profit_per_second = profit_per_second;
        
        // Startup time is the time to produce the first batch (before steady-state begins)
        // This equals total_time for the first unit/batch
        let startup_time = total_time;

        efficiencies.push(ProductionEfficiency {
            item: item.clone(),
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
/// let counts = FacilityCounts {
///     farmland: (4, 3),        // 4 farmlands at level 3
///     woodland: (1, 2),        // 1 woodland at level 2
///     mineral_pile: (1, 1),    // 1 mineral pile at level 1
///     carousel_mill: (2, 2),   // 2 carousel mills at level 2
///     jukebox_dryer: (1, 1),
///     crafting_table: (1, 1),
///     dance_pad_polisher: (1, 1),
///     aniipod_maker: (1, 1),
///     nimbus_bed: (1, 1),      // 1 nimbus bed (for fertilizer)
/// };
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
    let mut chain_id: u32 = 0;

    for eff in &selected_items {
        let current_chain_id = chain_id;
        chain_id += 1;
        
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
