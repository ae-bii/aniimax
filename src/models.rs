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
///     workload: None,
///     byproduct: None,
///     environment: None,
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
    /// The currency received when selling ("coins" or "bud_tickets")
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
    /// Workload stat (new-beta Aniimo-dispatch facilities only). Informational; `production_time`
    /// already reflects the derived time at 100% efficiency; see [`WORKLOAD_RATE_ESTIMATE`].
    pub workload: Option<f64>,
    /// Secondary byproduct yielded alongside the main product: (resource_name, amount).
    /// E.g. Woodland yields Wood Blocks, Mineral Pile yields Mineral Sand. These are
    /// progression resources (Homeland/RV upgrades), not currency, so they are not folded
    /// into the profit optimizer; informational only.
    pub byproduct: Option<(String, u32)>,
    /// Growing environment this item needs to be planted (e.g. "Cool", "Warm", "Freeze",
    /// "Scorching", "Adequate"), or `None` if it has no environment requirement. Only ever
    /// set on grower items (Farmland/Woodland/Aniimo-material facilities); a processed item
    /// is made indoors and never needs one. Capacity for environment-gated items is bounded by
    /// how many plots the player's environment buildings (Heat Furnace/Cooling Unit/Sunlamp)
    /// actually cover, not just by how many plots they own; see
    /// `crate::optimizer::solve_facility_allocation`.
    pub environment: Option<String>,
}

/// Calibration constant for converting `workload` (any workload-driven, Aniimo-dispatch
/// facility) into an estimated production time at 100% Aniimo efficiency, in
/// workload-units-per-second. Applied universally across Mineral Pile, Nimbus Bed, Carousel
/// Mill, Crafting Table, and any other workload-based facility.
///
/// Derived from a single direct data point: Shell at Mineral Pile (workload 300) took 3m29s
/// (209s) at 100% efficiency, giving 300/209 ≈ 1.4354 workload/sec. A second data point, Petals
/// at Nimbus Bed, was only observed at 140% efficiency and required assuming linear scaling to
/// back out an implied 100%-rate; a weaker estimate. Identical workload "tier" values turn up
/// across unrelated facilities (e.g. Wheatmeal at Carousel Mill and Shell Ornament at Crafting
/// Table both have workload 18), suggesting workload is a shared game-wide unit rather than
/// facility-specific, so this rate is applied universally as the best available estimate. Still
/// provisional; revisit if a direct 100%-efficiency calibration point for a different facility
/// ever contradicts it.
pub const WORKLOAD_RATE_ESTIMATE: f64 = 300.0 / 209.0;

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

/// What role a facility plays within a [`ProductionPlan`]; lets the result explain itself
/// instead of just listing an item name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanStepStatus {
    /// Actively dedicated to producing `item_name`.
    Producing,
    /// Owned, but has nothing profitable to produce right now (e.g. its raw materials aren't
    /// being produced by anything, since the facility that would make them is dedicated
    /// elsewhere, or nothing is unlocked at its current level/module tier).
    NothingAvailable,
    /// Has a profitable item, but isn't needed for this plan.
    NotNeeded,
    /// Some of this facility's capacity is producing (see the sibling rows for the same
    /// `facility`), but this portion has no further profitable use and sits idle. For a grower
    /// facility this is unassigned whole-unit plots; for a processor facility it's spare
    /// dedicated-unit capacity left over after every contributor got its own whole unit (see
    /// `PlanStep::facility_count`).
    Idle,
}

/// A single row within a [`ProductionPlan`]'s facility-plan table; describes exactly one
/// facility producing exactly one item (or its idle/unused capacity, or having nothing
/// available). A facility that splits its capacity across multiple items gets multiple rows, one
/// per item, so every row is always single-product; never a joined "X + Y" description.
#[derive(Debug, Clone)]
pub struct PlanStep {
    /// Item this row describes. `None` when `status` isn't `Producing`.
    pub item_name: Option<String>,
    /// Facility producing it
    pub facility: String,
    /// Number of this facility's units dedicated to this row's item (or left idle). For a
    /// processor facility this is a whole dedicated unit whenever there's enough owned capacity
    /// for every contributor to get one (always true when it's the only contributor); dedicating
    /// a whole unit never changes the achievable rate computed above, since a unit's throughput
    /// ceiling only ever exceeds its actual share of jointly-run time. Only when more distinct
    /// items want this facility than can each get a dedicated unit does this fall back to
    /// reporting the full owned count on every contending row, with `reason` stating each item's
    /// time share instead.
    pub facility_count: u32,
    /// What role this row plays in the plan
    pub status: PlanStepStatus,
    /// Human-readable explanation of `status`, for display
    pub reason: String,
    /// Whether this facility grows/mines something (Farmland, Woodland, Mineral Pile, ...) as
    /// opposed to processing ingredients (Carousel Mill, ...); used for whole-unit plot rounding
    /// (a grower dedicates a whole plot to one crop for its whole cycle; a processor can be
    /// re-dedicated). NOT the same thing as "needs a seed": only Farmland and Woodland are
    /// actually planted; see `SeedRequirement`'s doc comment.
    pub is_grower: bool,
    /// Seconds for one full production cycle of `item_name` at this facility. `None` for
    /// Idle/NothingAvailable/NotNeeded rows, since nothing is cycling there.
    pub cycle_time: Option<f64>,
    /// The growing environment this row's item needs ("Cool"/"Warm"/"Freeze"/"Scorching"/
    /// "Adequate"), if any; lets the frontend group Farmland/Woodland rows by which environment
    /// they rely on (see `ProductionItem::environment`) instead of leaving that connection to a
    /// separate table the player has to cross-reference by hand. `None` for anything that isn't a
    /// Producing row for an environment-gated crop (processor rows, idle/unavailable rows, and
    /// ungated crops all leave this `None`).
    pub environment: Option<String>,
}

/// How many times a Farmland/Woodland plot needs to be (re-)planted with a fresh seed over the
/// whole goal duration; one seed per planting, matching the crop's existing `cost` field. Only
/// Farmland and Woodland are actually planted: Mineral Pile is mined, and the Aniimo-dispatch
/// facilities (Nimbus Bed, Grass Blossom Mat, Starfall Hammock, Tidewhisper Sandcastle, Dewy
/// House) are harvested via family dispatch; neither needs a seed, so they never appear here,
/// and nor do processors (they don't plant anything either). See
/// `crate::optimizer::time_to_reach_goal`.
#[derive(Debug, Clone)]
pub struct SeedRequirement {
    /// Facility being planted (e.g. "Farmland")
    pub facility: String,
    /// Crop being planted (e.g. "rose")
    pub item_name: String,
    /// Number of this facility's plots dedicated to this crop
    pub facility_count: u32,
    /// Plantings needed per plot over the goal duration: `ceil(total_time / cycle_time)`; the
    /// ceiling matters here (unlike the floored `total_units` elsewhere) because a seed must
    /// already be planted to make any progress on a still-in-progress final cycle, even though
    /// that cycle's output isn't counted as a completed unit yet.
    pub seeds_per_plot: u64,
    /// `facility_count * seeds_per_plot`; the number to actually go plant.
    pub total_seeds: u64,
}

/// A single income stream within a [`ProductionPlan`]; either a fully-selected item, or the
/// leftover-capacity portion of a facility that's split between feeding a recipe and selling
/// directly (section 36). Reported as "what actually got made," as opposed to [`PlanStep`] which
/// reports "what each facility does"; this is the item-level breakdown: how much of each thing,
/// its rate, and its total contribution to the plan.
///
/// Doubles as both the target-independent form (as found in `ProductionPlan.income_streams`,
/// where `total_units`/`total_value` are left at `0.0`) and the target-dependent form (as
/// returned in `GoalResult.products`, where those two fields are filled in); see
/// `crate::optimizer::time_to_reach_goal`.
#[derive(Debug, Clone)]
pub struct PlanProduct {
    /// Name of the item produced/sold
    pub item_name: String,
    /// Facility where it's produced
    pub facility: String,
    /// Sell price per unit; lets a viewer verify `total_units * sell_value` against the
    /// reported worth by hand, since `total_units` gets floored for display (you can't sell a
    /// fractional item) while `total_value` below is the unrounded net-profit figure.
    pub sell_value: f64,
    /// Currency units earned per second while this stream is active (net of ingredient costs)
    pub rate_per_second: f64,
    /// Units produced per second while active
    pub units_per_second: f64,
    /// Seconds before this stream's first output exists (0 if nothing is blocking it)
    pub lead_time: f64,
    /// Total units produced over the whole plan (0 until filled in by `time_to_reach_goal`)
    pub total_units: f64,
    /// Total currency earned from this item over the whole plan (net of ingredient costs,
    /// unrounded; the UI's gross "worth" column is computed from `sell_value *
    /// floor(total_units)` instead, so it reconciles with the whole-number amount actually
    /// shown). 0 until filled in by `time_to_reach_goal`.
    pub total_value: f64,
}

/// One facility's exact placement around a single environment building, in the same coordinate
/// space `crate::coverage`'s packing solver reasons in (building center = origin); lets the
/// frontend render the solver's actual chosen layout (a simple diagram) instead of an invented
/// illustration.
#[derive(Debug, Clone)]
pub struct FacilityPlacement {
    pub facility: String,
    pub x: f64,
    pub y: f64,
    /// Footprint side length (all environment-gated facilities are square); lets the frontend
    /// draw the right size rectangle.
    pub size: f64,
}

/// How a single environment building (Heat Furnace / Cooling Unit / Sunlamp) is configured:
/// which temperature mode it runs, and which facilities its owned units host. Unlike [`PlanStep`],
/// a building doesn't produce a sellable item; it produces *coverage* that other grower plots
/// need to be plantable at all (see `ProductionItem::environment` and
/// `crate::optimizer::solve_facility_allocation`). Coverage is computed via exact 2D geometric
/// packing (`crate::coverage`), matching the game's real continuous-area coverage mechanic rather
/// than a small set of presets.
#[derive(Debug, Clone)]
pub struct EnvironmentAssignment {
    /// "Heat Furnace" / "Cooling Unit" / "Sunlamp"
    pub building: String,
    /// Temperature mode this group of units is running: "Warm"/"Scorching" (Heat Furnace),
    /// "Cool"/"Freeze" (Cooling Unit), or "Adequate" (Sunlamp, its only mode)
    pub mode: String,
    /// Number of this building's units configured this way
    pub units: u32,
    /// Total plots of each facility type this group of units covers, e.g.
    /// `[("Farmland", 24), ("Woodland", 12)]`.
    pub covered: Vec<(String, u32)>,
    /// One entry per individual building instance (length == `units`), each holding that
    /// specific building's exact facility layout; for the frontend's per-building table and
    /// visual diagram.
    pub layouts: Vec<Vec<FacilityPlacement>>,
}

/// The provably-optimal simultaneous use of every owned facility for one target, a currency
/// (`"coins"`/`"bud_tickets"`) or a byproduct pseudo-currency (`"wood_blocks"`/`"mineral_sand"`,
/// see `crate::optimizer::byproduct_resource_name`). Target-independent: this is "what's the
/// best I can do," computed before any goal amount is known. See
/// `crate::optimizer::find_production_plan` for the algorithm, and
/// `crate::optimizer::time_to_reach_goal` for turning this plan plus a goal amount into a
/// [`GoalResult`]. Wood Blocks/Mineral Sand can also be targeted directly (not just tracked as a
/// passive `byproduct_rates` side effect), since they can become an actual chokepoint at high
/// Homeland levels; a run targeting one of them dedicates Woodland/Mineral Pile to whichever item
/// yields the most of it, and `byproduct_rates` is simply left empty for that run (it would
/// otherwise double-count the same total).
#[derive(Debug, Clone)]
pub struct ProductionPlan {
    /// The target this plan was optimized for: a currency (`"coins"`/`"bud_tickets"`) or a
    /// byproduct pseudo-currency (`"wood_blocks"`/`"mineral_sand"`)
    pub currency: String,
    /// Combined steady-state rate (currency units/sec) once every income stream's lead time has
    /// passed; the sum of `income_streams`' `rate_per_second`. The headline "your rate" number.
    pub rate_per_second: f64,
    /// One entry per item the plan actually produces, `total_units`/`total_value` left at `0.0`
    /// until `time_to_reach_goal` fills them in for a specific goal.
    pub income_streams: Vec<PlanProduct>,
    /// One entry per owned facility, each running its own best item simultaneously. `item_name`
    /// is `None` if that facility currently has nothing profitable to produce.
    pub coin_items: Vec<PlanStep>,
    /// Every grower-facility byproduct contribution (Wood Blocks/Mineral Sand, purely
    /// informational, not optimized for), kept as separate `(resource_name,
    /// rate_per_second, lead_time)` triples rather than pre-summed by resource, since different
    /// contributions to the same resource can have different lead times; summed into totals by
    /// `time_to_reach_goal` once a plan's duration is known.
    pub byproduct_rates: Vec<(String, f64, f64)>,
    /// How each owned environment building (Heat Furnace/Cooling Unit/Sunlamp) is configured:
    /// one entry per (building, mode) combination actually in use. Empty for buildings
    /// with no profitable coverage to provide, same "just doesn't appear" convention as an
    /// unused processor recipe.
    pub environment_assignments: Vec<EnvironmentAssignment>,
    /// Number of distinct candidate items `calculate_efficiencies` found profitable enough to
    /// consider at all, before any facility-allocation solving happened.
    pub candidates_evaluated: u32,
    /// Total number of facility-allocation LP/ILP solves performed while finding this plan,
    /// across every exclusion and refinement pass in `find_production_plan`; a rough measure of
    /// how much alternative-plan comparison went into settling on this one.
    pub trial_solves: u32,
}

/// Result of turning a [`ProductionPlan`] plus a goal amount into a concrete time-to-target; the
/// fastest way to reach a target currency balance given the current balance, using the plan's
/// already-computed rates (no facility-allocation re-solve).
#[derive(Debug, Clone)]
pub struct GoalResult {
    /// Total time (seconds) for the target to be met
    pub total_time: f64,
    /// Total currency produced over `total_time`
    pub amount_produced: f64,
    /// Item-level production breakdown; one entry per income stream that actually produced
    /// something before `total_time` elapsed, sorted by `total_value` descending. Replaces the
    /// coin-income-over-time chart (section 33, removed in section 37) with a table instead.
    pub products: Vec<PlanProduct>,
    /// Total Wood Blocks/Mineral Sand produced as a side effect over `total_time`; purely
    /// informational (section 38b). `(resource_name, total_amount)` pairs, omitting any resource
    /// with a zero total.
    pub byproducts: Vec<(String, f64)>,
    /// How many seeds to have ready for each grower crop actually being planted, so a player can
    /// plan ahead rather than run out mid-plan. Empty entries (zero plantings needed, e.g. the
    /// goal is already met) are omitted.
    pub seed_requirements: Vec<SeedRequirement>,
}

/// Calculated efficiency metrics for a production item.
///
/// Used to compare and rank different production options.
#[derive(Debug, Clone)]
pub struct ProductionEfficiency {
    /// The production item being evaluated
    pub item: ProductionItem,
    /// The LP objective value for one batch of `item`, given whatever target
    /// `calculate_efficiencies` was called with: net profit (sell revenue minus ingredient cost)
    /// for a currency target (`"coins"`/`"bud_tickets"`), or just the raw byproduct amount for a
    /// byproduct target (`"wood_blocks"`/`"mineral_sand"`; see
    /// `crate::optimizer::byproduct_resource_name`), since there's no currency involved there and
    /// maximizing raw output IS the goal. `solve_facility_allocation` and the rest of the modern
    /// pipeline use this directly instead of recomputing `sell_value * yield_amount - raw_cost`,
    /// so a byproduct target is a drop-in substitution rather than a second code path.
    pub batch_value: f64,
    /// `batch_value` divided by `total_time_per_unit`'s steady-state counterpart; see
    /// `batch_value`'s doc comment for what "value" means depending on the target.
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
    /// Every (facility, item) pair touched anywhere in this item's ingredient tree (including
    /// this item's own facility/name, and any intermediate processing steps, not just
    /// direct/root raw materials), paired with its accumulated utilization (batches/sec of
    /// whatever runs there, weighted by that item's own production time; see
    /// `optimizer::compute_resource_demand`) required per one batch/sec of this item.
    ///
    /// One entry per DISTINCT item hosted at a facility; a facility can appear multiple times
    /// here (once per distinct item grown/processed there for this chain), e.g. caramel_nut_chips
    /// needs walnut, chestnut, AND maple_syrup all from Woodland, three separate entries. A single
    /// combined `(facility, total_utilization)` entry per facility would collapse multiple items'
    /// utilization together and leave no way to know which specific items share that facility, so
    /// per-item entries are required; anything needing a facility-wide total sums across every
    /// entry matching that facility name.
    ///
    /// Used both to compute `effective_profit_per_second` correctly when multiple branches (or
    /// multiple different items) share one facility (e.g. soy_sauce_tofu's soy_sauce and tofu
    /// both drawing from the same Farmland soybean supply), and to find leftover capacity on any
    /// touched facility, including intermediate processors like Carousel Mill, not just direct
    /// raw-material suppliers, whose owned count exceeds what this item's true bottleneck-limited
    /// rate actually needs.
    pub facility_demand: Vec<(String, String, f64)>,
}

/// Tracks the number of each facility type available.
///
/// Multiple facilities of the same type allow for parallel production,
/// reducing overall production time.
///
/// Backed by a name → tiers map rather than fixed struct fields, so new facilities can be added
/// without touching this struct's definition or any of its call sites.
///
/// Each facility can own several TIERS; e.g. 5 plots upgraded to level 3 and 4 more upgraded
/// to level 5; since a player commonly upgrades some but not all of their plots of a given
/// facility type. A tier's units aren't walled off from lower-level recipes: an item requiring
/// level R can run on ANY tier whose level is >= R (a level-5 plot can still run a level-3
/// recipe), so [`FacilityCounts::capacity_at_level`] sums every tier meeting that bar rather
/// than just the tier matching a level exactly.
///
/// # Example
///
/// ```
/// use aniimax::models::FacilityCounts;
///
/// let mut counts = FacilityCounts::from_pairs(&[
///     ("Farmland", 4, 2),      // 4 plots at level 2
///     ("Woodland", 2, 1),      // 2 plots at level 1
///     ("Mineral Pile", 1, 1),
/// ]);
/// counts.add_tier("Farmland", 3, 4); // + 3 more plots upgraded to level 4
///
/// assert_eq!(counts.get_count("Farmland"), 7);   // 4 + 3, level-agnostic total
/// assert_eq!(counts.get_level("Farmland"), 4);    // highest tier owned
/// assert_eq!(counts.capacity_at_level("Farmland", 2), 7); // both tiers can run a level-2 recipe
/// assert_eq!(counts.capacity_at_level("Farmland", 4), 3); // only the level-4 tier can
/// // Facilities not set default to count/level 1 (matches old "unknown facility" behavior).
/// assert_eq!(counts.get_count("Carousel Mill"), 1);
/// ```
#[derive(Debug, Clone)]
pub struct FacilityCounts {
    facilities: std::collections::HashMap<String, Vec<(u32, u32)>>,
    /// (count, level) reported for any facility not explicitly `set()`/`add_tier()`'d; a single
    /// implicit tier, normally `(1, 1)`. Used by [`FacilityCounts::show_all_levels`] to report
    /// every facility as maximally unlocked.
    default_tier: (u32, u32),
}

impl Default for FacilityCounts {
    fn default() -> Self {
        Self {
            facilities: std::collections::HashMap::new(),
            default_tier: (1, 1),
        }
    }
}

impl FacilityCounts {
    /// Creates an empty `FacilityCounts` (every facility defaults to count=1, level=1).
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a `FacilityCounts` from a list of `(facility_name, count, level)` triples, one
    /// single-tier facility per entry (a repeated name overwrites the earlier one; use
    /// [`FacilityCounts::add_tier`] to accumulate multiple tiers for the same facility instead).
    pub fn from_pairs(pairs: &[(&str, u32, u32)]) -> Self {
        let mut fc = Self::new();
        for (name, count, level) in pairs {
            fc.set(name, *count, *level);
        }
        fc
    }

    /// Returns a `FacilityCounts` where every facility (known or not) reports level 99,
    /// count 1; used to list all possible items regardless of facility-level gating.
    pub fn show_all_levels() -> Self {
        Self {
            facilities: std::collections::HashMap::new(),
            default_tier: (1, 99),
        }
    }

    /// Sets a facility to a single tier, replacing any tiers set for it previously. The common
    /// case for a facility owned entirely at one level.
    pub fn set(&mut self, facility: &str, count: u32, level: u32) -> &mut Self {
        self.facilities.insert(facility.to_string(), vec![(count, level)]);
        self
    }

    /// Appends one more owned tier to a facility (e.g. "5 more plots upgraded to level 4"),
    /// keeping whatever tiers were already set rather than replacing them. Call this once per
    /// tier to build up a facility owned at multiple levels.
    pub fn add_tier(&mut self, facility: &str, count: u32, level: u32) -> &mut Self {
        self.facilities.entry(facility.to_string()).or_default().push((count, level));
        self
    }

    /// Replaces all of a facility's tiers wholesale with the given list.
    pub fn set_tiers(&mut self, facility: &str, tiers: Vec<(u32, u32)>) -> &mut Self {
        self.facilities.insert(facility.to_string(), tiers);
        self
    }

    /// Returns every owned tier `(count, level)` for a facility, or a single implicit default
    /// tier if it was never explicitly set.
    pub fn tiers(&self, facility: &str) -> Vec<(u32, u32)> {
        self.facilities.get(facility).cloned().unwrap_or_else(|| vec![self.default_tier])
    }

    /// Returns the total owned count for a given facility name, summed across every tier
    /// regardless of level; the level-agnostic "how many physical units do you own" question
    /// (used for things like environment-building coverage, which has no level of its own).
    ///
    /// # Arguments
    ///
    /// * `facility` - The name of the facility (e.g., "Farmland", "Carousel Mill")
    ///
    /// # Returns
    ///
    /// The number of that facility type available. Returns 1 for unset/unknown facility types.
    pub fn get_count(&self, facility: &str) -> u32 {
        self.tiers(facility).iter().map(|(c, _)| c).sum()
    }

    /// Returns the highest owned tier's level for a given facility name; the ceiling of what
    /// that facility type can produce at all, ignoring how much capacity exists at that ceiling
    /// (see [`FacilityCounts::capacity_at_level`] for the count actually usable by an item
    /// requiring a specific level).
    ///
    /// # Arguments
    ///
    /// * `facility` - The name of the facility (e.g., "Farmland", "Carousel Mill")
    ///
    /// # Returns
    ///
    /// The level of that facility type. Returns 1 for unset/unknown facility types.
    pub fn get_level(&self, facility: &str) -> u32 {
        self.tiers(facility).iter().map(|(_, l)| *l).max().unwrap_or(self.default_tier.1)
    }

    /// Returns how many owned units of a facility can produce an item requiring at least
    /// `required_level`; the sum of every tier whose own level meets that bar, since a
    /// higher-level plot can always run a lower-level recipe too (an upgrade never takes
    /// capability away). This is the number that actually bounds an item's achievable rate;
    /// [`FacilityCounts::get_count`] (level-agnostic total) is too generous whenever tiers are
    /// mixed, and [`FacilityCounts::get_level`] alone doesn't say how much capacity exists there.
    pub fn capacity_at_level(&self, facility: &str, required_level: u32) -> u32 {
        self.tiers(facility).iter().filter(|(_, l)| *l >= required_level).map(|(c, _)| c).sum()
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
    /// `true` if at least one owned tier's level is >= required level
    pub fn can_produce(&self, facility: &str, required_level: u32) -> bool {
        self.capacity_at_level(facility, required_level) > 0
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
#[derive(Debug, Clone, Default)]
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
    /// Growing environment required (e.g. "Cool", "Warm", "Adequate"), empty if none
    #[serde(default)]
    pub environment: Option<String>,
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
    /// Secondary Wood Blocks yield (new-beta byproduct, used for Homeland upgrades)
    #[serde(default)]
    pub byproduct_yield: Option<u32>,
    /// Energy consumed (may be "NULL" string)
    pub energy: Option<String>,
    /// Required facility level
    pub facility_level: u32,
    /// Module requirement (format: "module_name:level" or empty)
    #[serde(default)]
    pub module_requirement: Option<String>,
    /// Growing environment required (e.g. "Cool", "Warm", "Scorching"), empty if none
    #[serde(default)]
    pub environment: Option<String>,
}

/// CSV row structure for Mineral Pile items.
///
/// New-beta Mineral Pile items are workload-based (Aniimo-dispatch driven) rather than
/// flat-time, so this row carries `workload` instead of `production_time`. See
/// [`WORKLOAD_RATE_ESTIMATE`] for how workload is converted into an estimated time.
#[derive(Debug, Deserialize)]
pub struct MineralRow {
    /// Item name
    pub name: String,
    /// Currency type when selling
    pub sell_currency: String,
    /// Sell value per unit
    pub sell_value: f64,
    /// Workload stat; converted to an estimated production time via
    /// [`WORKLOAD_RATE_ESTIMATE`]
    pub workload: f64,
    /// Number of items yielded
    #[serde(rename = "yield")]
    pub yield_amount: u32,
    /// Secondary Mineral Sand yield (new-beta byproduct, used for Homeland upgrades)
    #[serde(default)]
    pub byproduct_yield: Option<u32>,
    /// Required facility level
    pub facility_level: u32,
    /// Module requirement (format: "module_name:level" or empty)
    #[serde(default)]
    pub module_requirement: Option<String>,
    /// Growing environment required (e.g. "Cool", "Freeze", "Adequate"), empty if none. Always
    /// empty for Mineral Pile itself (mining isn't weather-dependent) even though this row shape
    /// is shared with the Aniimo-material facilities that do need one.
    #[serde(default)]
    pub environment: Option<String>,
}

/// CSV row structure for processing facilities with energy tracking.
///
/// Carries either a flat `production_time` (old-style facilities not yet updated for the new
/// beta) or a `workload` (new-beta facilities; converted to time via
/// [`WORKLOAD_RATE_ESTIMATE`]). At least one of the two must be present in the CSV.
/// `sell_currency` is optional (defaults to "coins" if the column is absent); added because
/// new-beta processing facilities (e.g. Claw Game Cooker) can sell for coins or Bud Tickets.
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
    /// Currency the item sells for ("coins" or "bud_tickets"). Defaults to "coins" if absent.
    #[serde(default)]
    pub sell_currency: Option<String>,
    /// Production time in seconds (old-style flat-time facilities)
    #[serde(default)]
    pub production_time: Option<f64>,
    /// Workload stat (new-beta facilities); converted to time via [`WORKLOAD_RATE_ESTIMATE`]
    #[serde(default)]
    pub workload: Option<f64>,
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
///
/// Carries either a flat `production_time` (old-style facilities not yet updated for the new
/// beta) or a `workload` (new-beta facilities; converted to time via
/// [`WORKLOAD_RATE_ESTIMATE`]). At least one of the two must be present in the CSV.
/// `sell_currency` is optional (defaults to "coins" if the column is absent); added because
/// new-beta Crafting Table items can sell for coins or Bud Tickets depending on the recipe.
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
    /// Currency the item sells for ("coins" or "bud_tickets"). Defaults to "coins" if absent.
    #[serde(default)]
    pub sell_currency: Option<String>,
    /// Production time in seconds (old-style flat-time facilities)
    #[serde(default)]
    pub production_time: Option<f64>,
    /// Workload stat (new-beta facilities); converted to time via [`WORKLOAD_RATE_ESTIMATE`]
    #[serde(default)]
    pub workload: Option<f64>,
    /// Required facility level
    pub facility_level: u32,
    /// Module requirement (format: "module_name:level" or empty)
    #[serde(default)]
    pub module_requirement: Option<String>,
}

/// CSV row structure for Nimbus Bed items.
///
/// Nimbus Bed items are workload-based (Aniimo-dispatch driven, requiring a specific Aniimo
/// Family) rather than flat-time. See [`WORKLOAD_RATE_ESTIMATE`] for the time conversion.
#[derive(Debug, Deserialize)]
pub struct NimbusBedRow {
    /// Item name
    pub name: String,
    /// Sell value per unit
    pub sell_value: f64,
    /// Workload stat; converted to an estimated production time via
    /// [`WORKLOAD_RATE_ESTIMATE`]
    pub workload: f64,
    /// Number of items yielded
    #[serde(rename = "yield")]
    pub yield_amount: u32,
}
