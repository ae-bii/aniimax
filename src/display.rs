//! Display and formatting utilities for Aniimax.
//!
//! This module provides functions for formatting output and displaying
//! optimization results to the user in a readable format.

use crate::models::{ProductionEfficiency, ProductionPath, ProductionStep};
use crate::locale::Language;

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

fn format_time_in(seconds: f64, language: Language) -> String {
    if language == Language::En { return format_time(seconds); }
    format_time(seconds).replace('h', "ч").replace('m', "м").replace('s', "с")
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
    display_results_in(path, efficiencies, optimize_energy, Language::En);
}

/// Prints the results in the selected display language.
pub fn display_results_in(
    path: &ProductionPath,
    efficiencies: &[ProductionEfficiency],
    optimize_energy: bool,
    language: Language,
) {
    println!();
    println!("+================================================================+");
    println!("{}", language.text("|           ANIIMO PRODUCTION OPTIMIZATION RESULTS              |"));
    println!("+================================================================+");
    println!();

    // Check if this is a parallel production path (has chain_ids)
    let is_parallel = path.steps.iter().any(|s| s.chain_id.is_some());
    
    if is_parallel {
        println!("{}", language.text("[PARALLEL PRODUCTION CHAINS]"));
        println!("----------------------------------------------------------------");
        println!("  {}", language.text("All chains run simultaneously. Total time = longest chain."));
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
                language.facility(facilities[0])
            } else {
                // Show unique facilities in order (raw → processed)
                let mut unique: Vec<&str> = Vec::new();
                for f in &facilities {
                    if !unique.contains(f) {
                        unique.push(f);
                    }
                }
                unique.into_iter().map(|name| language.facility(name)).collect::<Vec<_>>().join(" → ")
            };
            
            let chain_profit: f64 = steps.iter().map(|s| s.profit_contribution).sum();
            let chain_time = steps.iter().map(|s| s.time).fold(0.0, f64::max);
            
            println!("  {} {}: {} ({:.0} {} {})", language.text("Chain"),
                chain_num + 1, 
                chain_desc,
                chain_profit,
                language.text("coins in"),
                format_time_in(chain_time, language)
            );
            
            for step in steps {
                if step.profit_contribution > 0.0 {
                    println!("    → {} x {} {} {}", step.quantity, language.item(&step.item_name), language.text("at"), language.facility(&step.facility));
                } else {
                    println!("    → {} x {} {} {} ({})", step.quantity, language.item(&step.item_name), language.text("at"), language.facility(&step.facility), language.text("raw material"));
                }
            }
            println!();
        }
    } else {
        println!("{}", language.text("[BEST PRODUCTION PATH]"));
        println!("----------------------------------------------------------------");

        for (i, step) in path.steps.iter().enumerate() {
            if step.facility.starts_with("Unknown") {
                println!(
                    "  {} {}: {} {} x {}",
                    language.text("Step"),
                    i + 1,
                    language.text("Gather"),
                    step.quantity,
                    language.item(&step.item_name)
                );
            } else {
                println!(
                    "  {} {}: {} {} x {} {} {}",
                    language.text("Step"),
                    i + 1,
                    language.text("Produce"),
                    step.quantity,
                    language.item(&step.item_name),
                    language.text("at"),
                    language.facility(&step.facility)
                );
            }
        }
    }

    println!();
    println!("{}", language.text("[SUMMARY]"));
    println!("----------------------------------------------------------------");
    println!("  {} {:.0} {}", language.text("Total Profit:"), path.total_profit, language.text(&path.currency));
    println!("  {} {}", language.text("Total Time:"), format_time_in(path.total_time, language));
    if path.startup_time > 0.0 {
        println!("    - {} {} ({})", language.text("Startup:"), format_time_in(path.startup_time, language), language.text("first batch"));
        println!("    - {} {}", language.text("Steady-state:"), format_time_in(path.total_time - path.startup_time, language));
    }
    if let Some(energy) = path.total_energy {
        println!("  {} {:.0}", language.text("Total Energy:"), energy);
    }
    println!("  {} {}", language.text("Items Produced:"), path.items_produced);
    
    if path.is_energy_self_sufficient {
        println!();
        println!("  {}", language.text("[ENERGY SELF-SUFFICIENT]"));
        if let Some(ref energy_item) = path.energy_item_name {
            if let Some(energy_count) = path.energy_items_produced {
                println!("  {} {}x {}", language.text("Energy Item:"), energy_count, language.item(energy_item));
            }
        }
    }

    println!();
    println!(
        "{} ({} {})",
        language.text("[ALL OPTIONS RANKED]"),
        language.text("by"),
        if optimize_energy {
            language.text("energy efficiency")
        } else {
            language.text("time efficiency")
        }
    );
    println!("----------------------------------------------------------------");
    println!(
        "{:<20} {:>12} {:>12} {:>12}",
        language.text("Item"), language.text("Profit/sec"), language.text("Profit/energy"), language.text("Time/unit")
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
            .unwrap_or_else(|| language.text("N/A").to_string());
        println!(
            "{:<20} {:>12.4} {:>12} {:>12}",
            language.item(&eff.item.name),
            eff.profit_per_second,
            energy_str,
            format_time_in(eff.total_time_per_unit, language)
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
    display_energy_recommendations_in(efficiencies, Language::En);
}

/// Prints energy recommendations in the selected display language.
pub fn display_energy_recommendations_in(efficiencies: &[ProductionEfficiency], language: Language) {
    let items_with_energy: Vec<_> = efficiencies
        .iter()
        .filter(|e| e.profit_per_energy.is_some())
        .collect();

    if items_with_energy.is_empty() {
        println!();
        println!("{}", language.text("[ENERGY] No items with energy data available."));
        return;
    }

    println!();
    println!("{}", language.text("[ENERGY EFFICIENCY RANKINGS]"));
    println!("----------------------------------------------------------------");
    println!(
        "{:<20} {:>15} {:>15}",
        language.text("Item"), language.text("Profit/Energy"), language.text("Energy/Unit")
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
            language.item(&eff.item.name),
            eff.profit_per_energy.unwrap_or(0.0),
            eff.total_energy_per_unit.unwrap_or(0.0)
        );
    }
}
