//! Data models and structures for Aniimax.
//!
//! This module contains all the core data structures used throughout the application,
//! including production items, efficiency calculations, and production paths.

use serde::Deserialize;
use std::collections::HashSet;

/// Represents a single production item that can be produced in the game.
///
/// This includes both raw materials (from Farmland, Woodland, Mineral Pile)
/// and processed items (from various processing facilities).
///
/// # Example
///
/// ```
/// use aniimax::models::ProductionItem;
///
/// let wheat = ProductionItem {
///     name: "wheat".to_string(),
///     facility: "Farmland".to_string(),
///     raw_materials: None,
///     required_amount: None,
///     cost: Some(0.0),
///     sell_currency: "coins".to_string(),
///     sell_value: 1.0,
///     production_time: 90.0,
///     yield_amount: 10,
///     energy: Some(809.0),
///     facility_level: 1,
///     module_requirement: None,
///     requires_fertilizer: false,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct ProductionItem {
    /// The name of the item (e.g., "wheat", "potato_chips")
    pub name: String,
    /// The facility where this item is produced (e.g., "Farmland", "Carousel Mill")
    pub facility: String,
    /// The raw materials required for processing (None for raw materials, can be multiple)
    pub raw_materials: Option<Vec<String>>,
    /// The amount of each raw material required per production (parallel to raw_materials)
    pub required_amount: Option<Vec<u32>>,
    /// The cost to plant/start production (for raw materials)
    pub cost: Option<f64>,
    /// The currency received when selling ("coins" or "coupons")
    pub sell_currency: String,
    /// The value received per unit when selling
    pub sell_value: f64,
    /// Time in seconds to complete one production cycle
    pub production_time: f64,
    /// Number of items yielded per production cycle
    pub yield_amount: u32,
    /// Energy gained when this item is consumed (None = cannot be consumed for energy)
    pub energy: Option<f64>,
    /// Minimum facility level required to produce this item
    pub facility_level: u32,
    /// Module requirement: (module_name, required_level) - None if no module needed
    pub module_requirement: Option<(String, u32)>,
    /// Whether this item requires fertilizer to produce
    pub requires_fertilizer: bool,
}

/// Efficiency metrics for an item when consumed for energy.
#[derive(Debug, Clone)]
pub struct EnergyItemEfficiency {
    /// The production item
    pub item: ProductionItem,
    /// Energy gained per second of production time
    pub energy_per_second: f64,
    /// Time to produce one batch
    pub time_per_batch: f64,
    /// Energy gained per batch when consumed
    pub energy_per_batch: f64,
    /// Cost (in coins) per batch
    pub cost_per_batch: f64,
}

/// Represents an optimized production path to achieve a target currency goal.
///
/// Contains the sequence of production steps, timing information,
/// and overall efficiency metrics.
#[derive(Debug, Clone)]
pub struct ProductionPath {
    /// Ordered list of production steps to execute
    pub steps: Vec<ProductionStep>,
    /// Total time required to complete all production (in seconds)
    pub total_time: f64,
    /// Startup time before steady-state production begins (max first-batch time across parallel chains)
    pub startup_time: f64,
    /// Total energy consumed (calculated as time * energy_cost_per_min / 60)
    pub total_energy: Option<f64>,
    /// Total profit generated
    pub total_profit: f64,
    /// The currency type being produced
    pub currency: String,
    /// Total number of items that will be produced for sale
    pub items_produced: u32,
    /// Whether this path is energy self-sufficient
    pub is_energy_self_sufficient: bool,
    /// Energy items produced for consumption (if self-sufficient)
    pub energy_items_produced: Option<u32>,
    /// Name of item used for energy (if self-sufficient)
    pub energy_item_name: Option<String>,
}

/// Represents a single step in a production path.
///
/// Each step describes what to produce, where, and in what quantity.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProductionStep {
    /// Name of the item to produce
    pub item_name: String,
    /// Facility where production occurs (includes count, e.g., "Farmland (x4)")
    pub facility: String,
    /// Number of production cycles to run
    pub quantity: u32,
    /// Time for this step (in seconds)
    pub time: f64,
    /// Energy consumed by this step
    pub energy: Option<f64>,
    /// Profit contribution from this step
    pub profit_contribution: f64,
    /// Chain ID for parallel production (steps with same ID run together)
    pub chain_id: Option<u32>,
    /// Optimal facility allocation: Vec<(material_name, batches_needed, facilities_to_allocate)>
    /// Shows how to split facilities when producing multiple materials to minimize time
    pub facility_allocation: Option<Vec<(String, u32, u32)>>,
}

/// Calculated efficiency metrics for a production item.
///
/// Used to compare and rank different production options.
#[derive(Debug, Clone)]
pub struct ProductionEfficiency {
    /// The production item being evaluated
    pub item: ProductionItem,
    /// Profit generated per second of production time
    pub profit_per_second: f64,
    /// Profit generated per unit of energy consumed
    pub profit_per_energy: Option<f64>,
    /// Total time to produce one unit (including raw material gathering)
    pub total_time_per_unit: f64,
    /// Total energy to produce one unit (including raw material gathering)
    pub total_energy_per_unit: Option<f64>,
    /// Name of required raw material (if any)
    pub requires_raw: Option<String>,
    /// Cost of raw materials per production
    pub raw_cost: f64,
    /// Facility that produces the raw material
    pub raw_facility: Option<String>,
    /// All facilities used in this production chain (including intermediate processing)
    pub all_facilities: HashSet<String>,
    /// Intermediate processing steps: Vec<(item_name, facility, required_amount_per_batch)>
    pub intermediate_steps: Vec<(String, String, u32)>,
    /// Time to produce the first batch (startup delay before steady-state)
    pub startup_time: f64,
    /// Effective profit per second considering parallel facility usage
    pub effective_profit_per_second: f64,
    /// Raw material details for optimal allocation: Vec<(name, amount_per_batch, time_per_batch)>
    pub raw_material_details: Option<Vec<(String, u32, f64)>>,
    /// Fertilizer batches needed per production batch (0 if no fertilizer required)
    pub fertilizer_per_batch: u32,
}

/// Tracks the number of each facility type available.
///
/// Multiple facilities of the same type allow for parallel production,
/// reducing overall production time.
///
/// # Example
///
/// ```
/// use aniimax::models::FacilityCounts;
///
/// let counts = FacilityCounts {
///     farmland: (4, 2),      // 4 plots at level 2
///     woodland: (2, 1),      // 2 plots at level 1
///     mineral_pile: (1, 1),
///     carousel_mill: (2, 2),
///     jukebox_dryer: (1, 1),
///     crafting_table: (1, 1),
///     dance_pad_polisher: (1, 1),
///     aniipod_maker: (1, 1),
///     nimbus_bed: (1, 1),
/// };
///
/// assert_eq!(counts.get_count("Farmland"), 4);
/// assert_eq!(counts.get_level("Farmland"), 2);
/// ```
#[derive(Debug, Clone)]
pub struct FacilityCounts {
    /// (count, level) for Farmland plots
    pub farmland: (u32, u32),
    /// (count, level) for Woodland plots
    pub woodland: (u32, u32),
    /// (count, level) for Mineral Pile slots
    pub mineral_pile: (u32, u32),
    /// (count, level) for Carousel Mill machines
    pub carousel_mill: (u32, u32),
    /// (count, level) for Jukebox Dryer machines
    pub jukebox_dryer: (u32, u32),
    /// (count, level) for Crafting Table slots
    pub crafting_table: (u32, u32),
    /// (count, level) for Dance Pad Polisher machines
    pub dance_pad_polisher: (u32, u32),
    /// (count, level) for Aniipod Maker machines
    pub aniipod_maker: (u32, u32),
    /// (count, level) for Nimbus Bed slots
    pub nimbus_bed: (u32, u32),
}

impl FacilityCounts {
    /// Returns the count for a given facility name.
    ///
    /// # Arguments
    ///
    /// * `facility` - The name of the facility (e.g., "Farmland", "Carousel Mill")
    ///
    /// # Returns
    ///
    /// The number of that facility type available. Returns 1 for unknown facility types.
    pub fn get_count(&self, facility: &str) -> u32 {
        match facility {
            "Farmland" => self.farmland.0,
            "Woodland" => self.woodland.0,
            "Mineral Pile" => self.mineral_pile.0,
            "Carousel Mill" => self.carousel_mill.0,
            "Jukebox Dryer" => self.jukebox_dryer.0,
            "Crafting Table" => self.crafting_table.0,
            "Dance Pad Polisher" => self.dance_pad_polisher.0,
            "Aniipod Maker" => self.aniipod_maker.0,
            "Nimbus Bed" => self.nimbus_bed.0,
            _ => 1,
        }
    }

    /// Returns the level for a given facility name.
    ///
    /// # Arguments
    ///
    /// * `facility` - The name of the facility (e.g., "Farmland", "Carousel Mill")
    ///
    /// # Returns
    ///
    /// The level of that facility type. Returns 1 for unknown facility types.
    pub fn get_level(&self, facility: &str) -> u32 {
        match facility {
            "Farmland" => self.farmland.1,
            "Woodland" => self.woodland.1,
            "Mineral Pile" => self.mineral_pile.1,
            "Carousel Mill" => self.carousel_mill.1,
            "Jukebox Dryer" => self.jukebox_dryer.1,
            "Crafting Table" => self.crafting_table.1,
            "Dance Pad Polisher" => self.dance_pad_polisher.1,
            "Aniipod Maker" => self.aniipod_maker.1,
            "Nimbus Bed" => self.nimbus_bed.1,
            _ => 1,
        }
    }

    /// Checks if a facility can produce an item at the given required level.
    ///
    /// # Arguments
    ///
    /// * `facility` - The name of the facility
    /// * `required_level` - The level required by the item
    ///
    /// # Returns
    ///
    /// `true` if the facility level is >= required level
    pub fn can_produce(&self, facility: &str, required_level: u32) -> bool {
        self.get_level(facility) >= required_level
    }
}

/// Tracks the levels of item upgrade modules.
///
/// Modules unlock upgraded versions of items with better yields or sell values.
///
/// # Example
///
/// ```
/// use aniimax::models::ModuleLevels;
///
/// let modules = ModuleLevels {
///     ecological_module: 2,  // Unlocks high-speed wheat and willow
///     kitchen_module: 2,     // Unlocks super wheat flour
///     mineral_detector: 1,   // Unlocks high-speed rock
///     crafting_module: 1,    // Unlocks advanced wood carving
/// };
///
/// assert!(modules.can_use("ecological_module", 1));
/// ```
#[derive(Debug, Clone)]
pub struct ModuleLevels {
    /// Level of Ecological Module (unlocks high-speed wheat at 1, high-speed willow at 2)
    pub ecological_module: u32,
    /// Level of Kitchen Module (unlocks super wheat flour at 2)
    pub kitchen_module: u32,
    /// Level of Mineral Detector (unlocks high-speed rock at 1)
    pub mineral_detector: u32,
    /// Level of Crafting Module (unlocks advanced wood carving at 1)
    pub crafting_module: u32,
}

impl Default for ModuleLevels {
    fn default() -> Self {
        ModuleLevels {
            ecological_module: 0,
            kitchen_module: 0,
            mineral_detector: 0,
            crafting_module: 0,
        }
    }
}

impl ModuleLevels {
    /// Checks if a module meets the required level.
    ///
    /// # Arguments
    ///
    /// * `module_name` - The name of the module
    /// * `required_level` - The level required
    ///
    /// # Returns
    ///
    /// `true` if the module level is >= required level
    pub fn can_use(&self, module_name: &str, required_level: u32) -> bool {
        self.get_level(module_name) >= required_level
    }

    /// Returns the level for a given module name.
    pub fn get_level(&self, module_name: &str) -> u32 {
        match module_name {
            "ecological_module" => self.ecological_module,
            "kitchen_module" => self.kitchen_module,
            "mineral_detector" => self.mineral_detector,
            "crafting_module" => self.crafting_module,
            _ => 0,
        }
    }
}

// ============================================================================
// CSV Row Structures
// ============================================================================

/// CSV row structure for Farmland items.
#[derive(Debug, Deserialize)]
pub struct FarmlandRow {
    /// Item name
    pub name: String,
    /// Cost to plant
    pub cost: f64,
    /// Sell value per unit
    pub sell_value: f64,
    /// Production time in seconds
    pub production_time: f64,
    /// Number of items yielded
    #[serde(rename = "yield")]
    pub yield_amount: u32,
    /// Energy consumed (optional)
    pub energy: Option<f64>,
    /// Required facility level
    pub facility_level: u32,
    /// Module requirement (format: "module_name:level" or empty)
    #[serde(default)]
    pub module_requirement: Option<String>,
}

/// CSV row structure for Woodland items.
#[derive(Debug, Deserialize)]
pub struct WoodlandRow {
    /// Item name
    pub name: String,
    /// Cost to plant
    pub cost: f64,
    /// Currency type when selling
    pub sell_currency: String,
    /// Sell value per unit
    pub sell_value: f64,
    /// Production time in seconds
    pub production_time: f64,
    /// Number of items yielded
    #[serde(rename = "yield")]
    pub yield_amount: u32,
    /// Energy consumed (may be "NULL" string)
    pub energy: Option<String>,
    /// Required facility level
    pub facility_level: u32,
    /// Module requirement (format: "module_name:level" or empty)
    #[serde(default)]
    pub module_requirement: Option<String>,
}

/// CSV row structure for Mineral Pile items.
#[derive(Debug, Deserialize)]
pub struct MineralRow {
    /// Item name
    pub name: String,
    /// Currency type when selling
    pub sell_currency: String,
    /// Sell value per unit
    pub sell_value: f64,
    /// Production time in seconds
    pub production_time: f64,
    /// Number of items yielded
    #[serde(rename = "yield")]
    pub yield_amount: u32,
    /// Required facility level
    pub facility_level: u32,
    /// Module requirement (format: "module_name:level" or empty)
    #[serde(default)]
    pub module_requirement: Option<String>,
}

/// CSV row structure for processing facilities with energy tracking.
#[derive(Debug, Deserialize)]
pub struct ProcessingRowWithEnergy {
    /// Item name
    pub name: String,
    /// Required raw material name(s), semicolon-separated if multiple
    pub raw_materials: String,
    /// Amount of raw materials needed, semicolon-separated if multiple
    pub required_amount: String,
    /// Sell value per unit
    pub sell_value: f64,
    /// Production time in seconds
    pub production_time: f64,
    /// Energy consumed (optional for items that don't consume energy)
    #[serde(default, deserialize_with = "crate::deserialize_optional_f64")]
    pub energy: Option<f64>,
    /// Required facility level
    pub facility_level: u32,
    /// Module requirement (format: "module_name:level" or empty)
    #[serde(default)]
    pub module_requirement: Option<String>,
}

/// CSV row structure for processing facilities without energy tracking.
#[derive(Debug, Deserialize)]
pub struct ProcessingRowNoEnergy {
    /// Item name
    pub name: String,
    /// Required raw material name(s), semicolon-separated if multiple
    pub raw_materials: String,
    /// Amount of raw materials needed, semicolon-separated if multiple
    pub required_amount: String,
    /// Sell value per unit
    pub sell_value: f64,
    /// Production time in seconds
    pub production_time: f64,
    /// Required facility level
    pub facility_level: u32,
    /// Module requirement (format: "module_name:level" or empty)
    #[serde(default)]
    pub module_requirement: Option<String>,
}

/// CSV row structure for Nimbus Bed items.
#[derive(Debug, Deserialize)]
pub struct NimbusBedRow {
    /// Item name
    pub name: String,
    /// Sell value per unit
    pub sell_value: f64,
    /// Production time in seconds
    pub production_time: f64,
    /// Number of items yielded
    #[serde(rename = "yield")]
    pub yield_amount: u32,
}
