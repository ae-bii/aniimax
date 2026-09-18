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
    models::{FacilityCounts, ModuleLevels, Worker, Workers},
    optimizer::{calculate_efficiencies, calculate_energy_efficiencies, find_best_production_path, find_parallel_production_path, find_self_sufficient_path},
};

/// Facilities an Aniimo works, where its ability level and personality bonus set the speed.
const WORKER_FACILITIES: [&str; 17] = [
    "Mine",
    "Well",
    "Tidewhisper Sandcastle",
    "Dewy House",
    "Nimbus Bed",
    "Starfall Hammock",
    "Floral Windmill",
    "Phonolfactory Table",
    "Bouncy Brew Keg",
    "Blazing Stove",
    "Pickling Jar",
    "Joy Wheel Loom",
    "Carousel Mill",
    "Crafting Table",
    "Claw Game Cooker",
    "Jukebox Dryer",
    "Simmering Pot",
];

/// Command-line arguments for Aniimax.
#[derive(Parser, Debug)]
#[command(name = "aniimax")]
#[command(author, version, about = "Optimize production paths for currency generation in Aniimo Homeland", long_about = None)]
struct Args {
    /// Target amount of currency to produce
    #[arg(short, long)]
    target: f64,

    /// What to optimize for: coins, or a byproduct (wood_blocks or mineral_sand)
    #[arg(short, long, default_value = "coins")]
    currency: String,

    /// Energy cost per minute (for energy self-sufficiency calculation)
    #[arg(short, long, default_value = "0.0")]
    energy_cost: f64,

    /// Enable energy self-sufficient mode (produce items for energy instead of buying)
    #[arg(long, default_value = "false")]
    energy_self_sufficient: bool,

    /// Enable cross-facility parallel production (run all facilities simultaneously)
    #[arg(long, default_value = "false")]
    parallel: bool,

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

    // ========== Mine ==========
    /// Number of Mine slots available
    #[arg(long, default_value = "1")]
    mine: u32,

    /// Mine facility level
    #[arg(long, default_value = "1")]
    mine_level: u32,

    // ========== Well ==========
    /// Number of Wells available
    #[arg(long, default_value = "0")]
    well: u32,

    /// Well facility level
    #[arg(long, default_value = "1")]
    well_level: u32,

    // ========== Tidewhisper Sandcastle ==========
    /// Number of Tidewhisper Sandcastles available
    #[arg(long, default_value = "0")]
    tidewhisper_sandcastle: u32,

    /// Tidewhisper Sandcastle facility level
    #[arg(long, default_value = "1")]
    tidewhisper_sandcastle_level: u32,

    // ========== Carousel Mill ==========
    /// Number of Carousel Mill machines available
    #[arg(long, default_value = "1")]
    carousel_mill: u32,

    /// Carousel Mill facility level
    #[arg(long, default_value = "1")]
    carousel_mill_level: u32,

    // ========== Claw Game Cooker ==========
    /// Number of Claw Game Cookers available
    #[arg(long, default_value = "1")]
    claw_game_cooker: u32,

    /// Claw Game Cooker facility level
    #[arg(long, default_value = "1")]
    claw_game_cooker_level: u32,

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

    // ========== Simmering Pot ==========
    /// Number of Simmering Pots available
    #[arg(long, default_value = "0")]
    simmering_pot: u32,

    /// Simmering Pot facility level
    #[arg(long, default_value = "1")]
    simmering_pot_level: u32,

    // ========== Aniimo ==========
    /// Ability level (1-3) of the Aniimo working the Mine, Well, Tidewhisper Sandcastle and processors
    #[arg(long, default_value = "1", value_parser = clap::value_parser!(u32).range(1..=3))]
    aniimo_level: u32,

    /// The working Aniimo has each facility's personality bonus (+20% speed)
    #[arg(long)]
    personality_bonus: bool,

    // ========== Item Upgrade Modules ==========
    /// Ecological Module level (unlocks quick crops, e.g. 1=quick wheat)
    #[arg(long, default_value = "0")]
    ecological_module: u32,

    /// Kitchen Module level (unlocks premium dishes, e.g. 2=premium bread)
    #[arg(long, default_value = "0")]
    kitchen_module: u32,

    /// Resource Detector level (unlocks quick gathered items, e.g. 1=quick well water)
    #[arg(long, default_value = "0")]
    resource_detector: u32,

    /// Crafting Module level (unlocks premium crafts, e.g. 1=premium river-washed stones)
    #[arg(long, default_value = "0")]
    crafting_module: u32,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // Determine data directory
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        eprintln!("Error: 'data' directory not found. Please run from the project root.");
        std::process::exit(1);
    }

    // Build facility counts from args (count, level) tuples; facilities without a flag aren't owned
    let facility_counts = FacilityCounts::only(&[
        ("Farmland", args.farmland, args.farmland_level),
        ("Woodland", args.woodland, args.woodland_level),
        ("Mine", args.mine, args.mine_level),
        ("Well", args.well, args.well_level),
        ("Tidewhisper Sandcastle", args.tidewhisper_sandcastle, args.tidewhisper_sandcastle_level),
        ("Carousel Mill", args.carousel_mill, args.carousel_mill_level),
        ("Claw Game Cooker", args.claw_game_cooker, args.claw_game_cooker_level),
        ("Jukebox Dryer", args.jukebox_dryer, args.jukebox_dryer_level),
        ("Crafting Table", args.crafting_table, args.crafting_table_level),
        ("Simmering Pot", args.simmering_pot, args.simmering_pot_level),
    ]);

    // Build module levels from args
    let module_levels = ModuleLevels {
        ecological_module: args.ecological_module,
        kitchen_module: args.kitchen_module,
        resource_detector: args.resource_detector,
        crafting_module: args.crafting_module,
    };

    println!("Aniimax - Aniimo Production Optimizer");
    println!("================================================================");
    println!();
    println!("Configuration:");
    println!("  Target:          {:.0} {}", args.target, args.currency);
    println!("  Energy Cost:     {}/min", args.energy_cost);
    println!(
        "  Mode:            {}",
        if args.energy_self_sufficient { 
            "Energy Self-Sufficient" 
        } else if args.parallel {
            "Cross-Facility Parallel"
        } else { 
            "Time Optimization" 
        }
    );

    println!();
    println!("Facilities (count x level):");
    println!("  Farmland:           {} x Lv.{}", args.farmland, args.farmland_level);
    println!("  Woodland:           {} x Lv.{}", args.woodland, args.woodland_level);
    println!("  Mine:               {} x Lv.{}", args.mine, args.mine_level);
    println!("  Well:               {} x Lv.{}", args.well, args.well_level);
    println!("  Tidewhisper:        {} x Lv.{}", args.tidewhisper_sandcastle, args.tidewhisper_sandcastle_level);
    println!("  Carousel Mill:      {} x Lv.{}", args.carousel_mill, args.carousel_mill_level);
    println!("  Claw Game Cooker:   {} x Lv.{}", args.claw_game_cooker, args.claw_game_cooker_level);
    println!("  Jukebox Dryer:      {} x Lv.{}", args.jukebox_dryer, args.jukebox_dryer_level);
    println!("  Crafting Table:     {} x Lv.{}", args.crafting_table, args.crafting_table_level);
    println!("  Simmering Pot:      {} x Lv.{}", args.simmering_pot, args.simmering_pot_level);

    println!();
    println!("Item Modules:");
    println!("  Ecological Module:  Lv.{}", args.ecological_module);
    println!("  Kitchen Module:     Lv.{}", args.kitchen_module);
    println!("  Resource Detector:  Lv.{}", args.resource_detector);
    println!("  Crafting Module:    Lv.{}", args.crafting_module);

    println!();
    println!(
        "Aniimo:             Lv.{} suitability{}",
        args.aniimo_level,
        if args.personality_bonus { ", personality bonus" } else { "" }
    );

    // Load all data, with times set for the Aniimo working each facility
    let mut items = load_all_data(data_dir)?;
    let mut workers = Workers::new();
    for facility in WORKER_FACILITIES {
        workers.set(facility, Worker::new(args.aniimo_level, args.personality_bonus));
    }
    workers.apply(&mut items);
    println!();
    println!("Loaded {} production items.", items.len());

    // Calculate efficiencies
    let efficiencies =
        calculate_efficiencies(&items, &args.currency, &facility_counts, &module_levels);

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
        let energy_efficiencies = calculate_energy_efficiencies(&items, &facility_counts, &module_levels);
        find_self_sufficient_path(
            &efficiencies,
            &energy_efficiencies,
            args.target,
            args.energy_cost,
            &facility_counts,
        )
    } else if args.parallel {
        // Compare parallel vs single-facility approach, use whichever is faster
        let parallel_path = find_parallel_production_path(&efficiencies, args.target, &facility_counts);
        let single_path = find_best_production_path(
            &efficiencies,
            args.target,
            false,
            0.0,
            &facility_counts,
        );
        
        match (parallel_path, single_path) {
            (Some(p), Some(s)) => {
                // Use the faster approach
                if p.total_time <= s.total_time {
                    Some(p)
                } else {
                    Some(s)
                }
            }
            (Some(p), None) => Some(p),
            (None, Some(s)) => Some(s),
            (None, None) => None,
        }
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
