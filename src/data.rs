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
    FarmlandRow, MineralRow, ProcessingRowNoEnergy, ProcessingRowWithEnergy,
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
/// Expected columns: `name, sell_currency, sell_value, production_time, yield, facility_level, module_requirement`
pub fn load_mineral_pile(path: &Path) -> Result<Vec<ProductionItem>, Box<dyn Error>> {
    let file = File::open(path)?;
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(file);

    let mut items = Vec::new();
    for result in rdr.deserialize() {
        let row: MineralRow = result?;
        items.push(ProductionItem {
            name: row.name,
            facility: "Mineral Pile".to_string(),
            raw_materials: None,
            required_amount: None,
            cost: None,
            sell_currency: row.sell_currency,
            sell_value: row.sell_value,
            production_time: row.production_time,
            yield_amount: row.yield_amount,
            energy: None,
            facility_level: row.facility_level,
            module_requirement: parse_module_requirement(&row.module_requirement),
        });
    }
    Ok(items)
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
/// Expected columns: `name, raw_materials, required_amount, sell_value, production_time, energy, facility_level, module_requirement`
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
        items.push(ProductionItem {
            name: row.name,
            facility: facility_name.to_string(),
            raw_materials: Some(row.raw_materials),
            required_amount: Some(row.required_amount),
            cost: None,
            sell_currency: "coins".to_string(),
            sell_value: row.sell_value,
            production_time: row.production_time,
            yield_amount: 1,
            energy: Some(row.energy),
            facility_level: row.facility_level,
            module_requirement: parse_module_requirement(&row.module_requirement),
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
/// Expected columns: `name, raw_materials, required_amount, sell_value, production_time, facility_level, module_requirement`
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
        items.push(ProductionItem {
            name: row.name,
            facility: facility_name.to_string(),
            raw_materials: Some(row.raw_materials),
            required_amount: Some(row.required_amount),
            cost: None,
            sell_currency: "coins".to_string(),
            sell_value: row.sell_value,
            production_time: row.production_time,
            yield_amount: 1,
            energy: None,
            facility_level: row.facility_level,
            module_requirement: parse_module_requirement(&row.module_requirement),
        });
    }
    Ok(items)
}

/// Loads all production data from the data directory.
///
/// This function loads data from all facility types:
/// - Raw materials: Farmland, Woodland, Mineral Pile
/// - Processing: Carousel Mill, Jukebox Dryer, Crafting Table, Dance Pad Polisher, Aniipod Maker
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

    // Load processing facilities
    all_items.extend(load_processing_with_energy(
        &data_dir.join("carousel_mill.csv"),
        "Carousel Mill",
    )?);
    all_items.extend(load_processing_with_energy(
        &data_dir.join("jukebox_dryer.csv"),
        "Jukebox Dryer",
    )?);
    all_items.extend(load_processing_no_energy(
        &data_dir.join("crafting_table.csv"),
        "Crafting Table",
    )?);
    all_items.extend(load_processing_no_energy(
        &data_dir.join("dance_pad_polisher.csv"),
        "Dance Pad Polisher",
    )?);
    all_items.extend(load_processing_no_energy(
        &data_dir.join("aniipod_maker.csv"),
        "Aniipod Maker",
    )?);

    Ok(all_items)
}
