//! WebAssembly bindings for Aniimax.
//!
//! This module provides JavaScript-accessible functions for the production optimizer.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::models::{FacilityCounts, ModuleLevels, ProductionEfficiency, ProductionItem};
use crate::optimizer::{
    calculate_efficiencies, calculate_energy_efficiencies, find_best_production_path,
    find_parallel_production_path, find_production_plan_with_progress, find_self_sufficient_path,
    time_to_reach_goal,
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
///
/// `facilities` maps facility display name (e.g. "Farmland", "Mineral Pile"; matching the
/// `facility` field used throughout the Rust data model) to its count/level config. Using a
/// map instead of fixed fields lets the web UI add new facilities (of which the new beta has
/// many) without changing this struct.
#[derive(Debug, Clone, Deserialize)]
pub struct JsOptimizeInput {
    pub target_amount: f64,
    pub currency: String,
    pub energy_self_sufficient: bool,
    pub energy_cost_per_min: f64,
    #[serde(default)]
    pub parallel: bool,
    #[serde(default)]
    pub facilities: std::collections::HashMap<String, JsFacilityConfig>,
    #[serde(default)]
    pub modules: JsModuleLevels,
}

impl JsOptimizeInput {
    /// Builds a [`FacilityCounts`] from the `facilities` map.
    fn facility_counts(&self) -> FacilityCounts {
        let mut fc = FacilityCounts::new();
        for (name, cfg) in &self.facilities {
            fc.set(name, cfg.count, cfg.level);
        }
        fc
    }
}

/// JavaScript-friendly production step output.
#[derive(Debug, Clone, Serialize)]
pub struct JsProductionStep {
    pub item_name: String,
    pub facility: String,
    pub quantity: u32,
    pub time_seconds: f64,
    pub energy: Option<f64>,
    pub chain_id: Option<u32>,
    /// Optimal facility allocation: Vec<(material_name, batches_needed, facilities_to_allocate)>
    pub facility_allocation: Option<Vec<(String, u32, u32)>>,
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
    for row in rdr.deserialize::<crate::models::FarmlandRow>().flatten() {
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

    // Woodland items
    let woodland_data = include_str!("../data/woodland.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(woodland_data.as_bytes());
    for row in rdr.deserialize::<crate::models::WoodlandRow>().flatten() {
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
            workload: None,
            byproduct: row
                .byproduct_yield
                .map(|amt| ("Wood Blocks".to_string(), amt)),
            environment: row.environment,
        });
    }

    // Mineral Pile items (workload-based; production_time derived via MINERAL_PILE_WORKLOAD_RATE)
    let mineral_data = include_str!("../data/mineral_pile.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(mineral_data.as_bytes());
    for row in rdr.deserialize::<crate::models::MineralRow>().flatten() {
        items.push(ProductionItem {
            name: row.name,
            facility: "Mineral Pile".to_string(),
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
                .map(|amt| ("Mineral Sand".to_string(), amt)),
            environment: row.environment,
        });
    }

    // Grass Blossom Mat items (same CSV shape as Mineral Pile; facility level/byproduct
    // unconfirmed for this facility)
    let grass_blossom_data = include_str!("../data/grass_blossom_mat.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(grass_blossom_data.as_bytes());
    for row in rdr.deserialize::<crate::models::MineralRow>().flatten() {
        items.push(ProductionItem {
            name: row.name,
            facility: "Grass Blossom Mat".to_string(),
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
                .map(|amt| ("Mineral Sand".to_string(), amt)),
            environment: row.environment,
        });
    }

    // Tidewhisper Sandcastle items (facility level guessed as 1; requires Cool/Freeze growing
    // environment, not yet modeled as a hard gate)
    let tidewhisper_data = include_str!("../data/tidewhisper_sandcastle.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(tidewhisper_data.as_bytes());
    for row in rdr.deserialize::<crate::models::MineralRow>().flatten() {
        items.push(ProductionItem {
            name: row.name,
            facility: "Tidewhisper Sandcastle".to_string(),
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
                .map(|amt| ("Mineral Sand".to_string(), amt)),
            environment: row.environment,
        });
    }

    // Starfall Hammock items (facility level guessed as 1; requires Cool growing environment,
    // not yet modeled as a hard gate)
    let starfall_data = include_str!("../data/starfall_hammock.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(starfall_data.as_bytes());
    for row in rdr.deserialize::<crate::models::MineralRow>().flatten() {
        items.push(ProductionItem {
            name: row.name,
            facility: "Starfall Hammock".to_string(),
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
                .map(|amt| ("Mineral Sand".to_string(), amt)),
            environment: row.environment,
        });
    }

    // Dewy House items (facility level guessed as 1; requires Warm growing environment, not yet
    // modeled as a hard gate)
    let dewy_data = include_str!("../data/dewy_house.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(dewy_data.as_bytes());
    for row in rdr.deserialize::<crate::models::MineralRow>().flatten() {
        items.push(ProductionItem {
            name: row.name,
            facility: "Dewy House".to_string(),
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
                .map(|amt| ("Mineral Sand".to_string(), amt)),
            environment: row.environment,
        });
    }

    // Carousel Mill items
    let carousel_data = include_str!("../data/carousel_mill.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(carousel_data.as_bytes());
    for row in rdr.deserialize::<crate::models::ProcessingRowWithEnergy>().flatten() {
        let raw_mats = parse_raw_materials(&row.raw_materials);
        let req_amounts = parse_required_amounts(&row.required_amount);
        let production_time = row
            .workload
            .map(|w| w / crate::models::WORKLOAD_RATE_ESTIMATE)
            .or(row.production_time)
            .expect("row must have either workload or production_time");
        items.push(ProductionItem {
            name: row.name,
            facility: "Carousel Mill".to_string(),
            raw_materials: Some(raw_mats),
            required_amount: Some(req_amounts),
            cost: None,
            sell_currency: row
                .sell_currency
                .clone()
                .unwrap_or_else(|| "coins".to_string()),
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

    // Jukebox Dryer items
    let jukebox_data = include_str!("../data/jukebox_dryer.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(jukebox_data.as_bytes());
    for row in rdr.deserialize::<crate::models::ProcessingRowWithEnergy>().flatten() {
        let raw_mats = parse_raw_materials(&row.raw_materials);
        let req_amounts = parse_required_amounts(&row.required_amount);
        let production_time = row
            .workload
            .map(|w| w / crate::models::WORKLOAD_RATE_ESTIMATE)
            .or(row.production_time)
            .expect("row must have either workload or production_time");
        items.push(ProductionItem {
            name: row.name,
            facility: "Jukebox Dryer".to_string(),
            raw_materials: Some(raw_mats),
            required_amount: Some(req_amounts),
            cost: None,
            sell_currency: row
                .sell_currency
                .clone()
                .unwrap_or_else(|| "coins".to_string()),
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

    // Claw Game Cooker items
    let claw_data = include_str!("../data/claw_game_cooker.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(claw_data.as_bytes());
    for row in rdr.deserialize::<crate::models::ProcessingRowWithEnergy>().flatten() {
        let raw_mats = parse_raw_materials(&row.raw_materials);
        let req_amounts = parse_required_amounts(&row.required_amount);
        let production_time = row
            .workload
            .map(|w| w / crate::models::WORKLOAD_RATE_ESTIMATE)
            .or(row.production_time)
            .expect("row must have either workload or production_time");
        items.push(ProductionItem {
            name: row.name,
            facility: "Claw Game Cooker".to_string(),
            raw_materials: Some(raw_mats),
            required_amount: Some(req_amounts),
            cost: None,
            sell_currency: row
                .sell_currency
                .clone()
                .unwrap_or_else(|| "coins".to_string()),
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

    // Crafting Table items
    let crafting_data = include_str!("../data/crafting_table.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(crafting_data.as_bytes());
    for row in rdr.deserialize::<crate::models::ProcessingRowNoEnergy>().flatten() {
        let raw_mats = parse_raw_materials(&row.raw_materials);
        let req_amounts = parse_required_amounts(&row.required_amount);
        let production_time = row
            .workload
            .map(|w| w / crate::models::WORKLOAD_RATE_ESTIMATE)
            .or(row.production_time)
            .expect("row must have either workload or production_time");
        items.push(ProductionItem {
            name: row.name,
            facility: "Crafting Table".to_string(),
            raw_materials: Some(raw_mats),
            required_amount: Some(req_amounts),
            cost: None,
            sell_currency: row
                .sell_currency
                .clone()
                .unwrap_or_else(|| "coins".to_string()),
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

    // Note: Dance Pad Polisher and Aniipod Maker are excluded: they don't produce coins/Bud
    // Tickets, so they're out of scope for this optimizer.

    // Phonolfactory Table items
    let phono_data = include_str!("../data/phonolfactory_table.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(phono_data.as_bytes());
    for row in rdr.deserialize::<crate::models::ProcessingRowNoEnergy>().flatten() {
        let raw_mats = parse_raw_materials(&row.raw_materials);
        let req_amounts = parse_required_amounts(&row.required_amount);
        let production_time = row
            .workload
            .map(|w| w / crate::models::WORKLOAD_RATE_ESTIMATE)
            .or(row.production_time)
            .expect("row must have either workload or production_time");
        items.push(ProductionItem {
            name: row.name,
            facility: "Phonolfactory Table".to_string(),
            raw_materials: Some(raw_mats),
            required_amount: Some(req_amounts),
            cost: None,
            sell_currency: row
                .sell_currency
                .clone()
                .unwrap_or_else(|| "coins".to_string()),
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

    // Bouncy Brew Keg items
    let brew_data = include_str!("../data/bouncy_brew_keg.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(brew_data.as_bytes());
    for row in rdr.deserialize::<crate::models::ProcessingRowNoEnergy>().flatten() {
        let raw_mats = parse_raw_materials(&row.raw_materials);
        let req_amounts = parse_required_amounts(&row.required_amount);
        let production_time = row
            .workload
            .map(|w| w / crate::models::WORKLOAD_RATE_ESTIMATE)
            .or(row.production_time)
            .expect("row must have either workload or production_time");
        items.push(ProductionItem {
            name: row.name,
            facility: "Bouncy Brew Keg".to_string(),
            raw_materials: Some(raw_mats),
            required_amount: Some(req_amounts),
            cost: None,
            sell_currency: row
                .sell_currency
                .clone()
                .unwrap_or_else(|| "coins".to_string()),
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

    // Joy Wheel Loom items
    let joy_wheel_data = include_str!("../data/joy_wheel_loom.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(joy_wheel_data.as_bytes());
    for row in rdr.deserialize::<crate::models::ProcessingRowNoEnergy>().flatten() {
        let raw_mats = parse_raw_materials(&row.raw_materials);
        let req_amounts = parse_required_amounts(&row.required_amount);
        let production_time = row
            .workload
            .map(|w| w / crate::models::WORKLOAD_RATE_ESTIMATE)
            .or(row.production_time)
            .expect("row must have either workload or production_time");
        items.push(ProductionItem {
            name: row.name,
            facility: "Joy Wheel Loom".to_string(),
            raw_materials: Some(raw_mats),
            required_amount: Some(req_amounts),
            cost: None,
            sell_currency: row
                .sell_currency
                .clone()
                .unwrap_or_else(|| "coins".to_string()),
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

    // Nimbus Bed items (produces Wool and Petals)
    let nimbus_data = include_str!("../data/nimbus_bed.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(nimbus_data.as_bytes());
    for row in rdr.deserialize::<crate::models::NimbusBedRow>().flatten() {
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

    let facility_counts = input.facility_counts();

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
        // Compare parallel vs single-facility approach, use whichever is faster
        let parallel_path = find_parallel_production_path(
            &efficiencies,
            input.target_amount,
            &facility_counts,
        );
        let single_path = find_best_production_path(
            &efficiencies,
            input.target_amount,
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
                    chain_id: s.chain_id,
                    facility_allocation: s.facility_allocation.clone(),
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

fn default_currency() -> String {
    "coins".to_string()
}

fn default_true() -> bool {
    true
}

/// JavaScript-friendly input for the plan solver; everything needed to know the best achievable
/// rate and facility plan, with no goal amount (see [`JsGoalInput`] for that).
#[derive(Debug, Clone, Deserialize)]
pub struct JsPlanInput {
    /// `"coins"`/`"bud_tickets"` (matches `ProductionItem::sell_currency`), or a byproduct
    /// pseudo-currency target: `"wood_blocks"`/`"mineral_sand"`; see
    /// `crate::optimizer::byproduct_resource_name`.
    #[serde(default = "default_currency")]
    pub currency: String,
    /// Maps facility name to a LIST of owned tiers, e.g. `"Farmland": [{count: 5, level: 3},
    /// {count: 4, level: 5}]` for 5 plots upgraded to level 3 and 4 more upgraded to level 5;
    /// see `crate::models::FacilityCounts`'s doc comment for why a player owning a facility type
    /// at mixed levels needs more than one `(count, level)` pair. The overwhelmingly common case
    /// of "all owned at one level" is just a one-element list.
    #[serde(default)]
    pub facilities: std::collections::HashMap<String, Vec<JsFacilityConfig>>,
    #[serde(default)]
    pub modules: JsModuleLevels,
    /// See `crate::optimizer::find_production_plan`'s doc comment on `prioritize_byproducts`;
    /// defaults to `true` (checked by default in the UI) since Wood Blocks/Mineral Sand can be a
    /// real in-game constraint players can't just buy their way around.
    #[serde(default = "default_true")]
    pub prioritize_byproducts: bool,
}

impl JsPlanInput {
    /// Builds a [`FacilityCounts`] from the `facilities` map.
    fn facility_counts(&self) -> FacilityCounts {
        let mut fc = FacilityCounts::new();
        for (name, tiers) in &self.facilities {
            fc.set_tiers(name, tiers.iter().map(|t| (t.count, t.level)).collect());
        }
        fc
    }
}

/// JavaScript-friendly single-product row within a plan's facility-plan table. A facility that
/// splits its capacity across multiple items appears as multiple rows sharing the same `facility`
/// name, one per item; see `crate::models::PlanStep`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsPlanStep {
    /// `None` unless `status` is "producing".
    pub item_name: Option<String>,
    pub facility: String,
    pub facility_count: u32,
    /// One of "producing", "nothing_available", "not_needed", "idle".
    pub status: String,
    pub reason: String,
    /// Whether this facility grows/mines something, as opposed to processing ingredients.
    pub is_grower: bool,
    /// Seconds per production cycle, when Producing; `None` otherwise.
    pub cycle_time: Option<f64>,
    /// The growing environment this row's item needs ("Cool"/"Warm"/"Freeze"/"Scorching"/
    /// "Adequate"), if any; see `crate::models::PlanStep::environment`.
    pub environment: Option<String>,
}

fn status_str(status: crate::models::PlanStepStatus) -> &'static str {
    match status {
        crate::models::PlanStepStatus::Producing => "producing",
        crate::models::PlanStepStatus::NothingAvailable => "nothing_available",
        crate::models::PlanStepStatus::NotNeeded => "not_needed",
        crate::models::PlanStepStatus::Idle => "idle",
    }
}

fn status_from_str(status: &str) -> crate::models::PlanStepStatus {
    match status {
        "producing" => crate::models::PlanStepStatus::Producing,
        "not_needed" => crate::models::PlanStepStatus::NotNeeded,
        "idle" => crate::models::PlanStepStatus::Idle,
        _ => crate::models::PlanStepStatus::NothingAvailable,
    }
}

impl From<crate::models::PlanStep> for JsPlanStep {
    fn from(s: crate::models::PlanStep) -> Self {
        JsPlanStep {
            item_name: s.item_name,
            facility: s.facility,
            facility_count: s.facility_count,
            status: status_str(s.status).to_string(),
            reason: s.reason,
            is_grower: s.is_grower,
            cycle_time: s.cycle_time,
            environment: s.environment,
        }
    }
}

impl From<JsPlanStep> for crate::models::PlanStep {
    fn from(s: JsPlanStep) -> Self {
        crate::models::PlanStep {
            item_name: s.item_name,
            facility: s.facility,
            facility_count: s.facility_count,
            status: status_from_str(&s.status),
            reason: s.reason,
            is_grower: s.is_grower,
            cycle_time: s.cycle_time,
            environment: s.environment,
        }
    }
}

/// JavaScript-friendly item-level production breakdown entry; see `crate::models::PlanProduct`.
/// Doubles as the rate-only form (in `JsProductionPlan::income_streams`, `total_units`/
/// `total_value` left at `0.0`) and the totals-filled form (in `JsGoalResult::products`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsPlanProduct {
    pub item_name: String,
    pub facility: String,
    pub sell_value: f64,
    pub rate_per_second: f64,
    pub units_per_second: f64,
    pub lead_time_seconds: f64,
    pub total_units: f64,
    pub total_value: f64,
}

impl From<crate::models::PlanProduct> for JsPlanProduct {
    fn from(p: crate::models::PlanProduct) -> Self {
        JsPlanProduct {
            item_name: p.item_name,
            facility: p.facility,
            sell_value: p.sell_value,
            rate_per_second: p.rate_per_second,
            units_per_second: p.units_per_second,
            lead_time_seconds: p.lead_time,
            total_units: p.total_units,
            total_value: p.total_value,
        }
    }
}

impl From<JsPlanProduct> for crate::models::PlanProduct {
    fn from(p: JsPlanProduct) -> Self {
        crate::models::PlanProduct {
            item_name: p.item_name,
            facility: p.facility,
            sell_value: p.sell_value,
            rate_per_second: p.rate_per_second,
            units_per_second: p.units_per_second,
            lead_time: p.lead_time_seconds,
            total_units: p.total_units,
            total_value: p.total_value,
        }
    }
}

/// JavaScript-friendly form of `crate::models::FacilityPlacement`; one facility's exact position
/// around a single environment building, for the frontend's layout diagram.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsFacilityPlacement {
    pub facility: String,
    pub x: f64,
    pub y: f64,
    pub size: f64,
}

impl From<crate::models::FacilityPlacement> for JsFacilityPlacement {
    fn from(p: crate::models::FacilityPlacement) -> Self {
        JsFacilityPlacement { facility: p.facility, x: p.x, y: p.y, size: p.size }
    }
}

impl From<JsFacilityPlacement> for crate::models::FacilityPlacement {
    fn from(p: JsFacilityPlacement) -> Self {
        crate::models::FacilityPlacement { facility: p.facility, x: p.x, y: p.y, size: p.size }
    }
}

/// One facility type's total covered plot count within a `JsEnvironmentAssignment`; a named
/// struct rather than a raw tuple for JSON friendliness, matching the rest of this module's style.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsFacilityCoverage {
    pub facility: String,
    pub count: u32,
}

/// JavaScript-friendly form of `crate::models::EnvironmentAssignment`; how an owned environment
/// building (Heat Furnace/Cooling Unit/Sunlamp) is configured.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsEnvironmentAssignment {
    pub building: String,
    pub mode: String,
    pub units: u32,
    pub covered: Vec<JsFacilityCoverage>,
    pub layouts: Vec<Vec<JsFacilityPlacement>>,
}

impl From<crate::models::EnvironmentAssignment> for JsEnvironmentAssignment {
    fn from(a: crate::models::EnvironmentAssignment) -> Self {
        JsEnvironmentAssignment {
            building: a.building,
            mode: a.mode,
            units: a.units,
            covered: a
                .covered
                .into_iter()
                .map(|(facility, count)| JsFacilityCoverage { facility, count })
                .collect(),
            layouts: a
                .layouts
                .into_iter()
                .map(|building_layout| building_layout.into_iter().map(Into::into).collect())
                .collect(),
        }
    }
}

impl From<JsEnvironmentAssignment> for crate::models::EnvironmentAssignment {
    fn from(a: JsEnvironmentAssignment) -> Self {
        crate::models::EnvironmentAssignment {
            building: a.building,
            mode: a.mode,
            units: a.units,
            covered: a.covered.into_iter().map(|c| (c.facility, c.count)).collect(),
            layouts: a
                .layouts
                .into_iter()
                .map(|building_layout| building_layout.into_iter().map(Into::into).collect())
                .collect(),
        }
    }
}

/// JavaScript-friendly, round-trippable form of `crate::models::ProductionPlan`; the JS caller
/// holds on to this after [`find_production_plan`] and passes it back unmodified as part of
/// [`JsGoalInput`] to [`time_to_reach_goal`], without ever needing to re-run the facility-
/// allocation solve just because the goal amount changed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsProductionPlan {
    pub success: bool,
    pub error: Option<String>,
    pub currency: String,
    /// Combined steady-state rate (currency units/sec); the headline "your rate" number.
    pub rate_per_second: f64,
    /// One entry per owned facility, all running simultaneously.
    pub coin_items: Vec<JsPlanStep>,
    /// One entry per item the plan produces, totals left at `0.0` until a goal is known.
    pub income_streams: Vec<JsPlanProduct>,
    /// `(resource_name, rate_per_second, lead_time_seconds)` triples; see
    /// `crate::models::ProductionPlan::byproduct_rates`.
    pub byproduct_rates: Vec<(String, f64, f64)>,
    /// How each owned environment building is configured; see
    /// `crate::models::ProductionPlan::environment_assignments`.
    pub environment_assignments: Vec<JsEnvironmentAssignment>,
    /// See `crate::models::ProductionPlan::candidates_evaluated`.
    pub candidates_evaluated: u32,
    /// See `crate::models::ProductionPlan::trial_solves`.
    pub trial_solves: u32,
}

fn empty_production_plan(success: bool, error: Option<String>) -> JsProductionPlan {
    JsProductionPlan {
        success,
        error,
        currency: default_currency(),
        rate_per_second: 0.0,
        coin_items: vec![],
        income_streams: vec![],
        byproduct_rates: vec![],
        environment_assignments: vec![],
        candidates_evaluated: 0,
        trial_solves: 0,
    }
}

impl JsProductionPlan {
    /// Reconstructs the `crate::models::ProductionPlan` this was serialized from, for feeding
    /// back into `crate::optimizer::time_to_reach_goal`.
    fn into_plan(self) -> crate::models::ProductionPlan {
        crate::models::ProductionPlan {
            currency: self.currency,
            rate_per_second: self.rate_per_second,
            income_streams: self.income_streams.into_iter().map(Into::into).collect(),
            coin_items: self.coin_items.into_iter().map(Into::into).collect(),
            byproduct_rates: self.byproduct_rates,
            environment_assignments: self.environment_assignments.into_iter().map(Into::into).collect(),
            candidates_evaluated: self.candidates_evaluated,
            trial_solves: self.trial_solves,
        }
    }
}

/// Solve for the best achievable production plan; no goal amount needed.
///
/// Takes a JSON string input ([`JsPlanInput`]) and returns a JSON string result
/// ([`JsProductionPlan`]). See [`crate::optimizer::find_production_plan`] for the algorithm.
///
/// `on_progress`, if given, is called with the solver's real, running trial-solve count after
/// every trial solve throughout the whole pipeline (see
/// [`crate::optimizer::find_production_plan_with_progress`]); this is genuine solve progress, not
/// a value simulated independently of the actual computation, so the caller (`web/worker.js`) can
/// forward it to the main thread for a real progress bar. Since this function itself already runs
/// off the main thread (called from a Web Worker; see `web/worker.js`), calling back into JS here
/// doesn't block anything else from rendering.
#[wasm_bindgen]
pub fn find_plan(input_json: &str, on_progress: Option<js_sys::Function>) -> String {
    let input: JsPlanInput = match serde_json::from_str(input_json) {
        Ok(i) => i,
        Err(e) => {
            return serde_json::to_string(&empty_production_plan(
                false,
                Some(format!("Invalid input: {}", e)),
            ))
            .unwrap_or_default();
        }
    };

    let facility_counts = input.facility_counts();
    let module_levels = ModuleLevels {
        ecological_module: input.modules.ecological_module,
        kitchen_module: input.modules.kitchen_module,
        mineral_detector: input.modules.mineral_detector,
        crafting_module: input.modules.crafting_module,
    };

    let items = get_embedded_items();

    // `js_sys::Function::call1` takes `&JsValue` for both the `this` receiver and the argument;
    // errors (e.g. the JS callback itself throwing) are deliberately swallowed with `let _ =`,
    // since a broken progress callback shouldn't be able to abort the actual calculation.
    let report: Option<Box<dyn Fn(u32)>> = on_progress.map(|f| {
        Box::new(move |count: u32| {
            let _ = f.call1(&JsValue::NULL, &JsValue::from(count));
        }) as Box<dyn Fn(u32)>
    });
    let report_ref: Option<&dyn Fn(u32)> = report.as_deref();

    match find_production_plan_with_progress(
        &items,
        &input.currency,
        &facility_counts,
        &module_levels,
        input.prioritize_byproducts,
        report_ref,
    ) {
        Some(plan) => {
            let result = JsProductionPlan {
                success: true,
                error: None,
                currency: plan.currency,
                rate_per_second: plan.rate_per_second,
                coin_items: plan.coin_items.into_iter().map(Into::into).collect(),
                income_streams: plan.income_streams.into_iter().map(Into::into).collect(),
                byproduct_rates: plan.byproduct_rates,
                environment_assignments: plan.environment_assignments.into_iter().map(Into::into).collect(),
                candidates_evaluated: plan.candidates_evaluated,
                trial_solves: plan.trial_solves,
            };
            serde_json::to_string(&result).unwrap_or_default()
        }
        None => serde_json::to_string(&empty_production_plan(
            false,
            Some(
                "Could not find a profitable production path. Try increasing facility counts."
                    .to_string(),
            ),
        ))
        .unwrap_or_default(),
    }
}

/// JavaScript-friendly input for turning a plan plus a goal amount into a time-to-target. `plan`
/// is exactly what [`find_plan`] returned, round-tripped by the JS caller unmodified.
#[derive(Debug, Clone, Deserialize)]
pub struct JsGoalInput {
    pub plan: JsProductionPlan,
    pub target: f64,
    #[serde(default)]
    pub current: f64,
}

/// JavaScript-friendly form of `crate::models::SeedRequirement`; how many times a Farmland or
/// Woodland crop needs to be replanted over the goal duration.
#[derive(Debug, Clone, Serialize)]
pub struct JsSeedRequirement {
    pub facility: String,
    pub item_name: String,
    pub facility_count: u32,
    pub seeds_per_plot: u64,
    pub total_seeds: u64,
}

impl From<crate::models::SeedRequirement> for JsSeedRequirement {
    fn from(s: crate::models::SeedRequirement) -> Self {
        JsSeedRequirement {
            facility: s.facility,
            item_name: s.item_name,
            facility_count: s.facility_count,
            seeds_per_plot: s.seeds_per_plot,
            total_seeds: s.total_seeds,
        }
    }
}

/// JavaScript-friendly output for the goal-timing calculation.
#[derive(Debug, Clone, Serialize)]
pub struct JsGoalResult {
    pub success: bool,
    pub error: Option<String>,
    pub total_time_seconds: f64,
    pub total_time_formatted: String,
    pub amount_produced: f64,
    /// Item-level production breakdown, sorted by `total_value` descending.
    pub products: Vec<JsPlanProduct>,
    /// Wood Blocks/Mineral Sand produced as a side effect; informational only. Serializes as
    /// `[[resource_name, amount], ...]`.
    pub byproducts: Vec<(String, f64)>,
    /// How many seeds to have ready for each grower crop actually being planted, sorted by
    /// total_seeds descending. Never includes processor facilities; they aren't planted.
    pub seed_requirements: Vec<JsSeedRequirement>,
}

fn empty_goal_result(success: bool, error: Option<String>) -> JsGoalResult {
    JsGoalResult {
        success,
        error,
        total_time_seconds: 0.0,
        total_time_formatted: "0s".to_string(),
        amount_produced: 0.0,
        products: vec![],
        byproducts: vec![],
        seed_requirements: vec![],
    }
}

/// Find how long a specific goal amount takes, given an already-computed plan. Cheap; no
/// facility-allocation re-solve; so this is safe to call on every keystroke of a goal input.
///
/// Takes a JSON string input ([`JsGoalInput`]) and returns a JSON string result
/// ([`JsGoalResult`]). See [`crate::optimizer::time_to_reach_goal`] for the algorithm.
#[wasm_bindgen]
pub fn time_to_reach(input_json: &str) -> String {
    let input: JsGoalInput = match serde_json::from_str(input_json) {
        Ok(i) => i,
        Err(e) => {
            return serde_json::to_string(&empty_goal_result(
                false,
                Some(format!("Invalid input: {}", e)),
            ))
            .unwrap_or_default();
        }
    };

    if !input.plan.success {
        return serde_json::to_string(&empty_goal_result(
            false,
            Some("No valid production plan to compute a goal from.".to_string()),
        ))
        .unwrap_or_default();
    }

    let plan = input.plan.into_plan();
    match time_to_reach_goal(&plan, input.target, input.current) {
        Some(goal) => {
            let result = JsGoalResult {
                success: true,
                error: None,
                total_time_seconds: goal.total_time,
                total_time_formatted: format_time(goal.total_time),
                amount_produced: goal.amount_produced,
                products: goal.products.into_iter().map(Into::into).collect(),
                byproducts: goal.byproducts,
                seed_requirements: goal.seed_requirements.into_iter().map(Into::into).collect(),
            };
            serde_json::to_string(&result).unwrap_or_default()
        }
        None => serde_json::to_string(&empty_goal_result(
            false,
            Some("This goal would take an unreasonably long time to reach.".to_string()),
        ))
        .unwrap_or_default(),
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
        Ok(i) => i.facility_counts(),
        Err(_) => FacilityCounts::show_all_levels(),
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

/// Full recipe info for one item, for the facilities reference page. Unlike
/// [`get_available_items`], this is never filtered by owned facility counts or levels; it lists
/// every recipe in the game data so the page can show what's needed to unlock each one.
#[derive(Serialize)]
struct RecipeInfo {
    name: String,
    facility: String,
    facility_level: u32,
    sell_currency: String,
    sell_value: f64,
    production_time: f64,
    yield_amount: u32,
    cost: Option<f64>,
    raw_materials: Option<Vec<String>>,
    required_amount: Option<Vec<u32>>,
    module_requirement: Option<(String, u32)>,
    byproduct: Option<(String, u32)>,
}

/// Get the full recipe list for every item in the game data, grouped by nothing in particular
/// (the caller groups by facility); used by the facilities reference page.
#[wasm_bindgen]
pub fn get_all_items() -> String {
    let items = get_embedded_items();
    let recipes: Vec<RecipeInfo> = items
        .iter()
        .map(|item| RecipeInfo {
            name: item.name.clone(),
            facility: item.facility.clone(),
            facility_level: item.facility_level,
            sell_currency: item.sell_currency.clone(),
            sell_value: item.sell_value,
            production_time: item.production_time,
            yield_amount: item.yield_amount,
            cost: item.cost,
            raw_materials: item.raw_materials.clone(),
            required_amount: item.required_amount.clone(),
            module_requirement: item.module_requirement.clone(),
            byproduct: item.byproduct.clone(),
        })
        .collect();

    serde_json::to_string(&recipes).unwrap_or_default()
}
