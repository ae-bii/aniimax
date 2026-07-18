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
    FarmlandRow, MineralRow, NimbusBedRow, ProcessingRowNoEnergy, ProcessingRowWithEnergy,
    ProductionItem, WoodlandRow,
};

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

/// Loads mineral pile data from a CSV file.
///
/// # Arguments
///
/// * `path` - Path to the mineral pile CSV file
///
/// # Returns
///
/// A vector of [`ProductionItem`] representing all mineral items,
/// or an error if the file cannot be read or parsed.
///
/// # CSV Format
///
/// Expected columns: `name, sell_currency, sell_value, workload, yield, byproduct_yield, facility_level, module_requirement`
///
/// `workload` is converted into an estimated production time via
/// [`crate::models::WORKLOAD_RATE_ESTIMATE`]; see that constant's docs for the
/// (currently single-data-point) calibration this is based on.
///
/// Shared by any "raw material, no cost, Aniimo-family/workload-driven" facility; currently
/// Mineral Pile and Grass Blossom Mat, since both follow the same CSV shape.
pub fn load_workload_raw_material(
    path: &Path,
    facility_name: &str,
    byproduct_name: &str,
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
            production_time: row.workload / crate::models::WORKLOAD_RATE_ESTIMATE,
            yield_amount: row.yield_amount,
            energy: None,
            facility_level: row.facility_level,
            module_requirement: parse_module_requirement(&row.module_requirement),
            workload: Some(row.workload),
            byproduct: row
                .byproduct_yield
                .map(|amt| (byproduct_name.to_string(), amt)),
            environment: row.environment,
        });
    }
    Ok(items)
}

/// Loads Mineral Pile data (thin wrapper over [`load_workload_raw_material`]).
pub fn load_mineral_pile(path: &Path) -> Result<Vec<ProductionItem>, Box<dyn Error>> {
    load_workload_raw_material(path, "Mineral Pile", "Mineral Sand")
}

/// Loads Grass Blossom Mat data (thin wrapper over [`load_workload_raw_material`]).
///
/// Facility level and byproduct are not yet confirmed for this facility. `byproduct_name` is a
/// placeholder ("Mineral Sand") that will only take effect if `byproduct_yield` is ever
/// populated in the CSV.
pub fn load_grass_blossom_mat(path: &Path) -> Result<Vec<ProductionItem>, Box<dyn Error>> {
    load_workload_raw_material(path, "Grass Blossom Mat", "Mineral Sand")
}

/// Loads Tidewhisper Sandcastle data (thin wrapper over [`load_workload_raw_material`]).
///
/// Facility level guessed as 1 (unconfirmed). Every item here requires a growing environment
/// (pearl/quick_pearl: Cool, love_bubble: Freeze); tracked via `environment` and capacity-gated
/// by owned environment buildings, see `crate::coverage`.
pub fn load_tidewhisper_sandcastle(path: &Path) -> Result<Vec<ProductionItem>, Box<dyn Error>> {
    load_workload_raw_material(path, "Tidewhisper Sandcastle", "Mineral Sand")
}

/// Loads Starfall Hammock data (thin wrapper over [`load_workload_raw_material`]).
///
/// Facility level guessed as 1 (unconfirmed). Requires a "Cool" growing environment; tracked via
/// `environment` and capacity-gated by owned environment buildings, see `crate::coverage`.
pub fn load_starfall_hammock(path: &Path) -> Result<Vec<ProductionItem>, Box<dyn Error>> {
    load_workload_raw_material(path, "Starfall Hammock", "Mineral Sand")
}

/// Loads Dewy House data (thin wrapper over [`load_workload_raw_material`]).
///
/// Facility level guessed as 1 (unconfirmed). Requires a "Warm" growing environment; tracked via
/// `environment` and capacity-gated by owned environment buildings, see `crate::coverage`.
pub fn load_dewy_house(path: &Path) -> Result<Vec<ProductionItem>, Box<dyn Error>> {
    load_workload_raw_material(path, "Dewy House", "Mineral Sand")
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
            .map(|w| w / crate::models::WORKLOAD_RATE_ESTIMATE)
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
            .map(|w| w / crate::models::WORKLOAD_RATE_ESTIMATE)
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

/// Loads nimbus bed data from a CSV file.
///
/// # Arguments
///
/// * `path` - Path to the nimbus bed CSV file
///
/// # Returns
///
/// A vector of [`ProductionItem`] representing all nimbus bed products,
/// or an error if the file cannot be read or parsed.
///
/// # CSV Format
///
/// Expected columns: `name, sell_value, workload, yield`
///
/// `workload` is converted into an estimated production time via
/// [`crate::models::WORKLOAD_RATE_ESTIMATE`].
pub fn load_nimbus_bed(path: &Path) -> Result<Vec<ProductionItem>, Box<dyn Error>> {
    let file = File::open(path)?;
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(file);

    let mut items = Vec::new();
    for result in rdr.deserialize() {
        let row: NimbusBedRow = result?;
        items.push(ProductionItem {
            name: row.name,
            facility: "Nimbus Bed".to_string(),
            raw_materials: None,
            required_amount: None,
            cost: None,
            sell_currency: "coins".to_string(),
            sell_value: row.sell_value,
            production_time: row.workload / crate::models::WORKLOAD_RATE_ESTIMATE,
            yield_amount: row.yield_amount,
            energy: None,
            facility_level: 1,
            module_requirement: None,
            workload: Some(row.workload),
            byproduct: None,
            environment: None,
        });
    }
    Ok(items)
}

/// Loads all production data from the data directory.
///
/// This function loads data from all facility types:
/// - Raw materials: Farmland, Woodland, Mineral Pile, Nimbus Bed, Grass Blossom Mat,
///   Tidewhisper Sandcastle, Starfall Hammock, Dewy House
/// - Processing: Carousel Mill, Jukebox Dryer, Claw Game Cooker, Crafting Table,
///   Phonolfactory Table, Bouncy Brew Keg, Joy Wheel Loom
///
/// (Dance Pad Polisher and Aniipod Maker are excluded: they don't produce coins/Bud Tickets, so
/// they're out of scope for this optimizer.)
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
    all_items.extend(load_mineral_pile(&data_dir.join("mineral_pile.csv"))?);
    all_items.extend(load_nimbus_bed(&data_dir.join("nimbus_bed.csv"))?);
    all_items.extend(load_grass_blossom_mat(
        &data_dir.join("grass_blossom_mat.csv"),
    )?);
    all_items.extend(load_tidewhisper_sandcastle(
        &data_dir.join("tidewhisper_sandcastle.csv"),
    )?);
    all_items.extend(load_starfall_hammock(
        &data_dir.join("starfall_hammock.csv"),
    )?);
    all_items.extend(load_dewy_house(&data_dir.join("dewy_house.csv"))?);

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
    all_items.extend(load_processing_no_energy(
        &data_dir.join("crafting_table.csv"),
        "Crafting Table",
    )?);
    all_items.extend(load_processing_no_energy(
        &data_dir.join("phonolfactory_table.csv"),
        "Phonolfactory Table",
    )?);
    all_items.extend(load_processing_no_energy(
        &data_dir.join("bouncy_brew_keg.csv"),
        "Bouncy Brew Keg",
    )?);
    all_items.extend(load_processing_no_energy(
        &data_dir.join("joy_wheel_loom.csv"),
        "Joy Wheel Loom",
    )?);

    Ok(all_items)
}
