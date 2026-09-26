//! Exact planner: finds the provably best plan as one mixed-integer program over the whole
//! production network, instead of a continuous solve followed by rounding and heuristic passes.
//!
//! ## Model
//! - Every available recipe `r` gets a rate `b_r` (batches/sec) and a whole number of units
//!   `u_r` set to it (plots for a crop, machines for a processed item): `b_r * t_r <= u_r`, where
//!   `t_r` is the recipe's time per batch. A unit makes one thing and is left running, so units
//!   are never shared between recipes. The exception is level-up materials (see [`takes_turns`]):
//!   one Woodworking Bench has to make each tier in turn, so its recipes share its time.
//! - Every item balances: what's made covers what recipes use plus what's sold. A quick variant
//!   (e.g. `quick_wheat`) makes the same item as the regular one (`wheat`).
//! - Each facility's units per recipe add up to at most what's owned, counting only units at a
//!   high enough level for each recipe.
//! - A crop that needs a growing environment needs its plots covered: each environment building
//!   runs one mode and one coverage mix (see [`crate::coverage::single_building_options`]).
//! - Woodland and Mine byproducts (Wood Blocks, Mineral Sand) balance like any other item, so the
//!   Woodworking Bench and Chimney Kiln can use them.
//! - The objective is the target currency per second from everything sold; for coins, minus seed
//!   costs (seeds are paid in coins, so they don't come off an Aniimo EXP total). A floor can
//!   name another currency, so a plan keeps up the Aniimo EXP or Aniipods an earlier solve found
//!   while earning as many coins as that leaves room for. For an RV level-up
//!   ([`Goal::LevelUp`]) it's the pace instead: level-ups per day, where making the coins and
//!   items it costs, on top of what's already in stock, takes a day per level-up. Stock enters each
//!   balance as `pace * stock`, and the cost as `-pace * cost`, which keeps the model linear.
//!
//! ## Search
//! Branch and bound over the whole-unit variables, with each node's continuous relaxation solved
//! from scratch by `microlp` (its own integer search isn't robust enough here). Nodes are explored
//! best bound first; a node whose relaxation can't beat the best plan found so far is dropped. When
//! the search runs out of nodes the plan is proven optimal; if it hits the time limit, the best
//! open bound says how far from optimal it could be.

use std::collections::{BTreeMap, BinaryHeap, HashMap};
use std::time::{Duration, Instant};

use microlp::{ComparisonOp, OptimizationDirection, Problem};

use crate::coverage::{
    facility_footprint, mode_temperature, single_building_options, temperature_mode, CoverageOption,
    building_size, pair_options_over_offsets, Offset, PairOption, PairSizes, Zone, ENVIRONMENT_GATED_FACILITIES,
};
use crate::models::{byproduct_item, FacilityCounts, ModuleLevels, ProductionItem};

/// Whether plans may place two environment buildings to overlap, for the zone their temperatures
/// make together. Confirmed in game: a plot reached by Scorching and Cool reads Warm and grows a
/// Warm crop at 100%, and one reached by Warm and Cool reads Room temp, growing a Scorching crop
/// at 50% and a Warm one at 80%.
const OVERLAP_ZONES: bool = true;

/// Switched on only to measure how much the coverage model could be leaving on the table. It lets
/// a building or a pair take a fraction of each arrangement instead of committing to one, so the
/// plan solves an easier problem than the game and earns at least what the true best possibly
/// could. Mixing arrangements is a far tighter relaxation than allowing the most of every type in
/// every zone at once, which no layout comes near: for a Farmland and Woodland pair that box
/// holds 103 plots where 55 really fit, so it reported a gap whatever the plan did. Off in every
/// real plan; the `coverage_gap` test turns it on.
static COVERAGE_RELAXED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[doc(hidden)]
pub fn set_coverage_relaxed(on: bool) {
    COVERAGE_RELAXED.store(on, std::sync::atomic::Ordering::Relaxed);
}

fn coverage_relaxed() -> bool {
    // A wasm build has no switch to flip from the outside, so the measurement builds its own copy
    // with `ANIIMAX_RELAX_COVERAGE` set and runs it through the same solver the app uses.
    option_env!("ANIIMAX_RELAX_COVERAGE").is_some()
        || COVERAGE_RELAXED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Environment buildings and the modes each can run.
const ENVIRONMENT_BUILDINGS: &[(&str, &[&str])] = &[
    ("Heat Furnace", &["Warm", "Scorching"]),
    ("Cooling Unit", &["Cool", "Freeze"]),
    ("Sunlamp", &["Adequate"]),
];

/// Two environment buildings placed to overlap, and what their three zones cover.
#[derive(Debug, Clone)]
pub struct ExactPair {
    /// The two buildings, e.g. `("Cooling Unit", "Heat Furnace")`.
    pub buildings: (String, String),
    /// What each is set to, e.g. `("Freeze", "Warm")`.
    pub modes: (String, String),
    /// Where the second building sits relative to the first.
    pub offset: Offset,
    /// How many pairs like this the plan uses.
    pub count: u32,
    /// The mode each zone reads as: the first building's, the two added, then the second's.
    pub zone_modes: [Option<String>; 3],
    /// Plots covered per zone, as `(facility, count)`.
    pub zone_covers: [Vec<(String, u32)>; 3],
}

/// How an environment building is set up in an exact plan.
#[derive(Debug, Clone)]
pub struct ExactEnvironment {
    pub building: String,
    pub mode: String,
    /// How many of the owned buildings run this mode and coverage mix.
    pub count: u32,
    /// Facilities covered per building, e.g. `[("Farmland", 16)]`.
    pub covers: Vec<(String, u32)>,
    pub option: CoverageOption,
}

/// The result of [`solve_exact`].
#[derive(Debug, Clone)]
pub struct ExactPlan {
    /// Coins (or other target currency) per second.
    pub rate_per_second: f64,
    /// The goal's value: the same as `rate_per_second` when earning, otherwise the most of a
    /// byproduct per second or the level-up pace.
    pub objective: f64,
    /// No plan can earn more than this: the same as `rate_per_second` once proven optimal.
    pub upper_bound: f64,
    /// `true` if the search finished, so `rate_per_second` is the best possible.
    pub proven_optimal: bool,
    /// Relaxations solved during the search.
    pub nodes: u32,
    /// Batches/sec of each recipe that runs.
    pub recipe_rates: BTreeMap<String, f64>,
    /// Whole units set to each recipe that runs.
    pub units: BTreeMap<String, u32>,
    /// Units/sec sold of each item.
    pub sold: BTreeMap<String, f64>,
    pub environment: Vec<ExactEnvironment>,
    /// Overlapping pairs of environment buildings (see [`ExactPair`]).
    pub pairs: Vec<ExactPair>,
    /// Level-ups per day, for a level-up goal (see [`PACE_UNIT`]).
    pub pace: Option<f64>,
}

/// What a plan optimizes.
#[derive(Debug, Clone, Copy)]
pub enum Goal<'a> {
    /// The most currency per second while making at least each `(byproduct, per second)` floor
    /// (e.g. `("Wood Blocks", 1.2)`), for "prioritize byproducts".
    Earn { floors: &'a [(String, f64)] },
    /// The most of one byproduct per second (e.g. `"Wood Blocks"`), ignoring currency: how high
    /// that byproduct's floor can go.
    MostOf(&'a str),
    /// The soonest RV level-up: the most level-ups per day (see [`PACE_UNIT`]).
    LevelUp(&'a LevelUp),
    /// The most currency per second while keeping up at least `pace` level-ups per day: the
    /// fastest level-up, earning as much as it leaves room for.
    EarnWhileLevelingUp(&'a LevelUp, f64),
    /// With the pace and coins per second both kept, the most extra of what the level-up costs
    /// (each as a share of its cost): spare Bench and Kiln time processes whatever the level-up
    /// doesn't need yet, instead of leaving it raw.
    StockUp(&'a LevelUp, f64, f64),
}

/// What an RV level-up costs and what's already in stock, as `(item, amount)` with `"coins"` for
/// coins, e.g. cost `[("coins", 69000.0), ("rough_lumber", 290.0), ("coarse_sifted_ore", 360.0)]`.
/// Stock can hold anything, including Wood Blocks and Mineral Sand (`wood_block`, `mineral_sand`)
/// or a lower tier the Bench or Kiln can still turn into what's needed.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct LevelUp {
    pub cost: Vec<(String, f64)>,
    #[serde(default)]
    pub stock: Vec<(String, f64)>,
}

impl LevelUp {
    fn amount(list: &[(String, f64)], name: &str) -> f64 {
        list.iter().filter(|(n, _)| n == name).map(|(_, a)| a).sum()
    }

    /// Whether the stock already covers the whole cost.
    pub fn ready(&self) -> bool {
        self.cost.iter().all(|(name, amount)| Self::amount(&self.stock, name) >= *amount)
    }
}

/// Seconds per unit of pace: a pace of 1 is one level-up per day. Keeps the pace variable near 1
/// rather than near 1e-6, where a solver's absolute tolerances would swamp it.
pub const PACE_UNIT: f64 = 86_400.0;

/// The weight of coins per second in the level-up stock solve, whose goal is extra level-up items:
/// enough to sell what would otherwise be left over, nowhere near enough to trade an item for.
const STOCK_UP_COIN_WEIGHT: f64 = 1e-6;

/// The highest pace the model allows (a level-up every second), so a level-up the stock already
/// covers doesn't leave the model unbounded.
const MAX_PACE: f64 = PACE_UNIT;

/// Every byproduct any recipe makes (e.g. `"Wood Blocks"`, `"Mineral Sand"`), sorted.
pub fn byproducts(items: &[ProductionItem]) -> Vec<String> {
    let mut names: Vec<String> = items.iter().filter_map(|i| i.byproduct.as_ref().map(|(r, _)| r.clone())).collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// Whether a recipe shares its facility's time with the facility's other recipes instead of getting
/// whole units of its own: level-up materials (sold for nothing), since each tier is made from the
/// one below on the same Woodworking Bench or Chimney Kiln.
pub fn takes_turns(recipe: &ProductionItem) -> bool {
    recipe.sell_currency == "none"
}

/// The item a recipe makes: a quick variant makes the regular item, and an uncovered crop (see
/// [`crate::models::add_uncovered_variants`]) makes the same crop, just slower.
fn made_item<'a>(name: &'a str, all: &HashMap<&str, &ProductionItem>) -> &'a str {
    let name = crate::models::base_item_name(name);
    match name.strip_prefix("quick_") {
        Some(base) if all.contains_key(base) => base,
        _ => name,
    }
}

/// What a model variable stands for.
enum VarKind<'a> {
    Rate(&'a ProductionItem),
    Units(&'a ProductionItem),
    Sold(&'a str),
    Pace,
    /// Made beyond what the level-up needs, of one of its costs.
    Extra,
    Environment { building: &'a str, mode: &'a str, types: Vec<&'a str>, option: CoverageOption },
    /// Two buildings placed to overlap, covering three zones at once.
    EnvironmentPair {
        buildings: (&'a str, &'a str),
        modes: (&'a str, &'a str),
        types: Vec<&'a str>,
        option: PairOption,
    },
}

/// One linear constraint: `(variable, coefficient)` terms, comparison, right-hand side.
type Constraint = (Vec<(usize, f64)>, ComparisonOp, f64);

/// The mixed-integer program, kept independent of any one `microlp` solve so every node of the
/// search can rebuild its relaxation with its own bounds.
struct Model<'a> {
    objective: Vec<f64>,
    /// Currency per second per unit of each variable, whatever the objective is.
    earnings: Vec<f64>,
    bounds: Vec<(f64, f64)>,
    integer: Vec<bool>,
    /// Branching order for whole-unit variables, lowest first: environment buildings, then
    /// gatherers, then processors.
    priority: Vec<u8>,
    kinds: Vec<VarKind<'a>>,
    constraints: Vec<Constraint>,
}

impl<'a> Model<'a> {
    fn add(&mut self, objective: f64, bounds: (f64, f64), integer: bool, kind: VarKind<'a>) -> usize {
        self.objective.push(objective);
        self.bounds.push(bounds);
        self.integer.push(integer);
        self.priority.push(match &kind {
            VarKind::Environment { .. } | VarKind::EnvironmentPair { .. } => 0,
            VarKind::Units(recipe) if recipe.raw_materials.is_none() => 1,
            _ => 2,
        });
        self.kinds.push(kind);
        self.objective.len() - 1
    }

    fn constrain(&mut self, terms: Vec<(usize, f64)>, op: ComparisonOp, rhs: f64) {
        // One term per variable: `microlp` rejects a constraint naming the same variable twice
        // (a recipe that uses an item twice, or makes and uses the same item).
        let mut merged: BTreeMap<usize, f64> = BTreeMap::new();
        for (v, c) in terms {
            *merged.entry(v).or_default() += c;
        }
        self.constraints.push((merged.into_iter().collect(), op, rhs));
    }

    /// Solves the relaxation with `bounds`: its objective and variable values, or `None` if
    /// infeasible.
    fn relax(&self, bounds: &[(f64, f64)]) -> Option<(f64, Vec<f64>)> {
        let mut problem = Problem::new(OptimizationDirection::Maximize);
        let vars: Vec<microlp::Variable> =
            self.objective.iter().zip(bounds).map(|(&c, &b)| problem.add_var(c, b)).collect();
        for (terms, op, rhs) in &self.constraints {
            let expr: Vec<(microlp::Variable, f64)> = terms.iter().map(|&(v, c)| (vars[v], c)).collect();
            problem.add_constraint(&expr, *op, *rhs);
        }
        let solution = problem.solve().ok()?;
        Some((solution.objective(), vars.iter().map(|v| solution[*v]).collect()))
    }
}

/// Builds the model for `items` (production times already set for the Aniimo working them).
fn build_model<'a>(
    items: &'a [ProductionItem],
    currency: &str,
    facility_counts: &FacilityCounts,
    module_levels: &ModuleLevels,
    goal: Goal,
) -> Model<'a> {
    let all: HashMap<&str, &ProductionItem> = items.iter().map(|i| (i.name.as_str(), i)).collect();
    let recipes: Vec<&ProductionItem> = items
        .iter()
        .filter(|item| {
            facility_counts.get_count(&item.facility) > 0
                && facility_counts.can_produce(&item.facility, item.facility_level)
                && item.module_requirement.as_ref().is_none_or(|(m, l)| module_levels.can_use(m, *l))
                && item.production_time > 0.0
        })
        .collect();
    let mut model = Model {
        objective: Vec::new(),
        earnings: Vec::new(),
        bounds: Vec::new(),
        integer: Vec::new(),
        priority: Vec::new(),
        kinds: Vec::new(),
        constraints: Vec::new(),
    };

    // Seeds are paid in coins, so they only come off a coin total.
    let seed_cost = |recipe: &ProductionItem| if currency == "coins" { recipe.cost.unwrap_or(0.0) } else { 0.0 };

    // Recipe rates and units.
    let mut rate_of: Vec<(&ProductionItem, usize)> = Vec::new();
    let mut units_of: Vec<(&ProductionItem, usize)> = Vec::new();
    for &recipe in &recipes {
        let rate = model.add(-seed_cost(recipe), (0.0, f64::INFINITY), false, VarKind::Rate(recipe));
        let max = facility_counts.get_count(&recipe.facility) as f64;
        let units = model.add(0.0, (0.0, max), !takes_turns(recipe), VarKind::Units(recipe));
        model.constrain(vec![(rate, recipe.production_time), (units, -1.0)], ComparisonOp::Le, 0.0);
        rate_of.push((recipe, rate));
        units_of.push((recipe, units));
    }

    // Item balances: made >= used + sold.
    let mut balance: BTreeMap<&str, Vec<(usize, f64)>> = BTreeMap::new();
    for &(recipe, rate) in &rate_of {
        balance.entry(made_item(&recipe.name, &all)).or_default().push((rate, recipe.yield_amount as f64));
        if let Some((resource, amount)) = &recipe.byproduct {
            if let Some(item) = byproduct_item(resource) {
                balance.entry(item).or_default().push((rate, *amount as f64));
            }
        }
        if let (Some(inputs), Some(amounts)) = (&recipe.raw_materials, &recipe.required_amount) {
            for (input, &amount) in inputs.iter().zip(amounts) {
                balance.entry(input.as_str()).or_default().push((rate, -(amount as f64)));
            }
        }
    }
    // A floor naming a currency ("aniimo_exp", "aniipods") keeps a plan making that much of it
    // while it earns `currency`; those items need sell variables too, worth nothing here.
    let floor_currencies: Vec<&str> = match goal {
        Goal::Earn { floors } => floors.iter().map(|(name, _)| name.as_str()).collect(),
        _ => Vec::new(),
    };
    let mut sold_of: Vec<(usize, &str, f64)> = Vec::new();
    for (&item_name, terms) in &mut balance {
        if let Some(item) = all.get(item_name) {
            let sells_for = item.sell_currency.as_str();
            let wanted = sells_for == currency || floor_currencies.contains(&sells_for);
            if wanted && item.sell_value > 0.0 {
                let earns = if sells_for == currency { item.sell_value } else { 0.0 };
                let sold = model.add(earns, (0.0, f64::INFINITY), false, VarKind::Sold(item.name.as_str()));
                sold_of.push((sold, sells_for, item.sell_value));
                terms.push((sold, -1.0));
            }
        }
    }
    // Every variable added from here on earns nothing.
    model.earnings = model.objective.clone();
    // A level-up: stock and cost per unit of pace, in coins and in every item they name.
    let level_up = match goal {
        Goal::LevelUp(level_up) => Some((level_up, 0.0)),
        // Slack of 0.01% (about 9 seconds a day): `pace` is another solve's maximum, and the pace
        // terms are small enough that a tighter floor sits inside the solver's tolerances.
        Goal::EarnWhileLevelingUp(level_up, pace) | Goal::StockUp(level_up, pace, _) => Some((level_up, pace * (1.0 - 1e-4))),
        _ => None,
    };
    if let Some((level_up, min_pace)) = level_up {
        let pace = model.add(0.0, (min_pace.min(MAX_PACE), MAX_PACE), false, VarKind::Pace);
        let per_pace = |name: &str| (LevelUp::amount(&level_up.stock, name) - LevelUp::amount(&level_up.cost, name)) / PACE_UNIT;
        let coin_terms: Vec<(usize, f64)> =
            model.earnings.iter().enumerate().filter(|(_, c)| **c != 0.0).map(|(v, c)| (v, *c)).collect();
        let mut earned = coin_terms.clone();
        earned.push((pace, per_pace(currency)));
        model.constrain(earned, ComparisonOp::Ge, 0.0);
        if let Goal::StockUp(_, _, coins) = goal {
            // Slack of 0.1%: `coins` is another solve's maximum, and a tighter floor can leave no
            // whole-unit plan the independent re-check accepts (seen at RV 9 and 14).
            model.constrain(coin_terms, ComparisonOp::Ge, coins - 1e-3 * coins.abs());
        }
        for (name, _) in level_up.cost.iter().chain(&level_up.stock) {
            if name != currency {
                balance.entry(name.as_str()).or_default();
            }
        }
        for (&name, terms) in &mut balance {
            let net = per_pace(name);
            if net != 0.0 {
                terms.push((pace, net));
            }
        }
        match goal {
            Goal::LevelUp(_) => {
                model.objective.iter_mut().for_each(|c| *c = 0.0);
                model.objective[pace] = 1.0;
            }
            Goal::StockUp(..) => {
                // Coins only break ties, so anything spare (clay a Mine makes beyond what's used,
                // say) still sells instead of piling up; far too little to cost a level-up item.
                for (c, &earned) in model.objective.iter_mut().zip(&model.earnings) {
                    *c = earned * STOCK_UP_COIN_WEIGHT;
                }
                for (name, need) in &level_up.cost {
                    if let Some(terms) = balance.get_mut(name.as_str()).filter(|_| *need > 0.0) {
                        // Per day, as a share of the cost: comparable across costs.
                        let extra = model.add(PACE_UNIT / need, (0.0, f64::INFINITY), false, VarKind::Extra);
                        terms.push((extra, -1.0));
                    }
                }
            }
            _ => {}
        }
    }
    for terms in balance.into_values() {
        model.constrain(terms, ComparisonOp::Ge, 0.0);
    }

    // Owned units per facility, by level: a recipe needing level L can only use units at L or
    // above, and higher-level units can run lower-level recipes too.
    let mut by_facility: BTreeMap<&str, Vec<(u32, usize)>> = BTreeMap::new();
    for &(recipe, units) in &units_of {
        by_facility.entry(recipe.facility.as_str()).or_default().push((recipe.facility_level, units));
    }
    for (&facility, entries) in &by_facility {
        let mut levels: Vec<u32> = entries.iter().map(|(l, _)| *l).collect();
        levels.sort_unstable();
        levels.dedup();
        for level in levels {
            let terms: Vec<(usize, f64)> = entries.iter().filter(|(l, _)| *l >= level).map(|(_, v)| (*v, 1.0)).collect();
            model.constrain(terms, ComparisonOp::Le, facility_counts.capacity_at_level(facility, level) as f64);
        }
    }

    // Growing environments: plots of a crop needing environment E at facility F must be covered.
    let gated_types: Vec<&str> = ENVIRONMENT_GATED_FACILITIES.iter().map(|(f, _)| *f).collect();
    let mut needs_cover: BTreeMap<(&str, &str), Vec<usize>> = BTreeMap::new();
    for &(recipe, units) in &units_of {
        if let Some(environment) = recipe.environment.as_deref() {
            if gated_types.contains(&recipe.facility.as_str()) {
                needs_cover.entry((recipe.facility.as_str(), environment)).or_default().push(units);
            }
        }
    }
    let mut cover_terms: BTreeMap<(&str, &str), Vec<(usize, f64)>> = BTreeMap::new();
    // Every building a variable uses, so no plan sets out more than are owned.
    let mut usage: BTreeMap<&str, Vec<(usize, f64)>> = BTreeMap::new();
    // The facility types wanting a given mode, largest footprint first (see
    // `single_building_options`).
    let types_for = |modes: &[&str]| -> Vec<&str> {
        let mut types: Vec<&str> =
            needs_cover.keys().filter(|(_, e)| modes.contains(e)).map(|(f, _)| *f).collect();
        types.sort_unstable();
        types.dedup();
        types.sort_by(|a, b| {
            let (fa, fb) = (facility_footprint(a).unwrap_or(0.0), facility_footprint(b).unwrap_or(0.0));
            fb.partial_cmp(&fa).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(b))
        });
        types
    };
    for &(building, modes) in ENVIRONMENT_BUILDINGS {
        let owned = facility_counts.get_count(building);
        if owned == 0 {
            continue;
        }
        for &mode in modes {
            let types = types_for(&[mode]);
            if types.is_empty() {
                continue;
            }
            for option in single_building_options(building_size(building), &types) {
                let counts = option.counts.clone();
                let kind = VarKind::Environment { building, mode, types: types.clone(), option };
                let count = model.add(0.0, (0.0, owned as f64), !coverage_relaxed(), kind);
                usage.entry(building).or_default().push((count, 1.0));
                for (t, &facility) in types.iter().enumerate() {
                    if counts[t] > 0 {
                        cover_terms.entry((facility, mode)).or_default().push((count, counts[t] as f64));
                    }
                }
            }
        }
    }
    // Two buildings placed to overlap: where both reach, their temperatures add, giving a third
    // zone. Only worth a variable when that zone is a mode some crop actually wants and neither
    // building already provides it on its own.
    for (i, &(first, first_modes)) in ENVIRONMENT_BUILDINGS.iter().enumerate().take(if OVERLAP_ZONES { usize::MAX } else { 0 }) {
        for &(second, second_modes) in &ENVIRONMENT_BUILDINGS[i..] {
            let (owned_first, owned_second) = (facility_counts.get_count(first), facility_counts.get_count(second));
            let most = if first == second { owned_first / 2 } else { owned_first.min(owned_second) };
            if most == 0 {
                continue;
            }
            for &mode_a in first_modes {
                for &mode_b in second_modes {
                    let (Some(a), Some(b)) = (mode_temperature(mode_a), mode_temperature(mode_b)) else {
                        continue; // the Sunlamp's Adequate is off the temperature line
                    };
                    let Some(middle) = temperature_mode(a + b) else { continue };
                    // A pair earns its place only by covering three temperatures from two
                    // buildings. Set both the same and the outer zones agree, so the pair offers
                    // the same two modes two separate buildings do, over less ground; likewise
                    // when the shared zone reads as one of the buildings' own settings.
                    if mode_a == mode_b
                        || middle == mode_a
                        || middle == mode_b
                        || !needs_cover.keys().any(|(_, e)| *e == middle)
                    {
                        continue;
                    }
                    let types = types_for(&[mode_a, mode_b, middle]);
                    if types.is_empty() {
                        continue;
                    }
                    for option in pair_options_over_offsets(PairSizes::of(first, second), &types) {
                        let zones = [(Zone::First, mode_a), (Zone::Both, middle), (Zone::Second, mode_b)];
                        let counts = option.counts.clone();
                        let kind = VarKind::EnvironmentPair {
                            buildings: (first, second),
                            modes: (mode_a, mode_b),
                            types: types.clone(),
                            option,
                        };
                        let count = model.add(0.0, (0.0, most as f64), !coverage_relaxed(), kind);
                        usage.entry(first).or_default().push((count, 1.0));
                        usage.entry(second).or_default().push((count, 1.0));
                        for (zone, mode) in zones {
                            for (t, &facility) in types.iter().enumerate() {
                                let covered = counts[zone as usize][t];
                                if covered > 0 {
                                    cover_terms.entry((facility, mode)).or_default().push((count, covered as f64));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    for (building, terms) in usage {
        model.constrain(terms, ComparisonOp::Le, facility_counts.get_count(building) as f64);
    }
    for (key, plots) in &needs_cover {
        let mut terms: Vec<(usize, f64)> = plots.iter().map(|v| (*v, 1.0)).collect();
        for &(count, covered) in cover_terms.get(key).map(Vec::as_slice).unwrap_or(&[]) {
            terms.push((count, -covered));
        }
        model.constrain(terms, ComparisonOp::Le, 0.0);
    }
    // Byproducts: made per batch of any recipe that yields one.
    let byproduct_terms = |resource: &str| -> Vec<(usize, f64)> {
        rate_of
            .iter()
            .filter_map(|&(recipe, rate)| match &recipe.byproduct {
                Some((r, amount)) if r == resource => Some((rate, *amount as f64)),
                _ => None,
            })
            .collect()
    };
    match goal {
        Goal::Earn { floors } => {
            for (resource, floor) in floors {
                if *floor <= 0.0 {
                    continue;
                }
                let byproduct = byproduct_terms(resource);
                if !byproduct.is_empty() {
                    // A hair of slack: the floor is another solve's exact maximum.
                    model.constrain(byproduct, ComparisonOp::Ge, floor * (1.0 - 1e-6));
                    continue;
                }
                // Otherwise it's a currency: everything sold for it, at its value, less seed costs
                // for coins (seeds are paid in coins).
                let mut sold: Vec<(usize, f64)> =
                    sold_of.iter().filter(|(_, c, _)| c == resource).map(|&(v, _, value)| (v, value)).collect();
                if resource == "coins" {
                    sold.extend(rate_of.iter().filter(|(r, _)| r.cost.unwrap_or(0.0) > 0.0).map(|&(r, v)| (v, -r.cost.unwrap_or(0.0))));
                }
                if !sold.is_empty() {
                    model.constrain(sold, ComparisonOp::Ge, floor * (1.0 - 1e-4));
                }
            }
            // Maximizing a byproduct (Wood Blocks, Mineral Sand) rather than a currency.
            let byproduct = byproduct_terms(currency);
            if !byproduct.is_empty() {
                model.objective.iter_mut().for_each(|c| *c = 0.0);
                for (v, amount) in byproduct {
                    model.objective[v] += amount;
                }
            }
        }
        Goal::MostOf(resource) => {
            model.objective.iter_mut().for_each(|c| *c = 0.0);
            for (v, amount) in byproduct_terms(resource) {
                model.objective[v] += amount;
            }
        }
        Goal::LevelUp(_) | Goal::EarnWhileLevelingUp(..) | Goal::StockUp(..) => {}
    }
    model
}

const INTEGRAL: f64 = 1e-6;

fn is_integral(value: f64) -> bool {
    (value - value.round()).abs() <= INTEGRAL
}

/// An open node of the search: its relaxation's bound and the bounds that define it.
struct Node {
    bound: f64,
    depth: u32,
    bounds: Vec<(f64, f64)>,
    values: Vec<f64>,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.bound == other.bound && self.depth == other.depth
    }
}
impl Eq for Node {}
impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Node {
    /// Highest bound first; deeper first among equal bounds, to reach whole-unit plans sooner.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.bound.partial_cmp(&other.bound).unwrap_or(std::cmp::Ordering::Equal).then(self.depth.cmp(&other.depth))
    }
}

/// Finds the best plan for `items` (production times already set for the Aniimo working them)
/// with the owned `facility_counts` and `module_levels`, earning `currency`. `time_limit` bounds
/// the search; `None` runs until optimality is proven.
pub fn solve_exact(
    items: &[ProductionItem],
    currency: &str,
    facility_counts: &FacilityCounts,
    module_levels: &ModuleLevels,
    goal: Goal,
    time_limit: Option<Duration>,
    start: Option<&crate::models::ProductionPlan>,
) -> Option<ExactPlan> {
    let model = build_model(items, currency, facility_counts, module_levels, goal);
    let began = Instant::now();
    let out_of_time = || time_limit.is_some_and(|limit| began.elapsed() >= limit);
    let mut nodes = 0u32;

    let (root_bound, root_values) = model.relax(&model.bounds)?;
    nodes += 1;
    if std::env::var("EXACT_DEBUG").is_ok() {
        for (i, kind) in model.kinds.iter().enumerate() {
            let v = root_values[i];
            if model.integer[i] && !is_integral(v) {
                let label = match kind {
                    VarKind::Units(r) => format!("units {} ({})", r.name, r.facility),
                    VarKind::Environment { building, mode, option, .. } => format!("env {building} {mode} {:?}", option.counts),
                    VarKind::EnvironmentPair { buildings, modes, option, .. } => {
                        format!("env pair {}+{} {}+{} at {}", buildings.0, buildings.1, modes.0, modes.1, option.offset)
                    }
                    _ => String::new(),
                };
                eprintln!("  fractional {v:.3} {label}");
            }
        }
    }
    let mut best: Option<(f64, Vec<f64>)> = None;
    // A plan only counts as better if it beats the best by more than rounding noise.
    let beats = |value: f64, best: &Option<(f64, Vec<f64>)>| {
        best.as_ref().is_none_or(|(b, _)| value > b + 1e-9 * b.abs().max(1.0))
    };

    // Rounds every whole-unit variable down (always still feasible: fewer units and less
    // coverage never break a limit) and re-solves the rates.
    let round_down = |values: &[f64], bounds: &[(f64, f64)]| -> Option<(f64, Vec<f64>)> {
        let fixed: Vec<(f64, f64)> = values
            .iter()
            .zip(bounds)
            .zip(&model.integer)
            .map(|((&v, &b), &int)| if int { let f = (v + INTEGRAL).floor().max(b.0).min(b.1); (f, f) } else { b })
            .collect();
        model.relax(&fixed)
    };

    // Dives from the root: repeatedly fixes the whole-unit variable closest to a whole number and
    // re-solves, which usually lands on a good plan quickly.
    let dive = |mut values: Vec<f64>, mut bounds: Vec<(f64, f64)>, nodes: &mut u32| -> Option<(f64, Vec<f64>)> {
        loop {
            let pick = (0..values.len())
                .filter(|&i| model.integer[i] && !is_integral(values[i]))
                .min_by(|&a, &b| {
                    let da = (values[a] - values[a].round()).abs();
                    let db = (values[b] - values[b].round()).abs();
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                });
            let Some(i) = pick else {
                let objective: f64 = values.iter().zip(&model.objective).map(|(v, c)| v * c).sum();
                return Some((objective, values));
            };
            let target = values[i].round().max(bounds[i].0).min(bounds[i].1);
            bounds[i] = (target, target);
            *nodes += 1;
            match model.relax(&bounds) {
                Some((_, v)) => values = v,
                None => return None,
            }
        }
    };

    let from_start = start.and_then(|plan| {
        nodes += 1;
        model.relax(&start_bounds(&model, plan)?)
    });
    for candidate in [from_start, round_down(&root_values, &model.bounds), dive(root_values.clone(), model.bounds.clone(), &mut nodes)]
        .into_iter()
        .flatten()
    {
        if beats(candidate.0, &best) {
            best = Some(candidate);
        }
    }

    let mut open: BinaryHeap<Node> = BinaryHeap::new();
    open.push(Node { bound: root_bound, depth: 0, bounds: model.bounds.clone(), values: root_values });
    let mut proven = true;
    while let Some(node) = open.pop() {
        if !beats(node.bound, &best) {
            // Best bound first: nothing left can beat the best plan.
            open.clear();
            break;
        }
        if out_of_time() {
            open.push(node);
            proven = false;
            break;
        }
        // Branch on the most fractional whole-unit variable of the most structural kind.
        let pick = (0..node.values.len()).filter(|&i| model.integer[i] && !is_integral(node.values[i])).min_by(|&a, &b| {
            let fa = (node.values[a] - node.values[a].floor() - 0.5).abs();
            let fb = (node.values[b] - node.values[b].floor() - 0.5).abs();
            model.priority[a].cmp(&model.priority[b]).then(fa.partial_cmp(&fb).unwrap_or(std::cmp::Ordering::Equal))
        });
        let Some(i) = pick else {
            // Whole units throughout: a plan.
            if beats(node.bound, &best) {
                best = Some((node.bound, node.values));
            }
            continue;
        };
        let value = node.values[i];
        for (lo, hi) in [(value.ceil(), node.bounds[i].1), (node.bounds[i].0, value.floor())] {
            if lo > hi {
                continue;
            }
            let mut bounds = node.bounds.clone();
            bounds[i] = (lo, hi);
            nodes += 1;
            let Some((bound, values)) = model.relax(&bounds) else { continue };
            if !beats(bound, &best) {
                continue;
            }
            // Cheap plan from every child: its rounded-down version.
            if let Some(candidate) = round_down(&values, &bounds) {
                if beats(candidate.0, &best) {
                    best = Some(candidate);
                }
            }
            open.push(Node { bound, depth: node.depth + 1, bounds, values });
        }
    }

    let (value, values) = best?;
    let upper_bound = if proven { value } else { open.iter().map(|n| n.bound).fold(value, f64::max) };
    Some(plan_from(&model, value, upper_bound.max(value), proven && open.is_empty(), nodes, &values, root_bound))
}

/// Bounds that fix every whole-unit variable to what `plan` (from the heuristic planner) uses, so
/// solving them gives that plan's units with the best rates for them: a strong first plan to beat.
/// `None` if the plan uses a recipe or coverage the model doesn't have.
fn start_bounds(model: &Model, plan: &crate::models::ProductionPlan) -> Option<Vec<(f64, f64)>> {
    let mut units: HashMap<&str, u32> = HashMap::new();
    for step in &plan.coin_items {
        if let (crate::models::PlanStepStatus::Producing, Some(item)) = (&step.status, &step.item_name) {
            *units.entry(item.as_str()).or_default() += step.facility_count;
        }
    }
    let mut bounds = model.bounds.clone();
    let mut used: HashMap<&str, u32> = HashMap::new();
    for (i, kind) in model.kinds.iter().enumerate() {
        if let VarKind::Units(recipe) = kind {
            let count = units.get(recipe.name.as_str()).copied().unwrap_or(0);
            used.insert(recipe.name.as_str(), count);
            bounds[i] = (count as f64, count as f64);
        } else if let VarKind::Environment { .. } | VarKind::EnvironmentPair { .. } = kind {
            bounds[i] = (0.0, 0.0);
        }
    }
    if units.keys().any(|name| !used.contains_key(name)) {
        return None;
    }
    // Each group of environment buildings gets the smallest coverage mix holding what it covers.
    for assignment in &plan.environment_assignments {
        if assignment.units == 0 {
            continue;
        }
        let mut choice: Option<(usize, u32)> = None;
        for (i, kind) in model.kinds.iter().enumerate() {
            let VarKind::Environment { building, mode, types, option } = kind else { continue };
            if *building != assignment.building || *mode != assignment.mode {
                continue;
            }
            let holds = assignment.covered.iter().all(|(facility, total)| {
                let per_building = total.div_ceil(assignment.units);
                types.iter().position(|t| t == facility).is_some_and(|t| option.counts[t] >= per_building)
            });
            let size: u32 = option.counts.iter().sum();
            if holds && choice.is_none_or(|(_, best)| size < best) {
                choice = Some((i, size));
            }
        }
        let (i, _) = choice?;
        let count = bounds[i].0 + assignment.units as f64;
        bounds[i] = (count, count);
    }
    Some(bounds)
}

/// Solves only the continuous relaxation: the most any plan could earn.
pub fn solve_relaxed(
    items: &[ProductionItem],
    currency: &str,
    facility_counts: &FacilityCounts,
    module_levels: &ModuleLevels,
) -> Option<f64> {
    let model = build_model(items, currency, facility_counts, module_levels, Goal::Earn { floors: &[] });
    model.relax(&model.bounds).map(|(value, _)| value)
}

fn plan_from(model: &Model, value: f64, upper_bound: f64, proven_optimal: bool, nodes: u32, values: &[f64], _root: f64) -> ExactPlan {
    let mut recipe_rates = BTreeMap::new();
    let mut units = BTreeMap::new();
    let mut sold = BTreeMap::new();
    let mut environment = Vec::new();
    let mut pairs: Vec<ExactPair> = Vec::new();
    let mut pace = None;
    for (kind, &v) in model.kinds.iter().zip(values) {
        match kind {
            VarKind::Pace => pace = Some(v),
            VarKind::Rate(recipe) if v > 1e-9 => {
                recipe_rates.insert(recipe.name.clone(), v);
            }
            VarKind::Units(recipe) if takes_turns(recipe) && v > 1e-9 => {
                // The units it runs on at least part of the time.
                units.insert(recipe.name.clone(), ((v - 1e-6).ceil().max(1.0)) as u32);
            }
            VarKind::Units(recipe) if v > 0.5 => {
                units.insert(recipe.name.clone(), v.round() as u32);
            }
            VarKind::Sold(name) if v > 1e-9 => {
                sold.insert(name.to_string(), v);
            }
            VarKind::EnvironmentPair { buildings, modes, types, option } if v > 0.5 => {
                let middle = crate::coverage::mode_temperature(modes.0)
                    .zip(crate::coverage::mode_temperature(modes.1))
                    .and_then(|(a, b)| temperature_mode(a + b));
                let covers = |zone: Zone| -> Vec<(String, u32)> {
                    types
                        .iter()
                        .enumerate()
                        .filter(|(t, _)| option.counts[zone as usize][*t] > 0)
                        .map(|(t, facility)| (facility.to_string(), option.counts[zone as usize][t]))
                        .collect()
                };
                pairs.push(ExactPair {
                    buildings: (buildings.0.to_string(), buildings.1.to_string()),
                    modes: (modes.0.to_string(), modes.1.to_string()),
                    offset: option.offset,
                    count: v.round() as u32,
                    zone_modes: [
                        Some(modes.0.to_string()),
                        middle.map(str::to_string),
                        Some(modes.1.to_string()),
                    ],
                    zone_covers: [covers(Zone::First), covers(Zone::Both), covers(Zone::Second)],
                });
            }
            VarKind::Environment { building, mode, types, option } if v > 0.5 => environment.push(ExactEnvironment {
                building: building.to_string(),
                mode: mode.to_string(),
                count: v.round() as u32,
                covers: types.iter().zip(&option.counts).filter(|(_, c)| **c > 0).map(|(t, c)| (t.to_string(), *c)).collect(),
                option: option.clone(),
            }),
            _ => {}
        }
    }
    // Units set to a recipe that doesn't run are just idle.
    units.retain(|name, _| recipe_rates.contains_key(name));
    let rate_per_second = model.earnings.iter().zip(values).map(|(c, v)| c * v).sum();
    ExactPlan {
        rate_per_second,
        objective: value,
        upper_bound,
        proven_optimal,
        nodes,
        recipe_rates,
        units,
        sold,
        environment,
        pairs,
        pace,
    }
}

/// Re-checks an [`ExactPlan`] from scratch, independently of the solver: whole units, owned units
/// per facility and level, item balances, environment coverage and, for a level-up, its pace,
/// then recomputes its earnings. Returns the recomputed coins/sec, or what's wrong.
pub fn check_plan(
    plan: &ExactPlan,
    items: &[ProductionItem],
    currency: &str,
    facility_counts: &FacilityCounts,
    module_levels: &ModuleLevels,
    level_up: Option<&LevelUp>,
) -> Result<f64, String> {
    const TOLERANCE: f64 = 1e-6;
    let all: HashMap<&str, &ProductionItem> = items.iter().map(|i| (i.name.as_str(), i)).collect();
    let mut earned = 0.0;
    let mut made: HashMap<&str, f64> = HashMap::new();
    let mut plots_needing: HashMap<(&str, &str), u32> = HashMap::new();
    // Units in use per facility and level; a recipe taking turns counts only its share of time.
    let mut units_at: HashMap<&str, Vec<(u32, f64)>> = HashMap::new();
    for (name, &rate) in &plan.recipe_rates {
        let recipe = all.get(name.as_str()).ok_or(format!("unknown recipe {name}"))?;
        if !facility_counts.can_produce(&recipe.facility, recipe.facility_level) {
            return Err(format!("{name} needs {} level {}", recipe.facility, recipe.facility_level));
        }
        if let Some((module, level)) = &recipe.module_requirement {
            if !module_levels.can_use(module, *level) {
                return Err(format!("{name} needs {module} {level}"));
            }
        }
        let units = plan.units.get(name).copied().unwrap_or(0);
        if rate * recipe.production_time > units as f64 + TOLERANCE {
            return Err(format!("{name} runs {rate}/s but has {units} units at {}s each", recipe.production_time));
        }
        let in_use = if takes_turns(recipe) { rate * recipe.production_time } else { units as f64 };
        units_at.entry(recipe.facility.as_str()).or_default().push((recipe.facility_level, in_use));
        if let Some(environment) = recipe.environment.as_deref() {
            *plots_needing.entry((recipe.facility.as_str(), environment)).or_default() += units;
        }
        *made.entry(made_item(&recipe.name, &all)).or_default() += rate * recipe.yield_amount as f64;
        if let Some(item) = recipe.byproduct.as_ref().and_then(|(resource, _)| byproduct_item(resource)) {
            *made.entry(item).or_default() += rate * recipe.byproduct.as_ref().map_or(0.0, |(_, a)| *a as f64);
        }
        if let (Some(inputs), Some(amounts)) = (&recipe.raw_materials, &recipe.required_amount) {
            for (input, &amount) in inputs.iter().zip(amounts) {
                *made.entry(input.as_str()).or_default() -= rate * amount as f64;
            }
        }
        if currency == "coins" {
            earned -= rate * recipe.cost.unwrap_or(0.0);
        }
    }
    for (name, &sold) in &plan.sold {
        let item = all.get(name.as_str()).ok_or(format!("unknown item {name}"))?;
        *made.entry(item.name.as_str()).or_default() -= sold;
        // An item sold for another currency (Aniimo EXP, Aniipods) leaves the balance the same
        // way, but earns nothing towards `currency`.
        if item.sell_currency == currency {
            earned += sold * item.sell_value;
        }
    }
    if let Some(level_up) = level_up {
        let pace = plan.pace.ok_or("the plan has no level-up pace")?;
        let per_second = |name: &str| pace * (LevelUp::amount(&level_up.stock, name) - LevelUp::amount(&level_up.cost, name)) / PACE_UNIT;
        for (name, _) in level_up.cost.iter().chain(&level_up.stock) {
            if name != currency {
                made.entry(name.as_str()).or_default();
            }
        }
        for (name, left) in made.iter_mut() {
            *left += per_second(name);
        }
        if earned + per_second(currency) < -TOLERANCE * earned.abs().max(1.0) {
            return Err(format!("earns {earned}/s, too little for a level-up every {} s", PACE_UNIT / pace));
        }
    }
    for (item, left) in &made {
        if *left < -TOLERANCE {
            return Err(format!("{item} is used or sold faster than it's made (short {:.6}/s)", -left));
        }
    }
    for (facility, entries) in &units_at {
        for &(level, _) in entries {
            let needed: f64 = entries.iter().filter(|(l, _)| *l >= level).map(|(_, u)| u).sum();
            let owned = facility_counts.capacity_at_level(facility, level);
            if needed > owned as f64 + TOLERANCE {
                return Err(format!("{facility}: {needed} units at level {level}+ but {owned} owned"));
            }
        }
    }
    let mut buildings_used: HashMap<&str, u32> = HashMap::new();
    let mut covered: HashMap<(&str, &str), u32> = HashMap::new();
    for env in &plan.environment {
        *buildings_used.entry(env.building.as_str()).or_default() += env.count;
        for (facility, count) in &env.covers {
            *covered.entry((facility.as_str(), env.mode.as_str())).or_default() += count * env.count;
        }
    }
    // Overlapping pairs: each uses one of both buildings, and covers a zone per temperature.
    for pair in &plan.pairs {
        *buildings_used.entry(pair.buildings.0.as_str()).or_default() += pair.count;
        *buildings_used.entry(pair.buildings.1.as_str()).or_default() += pair.count;
        let middle = crate::coverage::mode_temperature(&pair.modes.0)
            .zip(crate::coverage::mode_temperature(&pair.modes.1))
            .and_then(|(a, b)| temperature_mode(a + b));
        let modes = [Some(pair.modes.0.as_str()), middle, Some(pair.modes.1.as_str())];
        for (zone, mode) in modes.iter().enumerate() {
            let Some(mode) = mode else { continue };
            if pair.zone_modes[zone].as_deref() != Some(mode) {
                return Err(format!("pair zone {zone} says {:?}, not {mode}", pair.zone_modes[zone]));
            }
            for (facility, count) in &pair.zone_covers[zone] {
                *covered.entry((facility.as_str(), mode)).or_default() += count * pair.count;
            }
        }
    }
    for (building, used) in buildings_used {
        if used > facility_counts.get_count(building) {
            return Err(format!("{used} {building} set up but {} owned", facility_counts.get_count(building)));
        }
    }
    // The measurement build lets a building mix arrangements, which no whole layout can hold, so
    // its plans are deliberately unbuildable and only their rate is of interest.
    for ((facility, environment), plots) in plots_needing {
        let have = covered.get(&(facility, environment)).copied().unwrap_or(0);
        if plots > have && !coverage_relaxed() {
            return Err(format!("{plots} {facility} plots need {environment} but {have} are covered"));
        }
    }
    if (earned - plan.rate_per_second).abs() > 1e-6 * earned.abs().max(1.0) {
        return Err(format!("plan says {} but its sales and costs add up to {earned}", plan.rate_per_second));
    }
    Ok(earned)
}

/// The model in CPLEX LP format, for solving with an external solver, and how many variables it
/// has (`x0` up to `x<count - 1>`; a solver may leave out any that no constraint mentions).
pub fn write_lp(
    items: &[ProductionItem],
    currency: &str,
    facility_counts: &FacilityCounts,
    module_levels: &ModuleLevels,
    goal: Goal,
) -> (String, usize) {
    let model = build_model(items, currency, facility_counts, module_levels, goal);
    let term = |c: f64, v: usize| format!("{} {} x{v}", if c < 0.0 { "-" } else { "+" }, c.abs());
    let mut out = String::from("Maximize\n obj:");
    for (v, &c) in model.objective.iter().enumerate() {
        if c != 0.0 {
            out.push(' ');
            out.push_str(&term(c, v));
        }
    }
    out.push_str("\nSubject To\n");
    for (k, (terms, op, rhs)) in model.constraints.iter().enumerate() {
        let op = match op {
            ComparisonOp::Le => "<=",
            ComparisonOp::Ge => ">=",
            ComparisonOp::Eq => "=",
        };
        let lhs: Vec<String> = terms.iter().map(|&(v, c)| term(c, v)).collect();
        out.push_str(&format!(" c{k}: {} {op} {rhs}\n", lhs.join(" ")));
    }
    out.push_str("Bounds\n");
    for (v, &(lo, hi)) in model.bounds.iter().enumerate() {
        if hi.is_finite() {
            out.push_str(&format!(" {lo} <= x{v} <= {hi}\n"));
        } else {
            out.push_str(&format!(" x{v} >= {lo}\n"));
        }
    }
    out.push_str("General\n");
    for (v, &int) in model.integer.iter().enumerate() {
        if int {
            out.push_str(&format!(" x{v}\n"));
        }
    }
    out.push_str("End\n");
    (out, model.objective.len())
}

/// Builds an [`ExactPlan`] from an external solver's variable values (in [`write_lp`]'s `x0`,
/// `x1`, ... order): rounds every whole-unit variable, then re-solves the rates for exactly those
/// units, so the plan is consistent even if the solver's own values carry rounding noise.
/// `proven_optimal` and `upper_bound` are what the solver reported.
#[allow(clippy::too_many_arguments)]
pub fn plan_from_values(
    items: &[ProductionItem],
    currency: &str,
    facility_counts: &FacilityCounts,
    module_levels: &ModuleLevels,
    goal: Goal,
    values: &[f64],
    proven_optimal: bool,
    upper_bound: f64,
) -> Option<ExactPlan> {
    let model = build_model(items, currency, facility_counts, module_levels, goal);
    if values.len() != model.objective.len() {
        return None;
    }
    let fixed: Vec<(f64, f64)> = model
        .bounds
        .iter()
        .zip(&model.integer)
        .zip(values)
        .map(|((&b, &int), &v)| {
            if int {
                let r = v.round().max(b.0).min(b.1);
                (r, r)
            } else {
                b
            }
        })
        .collect();
    let (value, solved) = model.relax(&fixed)?;
    Some(plan_from(&model, value, upper_bound.max(value), proven_optimal, 0, &solved, value))
}

/// What `exact` makes per second of `currency`, at its items' sell values: the Aniimo EXP or
/// Aniipods a coin plan keeps up alongside its coins.
pub fn currency_rate(exact: &ExactPlan, items: &[ProductionItem], currency: &str) -> f64 {
    items
        .iter()
        .filter(|item| item.sell_currency == currency)
        .map(|item| exact.sold.get(&item.name).copied().unwrap_or(0.0) * item.sell_value)
        .sum()
}

/// The items `exact` makes for a priority `target` that's a currency, with how many per second:
/// the Growth items behind Aniimo EXP, or the Aniipods. Empty for coins and byproducts.
pub fn target_items(exact: &ExactPlan, items: &[ProductionItem], target: &str) -> Vec<(String, f64)> {
    if target == "coins" {
        return Vec::new();
    }
    items
        .iter()
        .filter(|item| item.sell_currency == target)
        .filter_map(|item| exact.sold.get(&item.name).filter(|&&n| n > 1e-9).map(|&n| (item.name.clone(), n)))
        .collect()
}

/// What `exact` makes per second of a priority `target`: coins (net of seed costs), another
/// currency ("aniimo_exp", "aniipods"), or a byproduct ("Wood Blocks", "Mineral Sand").
pub fn target_rate(exact: &ExactPlan, items: &[ProductionItem], target: &str) -> f64 {
    let all: HashMap<&str, &ProductionItem> = items.iter().map(|i| (i.name.as_str(), i)).collect();
    let byproduct: f64 = exact
        .recipe_rates
        .iter()
        .filter_map(|(name, rate)| match &all.get(name.as_str())?.byproduct {
            Some((resource, amount)) if resource == target => Some(rate * *amount as f64),
            _ => None,
        })
        .sum();
    if byproduct > 0.0 {
        return byproduct;
    }
    let seeds: f64 = if target == "coins" {
        exact.recipe_rates.iter().filter_map(|(name, rate)| Some(rate * all.get(name.as_str())?.cost.unwrap_or(0.0))).sum()
    } else {
        0.0
    };
    currency_rate(exact, items, target) - seeds
}

/// Each item's rate per second made, less what the plan's recipes use and sell, including Wood
/// Blocks and Mineral Sand (as `wood_block` and `mineral_sand`). Negative for an item drawn from
/// stock.
pub fn net_rates(exact: &ExactPlan, items: &[ProductionItem]) -> BTreeMap<String, f64> {
    let all: HashMap<&str, &ProductionItem> = items.iter().map(|i| (i.name.as_str(), i)).collect();
    let mut net: BTreeMap<String, f64> = BTreeMap::new();
    for (name, &rate) in &exact.recipe_rates {
        let Some(recipe) = all.get(name.as_str()) else { continue };
        *net.entry(made_item(name, &all).to_string()).or_default() += rate * recipe.yield_amount as f64;
        if let Some((resource, amount)) = &recipe.byproduct {
            if let Some(item) = byproduct_item(resource) {
                *net.entry(item.to_string()).or_default() += rate * *amount as f64;
            }
        }
        if let (Some(inputs), Some(amounts)) = (&recipe.raw_materials, &recipe.required_amount) {
            for (input, &amount) in inputs.iter().zip(amounts) {
                *net.entry(input.clone()).or_default() -= rate * amount as f64;
            }
        }
    }
    for (name, &sold) in &exact.sold {
        *net.entry(name.clone()).or_default() -= sold;
    }
    net
}

/// The seed cost in each unit of every item a plan uses, and the plan's total seed cost, both in
/// coins per second. A crop's seeds are shared by every unit of it that's sold or used, and what's
/// made from it carries its share on, so Rose Concentrate pays for the roses that go into it
/// rather than the few roses sold raw.
fn seed_costs_by_item<'a>(exact: &'a ExactPlan, all: &HashMap<&str, &'a ProductionItem>) -> (HashMap<&'a str, f64>, f64) {
    // Units/sec of each item sold or used as an ingredient.
    let mut used: HashMap<&str, f64> = HashMap::new();
    for (name, &sold) in &exact.sold {
        *used.entry(name.as_str()).or_default() += sold;
    }
    let recipes: Vec<(&ProductionItem, f64)> =
        exact.recipe_rates.iter().filter_map(|(name, &rate)| Some((*all.get(name.as_str())?, rate))).collect();
    for (recipe, rate) in &recipes {
        if let (Some(inputs), Some(amounts)) = (&recipe.raw_materials, &recipe.required_amount) {
            for (input, &amount) in inputs.iter().zip(amounts) {
                *used.entry(input.as_str()).or_default() += rate * amount as f64;
            }
        }
    }
    let total = recipes.iter().map(|(recipe, rate)| recipe.cost.unwrap_or(0.0) * rate).sum();
    // Recipes chain only a few deep, so passing costs along until nothing changes settles fast.
    let mut per_unit: HashMap<&str, f64> = HashMap::new();
    for _ in 0..32 {
        let mut into: HashMap<&str, f64> = HashMap::new();
        for (recipe, rate) in &recipes {
            let mut cost = recipe.cost.unwrap_or(0.0) * rate;
            if let (Some(inputs), Some(amounts)) = (&recipe.raw_materials, &recipe.required_amount) {
                for (input, &amount) in inputs.iter().zip(amounts) {
                    cost += rate * amount as f64 * per_unit.get(input.as_str()).copied().unwrap_or(0.0);
                }
            }
            *into.entry(made_item(&recipe.name, all)).or_default() += cost;
        }
        let next: HashMap<&str, f64> = into
            .into_iter()
            .filter_map(|(item, cost)| {
                let used = used.get(item).copied().unwrap_or(0.0);
                (used > 1e-12 && cost > 0.0).then(|| (item, cost / used))
            })
            .collect();
        let settled = next.len() == per_unit.len()
            && next.iter().all(|(item, cost)| per_unit.get(item).is_some_and(|c| (c - cost).abs() <= 1e-12 * cost.max(1.0)));
        per_unit = next;
        if settled {
            break;
        }
    }
    (per_unit, total)
}

/// Turns an [`ExactPlan`] into the [`crate::models::ProductionPlan`] the rest of the app shows:
/// one row per facility and recipe, what each is used for, income per item sold, byproducts, and
/// environment building layouts.
pub fn to_production_plan(
    exact: &ExactPlan,
    items: &[ProductionItem],
    currency: &str,
    facility_counts: &FacilityCounts,
) -> crate::models::ProductionPlan {
    use crate::models::{EnvironmentAssignment, FacilityPlacement, PlanProduct, PlanStep, PlanStepStatus};
    let all: HashMap<&str, &ProductionItem> = items.iter().map(|i| (i.name.as_str(), i)).collect();
    let is_grower = |facility: &str| !items.iter().any(|i| i.facility == facility && i.raw_materials.is_some());

    // What each made item goes to: the recipes using it, and whether the rest sells.
    let uses_of = |recipe: &ProductionItem| -> String {
        let made = made_item(&recipe.name, &all);
        let mut uses: Vec<&str> = exact
            .recipe_rates
            .keys()
            .filter_map(|name| all.get(name.as_str()))
            .filter(|r| r.raw_materials.as_ref().is_some_and(|inputs| inputs.iter().any(|i| i == made)))
            .map(|r| crate::models::base_item_name(&r.name))
            .collect();
        uses.sort_unstable();
        uses.dedup();
        let sells = exact.sold.get(made).is_some_and(|&s| s > 1e-9);
        // Made, not sold, not all used up and not sellable: kept for the level-up.
        let sellable = all.get(made).is_some_and(|item| item.sell_currency != "none" && item.sell_value > 0.0);
        let kept = exact.pace.is_some() && !sells && !sellable && net_rates(exact, items).get(made).is_some_and(|&n| n > 1e-9);
        match (uses.is_empty(), sells || kept) {
            (true, _) if kept => "For the level-up".to_string(),
            (true, _) => "Sells directly".to_string(),
            (false, false) => format!("Used for {}", uses.join(", ")),
            (false, true) if kept => format!("Used for {}; the rest goes to the level-up", uses.join(", ")),
            (false, true) => format!("Used for {}; the rest sells directly", uses.join(", ")),
        }
    };

    let mut facility_names: Vec<&str> = items.iter().map(|i| i.facility.as_str()).collect();
    facility_names.sort_unstable();
    facility_names.dedup();
    let mut coin_items: Vec<PlanStep> = Vec::new();
    for facility in facility_names {
        let owned = facility_counts.get_count(facility);
        if owned == 0 {
            continue;
        }
        let grower = is_grower(facility);
        let mut rows: Vec<(&ProductionItem, u32, f64)> = exact
            .units
            .iter()
            .filter_map(|(name, &units)| all.get(name.as_str()).map(|r| (*r, units)))
            .filter(|(r, _)| r.facility == facility)
            .map(|(r, units)| (r, units, exact.recipe_rates.get(&r.name).copied().unwrap_or(0.0)))
            .collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.name.cmp(&b.0.name)));
        if rows.is_empty() {
            coin_items.push(PlanStep {
                item_name: None,
                facility: facility.to_string(),
                facility_count: owned,
                status: PlanStepStatus::NothingAvailable,
                reason: "Nothing it can make helps this plan".to_string(),
                is_grower: grower,
                cycle_time: None,
                environment: None,
                busy_units: None,
            });
            continue;
        }
        // Recipes taking turns share units, so what's in use is their combined time.
        let shared: Vec<&str> = rows.iter().filter(|(r, _, _)| takes_turns(r)).map(|(r, _, _)| r.name.as_str()).collect();
        let shared_time: f64 = rows.iter().filter(|(r, _, _)| takes_turns(r)).map(|(r, _, rate)| rate * r.production_time).sum();
        let used: u32 = rows.iter().filter(|(r, _, _)| !takes_turns(r)).map(|(_, u, _)| u).sum::<u32>()
            + ((shared_time - 1e-6).ceil().max(0.0) as u32);
        let used = used.min(owned);
        for (recipe, units, rate) in rows {
            let mut reason = uses_of(recipe);
            if takes_turns(recipe) {
                let others: Vec<&str> = shared.iter().copied().filter(|n| *n != recipe.name).collect();
                if !others.is_empty() {
                    reason = format!("{reason}; takes turns with {}", others.join(", "));
                }
            }
            if recipe.name.ends_with(crate::models::UNCOVERED_SUFFIX) {
                let base = all.get(crate::models::base_item_name(&recipe.name)).and_then(|i| i.environment.as_deref());
                let speed = base.and_then(crate::models::uncovered_efficiency).unwrap_or(1.0) * 100.0;
                let wants = base.unwrap_or("its environment");
                reason = format!("{reason}; grown without {wants} at {speed:.0}% speed");
            }
            coin_items.push(PlanStep {
                item_name: Some(crate::models::base_item_name(&recipe.name).to_string()),
                facility: facility.to_string(),
                facility_count: units,
                status: PlanStepStatus::Producing,
                reason,
                is_grower: grower,
                cycle_time: grower.then_some(recipe.production_time),
                environment: if grower { recipe.environment.clone() } else { None },
                busy_units: (!grower).then(|| (rate * recipe.production_time).min(units as f64)),
            });
        }
        if owned > used {
            coin_items.push(PlanStep {
                item_name: None,
                facility: facility.to_string(),
                facility_count: owned - used,
                status: PlanStepStatus::Idle,
                reason: "No further profitable use found".to_string(),
                is_grower: grower,
                cycle_time: None,
                environment: None,
                busy_units: None,
            });
        }
    }

    // One income stream per item sold, less the seed costs that went into it (see
    // `seed_costs_by_item`).
    let mut income_streams: Vec<PlanProduct> = exact
        .sold
        .iter()
        .filter_map(|(name, &units)| {
            let item = all.get(name.as_str())?;
            // Only what sells for the plan's currency is income; Growth items or Aniipods a plan
            // also makes (for a priority) are counted separately, not as coins.
            if item.sell_currency != currency {
                return None;
            }
            Some(PlanProduct {
                item_name: name.clone(),
                facility: item.facility.clone(),
                sell_value: item.sell_value,
                rate_per_second: units * item.sell_value,
                units_per_second: units,
                lead_time: crate::optimizer::item_lead_time(name, &all, 0),
                total_units: 0.0,
                total_value: 0.0,
            })
        })
        .collect();
    if currency == "coins" {
        let (per_unit, total) = seed_costs_by_item(exact, &all);
        let mut assigned = 0.0;
        for stream in &mut income_streams {
            let cost = stream.units_per_second * per_unit.get(stream.item_name.as_str()).copied().unwrap_or(0.0);
            stream.rate_per_second -= cost;
            assigned += cost;
        }
        // Seeds for anything not sold for coins (a crop only kept for a level-up, say) still come
        // out of the coins; the biggest stream takes them, so the streams add up to the rate.
        let rest = total - assigned;
        if rest > 1e-9 {
            if let Some(biggest) = income_streams.iter_mut().max_by(|a, b| {
                a.rate_per_second.partial_cmp(&b.rate_per_second).unwrap_or(std::cmp::Ordering::Equal)
            }) {
                biggest.rate_per_second -= rest;
            }
        }
    }

    let mut byproduct_rates: Vec<(String, f64, f64)> = Vec::new();
    for (name, &units) in &exact.units {
        let Some(recipe) = all.get(name.as_str()) else { continue };
        if let Some((resource, amount)) = &recipe.byproduct {
            let rate = units as f64 * *amount as f64 / recipe.production_time;
            byproduct_rates.push((resource.clone(), rate, crate::optimizer::item_lead_time(name, &all, 0)));
        }
    }

    // Environment layouts, trimmed to the plots the plan actually uses.
    let mut still_needed: HashMap<(String, String), u32> = HashMap::new();
    for (name, &units) in &exact.units {
        if let Some(recipe) = all.get(name.as_str()) {
            if let Some(environment) = &recipe.environment {
                *still_needed.entry((recipe.facility.clone(), environment.clone())).or_default() += units;
            }
        }
    }
    // Overlapping pairs show as one assignment per zone, all in the same frame so the diagram can
    // draw both buildings. Worked out before the buildings standing on their own, so the plots
    // they cover are taken off what those still have to cover.
    let mut pair_assignments: Vec<EnvironmentAssignment> = Vec::new();
    for pair in &exact.pairs {
        // Only what the plan actually grows: a zone whose plots went unused isn't worth drawing,
        // and a pair with nothing in the zone they share is really two separate buildings.
        let used: [Vec<(String, u32)>; 3] = std::array::from_fn(|zone| {
            let Some(mode) = pair.zone_modes[zone].clone() else { return Vec::new() };
            pair.zone_covers[zone]
                .iter()
                .filter_map(|(facility, covered)| {
                    let left = still_needed.entry((facility.clone(), mode.clone())).or_default();
                    let take = (*covered * pair.count).min(*left);
                    *left -= take;
                    (take > 0).then(|| (facility.clone(), take))
                })
                .collect()
        });
        let holds = |zone: usize| used[zone].iter().map(|(_, n)| n).sum::<u32>() > 0;
        let types: Vec<&str> = {
            let mut names: Vec<&str> = used.iter().flatten().map(|(f, _)| f.as_str()).collect();
            names.sort_unstable();
            names.dedup();
            names
        };
        let counts: [Vec<u32>; 3] = std::array::from_fn(|zone| {
            types
                .iter()
                .map(|facility| used[zone].iter().find(|(f, _)| f == facility).map_or(0, |(_, n)| *n))
                .collect()
        });
        let sizes = PairSizes::of(&pair.buildings.0, &pair.buildings.1);
        // A plain arrangement of the same plots, for when the tidy one can't be worked out.
        let plain = || {
            crate::coverage::pair_layout_for(sizes, &types, pair.offset, &counts).unwrap_or_default()
        };
        // Standing two buildings a measured distance apart is fiddly, and it only buys anything
        // while all three zones are growing something: with one of them empty the plan wants two
        // temperatures, which two buildings give wherever they stand. So the pair is shown as
        // separate buildings, each set to a zone that is in use, as long as each can be set to
        // the mode it is being given (the shared zone's mode is one of theirs for every pair the
        // plan can build; if some later building breaks that, the pair stands as it is).
        let zones_used: Vec<usize> = (0..3).filter(|&zone| holds(zone)).collect();
        let supports = |building: &String, mode: &String| {
            ENVIRONMENT_BUILDINGS
                .iter()
                .find(|(name, _)| name == building)
                .is_some_and(|(_, modes)| modes.contains(&mode.as_str()))
        };
        let apart: Option<Vec<(usize, &String, String)>> = (zones_used.len() < 3)
            .then(|| {
                let wanted: Vec<(usize, String)> = zones_used
                    .iter()
                    .filter_map(|&zone| pair.zone_modes[zone].clone().map(|mode| (zone, mode)))
                    .collect();
                let buildings = [&pair.buildings.0, &pair.buildings.1];
                // At most two zones are in use, so either order of the two buildings will do.
                [[0, 1], [1, 0]].into_iter().find_map(|order| {
                    wanted
                        .iter()
                        .enumerate()
                        .map(|(i, (zone, mode))| {
                            let building = buildings[order[i]];
                            supports(building, mode).then(|| (*zone, building, mode.clone()))
                        })
                        .collect::<Option<Vec<_>>>()
                })
            })
            .flatten();
        if let Some(apart) = apart {
            for (zone, building, mode) in apart {
                let layout = crate::coverage::tidy_layout(building_size(building), &used[zone])
                    .unwrap_or_else(|| plain()[zone].clone())
                    .into_iter()
                    .map(|p| FacilityPlacement { facility: p.facility, x: p.x, y: p.y, size: p.size })
                    .collect::<Vec<_>>();
                pair_assignments.push(EnvironmentAssignment {
                    building: building.clone(),
                    mode,
                    units: pair.count,
                    covered: used[zone].clone(),
                    layouts: vec![layout; pair.count as usize],
                    partner: None,
                    zone: None,
                    pair_modes: None,
                });
            }
            continue;
        }
        let layouts: [Vec<crate::coverage::Placement>; 3] =
            crate::coverage::tidy_pair_layout(sizes, &types, pair.offset, &counts).unwrap_or_else(plain);
        for zone in 0..3 {
            let (Some(mode), true) = (pair.zone_modes[zone].clone(), holds(zone)) else { continue };
            let layout: Vec<FacilityPlacement> = layouts[zone]
                .iter()
                .map(|p| FacilityPlacement { facility: p.facility.clone(), x: p.x, y: p.y, size: p.size })
                .collect();
            pair_assignments.push(EnvironmentAssignment {
                building: pair.buildings.0.clone(),
                mode,
                units: pair.count,
                covered: used[zone].clone(),
                layouts: vec![layout; pair.count as usize],
                partner: Some((pair.buildings.1.clone(), pair.offset.dx, pair.offset.dy)),
                zone: Some(zone as u8),
                pair_modes: Some((pair.modes.0.clone(), pair.modes.1.clone())),
            });
        }
    }

    let environment_assignments: Vec<EnvironmentAssignment> = exact
        .environment
        .iter()
        .map(|env| {
            let mut layouts: Vec<Vec<FacilityPlacement>> = Vec::new();
            let mut covered: Vec<(String, u32)> = Vec::new();
            // Plots closest to the building first, so a building covering fewer plots than its
            // layout holds shows a compact cluster rather than whichever plots came first. Any
            // subset of a non-overlapping layout is still one.
            let center = crate::coverage::building_size(&env.building) / 2.0;
            let mut nearest_first = env.option.layout.clone();
            nearest_first.sort_by(|a, b| {
                let distance = |p: &crate::coverage::Placement| {
                    let (dx, dy) = (p.x + p.size / 2.0 - center, p.y + p.size / 2.0 - center);
                    dx * dx + dy * dy
                };
                distance(a).partial_cmp(&distance(b)).unwrap_or(std::cmp::Ordering::Equal)
            });
            for _ in 0..env.count {
                let mut layout = Vec::new();
                for placement in &nearest_first {
                    let need = still_needed.entry((placement.facility.clone(), env.mode.clone())).or_default();
                    if *need == 0 {
                        continue;
                    }
                    *need -= 1;
                    match covered.iter_mut().find(|(f, _)| *f == placement.facility) {
                        Some((_, n)) => *n += 1,
                        None => covered.push((placement.facility.clone(), 1)),
                    }
                    layout.push(FacilityPlacement {
                        facility: placement.facility.clone(),
                        x: placement.x,
                        y: placement.y,
                        size: placement.size,
                    });
                }
                // Show a tidy arrangement of the same plots when one fits on whole tiles; the
                // packing's own layout only matters for how many fit, not where they go.
                let mut here: Vec<(String, u32)> = Vec::new();
                for p in &layout {
                    match here.iter_mut().find(|(f, _)| *f == p.facility) {
                        Some((_, n)) => *n += 1,
                        None => here.push((p.facility.clone(), 1)),
                    }
                }
                if let Some(tidy) = crate::coverage::tidy_layout(building_size(&env.building), &here) {
                    layout = tidy
                        .into_iter()
                        .map(|p| FacilityPlacement { facility: p.facility, x: p.x, y: p.y, size: p.size })
                        .collect();
                }
                layouts.push(layout);
            }
            EnvironmentAssignment {
                building: env.building.clone(),
                mode: env.mode.clone(),
                units: env.count,
                covered,
                layouts,
                partner: None,
                zone: None,
                pair_modes: None,
            }
        })
        .filter(|a| !a.covered.is_empty())
        .chain(pair_assignments)
        .collect();

    crate::models::ProductionPlan {
        currency: currency.to_string(),
        rate_per_second: exact.rate_per_second,
        income_streams,
        coin_items,
        byproduct_rates,
        environment_assignments,
        candidates_evaluated: exact.recipe_rates.len() as u32,
        trial_solves: exact.nodes,
    }
}
