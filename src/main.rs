//! Aniimax - Command Line Interface
//!
//! This is the main entry point for the production optimization tool.
//! Run with `--help` to see all available options.

use clap::Parser;
use std::error::Error;
use std::path::Path;

use aniimax::{
    data::load_all_data,
    display::{display_energy_recommendations, display_results},
    models::FacilityCounts,
    optimizer::{calculate_efficiencies, calculate_energy_efficiencies, find_best_production_path, find_self_sufficient_path},
};

/// Command-line arguments for Aniimax.
#[derive(Parser, Debug)]
#[command(name = "aniimax")]
#[command(author, version, about = "Optimize production paths for currency generation in Aniimo Homeland", long_about = None)]
struct Args {
    /// Target amount of currency to produce
    #[arg(short, long)]
    target: f64,

    /// Currency type to optimize for (coins or coupons)
    #[arg(short, long, default_value = "coins")]
    currency: String,

    /// Energy cost per minute (for energy self-sufficiency calculation)
    #[arg(short, long, default_value = "0.0")]
    energy_cost: f64,

    /// Enable energy self-sufficient mode (produce items for energy instead of buying)
    #[arg(long, default_value = "false")]
    energy_self_sufficient: bool,

    // ========== Farmland ==========
    /// Number of Farmland plots available
    #[arg(long, default_value = "1")]
    farmland: u32,

    /// Farmland facility level
    #[arg(long, default_value = "1")]
    farmland_level: u32,

    // ========== Woodland ==========
    /// Number of Woodland plots available
    #[arg(long, default_value = "1")]
    woodland: u32,

    /// Woodland facility level
    #[arg(long, default_value = "1")]
    woodland_level: u32,

    // ========== Mineral Pile ==========
    /// Number of Mineral Pile slots available
    #[arg(long, default_value = "1")]
    mineral_pile: u32,

    /// Mineral Pile facility level
    #[arg(long, default_value = "1")]
    mineral_pile_level: u32,

    // ========== Carousel Mill ==========
    /// Number of Carousel Mill machines available
    #[arg(long, default_value = "1")]
    carousel_mill: u32,

    /// Carousel Mill facility level
    #[arg(long, default_value = "1")]
    carousel_mill_level: u32,

    // ========== Jukebox Dryer ==========
    /// Number of Jukebox Dryer machines available
    #[arg(long, default_value = "1")]
    jukebox_dryer: u32,

    /// Jukebox Dryer facility level
    #[arg(long, default_value = "1")]
    jukebox_dryer_level: u32,

    // ========== Crafting Table ==========
    /// Number of Crafting Table slots available
    #[arg(long, default_value = "1")]
    crafting_table: u32,

    /// Crafting Table facility level
    #[arg(long, default_value = "1")]
    crafting_table_level: u32,

    // ========== Dance Pad Polisher ==========
    /// Number of Dance Pad Polisher machines available
    #[arg(long, default_value = "1")]
    dance_pad_polisher: u32,

    /// Dance Pad Polisher facility level
    #[arg(long, default_value = "1")]
    dance_pad_polisher_level: u32,

    // ========== Aniipod Maker ==========
    /// Number of Aniipod Maker machines available
    #[arg(long, default_value = "1")]
    aniipod_maker: u32,

    /// Aniipod Maker facility level
    #[arg(long, default_value = "1")]
    aniipod_maker_level: u32,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // Determine data directory
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        eprintln!("Error: 'data' directory not found. Please run from the project root.");
        std::process::exit(1);
    }

    // Build facility counts from args (count, level) tuples
    let facility_counts = FacilityCounts {
        farmland: (args.farmland, args.farmland_level),
        woodland: (args.woodland, args.woodland_level),
        mineral_pile: (args.mineral_pile, args.mineral_pile_level),
        carousel_mill: (args.carousel_mill, args.carousel_mill_level),
        jukebox_dryer: (args.jukebox_dryer, args.jukebox_dryer_level),
        crafting_table: (args.crafting_table, args.crafting_table_level),
        dance_pad_polisher: (args.dance_pad_polisher, args.dance_pad_polisher_level),
        aniipod_maker: (args.aniipod_maker, args.aniipod_maker_level),
    };

    println!("Aniimax - Aniimo Production Optimizer");
    println!("================================================================");
    println!();
    println!("Configuration:");
    println!("  Target:          {:.0} {}", args.target, args.currency);
    println!("  Energy Cost:     {}/min", args.energy_cost);
    println!(
        "  Mode:            {}",
        if args.energy_self_sufficient { "Energy Self-Sufficient" } else { "Time Optimization" }
    );

    println!();
    println!("Facilities (count x level):");
    println!("  Farmland:           {} x Lv.{}", args.farmland, args.farmland_level);
    println!("  Woodland:           {} x Lv.{}", args.woodland, args.woodland_level);
    println!("  Mineral Pile:       {} x Lv.{}", args.mineral_pile, args.mineral_pile_level);
    println!("  Carousel Mill:      {} x Lv.{}", args.carousel_mill, args.carousel_mill_level);
    println!("  Jukebox Dryer:      {} x Lv.{}", args.jukebox_dryer, args.jukebox_dryer_level);
    println!("  Crafting Table:     {} x Lv.{}", args.crafting_table, args.crafting_table_level);
    println!("  Dance Pad Polisher: {} x Lv.{}", args.dance_pad_polisher, args.dance_pad_polisher_level);
    println!("  Aniipod Maker:      {} x Lv.{}", args.aniipod_maker, args.aniipod_maker_level);

    // Load all data
    let items = load_all_data(data_dir)?;
    println!();
    println!("Loaded {} production items.", items.len());

    // Calculate efficiencies
    let efficiencies =
        calculate_efficiencies(&items, &args.currency, &facility_counts);

    if efficiencies.is_empty() {
        println!();
        println!(
            "[WARNING] No items found that produce {} with current facility levels.",
            args.currency
        );
        return Ok(());
    }

    // Find best production path based on mode
    let path_result = if args.energy_self_sufficient && args.energy_cost > 0.0 {
        let energy_efficiencies = calculate_energy_efficiencies(&items, &facility_counts);
        find_self_sufficient_path(
            &efficiencies,
            &energy_efficiencies,
            args.target,
            args.energy_cost,
            &facility_counts,
        )
    } else {
        find_best_production_path(
            &efficiencies,
            args.target,
            false,
            0.0,
            &facility_counts,
        )
    };

    if let Some(path) = path_result {
        display_results(&path, &efficiencies, false);

        if args.energy_cost > 0.0 && !args.energy_self_sufficient {
            display_energy_recommendations(&efficiencies);
        }
    } else {
        println!();
        if args.energy_self_sufficient {
            println!("[WARNING] Cannot achieve energy self-sufficiency with current setup.");
            println!("Try increasing facility counts or reducing energy cost.");
        } else {
            println!("[WARNING] Could not find a valid production path.");
        }
    }

    Ok(())
}
