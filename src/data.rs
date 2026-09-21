//! Data loading functionality for Aniimax.
//!
//! This module handles loading production data from CSV files located
//! in the `data/` directory. Each facility type has its own CSV format
//! and dedicated loading function.

use csv::ReaderBuilder;
use std::error::Error;
use std::fs::File;
use std::path::Path;

use crate::models::{
    AniimoRequirements, FarmlandRow, GrowerStep, GrowerSteps, MineralRow, ProcessingRowNoEnergy, ProcessingRowWithEnergy,
    ProductionItem, WoodlandRow,
};

/// One row of `aniimo_requirements.csv`.
#[derive(Debug, serde::Deserialize)]
struct AniimoRequirementRow {
    name: String,
    ability: String,
    min_level: u32,
}

/// Parses the contents of `aniimo_requirements.csv` (columns `name, facility, ability,
/// min_level`). Takes the CSV text rather than a path so the web build can pass its embedded copy.
pub fn parse_aniimo_requirements(csv_text: &str) -> Result<AniimoRequirements, Box<dyn Error>> {
    let mut reqs = AniimoRequirements::default();
    let mut rdr = ReaderBuilder::new().trim(csv::Trim::All).from_reader(csv_text.as_bytes());
    for row in rdr.deserialize::<AniimoRequirementRow>() {
        let row = row?;
        reqs.insert(&row.name, &row.ability, row.min_level);
    }
    Ok(reqs)
}

/// Loads `aniimo_requirements.csv` from the data directory; see [`parse_aniimo_requirements`].
pub fn load_aniimo_requirements(data_dir: &Path) -> Result<AniimoRequirements, Box<dyn Error>> {
    parse_aniimo_requirements(&std::fs::read_to_string(data_dir.join("aniimo_requirements.csv"))?)
}

/// One row of `grower_steps.csv`.
#[derive(Debug, serde::Deserialize)]
struct GrowerStepRow {
    name: String,
    step: String,
    ability: String,
    min_level: u32,
    workload: f64,
}

/// Parses the contents of `grower_steps.csv` (columns `name, facility, step, ability, min_level,
/// workload`), keeping each item's steps in file order.
pub fn parse_grower_steps(csv_text: &str) -> Result<GrowerSteps, Box<dyn Error>> {
    let mut steps = GrowerSteps::default();
    let mut rdr = ReaderBuilder::new().trim(csv::Trim::All).from_reader(csv_text.as_bytes());
    for row in rdr.deserialize::<GrowerStepRow>() {
        let row = row?;
        steps.insert(
            &row.name,
            GrowerStep { step: row.step, ability: row.ability, min_level: row.min_level, workload: row.workload },
        );
    }
    Ok(steps)
}

/// Loads `grower_steps.csv` from the data directory; see [`parse_grower_steps`].
pub fn load_grower_steps(data_dir: &Path) -> Result<GrowerSteps, Box<dyn Error>> {
    parse_grower_steps(&std::fs::read_to_string(data_dir.join("grower_steps.csv"))?)
}

/// One row of `unverified.csv`.
#[derive(Debug, serde::Deserialize)]
struct UnverifiedRow {
    name: String,
    facility: String,
}

/// Parses the contents of `unverified.csv` (columns `name, facility`): every recipe whose
/// numbers haven't been checked in game yet, as `(recipe, facility)` pairs. A recipe comes off the
/// list once someone confirms it in game.
pub fn parse_unverified(csv_text: &str) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let mut rdr = ReaderBuilder::new().trim(csv::Trim::All).from_reader(csv_text.as_bytes());
    let mut rows = Vec::new();
    for row in rdr.deserialize::<UnverifiedRow>() {
        let row = row?;
        rows.push((row.name, row.facility));
    }
    Ok(rows)
}

/// Loads `unverified.csv` from the data directory; see [`parse_unverified`].
pub fn load_unverified(data_dir: &Path) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    parse_unverified(&std::fs::read_to_string(data_dir.join("unverified.csv"))?)
}

/// Parses a module requirement string (e.g., "ecological_module:1") into a tuple.
///
/// Returns `None` if the string is empty or invalid.
fn parse_module_requirement(req: &Option<String>) -> Option<(String, u32)> {
    req.as_ref().and_then(|s| {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 2 {
            if let Ok(level) = parts[1].parse::<u32>() {
                return Some((parts[0].to_string(), level));
            }
        }
        None
    })
}

/// Parses a semicolon-separated list of raw material names.
///
/// # Example
/// - "wheat" -> vec!["wheat"]
/// - "lavender;rose" -> vec!["lavender", "rose"]
fn parse_raw_materials(s: &str) -> Vec<String> {
    s.split(';')
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

/// Parses a semicolon-separated list of required amounts.
///
/// # Example
/// - "3" -> vec![3]
/// - "3;3" -> vec![3, 3]
fn parse_required_amounts(s: &str) -> Vec<u32> {
    s.split(';')
        .filter_map(|part| part.trim().parse::<u32>().ok())
        .collect()
}

/// Loads farmland crop data from a CSV file.
///
/// # Arguments
///
/// * `path` - Path to the farmland CSV file
///
/// # Returns
///
/// A vector of [`ProductionItem`] representing all farmland crops,
/// or an error if the file cannot be read or parsed.
///
/// # CSV Format
///
/// Expected columns: `name, cost, sell_value, production_time, yield, energy, facility_level, module_requirement`
pub fn load_farmland(path: &Path) -> Result<Vec<ProductionItem>, Box<dyn Error>> {
    let file = File::open(path)?;
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(file);

    let mut items = Vec::new();
    for result in rdr.deserialize() {
        let row: FarmlandRow = result?;
        items.push(ProductionItem {
            name: row.name,
            facility: "Farmland".to_string(),
            raw_materials: None,
            required_amount: None,
            cost: Some(row.cost),
            sell_currency: "coins".to_string(),
            sell_value: row.sell_value,
            production_time: row.production_time,
            yield_amount: row.yield_amount,
            energy: row.energy,
            facility_level: row.facility_level,
            module_requirement: parse_module_requirement(&row.module_requirement),
            workload: None,
            byproduct: None,
            environment: row.environment,
        });
    }
    Ok(items)
}

/// Loads woodland tree data from a CSV file.
///
/// # Arguments
///
/// * `path` - Path to the woodland CSV file
///
/// # Returns
///
/// A vector of [`ProductionItem`] representing all woodland trees,
/// or an error if the file cannot be read or parsed.
///
/// # CSV Format
///
/// Expected columns: `name, cost, sell_currency, sell_value, production_time, yield, energy, facility_level, module_requirement`
///
/// # Notes
///
/// The energy field may contain "NULL" as a string value, which is converted to `None`.
pub fn load_woodland(path: &Path) -> Result<Vec<ProductionItem>, Box<dyn Error>> {
    let file = File::open(path)?;
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(file);

    let mut items = Vec::new();
    for result in rdr.deserialize() {
        let row: WoodlandRow = result?;
        let energy = row
            .energy
            .and_then(|e| if e == "NULL" { None } else { e.parse().ok() });
        items.push(ProductionItem {
            name: row.name,
            facility: "Woodland".to_string(),
            raw_materials: None,
            required_amount: None,
            cost: Some(row.cost),
            sell_currency: row.sell_currency,
            sell_value: row.sell_value,
            production_time: row.production_time,
            yield_amount: row.yield_amount,
            energy,
            facility_level: row.facility_level,
            module_requirement: parse_module_requirement(&row.module_requirement),
            workload: None,
            byproduct: row
                .byproduct_yield
                .map(|amt| ("Wood Blocks".to_string(), amt)),
            environment: row.environment,
        });
    }
    Ok(items)
}

/// Loads a workload-driven gathering facility (no planting cost) from a CSV file.
///
/// # Arguments
///
/// * `path` - Path to the facility's CSV file
/// * `facility_name` - Name of the facility (e.g., "Mine")
/// * `byproduct_name` - Resource the `byproduct_yield` column produces, or `None` for a facility
///   with no byproduct; any `byproduct_yield` value is ignored in that case
///
/// # Returns
///
/// A vector of [`ProductionItem`] representing the facility's items,
/// or an error if the file cannot be read or parsed.
///
/// # CSV Format
///
/// Expected columns: `name, sell_currency, sell_value, workload, yield, byproduct_yield, facility_level, module_requirement, environment`
///
/// `workload` is converted into a production time for a level-1 Aniimo; call
/// [`crate::models::Workers::apply`] afterward to use the player's own (see
/// [`crate::models::Worker`] for the measured speeds).
///
/// Shared by every gathering facility that uses this CSV shape: Mine, Well and Tidewhisper
/// Sandcastle.
pub fn load_workload_raw_material(
    path: &Path,
    facility_name: &str,
    byproduct_name: Option<&str>,
) -> Result<Vec<ProductionItem>, Box<dyn Error>> {
    let file = File::open(path)?;
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(file);

    let mut items = Vec::new();
    for result in rdr.deserialize() {
        let row: MineralRow = result?;
        items.push(ProductionItem {
            name: row.name,
            facility: facility_name.to_string(),
            raw_materials: None,
            required_amount: None,
            cost: None,
            sell_currency: row.sell_currency,
            sell_value: row.sell_value,
            production_time: crate::models::Worker::default().seconds_for(row.workload, 1, false),
            yield_amount: row.yield_amount,
            energy: None,
            facility_level: row.facility_level,
            module_requirement: parse_module_requirement(&row.module_requirement),
            workload: Some(row.workload),
            byproduct: byproduct_name.zip(row.byproduct_yield).map(|(name, amt)| (name.to_string(), amt)),
            environment: row.environment,
        });
    }
    Ok(items)
}

/// Loads Mine data (thin wrapper over [`load_workload_raw_material`]); yields Mineral Sand as a
/// byproduct.
pub fn load_mine(path: &Path) -> Result<Vec<ProductionItem>, Box<dyn Error>> {
    load_workload_raw_material(path, "Mine", Some("Mineral Sand"))
}

/// Loads Well data (thin wrapper over [`load_workload_raw_material`]); no byproduct.
pub fn load_well(path: &Path) -> Result<Vec<ProductionItem>, Box<dyn Error>> {
    load_workload_raw_material(path, "Well", None)
}

/// Loads Tidewhisper Sandcastle data (thin wrapper over [`load_workload_raw_material`]); no
/// byproduct. Pearl needs a Warm environment.
pub fn load_tidewhisper_sandcastle(path: &Path) -> Result<Vec<ProductionItem>, Box<dyn Error>> {
    load_workload_raw_material(path, "Tidewhisper Sandcastle", None)
}

/// Loads processing facility data that includes energy tracking.
///
/// # Arguments
///
/// * `path` - Path to the facility's CSV file
/// * `facility_name` - Name of the facility (e.g., "Carousel Mill")
///
/// # Returns
///
/// A vector of [`ProductionItem`] representing all recipes for this facility,
/// or an error if the file cannot be read or parsed.
///
/// # CSV Format
///
/// Expected columns: `name, raw_materials, required_amount, sell_value, production_time OR workload, energy, facility_level, module_requirement`
pub fn load_processing_with_energy(
    path: &Path,
    facility_name: &str,
) -> Result<Vec<ProductionItem>, Box<dyn Error>> {
    let file = File::open(path)?;
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(file);

    let mut items = Vec::new();
    for result in rdr.deserialize() {
        let row: ProcessingRowWithEnergy = result?;
        let raw_mats = parse_raw_materials(&row.raw_materials);
        let req_amounts = parse_required_amounts(&row.required_amount);
        let production_time = row
            .workload
            .map(|w| crate::models::Worker::default().seconds_for(w, 1, false))
            .or(row.production_time)
            .expect("row must have either workload or production_time");
        items.push(ProductionItem {
            name: row.name,
            facility: facility_name.to_string(),
            raw_materials: Some(raw_mats),
            required_amount: Some(req_amounts),
            cost: None,
            sell_currency: row.sell_currency.unwrap_or_else(|| "coins".to_string()),
            sell_value: row.sell_value,
            production_time,
            yield_amount: 1,
            energy: row.energy,
            facility_level: row.facility_level,
            module_requirement: parse_module_requirement(&row.module_requirement),
            workload: row.workload,
            byproduct: None,
            environment: None,
        });
    }
    Ok(items)
}

/// Loads processing facility data without energy tracking.
///
/// # Arguments
///
/// * `path` - Path to the facility's CSV file
/// * `facility_name` - Name of the facility (e.g., "Crafting Table")
///
/// # Returns
///
/// A vector of [`ProductionItem`] representing all recipes for this facility,
/// or an error if the file cannot be read or parsed.
///
/// # CSV Format
///
/// Expected columns: `name, raw_materials, required_amount, sell_value, sell_currency (optional, default coins), production_time OR workload, facility_level, module_requirement`
pub fn load_processing_no_energy(
    path: &Path,
    facility_name: &str,
) -> Result<Vec<ProductionItem>, Box<dyn Error>> {
    let file = File::open(path)?;
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(file);

    let mut items = Vec::new();
    for result in rdr.deserialize() {
        let row: ProcessingRowNoEnergy = result?;
        let raw_mats = parse_raw_materials(&row.raw_materials);
        let req_amounts = parse_required_amounts(&row.required_amount);
        let production_time = row
            .workload
            .map(|w| crate::models::Worker::default().seconds_for(w, 1, false))
            .or(row.production_time)
            .expect("row must have either workload or production_time");
        items.push(ProductionItem {
            name: row.name,
            facility: facility_name.to_string(),
            raw_materials: Some(raw_mats),
            required_amount: Some(req_amounts),
            cost: None,
            sell_currency: row.sell_currency.unwrap_or_else(|| "coins".to_string()),
            sell_value: row.sell_value,
            production_time,
            yield_amount: row.yield_amount.unwrap_or(1),
            energy: None,
            facility_level: row.facility_level,
            module_requirement: parse_module_requirement(&row.module_requirement),
            workload: row.workload,
            byproduct: None,
            environment: None,
        });
    }
    Ok(items)
}

/// Loads all production data from the data directory.
///
/// This function loads data from all facility types:
/// - Raw materials: Farmland, Woodland, Mine, Well, and the Aniimo material facilities
///   (Tidewhisper Sandcastle, Dewy House, Nimbus Bed, Starfall Hammock, Floral Windmill)
/// - Processing: Carousel Mill, Jukebox Dryer, Claw Game Cooker, Crafting Table, Simmering Pot,
///   Phonolfactory Table, Bouncy Brew Keg, Blazing Stove, Pickling Jar, Joy Wheel Loom
///
/// Facilities that make nothing sellable for coins are left out: Aniipod Maker, Dance Pad
/// Polisher, Woodworking Bench and Chimney Kiln.
///
/// # Arguments
///
/// * `data_dir` - Path to the directory containing CSV files
///
/// # Returns
///
/// A vector containing all [`ProductionItem`]s from all facilities,
/// or an error if any file cannot be read.
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
/// use aniimax::data::load_all_data;
///
/// let items = load_all_data(Path::new("data")).unwrap();
/// println!("Loaded {} items", items.len());
/// ```
pub fn load_all_data(data_dir: &Path) -> Result<Vec<ProductionItem>, Box<dyn Error>> {
    let mut all_items = Vec::new();

    // Load raw material sources
    all_items.extend(load_farmland(&data_dir.join("farmland.csv"))?);
    all_items.extend(load_woodland(&data_dir.join("woodland.csv"))?);
    all_items.extend(load_mine(&data_dir.join("mine.csv"))?);
    all_items.extend(load_well(&data_dir.join("well.csv"))?);
    for (file, facility) in ANIIMO_MATERIAL_FILES {
        all_items.extend(load_workload_raw_material(&data_dir.join(file), facility, None)?);
    }

    // Load processing facilities
    all_items.extend(load_processing_with_energy(
        &data_dir.join("carousel_mill.csv"),
        "Carousel Mill",
    )?);
    all_items.extend(load_processing_with_energy(
        &data_dir.join("jukebox_dryer.csv"),
        "Jukebox Dryer",
    )?);
    all_items.extend(load_processing_with_energy(
        &data_dir.join("claw_game_cooker.csv"),
        "Claw Game Cooker",
    )?);
    for (file, facility) in PROCESSOR_FILES {
        all_items.extend(load_processing_no_energy(&data_dir.join(file), facility)?);
    }

    Ok(all_items)
}

/// Aniimo-worked gathering facilities that share the Mine's CSV layout but have no byproduct.
const ANIIMO_MATERIAL_FILES: [(&str, &str); 5] = [
    ("tidewhisper_sandcastle.csv", "Tidewhisper Sandcastle"),
    ("dewy_house.csv", "Dewy House"),
    ("nimbus_bed.csv", "Nimbus Bed"),
    ("starfall_hammock.csv", "Starfall Hammock"),
    ("floral_windmill.csv", "Floral Windmill"),
];

/// Processors whose CSVs use the no-energy layout (see [`load_processing_no_energy`]).
const PROCESSOR_FILES: [(&str, &str); 11] = [
    ("crafting_table.csv", "Crafting Table"),
    ("simmering_pot.csv", "Simmering Pot"),
    ("phonolfactory_table.csv", "Phonolfactory Table"),
    ("bouncy_brew_keg.csv", "Bouncy Brew Keg"),
    ("blazing_stove.csv", "Blazing Stove"),
    ("pickling_jar.csv", "Pickling Jar"),
    ("joy_wheel_loom.csv", "Joy Wheel Loom"),
    ("woodworking_bench.csv", "Woodworking Bench"),
    ("chimney_kiln.csv", "Chimney Kiln"),
    ("dance_pad_polisher.csv", "Dance Pad Polisher"),
    ("aniipod_maker.csv", "Aniipod Maker"),
];
