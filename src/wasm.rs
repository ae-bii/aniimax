//! WebAssembly bindings for Aniimax.
//!
//! This module provides JavaScript-accessible functions for the production optimizer.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::models::{FacilityCounts, ModuleLevels, ProductionEfficiency, ProductionItem};
use crate::optimizer::{
    calculate_efficiencies, calculate_energy_efficiencies, find_best_production_path,
    find_parallel_production_path, find_self_sufficient_path,
};

/// JavaScript-friendly facility configuration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct JsFacilityConfig {
    #[serde(default)]
    pub count: u32,
    #[serde(default = "default_level")]
    pub level: u32,
}

fn default_level() -> u32 {
    1
}

/// JavaScript-friendly module levels configuration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct JsModuleLevels {
    #[serde(default)]
    pub ecological_module: u32,
    #[serde(default)]
    pub kitchen_module: u32,
    #[serde(default)]
    pub mineral_detector: u32,
    #[serde(default)]
    pub crafting_module: u32,
}

/// JavaScript-friendly input for optimization.
#[derive(Debug, Clone, Deserialize)]
pub struct JsOptimizeInput {
    pub target_amount: f64,
    pub currency: String,
    pub energy_self_sufficient: bool,
    pub energy_cost_per_min: f64,
    #[serde(default)]
    pub parallel: bool,
    pub farmland: JsFacilityConfig,
    pub woodland: JsFacilityConfig,
    pub mineral_pile: JsFacilityConfig,
    pub carousel_mill: JsFacilityConfig,
    pub jukebox_dryer: JsFacilityConfig,
    pub crafting_table: JsFacilityConfig,
    pub dance_pad_polisher: JsFacilityConfig,
    pub aniipod_maker: JsFacilityConfig,
    #[serde(default)]
    pub nimbus_bed: JsFacilityConfig,
    #[serde(default)]
    pub modules: JsModuleLevels,
}

/// JavaScript-friendly production step output.
#[derive(Debug, Clone, Serialize)]
pub struct JsProductionStep {
    pub item_name: String,
    pub facility: String,
    pub quantity: u32,
    pub time_seconds: f64,
    pub energy: Option<f64>,
}

/// JavaScript-friendly efficiency output.
#[derive(Debug, Clone, Serialize)]
pub struct JsEfficiency {
    pub item_name: String,
    pub facility: String,
    pub facility_level: u32,
    pub profit_per_second: f64,
    pub profit_per_energy: Option<f64>,
    pub total_time_per_unit: f64,
    pub total_energy_per_unit: Option<f64>,
    pub sell_value: f64,
    pub yield_amount: u32,
    pub requires_raw: Option<String>,
}

/// JavaScript-friendly optimization result.
#[derive(Debug, Clone, Serialize)]
pub struct JsOptimizeResult {
    pub success: bool,
    pub error: Option<String>,
    pub steps: Vec<JsProductionStep>,
    pub total_time_seconds: f64,
    pub total_time_formatted: String,
    pub total_energy: Option<f64>,
    pub total_profit: f64,
    pub items_produced: u32,
    pub currency: String,
    pub all_efficiencies: Vec<JsEfficiency>,
    pub is_energy_self_sufficient: bool,
    pub energy_items_produced: Option<u32>,
    pub energy_item_name: Option<String>,
}

impl From<&ProductionEfficiency> for JsEfficiency {
    fn from(eff: &ProductionEfficiency) -> Self {
        JsEfficiency {
            item_name: eff.item.name.clone(),
            facility: eff.item.facility.clone(),
            facility_level: eff.item.facility_level,
            profit_per_second: eff.profit_per_second,
            profit_per_energy: eff.profit_per_energy,
            total_time_per_unit: eff.total_time_per_unit,
            total_energy_per_unit: eff.total_energy_per_unit,
            sell_value: eff.item.sell_value,
            yield_amount: eff.item.yield_amount,
            requires_raw: eff.requires_raw.clone(),
        }
    }
}

/// Format seconds into human-readable time string.
fn format_time(seconds: f64) -> String {
    let total_secs = seconds as u64;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

/// Get embedded production data.
/// This embeds the CSV data directly into the WASM binary.
fn get_embedded_items() -> Vec<ProductionItem> {
    use csv::ReaderBuilder;

    // Helper to parse module requirement string
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

    // Helper to parse semicolon-separated raw material names
    fn parse_raw_materials(s: &str) -> Vec<String> {
        s.split(';')
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect()
    }

    // Helper to parse semicolon-separated required amounts
    fn parse_required_amounts(s: &str) -> Vec<u32> {
        s.split(';')
            .filter_map(|part| part.trim().parse::<u32>().ok())
            .collect()
    }

    let mut items = Vec::new();

    // Farmland items
    let farmland_data = include_str!("../data/farmland.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(farmland_data.as_bytes());
    for result in rdr.deserialize::<crate::models::FarmlandRow>() {
        if let Ok(row) = result {
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
                requires_fertilizer: row.facility_level >= 4,
            });
        }
    }

    // Woodland items
    let woodland_data = include_str!("../data/woodland.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(woodland_data.as_bytes());
    for result in rdr.deserialize::<crate::models::WoodlandRow>() {
        if let Ok(row) = result {
            let energy = row.energy.and_then(|e| {
                if e == "NULL" { None } else { e.parse().ok() }
            });
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
                requires_fertilizer: row.facility_level >= 3,
            });
        }
    }

    // Mineral Pile items
    let mineral_data = include_str!("../data/mineral_pile.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(mineral_data.as_bytes());
    for result in rdr.deserialize::<crate::models::MineralRow>() {
        if let Ok(row) = result {
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
                requires_fertilizer: false,
            });
        }
    }

    // Carousel Mill items
    let carousel_data = include_str!("../data/carousel_mill.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(carousel_data.as_bytes());
    for result in rdr.deserialize::<crate::models::ProcessingRowWithEnergy>() {
        if let Ok(row) = result {
            let raw_mats = parse_raw_materials(&row.raw_materials);
            let req_amounts = parse_required_amounts(&row.required_amount);
            items.push(ProductionItem {
                name: row.name,
                facility: "Carousel Mill".to_string(),
                raw_materials: Some(raw_mats),
                required_amount: Some(req_amounts),
                cost: None,
                sell_currency: "coins".to_string(),
                sell_value: row.sell_value,
                production_time: row.production_time,
                yield_amount: 1,
                energy: row.energy,
                facility_level: row.facility_level,
                module_requirement: parse_module_requirement(&row.module_requirement),
                requires_fertilizer: false,
            });
        }
    }

    // Jukebox Dryer items
    let jukebox_data = include_str!("../data/jukebox_dryer.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(jukebox_data.as_bytes());
    for result in rdr.deserialize::<crate::models::ProcessingRowWithEnergy>() {
        if let Ok(row) = result {
            let raw_mats = parse_raw_materials(&row.raw_materials);
            let req_amounts = parse_required_amounts(&row.required_amount);
            items.push(ProductionItem {
                name: row.name,
                facility: "Jukebox Dryer".to_string(),
                raw_materials: Some(raw_mats),
                required_amount: Some(req_amounts),
                cost: None,
                sell_currency: "coins".to_string(),
                sell_value: row.sell_value,
                production_time: row.production_time,
                yield_amount: 1,
                energy: row.energy,
                facility_level: row.facility_level,
                module_requirement: parse_module_requirement(&row.module_requirement),
                requires_fertilizer: false,
            });
        }
    }

    // Crafting Table items
    let crafting_data = include_str!("../data/crafting_table.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(crafting_data.as_bytes());
    for result in rdr.deserialize::<crate::models::ProcessingRowNoEnergy>() {
        if let Ok(row) = result {
            let raw_mats = parse_raw_materials(&row.raw_materials);
            let req_amounts = parse_required_amounts(&row.required_amount);
            items.push(ProductionItem {
                name: row.name,
                facility: "Crafting Table".to_string(),
                raw_materials: Some(raw_mats),
                required_amount: Some(req_amounts),
                cost: None,
                sell_currency: "coupons".to_string(),
                sell_value: row.sell_value,
                production_time: row.production_time,
                yield_amount: 1,
                energy: None,
                facility_level: row.facility_level,
                module_requirement: parse_module_requirement(&row.module_requirement),
                requires_fertilizer: false,
            });
        }
    }

    // Dance Pad Polisher items
    let dance_data = include_str!("../data/dance_pad_polisher.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(dance_data.as_bytes());
    for result in rdr.deserialize::<crate::models::ProcessingRowNoEnergy>() {
        if let Ok(row) = result {
            let raw_mats = parse_raw_materials(&row.raw_materials);
            let req_amounts = parse_required_amounts(&row.required_amount);
            items.push(ProductionItem {
                name: row.name,
                facility: "Dance Pad Polisher".to_string(),
                raw_materials: Some(raw_mats),
                required_amount: Some(req_amounts),
                cost: None,
                sell_currency: "coupons".to_string(),
                sell_value: row.sell_value,
                production_time: row.production_time,
                yield_amount: 1,
                energy: None,
                facility_level: row.facility_level,
                module_requirement: parse_module_requirement(&row.module_requirement),
                requires_fertilizer: false,
            });
        }
    }

    // Aniipod Maker items
    let aniipod_data = include_str!("../data/aniipod_maker.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(aniipod_data.as_bytes());
    for result in rdr.deserialize::<crate::models::ProcessingRowNoEnergy>() {
        if let Ok(row) = result {
            let raw_mats = parse_raw_materials(&row.raw_materials);
            let req_amounts = parse_required_amounts(&row.required_amount);
            items.push(ProductionItem {
                name: row.name,
                facility: "Aniipod Maker".to_string(),
                raw_materials: Some(raw_mats),
                required_amount: Some(req_amounts),
                cost: None,
                sell_currency: "coins".to_string(),
                sell_value: row.sell_value,
                production_time: row.production_time,
                yield_amount: 1,
                energy: None,
                facility_level: row.facility_level,
                module_requirement: parse_module_requirement(&row.module_requirement),
                requires_fertilizer: false,
            });
        }
    }

    // Nimbus Bed items (produces fertilizer, wool, petals)
    let nimbus_data = include_str!("../data/nimbus_bed.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(nimbus_data.as_bytes());
    for result in rdr.deserialize::<crate::models::NimbusBedRow>() {
        if let Ok(row) = result {
            items.push(ProductionItem {
                name: row.name,
                facility: "Nimbus Bed".to_string(),
                raw_materials: None,
                required_amount: None,
                cost: None,
                sell_currency: "coins".to_string(),
                sell_value: row.sell_value,
                production_time: row.production_time,
                yield_amount: row.yield_amount,
                energy: None,
                facility_level: 1,
                module_requirement: None,
                requires_fertilizer: false,
            });
        }
    }

    items
}

/// Run the production optimizer with the given configuration.
///
/// Takes a JSON string input and returns a JSON string result.
#[wasm_bindgen]
pub fn optimize(input_json: &str) -> String {
    let input: JsOptimizeInput = match serde_json::from_str(input_json) {
        Ok(i) => i,
        Err(e) => {
            return serde_json::to_string(&JsOptimizeResult {
                success: false,
                error: Some(format!("Invalid input: {}", e)),
                steps: vec![],
                total_time_seconds: 0.0,
                total_time_formatted: "0s".to_string(),
                total_energy: None,
                total_profit: 0.0,
                items_produced: 0,
                currency: String::new(),
                all_efficiencies: vec![],
                is_energy_self_sufficient: false,
                energy_items_produced: None,
                energy_item_name: None,
            })
            .unwrap_or_default();
        }
    };

    let facility_counts = FacilityCounts {
        farmland: (input.farmland.count, input.farmland.level),
        woodland: (input.woodland.count, input.woodland.level),
        mineral_pile: (input.mineral_pile.count, input.mineral_pile.level),
        carousel_mill: (input.carousel_mill.count, input.carousel_mill.level),
        jukebox_dryer: (input.jukebox_dryer.count, input.jukebox_dryer.level),
        crafting_table: (input.crafting_table.count, input.crafting_table.level),
        dance_pad_polisher: (input.dance_pad_polisher.count, input.dance_pad_polisher.level),
        aniipod_maker: (input.aniipod_maker.count, input.aniipod_maker.level),
        nimbus_bed: (input.nimbus_bed.count, input.nimbus_bed.level),
    };

    let module_levels = ModuleLevels {
        ecological_module: input.modules.ecological_module,
        kitchen_module: input.modules.kitchen_module,
        mineral_detector: input.modules.mineral_detector,
        crafting_module: input.modules.crafting_module,
    };

    let items = get_embedded_items();
    let efficiencies = calculate_efficiencies(&items, &input.currency, &facility_counts, &module_levels);

    if efficiencies.is_empty() {
        return serde_json::to_string(&JsOptimizeResult {
            success: false,
            error: Some(format!(
                "No items found that produce {} with current facility levels.",
                input.currency
            )),
            steps: vec![],
            total_time_seconds: 0.0,
            total_time_formatted: "0s".to_string(),
            total_energy: None,
            total_profit: 0.0,
            items_produced: 0,
            currency: input.currency,
            all_efficiencies: vec![],
            is_energy_self_sufficient: false,
            energy_items_produced: None,
            energy_item_name: None,
        })
        .unwrap_or_default();
    }

    let all_efficiencies: Vec<JsEfficiency> = efficiencies.iter().map(JsEfficiency::from).collect();

    // Choose optimization mode
    let path_result = if input.energy_self_sufficient && input.energy_cost_per_min > 0.0 {
        // Energy self-sufficient mode
        let energy_efficiencies = calculate_energy_efficiencies(&items, &facility_counts, &module_levels);
        find_self_sufficient_path(
            &efficiencies,
            &energy_efficiencies,
            input.target_amount,
            input.energy_cost_per_min,
            &facility_counts,
        )
    } else if input.parallel {
        // Cross-facility parallel production mode
        // Try parallel first, fall back to single-facility if not beneficial
        find_parallel_production_path(
            &efficiencies,
            input.target_amount,
            &facility_counts,
        ).or_else(|| find_best_production_path(
            &efficiencies,
            input.target_amount,
            false,
            0.0,
            &facility_counts,
        ))
    } else {
        // Simple time optimization (ignore energy)
        find_best_production_path(
            &efficiencies,
            input.target_amount,
            false,
            0.0,
            &facility_counts,
        )
    };

    match path_result {
        Some(path) => {
            let steps: Vec<JsProductionStep> = path
                .steps
                .iter()
                .map(|s| JsProductionStep {
                    item_name: s.item_name.clone(),
                    facility: s.facility.clone(),
                    quantity: s.quantity,
                    time_seconds: s.time,
                    energy: s.energy,
                })
                .collect();

            serde_json::to_string(&JsOptimizeResult {
                success: true,
                error: None,
                steps,
                total_time_seconds: path.total_time,
                total_time_formatted: format_time(path.total_time),
                total_energy: path.total_energy,
                total_profit: path.total_profit,
                items_produced: path.items_produced,
                currency: path.currency,
                all_efficiencies,
                is_energy_self_sufficient: path.is_energy_self_sufficient,
                energy_items_produced: path.energy_items_produced,
                energy_item_name: path.energy_item_name,
            })
            .unwrap_or_default()
        }
        None => {
            let error_msg = if input.energy_self_sufficient {
                "Cannot achieve energy self-sufficiency with current setup. Try increasing facility counts or reducing energy cost."
            } else {
                "Could not find a valid production path."
            };
            serde_json::to_string(&JsOptimizeResult {
                success: false,
                error: Some(error_msg.to_string()),
                steps: vec![],
                total_time_seconds: 0.0,
                total_time_formatted: "0s".to_string(),
                total_energy: None,
                total_profit: 0.0,
                items_produced: 0,
                currency: input.currency,
                all_efficiencies,
                is_energy_self_sufficient: false,
                energy_items_produced: None,
                energy_item_name: None,
            })
            .unwrap_or_default()
        }
    }
}

/// Get the version of the optimizer.
#[wasm_bindgen]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Get the list of available items for a given facility configuration.
/// Returns JSON array of item names and their facilities.
#[wasm_bindgen]
pub fn get_available_items(input_json: &str) -> String {
    #[derive(Serialize)]
    struct ItemInfo {
        name: String,
        facility: String,
        facility_level: u32,
        sell_currency: String,
    }

    let input: Result<JsOptimizeInput, _> = serde_json::from_str(input_json);
    let facility_counts = match input {
        Ok(i) => FacilityCounts {
            farmland: (i.farmland.count, i.farmland.level),
            woodland: (i.woodland.count, i.woodland.level),
            mineral_pile: (i.mineral_pile.count, i.mineral_pile.level),
            carousel_mill: (i.carousel_mill.count, i.carousel_mill.level),
            jukebox_dryer: (i.jukebox_dryer.count, i.jukebox_dryer.level),
            crafting_table: (i.crafting_table.count, i.crafting_table.level),
            dance_pad_polisher: (i.dance_pad_polisher.count, i.dance_pad_polisher.level),
            aniipod_maker: (i.aniipod_maker.count, i.aniipod_maker.level),
            nimbus_bed: (i.nimbus_bed.count, i.nimbus_bed.level),
        },
        Err(_) => FacilityCounts {
            farmland: (1, 99),
            woodland: (1, 99),
            mineral_pile: (1, 99),
            carousel_mill: (1, 99),
            jukebox_dryer: (1, 99),
            crafting_table: (1, 99),
            dance_pad_polisher: (1, 99),
            aniipod_maker: (1, 99),
            nimbus_bed: (1, 99),
        },
    };

    let items = get_embedded_items();
    let available: Vec<ItemInfo> = items
        .iter()
        .filter(|item| facility_counts.can_produce(&item.facility, item.facility_level))
        .map(|item| ItemInfo {
            name: item.name.clone(),
            facility: item.facility.clone(),
            facility_level: item.facility_level,
            sell_currency: item.sell_currency.clone(),
        })
        .collect();

    serde_json::to_string(&available).unwrap_or_default()
}
