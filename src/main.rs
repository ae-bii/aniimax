//! Aniimax - Command Line Interface
//!
//! This is the main entry point for the production optimization tool.
//! Run with `--help` to see all available options.

use clap::{Arg, ArgAction, CommandFactory, FromArgMatches, Parser};
use std::error::Error;
use std::path::Path;

use aniimax::{
    data::load_all_data,
    display::{display_energy_recommendations_in, display_results_in},
    locale::Language,
    models::{FacilityCounts, ModuleLevels, Worker, Workers},
    optimizer::{calculate_efficiencies, calculate_energy_efficiencies, find_best_production_path, find_parallel_production_path, find_self_sufficient_path},
};
use aniimax::locale::zh_tw;

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
    /// Display language: en or ru
    #[arg(long, value_enum, default_value_t = Language::En)]
    language: Language,

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
    let argv: Vec<String> = std::env::args().collect();
    let requested = argv.windows(2).find(|pair| pair[0] == "--language").map(|pair| pair[1].as_str())
        .or_else(|| argv.iter().find_map(|arg| arg.strip_prefix("--language=")));
    let language = if requested == Some("ru") { Language::Ru } else { Language::En };
    let language = if requested.is_some_and(zh_tw::is_code) { Language::ZhTw } else { language };
    let mut command = Args::command();
    if language == Language::Ru {
        command = command.about(language.text("Optimize production paths for currency generation in Aniimo Homeland").to_owned())
            .help_template("{about-with-newline}\nИспользование: {usage}\n\nПараметры:\n{options}")
            .disable_help_flag(true)
            .disable_version_flag(true)
            .arg(Arg::new("help").short('h').long("help").action(ArgAction::Help).help("Показать справку"))
            .arg(Arg::new("version").short('V').long("version").action(ArgAction::Version).help("Показать версию"));
        let helps: Vec<_> = command.get_arguments().filter_map(|arg| arg.get_help().map(|help| (arg.get_id().clone(), help.to_string()))).collect();
        for (id, help) in helps {
            command = command.mut_arg(id, |arg| arg.help(language.text(&help).to_owned()));
        }
        if argv.iter().any(|arg| arg == "--help" || arg == "-h") {
            print!("{}", command.render_long_help().to_string()
                .replace("[OPTIONS]", "[ПАРАМЕТРЫ]")
                .replace("[default:", "[по умолчанию:")
                .replace("[possible values:", "[доступные значения:"));
            return Ok(());
        }
    }
    if language == Language::ZhTw {
        command = zh_tw::localize_command(command);
        if argv.iter().any(|arg| arg == "--help" || arg == "-h") {
            print!("{}", zh_tw::help_text(command.render_long_help().to_string()));
            return Ok(());
        }
    }
    let matches = command.try_get_matches_from(argv).unwrap_or_else(|error| {
        if language == Language::ZhTw {
            if error.exit_code() == 0 {
                print!("{error}");
                std::process::exit(0);
            }
            eprint!("{}", zh_tw::error_text(error.to_string()));
            std::process::exit(error.exit_code());
        }
        if language == Language::Ru {
            if error.exit_code() == 0 {
                print!("{error}");
                std::process::exit(0);
            }
            eprint!("{}", error.to_string()
                .replace("error:", "ошибка:")
                .replace("the following required arguments were not provided:", "не указаны обязательные аргументы:")
                .replace("a value is required for", "для параметра требуется значение")
                .replace("invalid value", "недопустимое значение")
                .replace(" for '", " для '")
                .replace("invalid float literal", "некорректное число")
                .replace("invalid digit found in string", "некорректная цифра в числе")
                .replace("value must be in range", "значение должно быть в диапазоне")
                .replace(" is not in ", " не входит в диапазон ")
                .replace("unexpected argument", "неизвестный параметр")
                .replace(" found", " обнаружен")
                .replace("Usage:", "Использование:")
                .replace("For more information, try '--help'.", "Подробнее: '--help'.")
                .replace("[OPTIONS]", "[ПАРАМЕТРЫ]"));
            std::process::exit(error.exit_code());
        }
        error.exit()
    });
    let args = Args::from_arg_matches(&matches)?;

    // Determine data directory
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        eprintln!("{}", args.language.text("Error: 'data' directory not found. Please run from the project root."));
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

    println!("{}", args.language.text("Aniimax - Aniimo Production Optimizer"));
    println!("================================================================");
    println!();
    println!("{}", args.language.text("Configuration:"));
    let target_label = match (args.language, args.currency.as_str()) {
        (Language::Ru, "wood_blocks") => "Wood Blocks",
        (Language::Ru, "mineral_sand") => "Mineral Sand",
        (Language::ZhTw, "wood_blocks") => "Wood Blocks",
        (Language::ZhTw, "mineral_sand") => "Mineral Sand",
        (_, currency) => currency,
    };
    println!("  {} {:.0} {}", args.language.text("Target:"), args.target, args.language.text(target_label));
    println!("  {} {}/{}", args.language.text("Energy Cost:"), args.energy_cost, args.language.text("min"));
    println!(
        "  {} {}", args.language.text("Mode:"),
        if args.energy_self_sufficient { 
            args.language.text("Energy Self-Sufficient")
        } else if args.parallel {
            args.language.text("Cross-Facility Parallel")
        } else { 
            args.language.text("Time Optimization")
        }
    );

    println!();
    println!("{}", args.language.text("Facilities (count x level):"));
    for (name, count, level) in [
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
    ] {
        if args.language == Language::ZhTw {
            println!("  {} {} x {}{}", zh_tw::pad_end(zh_tw::text(name), 22), count, zh_tw::text("Lv."), level);
            continue;
        }
        println!("  {:<22} {} x {}{}", args.language.text(name), count, args.language.text("Lv."), level);
    }

    println!();
    println!("{}", args.language.text("Item Modules:"));
    for (name, level) in [
        ("Ecological Module", args.ecological_module),
        ("Kitchen Module", args.kitchen_module),
        ("Resource Detector", args.resource_detector),
        ("Crafting Module", args.crafting_module),
    ] {
        if args.language == Language::ZhTw {
            println!("  {} {}{}", zh_tw::pad_end(zh_tw::text(name), 22), zh_tw::text("Lv."), level);
            continue;
        }
        println!("  {:<22} {}{}", args.language.text(name), args.language.text("Lv."), level);
    }

    println!();
    println!(
        "{} {}{} {}{}",
        args.language.text("Aniimo:"),
        args.language.text("Lv."),
        args.aniimo_level,
        args.language.text("suitability"),
        if args.personality_bonus { args.language.text(", personality bonus") } else { "" }
    );

    // Load all data, with times set for the Aniimo working each facility
    let mut items = load_all_data(data_dir)?;
    let mut workers = Workers::new();
    for facility in WORKER_FACILITIES {
        workers.set(facility, Worker::new(args.aniimo_level, args.personality_bonus));
    }
    let requirements = aniimax::data::load_aniimo_requirements(data_dir)?;
    workers.apply(&requirements, &mut items);
    println!();
    println!("{} {} {}", args.language.text("Loaded"), items.len(), args.language.text("production items."));

    // Calculate efficiencies
    let efficiencies =
        calculate_efficiencies(&items, &args.currency, &facility_counts, &module_levels);

    if efficiencies.is_empty() {
        println!();
        println!(
            "{} {} {}",
            args.language.text("[WARNING] No items found that produce"),
            args.language.text(&args.currency),
            args.language.text("with current facility levels.")
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
        display_results_in(&path, &efficiencies, false, args.language);

        if args.energy_cost > 0.0 && !args.energy_self_sufficient {
            display_energy_recommendations_in(&efficiencies, args.language);
        }
    } else {
        println!();
        if args.energy_self_sufficient {
            println!("{}", args.language.text("[WARNING] Cannot achieve energy self-sufficiency with current setup."));
            println!("{}", args.language.text("Try increasing facility counts or reducing energy cost."));
        } else {
            println!("{}", args.language.text("[WARNING] Could not find a valid production path."));
        }
    }

    Ok(())
}
