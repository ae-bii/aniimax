//! Display and formatting utilities for Aniimax.
//!
//! This module provides functions for formatting output and displaying
//! optimization results to the user in a readable format.

use crate::models::{ProductionEfficiency, ProductionPath, ProductionStep};

/// Formats a duration in seconds to a human-readable string.
///
/// # Arguments
///
/// * `seconds` - Duration in seconds
///
/// # Returns
///
/// A formatted string like "1h 30m 45s", "15m 30s", or "45s"
///
/// # Example
///
/// ```
/// use aniimax::display::format_time;
///
/// assert_eq!(format_time(3665.0), "1h 1m 5s");
/// assert_eq!(format_time(125.0), "2m 5s");
/// assert_eq!(format_time(45.0), "45s");
/// ```
pub fn format_time(seconds: f64) -> String {
    let hours = (seconds / 3600.0).floor();
    let minutes = ((seconds % 3600.0) / 60.0).floor();
    let secs = seconds % 60.0;

    if hours > 0.0 {
        format!("{}h {}m {:.0}s", hours, minutes, secs)
    } else if minutes > 0.0 {
        format!("{}m {:.0}s", minutes, secs)
    } else {
        format!("{:.0}s", secs)
    }
}

/// Displays the complete optimization results to stdout.
///
/// This function prints:
/// - The recommended production path with steps
/// - Summary statistics (profit, time, energy, items)
/// - A ranked list of all production options
///
/// # Arguments
///
/// * `path` - The optimal production path
/// * `efficiencies` - All calculated efficiency metrics
/// * `optimize_energy` - Whether energy optimization mode was used
pub fn display_results(
    path: &ProductionPath,
    efficiencies: &[ProductionEfficiency],
    optimize_energy: bool,
) {
    println!();
    println!("+================================================================+");
    println!("|           ANIIMO PRODUCTION OPTIMIZATION RESULTS              |");
    println!("+================================================================+");
    println!();

    // Check if this is a parallel production path (has chain_ids)
    let is_parallel = path.steps.iter().any(|s| s.chain_id.is_some());
    
    if is_parallel {
        println!("[PARALLEL PRODUCTION CHAINS]");
        println!("----------------------------------------------------------------");
        println!("  All chains run simultaneously. Total time = longest chain.");
        println!();
        
        // Group steps by chain_id
        let mut chains: std::collections::BTreeMap<u32, Vec<&ProductionStep>> = std::collections::BTreeMap::new();
        for step in &path.steps {
            if let Some(chain_id) = step.chain_id {
                chains.entry(chain_id).or_default().push(step);
            }
        }
        
        for (chain_num, (_chain_id, steps)) in chains.iter().enumerate() {
            // Determine chain description from facilities
            let facilities: Vec<&str> = steps.iter()
                .map(|s| s.facility.split(" (").next().unwrap_or(&s.facility))
                .collect();
            let chain_desc = if facilities.len() == 1 {
                facilities[0].to_string()
            } else {
                // Show unique facilities in order (raw → processed)
                let mut unique: Vec<&str> = Vec::new();
                for f in &facilities {
                    if !unique.contains(f) {
                        unique.push(f);
                    }
                }
                unique.join(" → ")
            };
            
            let chain_profit: f64 = steps.iter().map(|s| s.profit_contribution).sum();
            let chain_time = steps.iter().map(|s| s.time).fold(0.0, f64::max);
            
            println!("  Chain {}: {} ({:.0} coins in {})", 
                chain_num + 1, 
                chain_desc,
                chain_profit,
                format_time(chain_time)
            );
            
            for step in steps {
                if step.profit_contribution > 0.0 {
                    println!("    → {} x {} at {}", step.quantity, step.item_name, step.facility);
                } else {
                    println!("    → {} x {} at {} (raw material)", step.quantity, step.item_name, step.facility);
                }
            }
            println!();
        }
    } else {
        println!("[BEST PRODUCTION PATH]");
        println!("----------------------------------------------------------------");

        for (i, step) in path.steps.iter().enumerate() {
            if step.facility.starts_with("Unknown") {
                println!(
                    "  Step {}: Gather {} x {}",
                    i + 1,
                    step.quantity,
                    step.item_name
                );
            } else {
                println!(
                    "  Step {}: Produce {} x {} at {}",
                    i + 1,
                    step.quantity,
                    step.item_name,
                    step.facility
                );
            }
        }
    }

    println!();
    println!("[SUMMARY]");
    println!("----------------------------------------------------------------");
    println!("  Total Profit:     {:.0} {}", path.total_profit, path.currency);
    println!("  Total Time:       {}", format_time(path.total_time));
    if path.startup_time > 0.0 {
        println!("    - Startup:      {} (first batch)", format_time(path.startup_time));
        println!("    - Steady-state: {}", format_time(path.total_time - path.startup_time));
    }
    if let Some(energy) = path.total_energy {
        println!("  Total Energy:     {:.0}", energy);
    }
    println!("  Items Produced:   {}", path.items_produced);
    
    if path.is_energy_self_sufficient {
        println!();
        println!("  [ENERGY SELF-SUFFICIENT]");
        if let Some(ref energy_item) = path.energy_item_name {
            if let Some(energy_count) = path.energy_items_produced {
                println!("  Energy Item:      {}x {}", energy_count, energy_item);
            }
        }
    }

    println!();
    println!(
        "[ALL OPTIONS RANKED] (by {})",
        if optimize_energy {
            "energy efficiency"
        } else {
            "time efficiency"
        }
    );
    println!("----------------------------------------------------------------");
    println!(
        "{:<20} {:>12} {:>12} {:>12}",
        "Item", "Profit/sec", "Profit/energy", "Time/unit"
    );
    println!("----------------------------------------------------------------");

    let mut sorted = efficiencies.to_vec();
    if optimize_energy {
        sorted.sort_by(|a, b| {
            b.profit_per_energy
                .unwrap_or(0.0)
                .partial_cmp(&a.profit_per_energy.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    } else {
        sorted.sort_by(|a, b| {
            b.profit_per_second
                .partial_cmp(&a.profit_per_second)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    for eff in sorted.iter().take(10) {
        let energy_str = eff
            .profit_per_energy
            .map(|e| format!("{:.4}", e))
            .unwrap_or_else(|| "N/A".to_string());
        println!(
            "{:<20} {:>12.4} {:>12} {:>12}",
            eff.item.name,
            eff.profit_per_second,
            energy_str,
            format_time(eff.total_time_per_unit)
        );
    }

    println!();
}

/// Displays energy efficiency recommendations.
///
/// Shows a ranked list of items sorted by profit per energy unit,
/// useful for players who want to maximize their energy usage.
///
/// # Arguments
///
/// * `efficiencies` - All calculated efficiency metrics
pub fn display_energy_recommendations(efficiencies: &[ProductionEfficiency]) {
    let items_with_energy: Vec<_> = efficiencies
        .iter()
        .filter(|e| e.profit_per_energy.is_some())
        .collect();

    if items_with_energy.is_empty() {
        println!();
        println!("[ENERGY] No items with energy data available.");
        return;
    }

    println!();
    println!("[ENERGY EFFICIENCY RANKINGS]");
    println!("----------------------------------------------------------------");
    println!(
        "{:<20} {:>15} {:>15}",
        "Item", "Profit/Energy", "Energy/Unit"
    );
    println!("----------------------------------------------------------------");

    let mut sorted: Vec<_> = items_with_energy.clone();
    sorted.sort_by(|a, b| {
        b.profit_per_energy
            .unwrap_or(0.0)
            .partial_cmp(&a.profit_per_energy.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for eff in sorted.iter().take(10) {
        println!(
            "{:<20} {:>15.6} {:>15.0}",
            eff.item.name,
            eff.profit_per_energy.unwrap_or(0.0),
            eff.total_energy_per_unit.unwrap_or(0.0)
        );
    }
}
