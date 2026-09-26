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

/// JavaScript-friendly description of the Aniimo working a facility; see
/// [`crate::models::Worker`].
#[derive(Debug, Clone, Deserialize)]
pub struct JsWorker {
    #[serde(default = "default_level")]
    pub suitability: u32,
    #[serde(default)]
    pub personality_bonus: bool,
}

/// Builds [`crate::models::Workers`] from a facility-name -> worker map.
fn workers_from(map: &std::collections::HashMap<String, JsWorker>) -> crate::models::Workers {
    let mut workers = crate::models::Workers::new();
    for (facility, w) in map {
        workers.set(facility, crate::models::Worker::new(w.suitability, w.personality_bonus));
    }
    workers
}

/// JavaScript-friendly module levels configuration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct JsModuleLevels {
    #[serde(default)]
    pub ecological_module: u32,
    #[serde(default)]
    pub kitchen_module: u32,
    #[serde(default)]
    pub resource_detector: u32,
    #[serde(default)]
    pub crafting_module: u32,
}

/// JavaScript-friendly input for optimization.
///
/// `facilities` maps facility display name (e.g. "Farmland", "Mine"; matching the
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
    /// Aniimo working each facility, keyed by facility name; facilities left out get a level-1
    /// Aniimo without the personality bonus.
    #[serde(default)]
    pub workers: std::collections::HashMap<String, JsWorker>,
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

    // Mine items (workload-based; production_time is for a level-1 Aniimo until Workers::apply)
    let mine_data = include_str!("../data/mine.csv");
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(mine_data.as_bytes());
    for row in rdr.deserialize::<crate::models::MineralRow>().flatten() {
        items.push(ProductionItem {
            name: row.name,
            facility: "Mine".to_string(),
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
            byproduct: row
                .byproduct_yield
                .map(|amt| ("Mineral Sand".to_string(), amt)),
            environment: row.environment,
        });
    }

    // Well and the Aniimo material facilities (same CSV shape as Mine; no byproduct)
    for (facility, data) in [
        ("Well", include_str!("../data/well.csv")),
        ("Tidewhisper Sandcastle", include_str!("../data/tidewhisper_sandcastle.csv")),
        ("Dewy House", include_str!("../data/dewy_house.csv")),
        ("Nimbus Bed", include_str!("../data/nimbus_bed.csv")),
        ("Starfall Hammock", include_str!("../data/starfall_hammock.csv")),
        ("Floral Windmill", include_str!("../data/floral_windmill.csv")),
    ] {
        let mut rdr = ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(data.as_bytes());
        for row in rdr.deserialize::<crate::models::MineralRow>().flatten() {
            items.push(ProductionItem {
                name: row.name,
                facility: facility.to_string(),
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
                byproduct: None,
                environment: row.environment,
            });
        }
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
            .map(|w| crate::models::Worker::default().seconds_for(w, 1, false))
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
            .map(|w| crate::models::Worker::default().seconds_for(w, 1, false))
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
            .map(|w| crate::models::Worker::default().seconds_for(w, 1, false))
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

    // Processors sharing the no-energy CSV layout
    for (facility, data) in [
        ("Crafting Table", include_str!("../data/crafting_table.csv")),
        ("Simmering Pot", include_str!("../data/simmering_pot.csv")),
        ("Phonolfactory Table", include_str!("../data/phonolfactory_table.csv")),
        ("Bouncy Brew Keg", include_str!("../data/bouncy_brew_keg.csv")),
        ("Blazing Stove", include_str!("../data/blazing_stove.csv")),
        ("Pickling Jar", include_str!("../data/pickling_jar.csv")),
        ("Joy Wheel Loom", include_str!("../data/joy_wheel_loom.csv")),
        ("Woodworking Bench", include_str!("../data/woodworking_bench.csv")),
        ("Chimney Kiln", include_str!("../data/chimney_kiln.csv")),
        ("Dance Pad Polisher", include_str!("../data/dance_pad_polisher.csv")),
        ("Aniipod Maker", include_str!("../data/aniipod_maker.csv")),
    ] {
        let mut rdr = ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(data.as_bytes());
        for row in rdr.deserialize::<crate::models::ProcessingRowNoEnergy>().flatten() {
            let raw_mats = parse_raw_materials(&row.raw_materials);
            let req_amounts = parse_required_amounts(&row.required_amount);
            let production_time = row
                .workload
                .map(|w| crate::models::Worker::default().seconds_for(w, 1, false))
                .or(row.production_time)
                .expect("row must have either workload or production_time");
            items.push(ProductionItem {
                name: row.name,
                facility: facility.to_string(),
                raw_materials: Some(raw_mats),
                required_amount: Some(req_amounts),
                cost: None,
                sell_currency: row
                    .sell_currency
                    .clone()
                    .unwrap_or_else(|| "coins".to_string()),
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
    }

    // Only facilities verified against the full release are embedded; see `crate::data::load_all_data`.

    crate::models::add_uncovered_variants(&mut items);
    crate::models::apply_watering(&mut items);
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
        resource_detector: input.modules.resource_detector,
        crafting_module: input.modules.crafting_module,
    };

    let mut items = get_embedded_items();
    workers_from(&input.workers).apply(&embedded_aniimo_requirements(), &mut items);

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
    /// `"coins"` (matches `ProductionItem::sell_currency`), or a byproduct
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
    /// Aniimo working each facility, keyed by facility name; facilities left out get a level-1
    /// Aniimo without the personality bonus. Ignored when `aniimo` is set.
    #[serde(default)]
    pub workers: std::collections::HashMap<String, JsWorker>,
    /// `"minimum"` or `"best"`: plan for that [`crate::models::AniimoSetup`] instead of
    /// `workers`, and report which Aniimo each row needs (see [`JsPlanStep::aniimo`]).
    #[serde(default)]
    pub aniimo: Option<String>,
    /// See `crate::optimizer::find_production_plan`'s doc comment on `prioritize_byproducts`;
    /// defaults to `true` (checked by default in the UI) since Wood Blocks/Mineral Sand can be a
    /// real in-game constraint players can't just buy their way around.
    #[serde(default = "default_true")]
    pub prioritize_byproducts: bool,
    /// Set for the level-up strategy: the next RV level-up's cost and what's in stock. The exact
    /// planner then finds the soonest level-up, earning as much as it leaves room for.
    #[serde(default)]
    pub level_up: Option<crate::exact::LevelUp>,
    /// Recipes the plan may not use, for comparing against a plan someone suggests. Not on the page.
    #[serde(default)]
    pub exclude: Vec<String>,
    /// What to maximize, in order, before earning `currency` with what's left: each of "coins",
    /// "aniimo_exp", "aniipods", "Wood Blocks" and "Mineral Sand", made as much as the ones
    /// before it allow.
    #[serde(default)]
    pub priorities: Vec<String>,
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
    /// The Aniimo this row's recipe needs under the plan's Aniimo setup; only set on producing
    /// rows of Aniimo-worked facilities, and only when the plan was made for a setup.
    #[serde(default)]
    pub aniimo: Option<JsAniimo>,
    /// See `crate::models::PlanStep::busy_units`.
    #[serde(default)]
    pub busy_units: Option<f64>,
    /// The Aniimo work this row creates under the plan's Aniimo setup, one entry per ability;
    /// empty when the plan wasn't made for a setup or the row produces nothing.
    #[serde(default)]
    pub aniimo_tasks: Vec<JsAniimoTask>,
}

/// Aniimo work a plan row creates for one ability: how many Aniimo of that ability and level it
/// keeps busy on average.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsAniimoTask {
    pub ability: String,
    pub level: u32,
    pub personality_bonus: bool,
    pub busy: f64,
    /// The growing jobs behind this task, e.g. `["Sowing", "Collecting"]`. Those read as the job
    /// itself across every crop rather than a line per plot; empty for a worked facility.
    #[serde(default)]
    pub jobs: Vec<String>,
}

/// Recipes not yet checked in game, embedded in the web build; see `data/unverified.csv`.
fn embedded_unverified() -> Vec<(String, String)> {
    crate::data::parse_unverified(include_str!("../data/unverified.csv")).expect("embedded unverified.csv is valid")
}

/// A recipe a plan relies on that hasn't been checked in game yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsUnverified {
    pub facility: String,
    pub item_name: String,
}

/// The Farmland/Woodland Aniimo jobs embedded in the web build; see `data/grower_steps.csv`.
fn embedded_grower_steps() -> crate::models::GrowerSteps {
    crate::data::parse_grower_steps(include_str!("../data/grower_steps.csv"))
        .expect("embedded grower_steps.csv is valid")
        .with_watering()
}

/// The Aniimo work one producing row creates. A worked facility keeps one Aniimo busy per busy
/// unit (every unit, for gatherers like the Mine, which never wait on ingredients). A crop or tree
/// needs each of its jobs done once per harvest by whichever Aniimo has the ability; those jobs
/// accept any level, so they're counted at their minimum level under either setup.
fn aniimo_tasks_for(
    step: &crate::models::PlanStep,
    setup: crate::models::AniimoSetup,
    requirements: &crate::models::AniimoRequirements,
    grower_steps: &crate::models::GrowerSteps,
) -> Vec<JsAniimoTask> {
    use crate::models::{PlanStepStatus, Worker};
    let Some(item) = step.item_name.as_deref() else {
        return Vec::new();
    };
    if step.status != PlanStepStatus::Producing {
        return Vec::new();
    }
    if let Some((ability, _)) = requirements.get(item) {
        let worker = requirements.worker_for_at(item, &step.facility, setup);
        return vec![JsAniimoTask {
            ability: ability.to_string(),
            level: worker.suitability,
            personality_bonus: worker.personality_bonus,
            busy: step.busy_units.unwrap_or(step.facility_count as f64),
            jobs: Vec::new(),
        }];
    }
    let Some(cycle_time) = step.cycle_time.filter(|t| *t > 0.0) else {
        return Vec::new();
    };
    let harvests_per_second = step.facility_count as f64 / cycle_time;
    let mut tasks: Vec<JsAniimoTask> = Vec::new();
    for job in grower_steps.get(item) {
        let level = job.min_level;
        let busy = harvests_per_second * Worker::new(level, false).seconds_for(job.workload, level, true);
        match tasks.iter_mut().find(|t| t.ability == job.ability) {
            Some(task) => {
                task.level = task.level.max(level);
                task.busy += busy;
                if !task.jobs.contains(&job.step) {
                    task.jobs.push(job.step.clone());
                }
            }
            None => tasks.push(JsAniimoTask {
                ability: job.ability.clone(),
                level,
                personality_bonus: false,
                busy,
                jobs: vec![job.step.clone()],
            }),
        }
    }
    tasks
}

/// The Aniimo a plan row needs: its ability, ability level, and whether the plan assumes the
/// facility's personality bonus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsAniimo {
    pub ability: String,
    pub level: u32,
    pub personality_bonus: bool,
}

/// The Aniimo requirements embedded in the web build; see `data/aniimo_requirements.csv`.
fn embedded_aniimo_requirements() -> crate::models::AniimoRequirements {
    crate::data::parse_aniimo_requirements(include_str!("../data/aniimo_requirements.csv"))
        .expect("embedded aniimo_requirements.csv is valid")
}

fn aniimo_setup_from(name: &str) -> Option<crate::models::AniimoSetup> {
    match name {
        "minimum" => Some(crate::models::AniimoSetup::Minimum),
        // "best" plans for the top ability level; "best3" for a player without level-4 Aniimo.
        "best" => Some(crate::models::AniimoSetup::Best(crate::models::MAX_ANIIMO_LEVEL)),
        "best3" => Some(crate::models::AniimoSetup::Best(3)),
        _ => None,
    }
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
            aniimo: None,
            busy_units: s.busy_units,
            aniimo_tasks: Vec::new(),
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
            busy_units: s.busy_units,
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
    /// Set when this zone belongs to two overlapping buildings: the other building's name, then
    /// how many tiles along and up it sits from this one.
    #[serde(default)]
    pub partner: Option<(String, u32, u32)>,
    /// Which zone of the pair: 0 the first building's own, 1 the overlap, 2 the second's.
    #[serde(default)]
    pub zone: Option<u8>,
    /// What each of the pair is set to, e.g. `("Scorching", "Cool")`.
    #[serde(default)]
    pub pair_modes: Option<(String, String)>,
}

impl From<crate::models::EnvironmentAssignment> for JsEnvironmentAssignment {
    fn from(a: crate::models::EnvironmentAssignment) -> Self {
        JsEnvironmentAssignment {
            building: a.building,
            mode: a.mode,
            units: a.units,
            partner: a.partner,
            zone: a.zone,
            pair_modes: a.pair_modes,
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
            partner: a.partner,
            zone: a.zone,
            pair_modes: a.pair_modes,
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
    /// Set when the exact planner made this plan: whether it's proven to be the best possible.
    #[serde(default)]
    pub proven_optimal: Option<bool>,
    /// Set when the exact planner made this plan: the most any plan could earn.
    #[serde(default)]
    pub upper_bound: Option<f64>,
    /// Recipes this plan produces that haven't been checked in game yet (see
    /// `data/unverified.csv`), so the page can say what its numbers rest on.
    #[serde(default)]
    pub unverified: Vec<JsUnverified>,
    /// Set for a level-up plan: how long the level-up takes.
    #[serde(default)]
    pub level_up: Option<JsLevelUpReport>,
    /// What the plan makes of each priority it was asked for, in order.
    #[serde(default)]
    pub priorities: Vec<JsPriority>,
}

/// What a plan makes of one priority.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsPriority {
    /// "coins", "aniimo_exp", "aniipods", "Wood Blocks" or "Mineral Sand".
    pub target: String,
    pub per_second: f64,
    /// For a currency other than coins, the items that make it and how many of each per second,
    /// e.g. `[["growth_flower", 0.00093]]` behind Aniimo EXP.
    pub items: Vec<(String, f64)>,
}

/// How long a level-up plan takes to cover the level-up's cost.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsLevelUpReport {
    /// Seconds until everything the level-up costs is in stock.
    pub seconds: f64,
    /// One entry per thing the level-up costs.
    pub requirements: Vec<JsLevelUpRequirement>,
    /// Anything else left over when it's ready (unprocessed Wood Blocks or Mineral Sand, a lower
    /// tier), as `(item, amount)`.
    #[serde(default)]
    pub leftovers: Vec<(String, f64)>,
}

/// One cost of a level-up and how the plan covers it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsLevelUpRequirement {
    /// `"coins"` or an item name.
    pub name: String,
    pub need: f64,
    pub have: f64,
    /// Made per second (for coins, earned).
    pub per_second: f64,
    /// Seconds until there's enough; 0 if the stock already covers it, `None` if never.
    pub seconds: Option<f64>,
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
        proven_optimal: None,
        upper_bound: None,
        unverified: vec![],
        level_up: None,
        priorities: vec![],
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
    let prepared = match PreparedInput::from_json(input_json) {
        Ok(p) => p,
        Err(error) => return error,
    };

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
        &prepared.items,
        &prepared.input.currency,
        &prepared.facility_counts,
        &prepared.module_levels,
        prepared.input.prioritize_byproducts,
        report_ref,
    ) {
        Some(plan) => serde_json::to_string(&prepared.to_js(plan, None)).unwrap_or_default(),
        None => no_plan(),
    }
}

/// With "prioritize byproducts" on, the exact planner first finds the most of each byproduct the
/// facilities can make: one model per byproduct, `[{"resource", "lp", "variables"}]` (see
/// [`exact_problem`] for the format). The caller solves each and passes
/// `[[resource, most per second], ...]` to [`exact_problem`] and [`exact_plan`] as the floors.
/// Empty when byproducts aren't prioritized.
#[wasm_bindgen]
pub fn exact_byproduct_problems(input_json: &str) -> String {
    let Ok(prepared) = PreparedInput::from_json(input_json) else { return "[]".to_string() };
    if !prepared.input.prioritize_byproducts || prepared.input.currency != "coins" || prepared.input.level_up.is_some() {
        return "[]".to_string();
    }
    let problems: Vec<serde_json::Value> = crate::exact::byproducts(&prepared.items)
        .iter()
        .map(|resource| {
            let (lp, variables) = crate::exact::write_lp(
                &prepared.items,
                &prepared.input.currency,
                &prepared.facility_counts,
                &prepared.module_levels,
                crate::exact::Goal::MostOf(resource),
            );
            serde_json::json!({ "resource": resource, "lp": lp, "variables": variables })
        })
        .collect();
    serde_json::Value::Array(problems).to_string()
}

/// The model for the most of one priority `target` (see [`JsPlanInput::priorities`]) this
/// homeland can make while keeping every floor in `stage_json` (the priorities before it):
/// `{"lp", "variables"}` as in [`exact_problem`]. The caller solves it and adds
/// `[target, per second]` to the floors for the next priority and the final coin solve.
#[wasm_bindgen]
pub fn exact_priority_problem(input_json: &str, stage_json: &str, target: &str) -> String {
    let stage: JsStage = serde_json::from_str(stage_json).unwrap_or_default();
    let lp = match PreparedInput::from_json(input_json) {
        Ok(prepared) => crate::exact::write_lp(
            &prepared.items,
            target,
            &prepared.facility_counts,
            &prepared.module_levels,
            crate::exact::Goal::Earn { floors: &stage.floors },
        ),
        Err(_) => (String::new(), 0),
    };
    serde_json::json!({ "lp": lp.0, "variables": lp.1 }).to_string()
}

/// For the level-up strategy, the model for the soonest level-up (see
/// [`crate::exact::Goal::LevelUp`]): `{"lp", "variables"}` as in [`exact_problem`]. The caller
/// solves it and passes the pace it finds (its objective) to [`exact_problem`] and [`exact_plan`].
/// `lp` is empty for the coins strategy, or when the stock already covers the level-up.
#[wasm_bindgen]
pub fn exact_level_up_problem(input_json: &str) -> String {
    let lp = match PreparedInput::from_json(input_json) {
        Ok(prepared) => match &prepared.input.level_up {
            Some(level_up) if prepared.input.currency == "coins" && !level_up.ready() => crate::exact::write_lp(
                &prepared.items,
                &prepared.input.currency,
                &prepared.facility_counts,
                &prepared.module_levels,
                crate::exact::Goal::LevelUp(level_up),
            ),
            _ => (String::new(), 0),
        },
        Err(_) => (String::new(), 0),
    };
    serde_json::json!({ "lp": lp.0, "variables": lp.1 }).to_string()
}

/// What the earlier solves settled, for [`exact_problem`] and [`exact_plan`].
#[derive(Debug, Clone, Default, Deserialize)]
struct JsStage {
    /// `[[byproduct, per second], ...]` from [`exact_byproduct_problems`].
    #[serde(default)]
    floors: Vec<(String, f64)>,
    /// Level-ups per day from [`exact_level_up_problem`].
    #[serde(default)]
    pace: Option<f64>,
    /// Coins per second at that pace, from solving [`exact_problem`] with just the pace; then the
    /// model spends spare Bench and Kiln time on the level-up (see
    /// [`crate::exact::Goal::StockUp`]).
    #[serde(default)]
    coins: Option<f64>,
}

impl JsStage {
    fn goal<'a>(&'a self, input: &'a JsPlanInput) -> crate::exact::Goal<'a> {
        match (&input.level_up, self.pace, self.coins) {
            (Some(level_up), Some(pace), Some(coins)) => crate::exact::Goal::StockUp(level_up, pace, coins),
            (Some(level_up), Some(pace), None) => crate::exact::Goal::EarnWhileLevelingUp(level_up, pace),
            _ => crate::exact::Goal::Earn { floors: &self.floors },
        }
    }
}

/// The exact planner's model for this input (see [`crate::exact`]), for the caller to solve with
/// HiGHS and hand back to [`exact_plan`]: `{"lp": <CPLEX LP text>, "variables": <count>}`, where
/// the variables are `x0` up to `x<count - 1>`. `stage_json` is `{"floors": [[byproduct, per
/// second], ...], "pace": <level-ups per day>}` from the earlier solves (see
/// [`exact_byproduct_problems`] and [`exact_level_up_problem`]); either can be left out. `lp` is
/// empty when the exact planner doesn't cover the input (a byproduct as the currency), so the
/// caller uses [`find_plan`] instead.
#[wasm_bindgen]
pub fn exact_problem(input_json: &str, stage_json: &str) -> String {
    let stage: JsStage = serde_json::from_str(stage_json).unwrap_or_default();
    let lp = match PreparedInput::from_json(input_json) {
        Ok(prepared) if !prepared.input.currency.is_empty() => crate::exact::write_lp(
            &prepared.items,
            &prepared.input.currency,
            &prepared.facility_counts,
            &prepared.module_levels,
            stage.goal(&prepared.input),
        ),
        _ => (String::new(), 0),
    };
    serde_json::json!({ "lp": lp.0, "variables": lp.1 }).to_string()
}

/// The solver's answer to [`exact_problem`]'s model.
#[derive(Debug, Clone, Deserialize)]
struct JsSolverResult {
    /// Every variable's value, in the model's `x0`, `x1`, ... order.
    values: Vec<f64>,
    /// Whether the solver proved the answer optimal.
    proven: bool,
    /// The solver's best bound on what any plan could earn.
    bound: f64,
}

/// Turns HiGHS's solution of [`exact_problem`]'s model into the same result [`find_plan`]
/// returns, plus whether it's proven optimal.
#[wasm_bindgen]
pub fn exact_plan(input_json: &str, stage_json: &str, solution_json: &str) -> String {
    let prepared = match PreparedInput::from_json(input_json) {
        Ok(p) => p,
        Err(error) => return error,
    };
    let stage: JsStage = serde_json::from_str(stage_json).unwrap_or_default();
    let Ok(result) = serde_json::from_str::<JsSolverResult>(solution_json) else { return no_plan() };
    let currency = prepared.input.currency.clone();
    let goal = stage.goal(&prepared.input);
    let level_up = match goal {
        crate::exact::Goal::EarnWhileLevelingUp(level_up, _) | crate::exact::Goal::StockUp(level_up, ..) => Some(level_up),
        _ => None,
    };
    let Some(exact) = crate::exact::plan_from_values(
        &prepared.items,
        &currency,
        &prepared.facility_counts,
        &prepared.module_levels,
        goal,
        &result.values,
        result.proven,
        result.bound,
    ) else {
        return no_plan();
    };
    if exact.rate_per_second <= 0.0 && level_up.is_none() {
        return no_plan();
    }
    // Independent re-check of every limit before trusting the plan; the caller falls back to the
    // heuristic planner if this ever fails.
    if let Err(problem) =
        crate::exact::check_plan(&exact, &prepared.items, &currency, &prepared.facility_counts, &prepared.module_levels, level_up)
    {
        return serde_json::to_string(&empty_production_plan(false, Some(format!("Exact plan failed its check: {problem}"))))
            .unwrap_or_default();
    }
    let proof = (exact.proven_optimal, exact.upper_bound);
    let report = level_up.and_then(|level_up| level_up_report(&exact, &prepared.items, level_up, &currency));
    let plan = crate::exact::to_production_plan(&exact, &prepared.items, &currency, &prepared.facility_counts);
    let mut js = prepared.to_js(plan, Some(proof));
    js.level_up = report;
    js.priorities = prepared
        .input
        .priorities
        .iter()
        .map(|target| JsPriority {
            target: target.clone(),
            per_second: crate::exact::target_rate(&exact, &prepared.items, target),
            items: crate::exact::target_items(&exact, &prepared.items, target),
        })
        .collect();
    serde_json::to_string(&js).unwrap_or_default()
}

/// How long `exact`, a level-up plan, takes to cover each of the level-up's costs.
fn level_up_report(
    exact: &crate::exact::ExactPlan,
    items: &[ProductionItem],
    level_up: &crate::exact::LevelUp,
    currency: &str,
) -> Option<JsLevelUpReport> {
    let pace = exact.pace.filter(|p| *p > 0.0)?;
    let net = crate::exact::net_rates(exact, items);
    let requirements = level_up
        .cost
        .iter()
        .map(|(name, need)| {
            let have = level_up.stock.iter().filter(|(n, _)| n == name).fold(0.0, |sum, (_, a)| sum + a);
            let per_second =
                if name == currency { exact.rate_per_second } else { net.get(name).copied().unwrap_or(0.0).max(0.0) };
            let short = (need - have).max(0.0);
            let seconds = if short <= 0.0 {
                Some(0.0)
            } else {
                (per_second > 0.0).then(|| short / per_second)
            };
            JsLevelUpRequirement { name: name.clone(), need: *need, have, per_second, seconds }
        })
        .collect();
    let seconds = crate::exact::PACE_UNIT / pace;
    let level_up_item = |name: &str| {
        crate::models::BYPRODUCT_ITEMS.iter().any(|(_, item)| *item == name)
            || items.iter().any(|i| i.name == name && crate::exact::takes_turns(i))
    };
    let mut names: Vec<&str> = net.keys().map(String::as_str).chain(level_up.stock.iter().map(|(n, _)| n.as_str())).collect();
    names.sort_unstable();
    names.dedup();
    let leftovers = names
        .into_iter()
        .filter(|name| level_up_item(name) && !level_up.cost.iter().any(|(n, _)| n == name))
        .filter_map(|name| {
            let have = level_up.stock.iter().filter(|(n, _)| n == name).fold(0.0, |sum, (_, a)| sum + a);
            let left = have + net.get(name).copied().unwrap_or(0.0) * seconds;
            (left >= 1.0).then(|| (name.to_string(), left))
        })
        .collect();
    Some(JsLevelUpReport { seconds, requirements, leftovers })
}

fn no_plan() -> String {
    serde_json::to_string(&empty_production_plan(
        false,
        Some("Could not find a profitable production path. Try increasing facility counts.".to_string()),
    ))
    .unwrap_or_default()
}

/// A plan request parsed and set up: owned facilities, modules, and items timed for the chosen
/// Aniimo.
struct PreparedInput {
    input: JsPlanInput,
    facility_counts: FacilityCounts,
    module_levels: ModuleLevels,
    items: Vec<ProductionItem>,
    setup: Option<crate::models::AniimoSetup>,
    requirements: crate::models::AniimoRequirements,
    grower_steps: crate::models::GrowerSteps,
}

impl PreparedInput {
    /// Parses `input_json`; on failure, the error result to return instead.
    fn from_json(input_json: &str) -> Result<Self, String> {
        let input: JsPlanInput = serde_json::from_str(input_json).map_err(|e| {
            serde_json::to_string(&empty_production_plan(false, Some(format!("Invalid input: {}", e)))).unwrap_or_default()
        })?;
        let facility_counts = input.facility_counts();
        let module_levels = ModuleLevels {
            ecological_module: input.modules.ecological_module,
            kitchen_module: input.modules.kitchen_module,
            resource_detector: input.modules.resource_detector,
            crafting_module: input.modules.crafting_module,
        };
        let mut items = get_embedded_items();
        items.retain(|item| !input.exclude.iter().any(|name| name == crate::models::base_item_name(&item.name)));
        let setup = input.aniimo.as_deref().and_then(aniimo_setup_from);
        let requirements = embedded_aniimo_requirements();
        match setup {
            Some(setup) => requirements.apply(setup, &mut items),
            None => workers_from(&input.workers).apply(&requirements, &mut items),
        }
        Ok(PreparedInput { input, facility_counts, module_levels, items, setup, requirements, grower_steps: embedded_grower_steps() })
    }

    /// The result the web page shows for `plan`, with each row's Aniimo; `proof` is the exact
    /// planner's `(proven optimal, upper bound)`, if it made the plan.
    fn to_js(&self, plan: crate::models::ProductionPlan, proof: Option<(bool, f64)>) -> JsProductionPlan {
        let listed = embedded_unverified();
        let mut unverified: Vec<JsUnverified> = plan
            .coin_items
            .iter()
            .filter(|step| step.status == crate::models::PlanStepStatus::Producing)
            .filter_map(|step| {
                let item = step.item_name.as_ref()?;
                listed
                    .iter()
                    .any(|(name, facility)| name == item && *facility == step.facility)
                    .then(|| JsUnverified { facility: step.facility.clone(), item_name: item.clone() })
            })
            .collect();
        unverified.sort_by(|a, b| (&a.facility, &a.item_name).cmp(&(&b.facility, &b.item_name)));
        unverified.dedup_by(|a, b| a.facility == b.facility && a.item_name == b.item_name);
        let coin_items = plan
            .coin_items
            .into_iter()
            .map(|step| {
                let aniimo = match (self.setup, &step.item_name) {
                    (Some(setup), Some(item)) if step.status == crate::models::PlanStepStatus::Producing => {
                        self.requirements.get(item).map(|(ability, _)| {
                            let worker = self.requirements.worker_for_at(item, &step.facility, setup);
                            JsAniimo {
                                ability: ability.to_string(),
                                level: worker.suitability,
                                personality_bonus: worker.personality_bonus,
                            }
                        })
                    }
                    _ => None,
                };
                let aniimo_tasks = self
                    .setup
                    .map(|setup| aniimo_tasks_for(&step, setup, &self.requirements, &self.grower_steps))
                    .unwrap_or_default();
                JsPlanStep { aniimo, aniimo_tasks, ..step.into() }
            })
            .collect();
        JsProductionPlan {
            success: true,
            error: None,
            currency: plan.currency,
            rate_per_second: plan.rate_per_second,
            coin_items,
            income_streams: plan.income_streams.into_iter().map(Into::into).collect(),
            byproduct_rates: plan.byproduct_rates,
            environment_assignments: plan.environment_assignments.into_iter().map(Into::into).collect(),
            candidates_evaluated: plan.candidates_evaluated,
            trial_solves: plan.trial_solves,
            proven_optimal: proof.map(|(proven, _)| proven),
            upper_bound: proof.map(|(_, bound)| bound),
            unverified,
            level_up: None,
            priorities: vec![],
        }
    }
}

/// JavaScript-friendly input for turning a plan plus a goal amount into a time-to-target. `plan`
/// is exactly what [`find_plan`] returned, round-tripped by the JS caller unmodified.
#[derive(Debug, Clone, Deserialize)]
pub struct JsGoalInput {
    pub plan: JsProductionPlan,
    #[serde(default)]
    pub target: f64,
    #[serde(default)]
    pub current: f64,
    /// Instead of a coin target: what the plan makes in this many seconds (a goal for something
    /// other than coins, whose time the page works out from its rate).
    #[serde(default)]
    pub seconds: Option<f64>,
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
    let goal = match input.seconds {
        Some(seconds) => Some(crate::optimizer::production_over(&plan, seconds.max(0.0))),
        None => time_to_reach_goal(&plan, input.target, input.current),
    };
    match goal {
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
    /// Set for Aniimo-worked items, whose real time depends on the Aniimo; see
    /// [`crate::models::Worker`].
    workload: Option<f64>,
    yield_amount: u32,
    cost: Option<f64>,
    raw_materials: Option<Vec<String>>,
    required_amount: Option<Vec<u32>>,
    module_requirement: Option<(String, u32)>,
    byproduct: Option<(String, u32)>,
    /// The Aniimo ability this recipe uses and the lowest ability level that can run it; `None`
    /// for crops and trees.
    aniimo: Option<(String, u32)>,
    /// `false` if the recipe's numbers haven't been checked in game yet (see
    /// `data/unverified.csv`).
    verified: bool,
    /// For crops and trees, each Aniimo job in growing order: `(step, ability, min level)`.
    jobs: Vec<(String, String, u32)>,
}

/// Get the full recipe list for every item in the game data, grouped by nothing in particular
/// (the caller groups by facility); used by the facilities reference page.
#[wasm_bindgen]
pub fn get_all_items() -> String {
    let items = get_embedded_items();
    let requirements = embedded_aniimo_requirements();
    let unverified = embedded_unverified();
    let grower_steps = embedded_grower_steps();
    let recipes: Vec<RecipeInfo> = items
        .iter()
        .filter(|item| !item.name.ends_with(crate::models::UNCOVERED_SUFFIX))
        .map(|item| RecipeInfo {
            name: item.name.clone(),
            facility: item.facility.clone(),
            facility_level: item.facility_level,
            sell_currency: item.sell_currency.clone(),
            sell_value: item.sell_value,
            production_time: item.production_time,
            workload: item.workload,
            yield_amount: item.yield_amount,
            cost: item.cost,
            raw_materials: item.raw_materials.clone(),
            required_amount: item.required_amount.clone(),
            module_requirement: item.module_requirement.clone(),
            byproduct: item.byproduct.clone(),
            aniimo: requirements.get(&item.name).map(|(ability, level)| (ability.to_string(), level)),
            verified: !unverified.iter().any(|(name, facility)| *name == item.name && *facility == item.facility),
            jobs: grower_steps.get(&item.name).iter().map(|s| (s.step.clone(), s.ability.clone(), s.min_level)).collect(),
        })
        .collect();

    serde_json::to_string(&recipes).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{embedded_aniimo_requirements, embedded_grower_steps, get_embedded_items};

    // Every crop and tree the web build knows has its Aniimo jobs listed.
    #[test]
    fn embedded_grower_steps_cover_farmland_and_woodland() {
        let steps = embedded_grower_steps();
        for item in get_embedded_items().iter().filter(|i| i.facility == "Farmland" || i.facility == "Woodland") {
            assert!(!steps.get(&item.name).is_empty(), "{} has no embedded grower steps", item.name);
        }
    }

    // Every Aniimo-worked recipe the web build knows has an embedded requirement row.
    #[test]
    fn embedded_aniimo_requirements_cover_embedded_items() {
        let reqs = embedded_aniimo_requirements();
        for item in get_embedded_items().iter().filter(|i| i.workload.is_some()) {
            assert!(reqs.get(&item.name).is_some(), "{} has no embedded Aniimo requirement", item.name);
        }
    }

    // The web build embeds its own copy of every CSV. A facility added to `load_all_data` but not
    // here (or the reverse) would silently disappear from the web app.
    #[test]
    fn embedded_items_match_the_data_directory() {
        let data_dir = std::path::Path::new("data");
        if !data_dir.exists() {
            return;
        }
        let describe = |items: Vec<crate::models::ProductionItem>| {
            let mut rows: Vec<String> = items.iter().map(|i| format!("{i:?}")).collect();
            rows.sort();
            rows
        };
        let from_files = describe(crate::data::load_all_data(data_dir).expect("Failed to load data"));
        let embedded = describe(get_embedded_items());
        let only_files: Vec<&String> = from_files.iter().filter(|r| !embedded.contains(r)).collect();
        let only_embedded: Vec<&String> = embedded.iter().filter(|r| !from_files.contains(r)).collect();
        assert!(
            only_files.is_empty() && only_embedded.is_empty(),
            "web build and data directory disagree.
only in data/: {only_files:#?}
only embedded: {only_embedded:#?}"
        );
    }
}
