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
        
        // Calculate how many batches of this processed item we need
        let batches_needed = (required_amount / item.yield_amount as f64).ceil();
        
        for (i, raw_mat) in raw_mats.iter().enumerate() {
            let ingredient_required = required_amounts.get(i).copied().unwrap_or(1) as f64 * batches_needed;
            
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
        
        ProductionRequirements {
            total_time,
            total_energy,
            total_cost,
            raw_names: vec![actual_name.to_string()],
            primary_facility: Some(item.facility.clone()),
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

        let (total_time, steady_state_time, total_energy, raw_cost, requires_raw, raw_facility) =
            if let Some(ref raw_mats) = item.raw_materials {
                // This is a processed item - use recursive calculation to handle nested dependencies
                let required_amounts = item.required_amount.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
                
                // Track totals across all raw materials
                let mut max_ingredient_time = 0.0;
                let mut total_ingredient_energy: Option<f64> = None;
                let mut total_ingredient_cost = 0.0;
                let mut all_raw_names: Vec<String> = Vec::new();
                let mut primary_facility: Option<String> = None;
                let mut skip_item = false;

                for (i, raw_mat) in raw_mats.iter().enumerate() {
                    let required = required_amounts.get(i).copied().unwrap_or(1) as f64;
                    
                    let mut visited = HashSet::new();
                    let reqs = calculate_item_requirements(
                        raw_mat,
                        required,
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

                (
                    total_time,
                    steady_state_time, // Pass steady_state_time for efficiency calculation
                    total_energy,
                    total_ingredient_cost,
                    Some(unique_raw_names.join("+")),
                    primary_facility,
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

                (effective_time_per_yield, steady_state_time, energy_per_batch, cost_per_batch, None, None)
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

        efficiencies.push(ProductionEfficiency {
            item: item.clone(),
            profit_per_second,
            profit_per_energy,
            total_time_per_unit: total_time,
            total_energy_per_unit: total_energy,
            requires_raw,
            raw_cost,
            raw_facility,
            effective_profit_per_second,
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
        steps.push(ProductionStep {
            item_name: raw_name.clone(),
            facility: format!("{} (x{})", raw_facility, raw_facility_count),
            quantity: raw_amount_needed,
            time: 0.0, // Time is included in total
            energy: None,
            profit_contribution: 0.0,
        });
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
    });

    // Calculate actual time with parallelization
    // Note: units_needed represents number of production batches
    let total_time = if best.requires_raw.is_some() {
        // For processed items, time is already calculated with parallelization
        best.total_time_per_unit * units_needed as f64 / main_facility_count as f64
    } else {
        // For raw materials, units_needed is already the number of batches
        best.item.production_time * (units_needed as f64 / main_facility_count as f64).ceil()
    };

    let total_energy = best
        .total_energy_per_unit
        .map(|e| e * units_needed as f64);

    Some(ProductionPath {
        steps,
        total_time,
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
/// This function considers that different facility types (farmland, woodland, etc.)
/// can operate simultaneously, maximizing overall profit per time.
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

    // For raw material facilities, find the best item for each
    let raw_facilities = ["Farmland", "Woodland", "Mineral Pile"];
    
    let mut best_per_facility: HashMap<String, &ProductionEfficiency> = HashMap::new();

    for eff in efficiencies {
        // Only consider raw materials for parallel production (no dependencies)
        if eff.requires_raw.is_some() {
            continue;
        }

        let facility = &eff.item.facility;
        
        // Check if this facility has any slots
        if facility_counts.get_count(facility) == 0 {
            continue;
        }

        let is_better = match best_per_facility.get(facility) {
            Some(existing) => eff.effective_profit_per_second > existing.effective_profit_per_second,
            None => true,
        };

        if is_better {
            best_per_facility.insert(facility.clone(), eff);
        }
    }

    if best_per_facility.is_empty() {
        return None;
    }

    // Collect selected items for each raw facility
    let mut selected_items: Vec<&ProductionEfficiency> = Vec::new();

    for facility in &raw_facilities {
        if let Some(eff) = best_per_facility.get(*facility) {
            selected_items.push(eff);
        }
    }

    // Need at least 2 facilities for parallel to make sense
    if selected_items.len() <= 1 {
        return None;
    }

    // Binary search for the minimum time T such that the total profit from
    // all facilities running for time T meets or exceeds the target
    
    // Helper to calculate profit given a time limit
    let calc_profit_and_batches = |time: f64| -> (f64, Vec<(&ProductionEfficiency, u32)>) {
        let mut total = 0.0;
        let mut batches_list = Vec::new();
        
        for eff in &selected_items {
            let facility_count = facility_counts.get_count(&eff.item.facility) as f64;
            let time_per_effective_batch = eff.item.production_time / facility_count;
            let batches = (time / time_per_effective_batch).floor() as u32;
            
            if batches > 0 {
                let profit_per_batch = eff.item.sell_value * eff.item.yield_amount as f64 - eff.raw_cost;
                total += profit_per_batch * batches as f64;
                batches_list.push((*eff, batches));
            }
        }
        
        (total, batches_list)
    };
    
    // Find lower bound (continuous profit model)
    let mut combined_profit_per_second = 0.0;
    for eff in &selected_items {
        combined_profit_per_second += eff.effective_profit_per_second;
    }
    let theoretical_min_time = target_amount / combined_profit_per_second;
    
    // Find the actual time needed - iterate through batch completion times
    // Start with the best item's batch times and check each one
    let best_item = selected_items
        .iter()
        .max_by(|a, b| {
            a.effective_profit_per_second
                .partial_cmp(&b.effective_profit_per_second)
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;
    
    let best_facility_count = facility_counts.get_count(&best_item.item.facility) as f64;
    let best_time_per_effective_batch = best_item.item.production_time / best_facility_count;
    
    // Start from the minimum batches that could theoretically work
    let min_batches = (theoretical_min_time / best_time_per_effective_batch).ceil() as u32;
    
    // Find the minimum number of batches for the best item such that total profit >= target
    let mut required_batches = min_batches;
    let final_batches_result: Vec<(&ProductionEfficiency, u32)>;
    
    loop {
        let time = required_batches as f64 * best_time_per_effective_batch;
        let (profit, batches) = calc_profit_and_batches(time);
        
        if profit >= target_amount {
            final_batches_result = batches;
            break;
        }
        
        required_batches += 1;
        
        // Safety check to avoid infinite loop
        if required_batches > 1_000_000 {
            return None;
        }
    }

    // Build production steps from final_batches_result
    let mut steps = Vec::new();
    let mut total_profit = 0.0;
    let mut total_energy: Option<f64> = None;
    let mut total_items = 0u32;

    for (eff, batches) in &final_batches_result {
        if *batches == 0 {
            continue;
        }
        
        let facility_count = facility_counts.get_count(&eff.item.facility) as f64;
        let profit_per_batch = eff.item.sell_value * eff.item.yield_amount as f64 - eff.raw_cost;
        let step_profit = profit_per_batch * *batches as f64;
        total_profit += step_profit;

        if let Some(energy) = eff.total_energy_per_unit {
            let step_energy = energy * *batches as f64;
            total_energy = Some(total_energy.unwrap_or(0.0) + step_energy);
        }

        total_items += *batches * eff.item.yield_amount;
        let step_time = eff.item.production_time * (*batches as f64 / facility_count).ceil();

        steps.push(ProductionStep {
            item_name: eff.item.name.clone(),
            facility: format!("{} (x{})", eff.item.facility, facility_counts.get_count(&eff.item.facility)),
            quantity: *batches,
            time: step_time,
            energy: eff.total_energy_per_unit.map(|e| e * *batches as f64),
            profit_contribution: step_profit,
        });
    }

    // Recalculate actual total time (the longest step determines total time since they run in parallel)
    let actual_total_time = steps.iter().map(|s| s.time).fold(0.0, f64::max);

    // Only return parallel path if we have multiple facilities running
    if steps.len() <= 1 {
        return None; // Fall back to single-facility optimization
    }

    Some(ProductionPath {
        steps,
        total_time: actual_total_time,
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
    });

    // Add raw material step for profit item if needed
    if let Some(ref raw_name) = best_profit.requires_raw {
        let raw_amount_needed = best_profit.item.required_amount
            .as_ref()
            .map(|amounts| amounts.iter().sum::<u32>())
            .unwrap_or(1) * batches_for_profit as u32;
        let raw_facility = best_profit.raw_facility.as_deref().unwrap_or("Unknown");
        let raw_facility_count = facility_counts.get_count(raw_facility);
        steps.push(ProductionStep {
            item_name: raw_name.clone(),
            facility: format!("{} (x{})", raw_facility, raw_facility_count),
            quantity: raw_amount_needed,
            time: 0.0,
            energy: None,
            profit_contribution: 0.0,
        });
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
    });

    // Calculate actual profit (minus seed costs for energy items)
    let energy_seed_cost = energy_batches as f64 * best_energy.cost_per_batch;
    let gross_profit = profit_per_batch * batches_for_profit;
    let net_profit = gross_profit - energy_seed_cost;

    Some(ProductionPath {
        steps,
        total_time,
        total_energy: Some(total_energy_needed),
        total_profit: net_profit,
        currency: best_profit.item.sell_currency.clone(),
        items_produced: batches_for_profit as u32 * best_profit.item.yield_amount,
        is_energy_self_sufficient: true,
        energy_items_produced: Some(energy_batches * best_energy.item.yield_amount),
        energy_item_name: Some(best_energy.item.name.clone()),
    })
}
