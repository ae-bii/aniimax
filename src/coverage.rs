//! Exact 2D geometric packing for environment-building coverage (Heat Furnace/Cooling Unit/
//! Sunlamp).
//!
//! ## Coverage geometry
//! - A Cooling Unit is a 2x2 footprint; a Heat Furnace and a Sunlamp sit on one tile
//!   ([`ENVIRONMENT_BUILDING_SIZES`]). All three cover the same 9x9, so the smaller ones'
//!   squares land on tile lines while the Cooling Unit's sits half a tile off.
//! - It radiates coverage as a square of side `2 * `[`COVERAGE_RADIUS`] centered on its own exact
//!   center (not its corner).
//! - A facility is covered only if its own footprint overlaps that coverage square by a real
//!   area; a corner-only touch is not enough. Since every footprint snaps to quarter tiles
//!   ([`GRID_STEP`]), any nonzero overlap between two quarter-tile-aligned rectangles is
//!   automatically at least 0.25x0.25, so this rule falls out for free from only ever generating
//!   candidate positions on that grid; no separate minimum-area check is needed.
//! - Facilities can't overlap the building itself or each other.
//! - Facility footprints: Farmland/Dewy House 2x2, Woodland 4x4, Starfall Hammock/Tidewhisper
//!   Sandcastle/Floral Windmill 5x5; see [`ENVIRONMENT_GATED_FACILITIES`].
//!
//! ## Candidate generation is a bounded heuristic, not exhaustive
//!
//! Sweeping every quarter-tile position for every facility type would generate thousands of
//! candidates per type; intractable for the MILP branch & bound this feeds into (see
//! `crate::optimizer::solve_facility_allocation`). Instead, [`candidate_positions`] finds the
//! single best quarter-tile alignment for each facility type on its own, plus a handful of
//! half-space variants (restricting that same search to one half of the region, split through the
//! building's center) so the ILP has enough raw material to reconstruct mixed layouts when two or
//! more types share one building's coverage. Candidate **generation** is this bounded, tuned
//! subset; candidate **selection** among them (which is what actually decides how many of each
//! type get used) is exact integer optimization.

use std::collections::HashMap;

const EPS: f64 = 1e-6;

/// Environment building footprints. The Cooling Unit takes a 2x2 like a Farmland; the Heat
/// Furnace and Sunlamp sit on a single tile. All three cover the same 9x9 area from their own
/// center, so a 1x1 building's coverage lands on tile lines while the Cooling Unit's is half a
/// tile off, which changes how many whole-tile plots reach its edge.
pub const ENVIRONMENT_BUILDING_SIZES: &[(&str, f64)] = &[
    ("Heat Furnace", 1.0),
    ("Cooling Unit", 2.0),
    ("Sunlamp", 1.0),
];

/// The footprint of an environment building; [`DEFAULT_BUILDING_SIZE`] for anything not listed.
pub fn building_size(name: &str) -> f64 {
    ENVIRONMENT_BUILDING_SIZES
        .iter()
        .find(|(n, _)| *n == name)
        .map_or(DEFAULT_BUILDING_SIZE, |(_, size)| *size)
}

/// Used where the building isn't known; the larger of the two, so a layout worked out without a
/// name never claims room the real building takes up.
pub const DEFAULT_BUILDING_SIZE: f64 = 2.0;
/// Coverage radiates this far from the building's exact center in every direction, i.e. total
/// coverage span is `2 * COVERAGE_RADIUS` (a 9x9 square).
pub const COVERAGE_RADIUS: f64 = 4.5;
/// Facilities and buildings snap to quarter tiles in game (screenshots show Farmland offset by
/// both a quarter and a half tile); also the smallest possible nonzero coverage overlap.
pub const GRID_STEP: f64 = 0.25;

/// Every facility type whose environment-gated items are capacity-bound by owned environment
/// buildings, with its fixed square footprint side length.
pub const ENVIRONMENT_GATED_FACILITIES: &[(&str, f64)] = &[
    ("Farmland", 2.0),
    ("Woodland", 4.0),
    ("Starfall Hammock", 5.0),
    ("Tidewhisper Sandcastle", 5.0),
    ("Floral Windmill", 5.0),
    ("Dewy House", 2.0),
];

/// Returns the footprint side length for an environment-gated facility type, if it is one.
pub fn facility_footprint(name: &str) -> Option<f64> {
    ENVIRONMENT_GATED_FACILITIES.iter().find(|(n, _)| *n == name).map(|(_, w)| *w)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Rect {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
}

impl Rect {
    fn new(x: f64, y: f64, w: f64) -> Self {
        Rect { x1: x, y1: y, x2: x + w, y2: y + w }
    }

    /// Real (positive-area) overlap; a shared edge or corner alone does not count. See this
    /// module's doc comment for the coverage rule this implements.
    fn overlaps(&self, o: &Rect) -> bool {
        self.x1 < o.x2 - EPS && self.x2 > o.x1 + EPS && self.y1 < o.y2 - EPS && self.y2 > o.y1 + EPS
    }
}

fn building_rect(building: f64) -> Rect {
    Rect { x1: 0.0, y1: 0.0, x2: building, y2: building }
}

/// The 9x9 a building of side `building` covers, centered on the building's own center and so
/// shifted by half a tile between the two sizes.
fn coverage_rect(building: f64) -> Rect {
    let c = building / 2.0;
    Rect { x1: c - COVERAGE_RADIUS, y1: c - COVERAGE_RADIUS, x2: c + COVERAGE_RADIUS, y2: c + COVERAGE_RADIUS }
}

/// One candidate placement: a facility of `facility` type, anchored at `(x, y)` (its
/// lower-left corner), footprint side `size`. Used both as a packing-ILP candidate and, once
/// selected, as the exact position rendered in the frontend's layout diagram, so this is real
/// game-grid geometry, not a display-only abstraction.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    pub facility: String,
    pub x: f64,
    pub y: f64,
    pub size: f64,
}

impl Placement {
    fn rect(&self) -> Rect {
        Rect::new(self.x, self.y, self.size)
    }
}

/// Which half of the region a candidate's footprint must stay within; used only to generate
/// extra candidates (see this module's doc comment); the ILP is free to ignore them.
#[derive(Debug, Clone, Copy)]
enum HalfSpace {
    Left(f64),
    Right(f64),
    Bottom(f64),
    Top(f64),
}

impl HalfSpace {
    fn allows(&self, r: &Rect) -> bool {
        match self {
            HalfSpace::Left(line) => r.x2 <= *line + EPS,
            HalfSpace::Right(line) => r.x1 >= *line - EPS,
            HalfSpace::Bottom(line) => r.y2 <= *line + EPS,
            HalfSpace::Top(line) => r.y1 >= *line - EPS,
        }
    }
}

/// Finds the single best quarter-tile alignment (offset) for tiling `size`-square facilities
/// around the fixed building/coverage geometry, optionally restricted to one `half`, and returns
/// every valid position from that best alignment (valid = doesn't overlap the building, overlaps
/// the coverage square by positive area). Also returns, second, the alignment fitting the same
/// number whose positions sit most evenly around the building (the same as the first if that one
/// already does), for tidying layouts (see `compact_layout`); the first can pack better alongside
/// other facility types, so it's the one offered to the packing.
fn best_grid_positions(building: f64, size: f64, half: Option<HalfSpace>) -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
    let building_box = building_rect(building);
    let coverage = coverage_rect(building);
    let mut best: Vec<(f64, f64)> = Vec::new();
    let mut centered: Vec<(f64, f64)> = Vec::new();

    let steps = (size / GRID_STEP).round() as i64;
    for oi in 0..steps {
        let offset = oi as f64 * GRID_STEP;
        for oj in 0..steps {
            let oy = oj as f64 * GRID_STEP;
            let mut positions: Vec<(f64, f64)> = Vec::new();

            let k_min = ((coverage.x1 - size - offset) / size).floor() as i64 - 2;
            let k_max = ((coverage.x2 - offset) / size).ceil() as i64 + 2;
            let m_min = ((coverage.y1 - size - oy) / size).floor() as i64 - 2;
            let m_max = ((coverage.y2 - oy) / size).ceil() as i64 + 2;

            for k in k_min..=k_max {
                let px = offset + k as f64 * size;
                if px + size < coverage.x1 - EPS || px > coverage.x2 + EPS {
                    continue;
                }
                for m in m_min..=m_max {
                    let py = oy + m as f64 * size;
                    let rect = Rect::new(px, py, size);
                    if !rect.overlaps(&coverage) {
                        continue;
                    }
                    if rect.overlaps(&building_box) {
                        continue;
                    }
                    if let Some(h) = half {
                        if !h.allows(&rect) {
                            continue;
                        }
                    }
                    positions.push((px, py));
                }
            }

            if positions.len() > best.len() {
                best = positions.clone();
                centered = positions;
            } else if positions.len() == best.len() && off_center(building, &positions, size) < off_center(building, &centered, size) - EPS {
                centered = positions;
            }
        }
    }
    (best, centered)
}

/// How far the middle of `positions`' bounding box (for `size`-square footprints) is from the
/// building's center, summed over both axes; 0 for a set laid out evenly around the building.
fn off_center(building: f64, positions: &[(f64, f64)], size: f64) -> f64 {
    if positions.is_empty() {
        return f64::INFINITY;
    }
    let center = building / 2.0;
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for &(x, y) in positions {
        min_x = min_x.min(x);
        max_x = max_x.max(x + size);
        min_y = min_y.min(y);
        max_y = max_y.max(y + size);
    }
    ((min_x + max_x) / 2.0 - center).abs() + ((min_y + max_y) / 2.0 - center).abs()
}

/// Every candidate position worth offering the packing solver for one facility type: its own
/// best full grid, plus its best grids restricted to each half of the region (so the solver can
/// reconstruct "Hybrid"-style splits when sharing coverage with another type); deduplicated.
pub fn candidate_positions(building: f64, size: f64) -> Vec<(f64, f64)> {
    let split = building / 2.0; // the building's own center coordinate on each axis
    let mut all: Vec<(f64, f64)> = best_grid_positions(building, size, None).0;
    let mut add = |positions: Vec<(f64, f64)>| {
        for pos in positions {
            if !all.contains(&pos) {
                all.push(pos);
            }
        }
    };
    for half in [
        HalfSpace::Left(split),
        HalfSpace::Right(split),
        HalfSpace::Bottom(split),
        HalfSpace::Top(split),
    ] {
        add(best_grid_positions(building, size, Some(half)).0);
    }
    all
}

/// Every candidate [`Placement`] for one facility type; see [`candidate_positions`].
pub fn candidate_placements(building: f64, facility: &str, size: f64) -> Vec<Placement> {
    candidate_positions(building, size)
        .into_iter()
        .map(|(x, y)| Placement { facility: facility.to_string(), x, y, size })
        .collect()
}

/// `true` if two placements' footprints overlap by positive area (so at most one of them,
/// across all K identical buildings sharing this coverage, can occupy that overlap at once).
pub fn placements_overlap(a: &Placement, b: &Placement) -> bool {
    a.rect().overlaps(&b.rect())
}

/// Whether a packing ILP's answer can be believed. Every packing here runs under a time limit,
/// and a solve that stops early hands back the relaxation, whose half-taken placements read as
/// taken against a `> 0.5` test and add up to far more than really fits. So a fractional answer
/// is thrown away rather than rounded, and the placements it did settle on are checked for
/// overlap, since an answer that fits nothing is worse than no answer at all.
fn packing_is_sound(vars: &[(Placement, microlp::Variable)], solution: &microlp::Solution) -> bool {
    if vars.iter().any(|(_, v)| (1e-6..=1.0 - 1e-6).contains(&solution[*v])) {
        return false;
    }
    let taken: Vec<&Placement> = vars.iter().filter(|(_, v)| solution[*v] > 0.5).map(|(p, _)| p).collect();
    !taken.iter().enumerate().any(|(i, p)| taken[i + 1..].iter().any(|q| placements_overlap(p, q)))
}

/// The exact packing solved for one (building type, mode): how many of each candidate placement
/// get used, aggregated across all `building_count` identical, independently-placed buildings.
pub struct PackingSolution {
    /// Total covered count per facility type, summed across every selected placement.
    pub covered: Vec<(String, u32)>,
    /// Every candidate placement that got used at least once, with how many of the
    /// `building_count` buildings host a facility there (`1..=building_count`).
    pub placements: Vec<(Placement, u32)>,
}

/// Adds the non-overlap constraints for a set of placement variables, bounding how many can cover
/// any single quarter-tile cell at once by `capacity`. This is a cell-based set-packing
/// formulation (standard for "no two selected rectangles overlap"), not naive pairwise
/// `var_i + var_j <= capacity` constraints; pairwise gives an extremely loose LP relaxation for
/// this kind of problem, whereas bounding how many placements can cover each individual cell is
/// both far fewer constraints and a much tighter relaxation, since it's the same structure as
/// classical interval scheduling.
///
/// `capacity` must be a fixed number, not itself a decision variable; a variable capacity (for
/// splitting one building type's owned count across modes within a single combined ILP) hangs at
/// moderate scale.
fn add_cell_conflict_constraints(problem: &mut microlp::Problem, vars: &[(Placement, microlp::Variable)], capacity: u32) {
    let (mut min_x, mut max_x, mut min_y, mut max_y) =
        (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for (p, _) in vars {
        let r = p.rect();
        min_x = min_x.min(r.x1);
        max_x = max_x.max(r.x2);
        min_y = min_y.min(r.y1);
        max_y = max_y.max(r.y2);
    }
    if !min_x.is_finite() {
        return; // no vars
    }
    let cols = ((max_x - min_x) / GRID_STEP).round() as i64;
    let rows = ((max_y - min_y) / GRID_STEP).round() as i64;
    for cx in 0..cols {
        let cell_x1 = min_x + cx as f64 * GRID_STEP;
        let cell_x2 = cell_x1 + GRID_STEP;
        for cy in 0..rows {
            let cell_y1 = min_y + cy as f64 * GRID_STEP;
            let cell_y2 = cell_y1 + GRID_STEP;
            let terms: Vec<(microlp::Variable, f64)> = vars
                .iter()
                .filter(|(p, _)| {
                    let r = p.rect();
                    r.x1 < cell_x2 - EPS && r.x2 > cell_x1 + EPS && r.y1 < cell_y2 - EPS && r.y2 > cell_y1 + EPS
                })
                .map(|(_, v)| (*v, 1.0))
                .collect();
            if terms.len() < 2 {
                continue; // a cell touched by at most one candidate can never conflict
            }
            problem.add_constraint(&terms, microlp::ComparisonOp::Le, capacity as f64);
        }
    }
}

/// Solves the exact packing ILP for `building_count` identical, independently-placed environment
/// buildings sharing one mode: which candidate placements (see [`candidate_placements`]) to use,
/// maximizing `Σ weight(type) * count`, such that no two selected placements' footprints overlap
/// by more than `building_count` (the K-buildings-share-one-candidate-set simplification; see
/// `crate::optimizer::solve_facility_allocation`'s doc comment for why this is exact, not an
/// approximation, given the buildings don't interact with each other).
///
/// `weighted_types` should list every environment-gated facility type with at least one
/// profitable candidate item needing this mode, paired with that type's per-plot profit weight.
/// Returns `None` if there's nothing to place or no buildings to place it in.
pub fn solve_packing(building: f64, weighted_types: &[(&str, f64)], building_count: u32) -> Option<PackingSolution> {
    if building_count == 0 || weighted_types.is_empty() {
        return None;
    }

    let mut problem = microlp::Problem::new(microlp::OptimizationDirection::Maximize);
    let mut vars: Vec<(Placement, microlp::Variable)> = Vec::new();
    for &(facility, weight) in weighted_types {
        if weight <= 0.0 {
            continue;
        }
        let Some(size) = facility_footprint(facility) else { continue };
        for placement in candidate_placements(building, facility, size) {
            let var = problem.add_integer_var(weight, (0, building_count as i32));
            vars.push((placement, var));
        }
    }
    if vars.is_empty() {
        return None;
    }

    add_cell_conflict_constraints(&mut problem, &vars, building_count);

    let solution = problem.solve().ok()?;

    let mut covered: Vec<(String, u32)> = Vec::new();
    let mut placements: Vec<(Placement, u32)> = Vec::new();
    for (placement, var) in vars {
        let count = solution[var].round() as u32;
        if count == 0 {
            continue;
        }
        match covered.iter_mut().find(|(name, _)| *name == placement.facility) {
            Some((_, total)) => *total += count,
            None => covered.push((placement.facility.clone(), count)),
        }
        placements.push((placement, count));
    }

    Some(PackingSolution { covered, placements })
}

/// Solves ONE building's own layout (a small, fast independent-set-style ILP: which candidate
/// placements can coexist within a single building's coverage without overlapping, maximizing
/// `Σ weight(type)`), considering only candidates whose facility type still has capacity left in
/// `remaining_owned`, and capping each facility type's total placements used here at that
/// remaining count. Without that cap, candidate positions of the same small footprint (e.g. Dewy
/// House, 2x2) can tile several non-overlapping spots within one building's coverage zone; the
/// cell-conflict constraint alone only forbids two placements from covering the same cell, so
/// without an explicit per-type cap, a facility type with positive weight but only 1 owned unit
/// would still get every one of those non-overlapping spots filled in the layout. Binary
/// variables, capacity 1 per cell; this stays fast even with all 6 facility types competing and a
/// tight ownership cap (see `packing_solve_is_fast_enough_for_the_worst_realistic_case` and
/// `building_packing_stays_fast_at_realistic_owned_counts`); unlike solving many buildings' shared
/// capacity jointly (see `solve_building_packing`'s doc comment), a single building's placement
/// choice has no multi-building ownership-cap trade-off baked into the ILP, so one extra linear
/// cap constraint on an already-tiny, single-building problem stays fast.
fn solve_one_building_layout<'a>(
    building: f64,
    weighted_types: &[(&'a str, f64)],
    remaining_owned: &HashMap<&'a str, u32>,
) -> (Vec<Placement>, f64) {
    let mut problem = microlp::Problem::new(microlp::OptimizationDirection::Maximize);
    let mut vars: Vec<(Placement, microlp::Variable)> = Vec::new();
    for &(facility, weight) in weighted_types {
        if weight <= 0.0 || remaining_owned.get(facility).copied().unwrap_or(0) == 0 {
            continue;
        }
        let Some(size) = facility_footprint(facility) else { continue };
        for placement in candidate_placements(building, facility, size) {
            let var = problem.add_binary_var(weight);
            vars.push((placement, var));
        }
    }
    if vars.is_empty() {
        return (Vec::new(), 0.0);
    }

    add_cell_conflict_constraints(&mut problem, &vars, 1);

    let mut facility_types: Vec<&str> = weighted_types.iter().map(|(f, _)| *f).collect();
    facility_types.sort_unstable();
    facility_types.dedup();
    for facility_type in facility_types {
        let owned_count = remaining_owned.get(facility_type).copied().unwrap_or(0);
        let terms: Vec<(microlp::Variable, f64)> =
            vars.iter().filter(|(p, _)| p.facility == facility_type).map(|(_, v)| (*v, 1.0)).collect();
        if terms.is_empty() {
            continue;
        }
        problem.add_constraint(&terms, microlp::ComparisonOp::Le, owned_count as f64);
    }

    // Bounds this single-building solve's worst case: with several facility types competing for
    // one building's coverage under a moderately tight per-type ownership cap, candidates sharing
    // one type's identical weight give `microlp`'s branch & bound a huge number of objectively-tied
    // ways to pick "which k of these non-overlapping spots"; proving optimality can take much
    // longer than finding the optimal value in the first place (see
    // `regression_tests::single_building_layout_time_limit_still_finds_true_optimum`). A modest
    // deadline keeps this bounded without sacrificing quality in the vast majority of cases: this
    // crate's B&B always evaluates a full incumbent before working on later ones, so an incumbent
    // is typically found almost immediately, with the remaining time spent trying (usually in
    // vain) to beat it.
    problem.set_time_limit(std::time::Duration::from_millis(300));
    let Ok(solution) = problem.solve() else { return (Vec::new(), 0.0) };

    // If the deadline hit before the solver ever found an integral incumbent (only realistically
    // possible on a slow device/debug build, or an even harder instance than any seen so far),
    // `solution` reflects an LP relaxation snapshot, not a real 0/1 assignment; thresholding
    // fractional values at 0.5 could select overlapping placements or exceed a facility's
    // ownership cap. Treat that the same as "nothing worth covering this round" (the same
    // safe/conservative fallback `solve_building_packing`'s caller already uses when a mode's best
    // value is `<= EPS`) rather than ever trusting a possibly-infeasible selection.
    let is_integral = vars.iter().all(|(_, v)| {
        let x = solution[*v];
        !(1e-6..=1.0 - 1e-6).contains(&x)
    });
    if !is_integral {
        return (Vec::new(), 0.0);
    }

    let value = solution.objective();
    let layout: Vec<Placement> =
        vars.into_iter().filter(|(_, v)| solution[*v] > 0.5).map(|(p, _)| p).collect();
    (layout, value)
}

/// Solves environment-building coverage for ONE building type's `owned` units, jointly across ALL
/// its modes (they share the same physical buildings; e.g. a Cooling Unit's Cool and Freeze both
/// compete for the same units). `weighted` lists every `(facility_type, mode, weight)` combo with
/// at least one candidate item, where `weight` is that combo's own per-plot value; independent of
/// any solved item rate (see below for why). `facility_owned` is how many of each facility type
/// the player actually owns (shared across every mode, since a physical plot can only serve one
/// mode's crop at a time).
///
/// Assigns the `owned` buildings ONE AT A TIME: for each, tries every active mode's best possible
/// single-building layout (see [`solve_one_building_layout`]) against whatever facility capacity
/// is still unclaimed, keeps whichever mode scores highest for that specific building, and
/// deducts its usage before moving to the next building. Two alternative designs were ruled out:
/// 1. One combined MILP mixing this packing with the continuous item-rate LP
///    (`crate::optimizer::solve_facility_allocation`); hangs even at modest scale (~50 continuous
///    plus ~120 integer variables).
/// 2. A single joint ILP across all `owned` buildings at once (one variable per candidate
///    placement bounded `[0, owned]`, plus a shared facility-ownership-cap constraint); still
///    hangs once fully decoupled from the item-rate LP, whenever the ownership cap is tight
///    relative to what geometry alone could pack. This is a classic NP-hard packing+knapsack
///    branch-and-bound cliff, not a gradual slowdown: a looser cap stays fast at the same `owned`.
///
/// Solving one binary-variable, capacity-1 building at a time sidesteps that cliff entirely: each
/// individual solve has no ownership trade-off encoded in the ILP (exhausted types are simply
/// excluded from the candidate set), so every one of the `owned` solves stays in the same fast,
/// well-behaved regime as a lone building. The cost is trading true joint optimality (across
/// buildings AND modes AND facility types simultaneously) for a sequential greedy approximation,
/// the same kind of accepted trade `find_production_plan`'s own stranded-chain exclusion loop
/// already makes elsewhere in this codebase, and unavoidable here since the exact joint problem is
/// genuinely intractable at realistic scale.
///
/// Returns `(mode_counts, placements, layouts)`:
/// - `mode_counts` keyed by `(building, mode)`; how many owned units ended up running that mode.
/// - `placements` keyed by `mode`; every used candidate placement, aggregated with its count
///   (feeds `crate::optimizer::solve_facility_allocation`'s `coverage_bounds`).
/// - `layouts` keyed by `mode`; one entry per building instance assigned to that mode, each a
///   concrete non-overlapping list of placements (feeds the frontend's per-building diagram).
pub fn solve_building_packing<'a>(
    building: &'a str,
    modes: &[&'a str],
    weighted: &[(&'a str, &'a str, f64)],
    owned: u32,
    facility_owned: &[(&'a str, u32)],
) -> (
    HashMap<(&'a str, &'a str), u32>,
    HashMap<&'a str, Vec<(Placement, u32)>>,
    HashMap<&'a str, Vec<Vec<Placement>>>,
) {
    let mut mode_counts: HashMap<(&'a str, &'a str), u32> = HashMap::new();
    let mut result_placements: HashMap<&'a str, Vec<(Placement, u32)>> = HashMap::new();
    let mut layouts: HashMap<&'a str, Vec<Vec<Placement>>> = HashMap::new();
    if owned == 0 || weighted.is_empty() {
        return (mode_counts, result_placements, layouts);
    }

    let mut by_mode: HashMap<&'a str, Vec<(&'a str, f64)>> = HashMap::new();
    for &(facility_type, mode, weight) in weighted {
        by_mode.entry(mode).or_default().push((facility_type, weight));
    }
    let active_modes: Vec<&'a str> = modes.iter().copied().filter(|m| by_mode.contains_key(m)).collect();
    if active_modes.is_empty() {
        return (mode_counts, result_placements, layouts);
    }

    let mut remaining_owned: HashMap<&'a str, u32> = facility_owned.iter().copied().collect();

    for _ in 0..owned {
        let mut best: Option<(&'a str, Vec<Placement>, f64)> = None;
        for &mode in &active_modes {
            let entries = &by_mode[mode];
            let (layout, value) = solve_one_building_layout(building_size(building), entries, &remaining_owned);
            if value > EPS && best.as_ref().is_none_or(|(_, _, best_value)| value > *best_value) {
                best = Some((mode, layout, value));
            }
        }
        let Some((mode, layout, _value)) = best else { break }; // nothing left worth covering anywhere
        for placement in &layout {
            if let Some(c) = remaining_owned.get_mut(placement.facility.as_str()) {
                *c = c.saturating_sub(1);
            }
        }
        *mode_counts.entry((building, mode)).or_insert(0) += 1;
        layouts.entry(mode).or_default().push(layout);
    }

    for (&mode, mode_layouts) in &layouts {
        let mut agg: Vec<(Placement, u32)> = Vec::new();
        for layout in mode_layouts {
            for placement in layout {
                match agg.iter_mut().find(|(p, _)| p == placement) {
                    Some((_, count)) => *count += 1,
                    None => agg.push((placement.clone(), 1)),
                }
            }
        }
        result_placements.insert(mode, agg);
    }

    (mode_counts, result_placements, layouts)
}

/// One way a single environment building can cover facilities: how many of each facility type
/// (in the order of the `types` it was computed for) fit in its coverage, and where they go.
#[derive(Debug, Clone, PartialEq)]
pub struct CoverageOption {
    pub counts: Vec<u32>,
    pub layout: Vec<Placement>,
}

/// The most facilities of `types[target]` one building's coverage can hold while also holding at
/// least `minimums[t]` of every other type, with the layout; `None` if the minimums don't fit.
/// Other types get a tiny weight so the layout is filled out rather than just meeting the minimums.
fn most_with_minimums(building: f64, types: &[&str], minimums: &[u32], target: usize) -> Option<CoverageOption> {
    let mut problem = microlp::Problem::new(microlp::OptimizationDirection::Maximize);
    let mut vars: Vec<(Placement, microlp::Variable)> = Vec::new();
    let mut type_of: Vec<usize> = Vec::new();
    for (t, &facility) in types.iter().enumerate() {
        let size = facility_footprint(facility)?;
        let weight = if t == target { 1.0 } else { 1e-3 };
        for placement in candidate_placements(building, facility, size) {
            vars.push((placement, problem.add_binary_var(weight)));
            type_of.push(t);
        }
    }
    add_cell_conflict_constraints(&mut problem, &vars, 1);
    for (t, &minimum) in minimums.iter().enumerate() {
        if t == target || minimum == 0 {
            continue;
        }
        let terms: Vec<(microlp::Variable, f64)> =
            vars.iter().zip(&type_of).filter(|(_, &ty)| ty == t).map(|((_, v), _)| (*v, 1.0)).collect();
        problem.add_constraint(&terms, microlp::ComparisonOp::Ge, minimum as f64);
    }
    problem.set_time_limit(std::time::Duration::from_secs(5));
    let solution = problem.solve().ok()?;
    let chosen: Vec<usize> = (0..vars.len()).filter(|&i| solution[vars[i].1] > 0.5).collect();
    if vars.iter().any(|(_, v)| (1e-6..=1.0 - 1e-6).contains(&solution[*v])) {
        return None;
    }
    let mut counts = vec![0u32; types.len()];
    for &i in &chosen {
        counts[type_of[i]] += 1;
    }
    if counts.iter().zip(minimums).enumerate().any(|(t, (c, m))| t != target && c < m) {
        return None;
    }
    let layout = compact_layout(building, chosen.into_iter().map(|i| vars[i].0.clone()).collect(), types);
    Some(CoverageOption { counts, layout })
}

/// Distance from a placement's center to the building's center.
fn distance_to_building(building: f64, p: &Placement) -> f64 {
    let center = building / 2.0;
    (p.x + p.size / 2.0 - center).hypot(p.y + p.size / 2.0 - center)
}

/// Pulls a layout in around the building without changing how many of each type it holds: moves
/// the farthest placement to the closest free candidate spot of its type that's nearer, and
/// repeats until nothing moves. Many layouts fit the same counts; this picks a compact, centered
/// one to show.
fn compact_layout(building: f64, mut layout: Vec<Placement>, types: &[&str]) -> Vec<Placement> {
    // The packing candidates plus each type's most centered grid, which the packing itself
    // doesn't need (it never fits more) but gives plots somewhere tidier to move to.
    let mut candidates: Vec<Placement> = Vec::new();
    for &facility in types {
        if let Some(size) = facility_footprint(facility) {
            candidates.extend(candidate_placements(building, facility, size));
            for (x, y) in best_grid_positions(building, size, None).1 {
                candidates.push(Placement { facility: facility.to_string(), x, y, size });
            }
        }
    }
    candidates.sort_by(|a, b| distance_to_building(building, a).partial_cmp(&distance_to_building(building, b)).unwrap_or(std::cmp::Ordering::Equal));
    loop {
        layout.sort_by(|a, b| distance_to_building(building, b).partial_cmp(&distance_to_building(building, a)).unwrap_or(std::cmp::Ordering::Equal));
        let mut moved = false;
        for i in 0..layout.len() {
            let current = distance_to_building(building, &layout[i]);
            let spot = candidates.iter().find(|c| {
                c.facility == layout[i].facility
                    && distance_to_building(building, c) < current - EPS
                    && layout.iter().enumerate().all(|(j, other)| j == i || !placements_overlap(c, other))
            });
            if let Some(spot) = spot {
                layout[i] = spot.clone();
                moved = true;
                break;
            }
        }
        if !moved {
            return layout;
        }
    }
}

thread_local! {
    static TIDY_CACHE: std::cell::RefCell<HashMap<(u64, Vec<(String, u32)>), Option<Vec<Placement>>>> =
        std::cell::RefCell::new(HashMap::new());
}

/// A tidy layout to show for exactly `counts` plots around one building: [`nearest_layout`], or
/// if that search fails, whichever of [`pinwheel_layout`] and [`grid_layout`] puts the plots
/// nearer the building. All keep every plot on the building's tile grid, inside the rules. Only
/// for drawing: how many plots a building covers comes from the packing, not from this. `None`
/// if nothing fits, so the packing's quarter-tile layout is shown instead. Cached per `counts`.
pub fn tidy_layout(building: f64, counts: &[(String, u32)]) -> Option<Vec<Placement>> {
    let counts: Vec<(String, u32)> = counts.iter().filter(|(_, n)| *n > 0).cloned().collect();
    let key = (building.to_bits(), counts.clone());
    if let Some(hit) = TIDY_CACHE.with(|cache| cache.borrow().get(&key).cloned()) {
        return hit;
    }
    let layout = nearest_layout(building, &counts).or_else(|| fallback_layout(building, &counts));
    TIDY_CACHE.with(|cache| cache.borrow_mut().insert(key, layout.clone()));
    layout
}

/// Whichever of [`pinwheel_layout`] and [`grid_layout`] puts the plots nearer the building.
fn fallback_layout(building: f64, counts: &[(String, u32)]) -> Option<Vec<Placement>> {
    let center = building / 2.0;
    let spread = |layout: &Vec<Placement>| -> f64 {
        layout.iter().map(|p| (p.x + p.size / 2.0 - center).hypot(p.y + p.size / 2.0 - center)).sum()
    };
    [pinwheel_layout(building, counts), grid_layout(building, counts)]
        .into_iter()
        .flatten()
        .min_by(|a, b| spread(a).partial_cmp(&spread(b)).unwrap_or(std::cmp::Ordering::Equal))
}

/// How much a full shared side between two plots is worth, in tiles of distance, in
/// [`nearest_layout`]: enough to line plots up in rows and columns, not so much that they drift
/// away from the building to do it.
const ALIGN_REWARD: f64 = 1.0;

/// The most compact arrangement of exactly `counts` plots: the one with the least total distance
/// from plot centers to the building's center. `None` if no alignment can hold them.
///
/// Plots snap to quarter tiles in game, and which alignment they share decides how many reach the
/// coverage square's edge: a 1x1 building's square lands on tile lines, so a whole-tile grid fits
/// a column fewer than a half-tile one (25 Farmland against 32). Every alignment is tried and the
/// nearest one that holds the plots wins, with all types on the same one, so the result still
/// reads as one grid rather than a staggered mess.
pub fn nearest_layout(building: f64, counts: &[(String, u32)]) -> Option<Vec<Placement>> {
    // Tidiest alignment first: both axes on whole tiles, then both on halves, then one of each,
    // then the quarters. The first that holds the plots wins, so the common case is one solve and
    // a layout that fitted before is found exactly where it always was.
    const ALIGNMENTS: [(f64, f64); 16] = [
        (0.0, 0.0),
        (0.5, 0.5),
        (0.0, 0.5),
        (0.5, 0.0),
        (0.25, 0.25),
        (0.75, 0.75),
        (0.25, 0.75),
        (0.75, 0.25),
        (0.0, 0.25),
        (0.25, 0.0),
        (0.0, 0.75),
        (0.75, 0.0),
        (0.5, 0.25),
        (0.25, 0.5),
        (0.5, 0.75),
        (0.75, 0.5),
    ];
    ALIGNMENTS
        .iter()
        .find_map(|&offset| nearest_layout_at(building, counts, offset))
        .map(|(_, layout)| layout)
}

/// [`nearest_layout`] on one quarter-tile alignment, with what that arrangement cost.
fn nearest_layout_at(
    building: f64,
    counts: &[(String, u32)],
    (ox, oy): (f64, f64),
) -> Option<(f64, Vec<Placement>)> {
    let building_box = building_rect(building);
    let coverage = coverage_rect(building);
    let center = building / 2.0;
    let mut problem = microlp::Problem::new(microlp::OptimizationDirection::Minimize);
    let mut vars: Vec<(Placement, microlp::Variable)> = Vec::new();
    for (facility, count) in counts.iter().filter(|(_, n)| *n > 0) {
        let size = facility_footprint(facility)?;
        let (lo, hi) = ((coverage.x1 - size).floor() as i64, coverage.x2.ceil() as i64);
        let mut of_type: Vec<(f64, microlp::Variable)> = Vec::new();
        for x in lo..=hi {
            for y in lo..=hi {
                let (x, y) = (x as f64 + ox, y as f64 + oy);
                let rect = Rect::new(x, y, size);
                if !rect.overlaps(&coverage) || rect.overlaps(&building_box) {
                    continue;
                }
                let distance = (x + size / 2.0 - center).hypot(y + size / 2.0 - center);
                let v = problem.add_binary_var(distance);
                of_type.push((1.0, v));
                vars.push((Placement { facility: facility.clone(), x, y, size }, v));
            }
        }
        problem.add_constraint(&of_type.iter().map(|&(c, v)| (v, c)).collect::<Vec<_>>(), microlp::ComparisonOp::Eq, *count as f64);
    }
    // Plots of a size lined up edge to edge, a whole side against a whole side, earn a reward, so
    // the nearest arrangement also comes out in rows and columns rather than staggered.
    let reward = ALIGN_REWARD;
    for (i, (a, va)) in vars.iter().enumerate() {
        for (b, vb) in &vars[i + 1..] {
            let side = (a.size - b.size).abs() < EPS
                && (((a.x - b.x).abs() - a.size).abs() < EPS && (a.y - b.y).abs() < EPS
                    || ((a.y - b.y).abs() - a.size).abs() < EPS && (a.x - b.x).abs() < EPS);
            if side {
                let both = problem.add_var(-reward, (0.0, 1.0));
                problem.add_constraint(&[(both, 1.0), (*va, -1.0)], microlp::ComparisonOp::Le, 0.0);
                problem.add_constraint(&[(both, 1.0), (*vb, -1.0)], microlp::ComparisonOp::Le, 0.0);
            }
        }
    }
    // No two plots may overlap, at the quarter-tile granularity they snap to.
    add_cell_conflict_constraints(&mut problem, &vars, 1);
    problem.set_time_limit(std::time::Duration::from_millis(300));
    let solution = problem.solve().ok()?;
    if !packing_is_sound(&vars, &solution) {
        return None;
    }
    let chosen: Vec<Placement> = vars.iter().filter(|(_, v)| solution[*v] > 0.5).map(|(p, _)| p.clone()).collect();
    let wanted: u32 = counts.iter().map(|(_, n)| n).sum();
    (chosen.len() == wanted as usize).then_some((solution.objective(), chosen))
}

/// Plots flush against the building and each other, nearest first: each plot takes the closest
/// free whole-tile spot, preferring spots touching the building with a corner on one of its
/// corner lines, so the next plot can sit flush beside it (a pinwheel). Largest footprint first.
/// `None` once plots no longer fit this way.
pub fn pinwheel_layout(building: f64, counts: &[(String, u32)]) -> Option<Vec<Placement>> {
    let building_box = building_rect(building);
    let coverage = coverage_rect(building);
    let center = building / 2.0;
    let mut wanted: Vec<(&str, f64, u32)> = counts
        .iter()
        .filter(|(_, n)| *n > 0)
        .map(|(facility, n)| facility_footprint(facility).map(|size| (facility.as_str(), size, *n)))
        .collect::<Option<_>>()?;
    wanted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(b.0)));
    let mut placed: Vec<Placement> = Vec::new();
    for (facility, size, count) in wanted {
        let lo = (coverage.x1 - size).floor() as i64;
        let hi = coverage.x2.ceil() as i64;
        let mut spots: Vec<((f64, f64, f64, f64), Placement)> = Vec::new();
        for x in lo..=hi {
            for y in lo..=hi {
                let (x, y) = (x as f64, y as f64);
                let rect = Rect::new(x, y, size);
                if !rect.overlaps(&coverage) || rect.overlaps(&building_box) {
                    continue;
                }
                let (dx, dy) = (x + size / 2.0 - center, y + size / 2.0 - center);
                // How far the plot's edge is from the building's, so plots touching it come first.
                let gap_x = (building_box.x1 - rect.x2).max(rect.x1 - building_box.x2).max(0.0);
                let gap_y = (building_box.y1 - rect.y2).max(rect.y1 - building_box.y2).max(0.0);
                // A plot with a corner on one of the building's corner lines leaves room for the
                // next one to sit flush beside it (a pinwheel), where a centered one sticks out
                // past the building both ways.
                let edges = [building_box.x1, building_box.x2];
                let aligned = (edges.contains(&rect.x1) || edges.contains(&rect.x2))
                    && (edges.contains(&rect.y1) || edges.contains(&rect.y2));
                // Clockwise from straight up, so equally good spots fill around the building.
                let angle = dx.atan2(dy).rem_euclid(std::f64::consts::TAU);
                let key = (gap_x.hypot(gap_y), if aligned { 0.0 } else { 1.0 }, (dx * dx + dy * dy).sqrt(), angle);
                spots.push((key, Placement { facility: facility.to_string(), x, y, size }));
            }
        }
        spots.sort_by(|a, b| {
            let rounded = |k: &(f64, f64, f64, f64)| [(k.0 * 1e6).round(), k.1, (k.2 * 1e6).round(), k.3];
            rounded(&a.0).partial_cmp(&rounded(&b.0)).unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut left = count;
        for (_, spot) in spots {
            if left == 0 {
                break;
            }
            if placed.iter().all(|p| !placements_overlap(&spot, p)) {
                placed.push(spot);
                left -= 1;
            }
        }
        if left > 0 {
            return None;
        }
    }
    Some(placed)
}

/// Plots of each type on a regular grid of plot-sized cells, so every row and column lines up.
/// The grid is shifted (in whole tiles) to whichever offset fits the plots nearest the building,
/// and the nearest cells are used. Largest footprint first; a smaller type fills cells its grid
/// has that the bigger plots don't cover. `None` if no grid fits them all.
pub fn grid_layout(building: f64, counts: &[(String, u32)]) -> Option<Vec<Placement>> {
    let building_box = building_rect(building);
    let coverage = coverage_rect(building);
    let center = building / 2.0;
    let mut wanted: Vec<(&str, f64, u32)> = counts
        .iter()
        .filter(|(_, n)| *n > 0)
        .map(|(facility, n)| facility_footprint(facility).map(|size| (facility.as_str(), size, *n)))
        .collect::<Option<_>>()?;
    wanted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(b.0)));
    let mut placed: Vec<Placement> = Vec::new();
    for (facility, size, count) in wanted {
        // Every whole-tile offset of a grid of `size` cells, keeping the one whose nearest
        // `count` free cells are closest to the building in total.
        let mut best: Option<(f64, Vec<Placement>)> = None;
        let steps = size.round() as i64;
        for ox in 0..steps {
            for oy in 0..steps {
                let mut cells: Vec<(f64, f64, Placement)> = Vec::new();
                let first = |offset: i64, lo: f64| offset as f64 + ((lo - size - offset as f64) / size).floor() * size;
                let mut x = first(ox, coverage.x1);
                while x <= coverage.x2 {
                    let mut y = first(oy, coverage.y1);
                    while y <= coverage.y2 {
                        let rect = Rect::new(x, y, size);
                        let cell = Placement { facility: facility.to_string(), x, y, size };
                        if rect.overlaps(&coverage) && !rect.overlaps(&building_box) && placed.iter().all(|p| !placements_overlap(&cell, p)) {
                            let (dx, dy) = (x + size / 2.0 - center, y + size / 2.0 - center);
                            // Clockwise from straight up, so equally near cells fill around evenly.
                            let angle = dx.atan2(dy).rem_euclid(std::f64::consts::TAU);
                            cells.push(((dx * dx + dy * dy).sqrt(), angle, cell));
                        }
                        y += size;
                    }
                    x += size;
                }
                if cells.len() < count as usize {
                    continue;
                }
                cells.sort_by(|a, b| {
                    let near = (a.0 * 1e6).round().partial_cmp(&(b.0 * 1e6).round()).unwrap_or(std::cmp::Ordering::Equal);
                    near.then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                });
                cells.truncate(count as usize);
                let total: f64 = cells.iter().map(|c| c.0).sum();
                if best.as_ref().is_none_or(|(t, _)| total < t - 1e-9) {
                    best = Some((total, cells.into_iter().map(|c| c.2).collect()));
                }
            }
        }
        placed.extend(best?.1);
    }
    Some(placed)
}

/// Tries every minimum count of `types[position]` from 0 up until it no longer fits, recursing into
/// the next position; at the last type, records the most of it that fits. Returns whether the
/// minimums fixed so far fit at all.
fn walk_minimums(building: f64, types: &[&str], minimums: &mut Vec<u32>, position: usize, found: &mut Vec<CoverageOption>) -> bool {
    let last = types.len() - 1;
    if position == last {
        return match most_with_minimums(building, types, minimums, last) {
            Some(option) => {
                found.push(option);
                true
            }
            None => false,
        };
    }
    let mut minimum = 0;
    loop {
        minimums[position] = minimum;
        if !walk_minimums(building, types, minimums, position + 1, found) {
            break;
        }
        minimum += 1;
    }
    minimums[position] = 0;
    minimum > 0
}

/// A growing environment's temperature: Freeze -2, Cool -1, Warm +1, Scorching +2. Where two
/// buildings both reach a plot the temperatures add, capped at -2 and +2. `None` for Adequate,
/// which is the Sunlamp's own thing and leaves temperature alone.
pub fn mode_temperature(mode: &str) -> Option<i32> {
    match mode {
        "Freeze" => Some(-2),
        "Cool" => Some(-1),
        "Warm" => Some(1),
        "Scorching" => Some(2),
        _ => None,
    }
}

/// The mode a temperature reads as, for a zone made by overlapping buildings; `None` at neutral.
pub fn temperature_mode(temperature: i32) -> Option<&'static str> {
    match temperature.clamp(-2, 2) {
        -2 => Some("Freeze"),
        -1 => Some("Cool"),
        1 => Some("Warm"),
        2 => Some("Scorching"),
        _ => None,
    }
}

/// Where a plot sits when two environment buildings overlap: reached by the first only, by both
/// (their temperatures added), or by the second only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    First = 0,
    Both = 1,
    Second = 2,
}

/// How the second environment building sits relative to the first: `dx` tiles along and `dy`
/// tiles up, between their near corners. Two 2x2 buildings at offset (2, 0) stand side by side
/// touching; (3, 2) puts one a tile along and two up from the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Offset {
    pub dx: u32,
    pub dy: u32,
}

impl Offset {
    pub fn new(dx: u32, dy: u32) -> Self {
        Offset { dx, dy }
    }

    fn as_f64(&self) -> (f64, f64) {
        (self.dx as f64, self.dy as f64)
    }
}

impl std::fmt::Display for Offset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{}", self.dx, self.dy)
    }
}

/// The footprints of an overlapping pair: the first building at the origin, the second at the
/// offset from it. They are not always the same size, and since each covers the same 9x9 from its
/// own center, a 2x2 Cooling Unit's square sits half a tile off a 1x1 Heat Furnace's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairSizes {
    pub first: f64,
    pub second: f64,
}

impl PairSizes {
    pub fn of(first: &str, second: &str) -> Self {
        PairSizes { first: building_size(first), second: building_size(second) }
    }

    fn key(&self) -> (u64, u64) {
        (self.first.to_bits(), self.second.to_bits())
    }
}

/// The coverage squares of two buildings at `offset` from each other.
fn pair_coverage(sizes: PairSizes, offset: Offset) -> (Rect, Rect) {
    let a = coverage_rect(sizes.first);
    let raw = coverage_rect(sizes.second);
    let (dx, dy) = offset.as_f64();
    let b = Rect { x1: raw.x1 + dx, y1: raw.y1 + dy, x2: raw.x2 + dx, y2: raw.y2 + dy };
    (a, b)
}

/// Every whole-tile placement of `facility` that either building reaches, with which zone it
/// lands in. A plot may hang over a coverage edge, exactly as for one building: what counts is
/// overlapping the square, not sitting inside it. Whole tiles (rather than the quarter tiles a
/// single building's packing sweeps) keep the search small and still fit as many: 32 Farmland
/// around one building either way.
fn pair_candidates(sizes: PairSizes, facility: &str, size: f64, offset: Offset) -> Vec<(Placement, Zone)> {
    let (a, b) = pair_coverage(sizes, offset);
    let (dx, dy) = offset.as_f64();
    let buildings = [
        building_rect(sizes.first),
        Rect { x1: dx, y1: dy, x2: dx + sizes.second, y2: dy + sizes.second },
    ];
    let mut out = Vec::new();
    let mut x = (a.x1 - size).ceil();
    while x <= b.x2 {
        let mut y = (a.y1 - size).ceil();
        while y <= b.y2 {
            let rect = Rect::new(x, y, size);
            let zone = match (rect.overlaps(&a), rect.overlaps(&b)) {
                (true, true) => Some(Zone::Both),
                (true, false) => Some(Zone::First),
                (false, true) => Some(Zone::Second),
                (false, false) => None,
            };
            if let Some(zone) = zone {
                if !buildings.iter().any(|building| rect.overlaps(building)) {
                    out.push((Placement { facility: facility.to_string(), x, y, size }, zone));
                }
            }
            y += 1.0;
        }
        x += 1.0;
    }
    out
}

/// One way to place two environment buildings at `offset` from each other: what each zone holds,
/// in `types` order. Zones run first-only, both, second-only. Where the plots actually go is
/// worked out only for the option a plan picks, by [`tidy_pair_layout`] or [`pair_layout_for`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairOption {
    pub offset: Offset,
    pub counts: [Vec<u32>; 3],
}

impl PairOption {
    /// How many of `types[t]` this option holds in `zone`.
    pub fn count(&self, zone: Zone, t: usize) -> u32 {
        self.counts[zone as usize][t]
    }
}

/// The best layout for two buildings at `offset` from each other when each (zone, type) is worth
/// `weights[zone][type]`, or `None` if the packing can't be solved.
fn pair_layout(sizes: PairSizes, types: &[&str], offset: Offset, weights: &[[f64; 3]]) -> Option<PairOption> {
    let mut problem = microlp::Problem::new(microlp::OptimizationDirection::Maximize);
    let mut vars: Vec<(Placement, microlp::Variable)> = Vec::new();
    let mut tagged: Vec<(usize, Zone)> = Vec::new();
    for (t, &facility) in types.iter().enumerate() {
        let size = facility_footprint(facility)?;
        for (placement, zone) in pair_candidates(sizes, facility, size, offset) {
            vars.push((placement, problem.add_binary_var(weights[t][zone as usize])));
            tagged.push((t, zone));
        }
    }
    if vars.is_empty() {
        return None;
    }
    add_cell_conflict_constraints(&mut problem, &vars, 1);
    problem.set_time_limit(std::time::Duration::from_millis(750));
    let solution = problem.solve().ok()?;
    if !packing_is_sound(&vars, &solution) {
        return None;
    }
    let mut counts = [vec![0u32; types.len()], vec![0u32; types.len()], vec![0u32; types.len()]];
    for (i, (_, var)) in vars.iter().enumerate() {
        if solution[*var] > 0.5 {
            let (t, zone) = tagged[i];
            counts[zone as usize][t] += 1;
        }
    }
    Some(PairOption { offset, counts })
}

/// Any arrangement holding exactly `counts` in each zone, with no attempt to make it tidy. The
/// fallback for when [`tidy_pair_layout`] can't gather a full packing in neatly: the counts came
/// from a packing, so they do fit somehow, and showing that beats showing nothing.
pub fn pair_layout_for(sizes: PairSizes, types: &[&str], offset: Offset, counts: &[Vec<u32>; 3]) -> Option<[Vec<Placement>; 3]> {
    let mut problem = microlp::Problem::new(microlp::OptimizationDirection::Maximize);
    let mut vars: Vec<(Placement, microlp::Variable)> = Vec::new();
    let mut tagged: Vec<(usize, Zone)> = Vec::new();
    for (t, &facility) in types.iter().enumerate() {
        let size = facility_footprint(facility)?;
        for (placement, zone) in pair_candidates(sizes, facility, size, offset) {
            vars.push((placement, problem.add_binary_var(0.0)));
            tagged.push((t, zone));
        }
    }
    if vars.is_empty() {
        return counts.iter().all(|zone| zone.iter().all(|n| *n == 0)).then(Default::default);
    }
    add_cell_conflict_constraints(&mut problem, &vars, 1);
    for zone in [Zone::First, Zone::Both, Zone::Second] {
        for t in 0..types.len() {
            let terms: Vec<(microlp::Variable, f64)> = vars
                .iter()
                .zip(&tagged)
                .filter(|(_, tag)| **tag == (t, zone))
                .map(|((_, var), _)| (*var, 1.0))
                .collect();
            if terms.is_empty() {
                if counts[zone as usize][t] > 0 {
                    return None;
                }
                continue;
            }
            problem.add_constraint(&terms, microlp::ComparisonOp::Eq, counts[zone as usize][t] as f64);
        }
    }
    problem.set_time_limit(std::time::Duration::from_secs(5));
    let solution = problem.solve().ok()?;
    if !packing_is_sound(&vars, &solution) {
        return None;
    }
    let mut layouts: [Vec<Placement>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for (i, (placement, var)) in vars.iter().enumerate() {
        if solution[*var] > 0.5 {
            layouts[tagged[i].1 as usize].push(placement.clone());
        }
    }
    for zone in 0..3 {
        if layouts[zone].len() as u32 != counts[zone].iter().sum::<u32>() {
            return None;
        }
    }
    Some(layouts)
}

/// A tidy arrangement of the same plots a [`PairOption`] holds: the same counts in the same zones,
/// but gathered in around the building they belong to (around the midpoint, in the zone both
/// reach) and lined up edge to edge, as [`nearest_layout`] does for one building. All three zones
/// are placed at once, so plots never land on each other. `None` if those counts can't be placed
/// this way, in which case the packing's own layout stands.
pub fn tidy_pair_layout(sizes: PairSizes, types: &[&str], offset: Offset, counts: &[Vec<u32>; 3]) -> Option<[Vec<Placement>; 3]> {
    let (first, second) = (sizes.first / 2.0, sizes.second / 2.0);
    let (dx, dy) = offset.as_f64();
    // Where each zone's plots gather: its own building, or between them for the shared zone.
    let anchors = [
        (first, first),
        ((first + second + dx) / 2.0, (first + second + dy) / 2.0),
        (second + dx, second + dy),
    ];
    let mut problem = microlp::Problem::new(microlp::OptimizationDirection::Minimize);
    let mut vars: Vec<(Placement, microlp::Variable)> = Vec::new();
    let mut tagged: Vec<(usize, Zone)> = Vec::new();
    for (t, &facility) in types.iter().enumerate() {
        let size = facility_footprint(facility)?;
        for (placement, zone) in pair_candidates(sizes, facility, size, offset) {
            let (ax, ay) = anchors[zone as usize];
            let (ox, oy) = (placement.x + size / 2.0 - ax, placement.y + size / 2.0 - ay);
            vars.push((placement, problem.add_binary_var(ox.hypot(oy))));
            tagged.push((t, zone));
        }
    }
    if vars.is_empty() {
        return counts.iter().all(|zone| zone.iter().all(|n| *n == 0)).then(Default::default);
    }
    // Plots of a size lined up edge to edge, a whole side against a whole side, earn a reward, so
    // the nearest arrangement also comes out in rows and columns rather than staggered.
    for (i, (a, va)) in vars.iter().enumerate() {
        for (b, vb) in &vars[i + 1..] {
            let side = (a.size - b.size).abs() < EPS
                && (((a.x - b.x).abs() - a.size).abs() < EPS && (a.y - b.y).abs() < EPS
                    || ((a.y - b.y).abs() - a.size).abs() < EPS && (a.x - b.x).abs() < EPS);
            if side {
                let both = problem.add_var(-ALIGN_REWARD, (0.0, 1.0));
                problem.add_constraint(&[(both, 1.0), (*va, -1.0)], microlp::ComparisonOp::Le, 0.0);
                problem.add_constraint(&[(both, 1.0), (*vb, -1.0)], microlp::ComparisonOp::Le, 0.0);
            }
        }
    }
    add_cell_conflict_constraints(&mut problem, &vars, 1);
    for zone in [Zone::First, Zone::Both, Zone::Second] {
        for t in 0..types.len() {
            let terms: Vec<(microlp::Variable, f64)> = vars
                .iter()
                .zip(&tagged)
                .filter(|(_, tag)| **tag == (t, zone))
                .map(|((_, var), _)| (*var, 1.0))
                .collect();
            if terms.is_empty() {
                if counts[zone as usize][t] > 0 {
                    return None;
                }
                continue;
            }
            problem.add_constraint(&terms, microlp::ComparisonOp::Eq, counts[zone as usize][t] as f64);
        }
    }
    problem.set_time_limit(std::time::Duration::from_secs(2));
    let solution = problem.solve().ok()?;
    if !packing_is_sound(&vars, &solution) {
        return None;
    }
    let mut layouts: [Vec<Placement>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for (i, (placement, var)) in vars.iter().enumerate() {
        if solution[*var] > 0.5 {
            layouts[tagged[i].1 as usize].push(placement.clone());
        }
    }
    // The solver may stop on the time limit with the counts unmet; only a layout holding exactly
    // what the packing found is worth showing.
    for zone in 0..3 {
        if layouts[zone].len() as u32 != counts[zone].iter().sum::<u32>() {
            return None;
        }
    }
    Some(layouts)
}

/// Shared across threads: the geometry never changes, and working a pair out takes seconds, so
/// one thread's answer serves every other.
static PAIR_CACHE: std::sync::Mutex<Option<HashMap<(Vec<String>, (u64, u64), Offset), Vec<PairOption>>>> =
    std::sync::Mutex::new(None);

/// Every way two environment buildings can be placed so their coverage still overlaps: the second
/// sits `dx` tiles along and `dy` tiles up from the first, from touching side by side out to a
/// sliver of shared corner. Each one splits the plots differently between the three zones (for
/// Farmland, 6/24/6 side by side, 16/16/16 at (6,0), 22/10/22 at (8,0), and the diagonals again
/// differently), and none of them covers another, so all of them are offered to the plan.
///
/// Only `dx >= dy` is listed: the map has no orientation the plan cares about, so the mirrored
/// offset is the same pair turned on its side. Offsets where the buildings themselves would
/// overlap are left out, and beyond 8 tiles on either axis the coverage squares miss each other,
/// which is two separate buildings anyway.
pub fn pair_offsets(sizes: PairSizes) -> Vec<Offset> {
    // Two 9-wide squares still share ground at 8 tiles apart; at 9 they only touch along an edge,
    // which is no overlap at all and so just two separate buildings.
    let mut out = Vec::new();
    for dx in 0..=(2.0 * COVERAGE_RADIUS) as u32 {
        for dy in 0..=dx {
            let offset = Offset::new(dx, dy);
            // The buildings must not sit on each other, and their coverage must still meet.
            let (a, b) = pair_coverage(PairSizes { first: sizes.first, second: sizes.second }, offset);
            let on_top = (dx as f64) < sizes.first && (dy as f64) < sizes.first;
            if !on_top && a.overlaps(&b) {
                out.push(offset);
            }
        }
    }
    out
}

/// Ways to place two environment buildings at `offset` from each other, covering the trade-offs
/// between their three zones: the best packing when each zone in turn is what matters, at three
/// strengths, and with each type in turn favoured. Not every undominated mix, which would need a
/// search per count of every zone and type; a plan may therefore miss a slightly better split,
/// never an unachievable one.
pub fn pair_options_uncached(sizes: PairSizes, types: &[&str], offset: Offset) -> Vec<PairOption> {
    // How much each zone is worth, from "only this one" to "all three equally".
    let strengths: [[f64; 3]; 3] = [[0.01, 1.0, 6.0], [1.0, 1.0, 1.0], [0.01, 0.01, 1.0]];
    let mut weightings: Vec<Vec<[f64; 3]>> = Vec::new();
    for a in strengths[0] {
        for b in strengths[0] {
            for c in strengths[0] {
                // Every type pulled the same way, then each type in turn given the whole weight,
                // so a pair shared by Farmland and Woodland can lean either way.
                weightings.push(vec![[a, b, c]; types.len()]);
                if types.len() > 1 {
                    for t in 0..types.len() {
                        let mut weights = vec![[0.01; 3]; types.len()];
                        weights[t] = [a, b, c];
                        weightings.push(weights);
                    }
                }
            }
        }
    }
    let mut found: Vec<PairOption> = Vec::new();
    for weights in weightings {
        if let Some(option) = pair_layout(sizes, types, offset, &weights) {
            if !found.contains(&option) {
                found.push(option);
            }
        }
    }
    undominated(found)
}

/// Drops every option another one matches or beats in every zone and type.
pub fn undominated(found: Vec<PairOption>) -> Vec<PairOption> {
    let flat = |o: &PairOption| o.counts.concat();
    let mut unique: Vec<PairOption> = Vec::new();
    for option in found {
        if unique.iter().any(|u| flat(u).iter().zip(flat(&option)).all(|(x, y)| *x >= y)) {
            continue;
        }
        let mine = flat(&option);
        unique.retain(|u| !mine.iter().zip(flat(u)).all(|(x, y)| *x >= y));
        unique.push(option);
    }
    unique
}

/// Every pair packing worth offering, worked out ahead of time by the `bake_pair_coverage` test:
/// one line per option, `facility;facility, first, second, dx, dy, count;count;...`, where
/// `first` and `second` are the two buildings' footprints, `dx, dy` an offset that achieves the
/// counts, and the counts run first zone then shared then second in the line's own type order.
/// The geometry never changes, so working these out live would burn minutes of every page load.
const BAKED_PAIR_COVERAGE: &str = include_str!("../data/pair_coverage.csv");

type BakedKey = ((u64, u64), Vec<String>);

fn baked_pair_coverage() -> &'static HashMap<BakedKey, Vec<PairOption>> {
    static TABLE: std::sync::OnceLock<HashMap<BakedKey, Vec<PairOption>>> =
        std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table: HashMap<BakedKey, Vec<PairOption>> = HashMap::new();
        for line in BAKED_PAIR_COVERAGE.lines().skip(1) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut fields = line.split(',').map(str::trim);
            let (Some(types), Some(first), Some(second), Some(dx), Some(dy), Some(counts)) = (
                fields.next(),
                fields.next(),
                fields.next(),
                fields.next(),
                fields.next(),
                fields.next(),
            ) else {
                continue;
            };
            let types: Vec<String> = types.split(';').map(str::to_string).collect();
            let (Ok(first), Ok(second)) = (first.parse::<f64>(), second.parse::<f64>()) else { continue };
            let (Ok(dx), Ok(dy)) = (dx.parse(), dy.parse()) else { continue };
            let flat: Vec<u32> = counts.split(';').filter_map(|n| n.parse().ok()).collect();
            if flat.len() != types.len() * 3 {
                continue;
            }
            let offset = Offset::new(dx, dy);
            let counts = std::array::from_fn(|zone| flat[zone * types.len()..(zone + 1) * types.len()].to_vec());
            table.entry((PairSizes { first, second }.key(), types)).or_default().push(PairOption { offset, counts });
        }
        table
    })
}

/// Every way a pair can cover `types` that no other way matches or beats, over all of
/// [`pair_offsets`]. What a plan gets out of a pair is the counts in its three zones; where the
/// two buildings stand to produce them only decides what the diagram tells the player to do. So
/// an option one offset reaches and another matches is the same offer twice, and only one of them
/// is worth a column in the solve.
///
/// Offsets are read in order, and ties keep the first, so a pair the player can stand in a row
/// wins over the same coverage on a diagonal.
pub fn pair_options_over_offsets(sizes: PairSizes, types: &[&str]) -> Vec<PairOption> {
    let key = (sizes.key(), types.iter().map(|t| t.to_string()).collect::<Vec<_>>());
    if let Some(hit) = baked_pair_coverage().get(&key) {
        return hit.clone();
    }
    let mut all: Vec<PairOption> = Vec::new();
    for offset in pair_offsets(sizes) {
        for option in pair_options_uncached(sizes, types, offset) {
            if !all.iter().any(|o| o.counts == option.counts) {
                all.push(option);
            }
        }
    }
    undominated(all)
}

/// Ways to place two environment buildings at `offset` from each other; see
/// [`pair_options_uncached`]. Cached, since the geometry never changes.
pub fn pair_options(sizes: PairSizes, types: &[&str], offset: Offset) -> Vec<PairOption> {
    let key = (types.iter().map(|t| t.to_string()).collect::<Vec<_>>(), sizes.key(), offset);
    if let Ok(cache) = PAIR_CACHE.lock() {
        if let Some(hit) = cache.as_ref().and_then(|map| map.get(&key)).cloned() {
            return hit;
        }
    }
    let unique = pair_options_uncached(sizes, types, offset);
    if let Ok(mut cache) = PAIR_CACHE.lock() {
        cache.get_or_insert_with(HashMap::new).insert(key, unique.clone());
    }
    unique
}

thread_local! {
    static OPTION_CACHE: std::cell::RefCell<HashMap<(u64, Vec<String>), Vec<CoverageOption>>> =
        std::cell::RefCell::new(HashMap::new());
}

/// Every undominated way one environment building can cover a mix of `types` (e.g. "16 Farmland"
/// or "2 Woodland and 8 Farmland"): no option covers at least as many of every type as another
/// and more of one. The geometry never changes, so results are cached per type list.
///
/// Walks every combination of minimum counts for all but the last type and asks the packing ILP
/// for the most of the last type that still fits, so types should be ordered largest footprint
/// first (the last type has the most possible counts, and is the one never enumerated).
pub fn single_building_options(building: f64, types: &[&str]) -> Vec<CoverageOption> {
    let key = (building.to_bits(), types.iter().map(|t| t.to_string()).collect::<Vec<_>>());
    if let Some(hit) = OPTION_CACHE.with(|cache| cache.borrow().get(&key).cloned()) {
        return hit;
    }
    let mut found: Vec<CoverageOption> = Vec::new();
    if !types.is_empty() {
        let mut minimums = vec![0u32; types.len()];
        walk_minimums(building, types, &mut minimums, 0, &mut found);
    }
    let undominated: Vec<CoverageOption> = found
        .iter()
        .filter(|a| {
            !found.iter().any(|b| b.counts != a.counts && b.counts.iter().zip(&a.counts).all(|(x, y)| x >= y))
        })
        .cloned()
        .collect();
    let mut unique: Vec<CoverageOption> = Vec::new();
    for option in undominated {
        if !unique.iter().any(|u| u.counts == option.counts) {
            unique.push(option);
        }
    }
    OPTION_CACHE.with(|cache| cache.borrow_mut().insert(key, unique.clone()));
    unique
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every arrangement in the baked table is one that really fits: the counts it claims can be
    // laid out, on the tile grid, in the right zones, with no two plots on top of each other.
    // A packing ILP that stops on its time limit hands back a relaxation that reads as far more
    // than fits, so this is the check that the table was built from finished solves.
    #[test]
    fn baked_pair_options_really_fit() {
        let table = baked_pair_coverage();
        assert!(!table.is_empty(), "the baked pair coverage table is empty");
        let mut checked = 0;
        for ((key, types), options) in table {
            let sizes = PairSizes { first: f64::from_bits(key.0), second: f64::from_bits(key.1) };
            let types: Vec<&str> = types.iter().map(String::as_str).collect();
            // One in seven, spread over every type set: laying each one out is its own packing.
            for option in options.iter().step_by(7) {
                let laid = pair_layout_for(sizes, &types, option.offset, &option.counts)
                    .unwrap_or_else(|| panic!("{types:?} at {} can't hold {:?}", option.offset, option.counts));
                for (zone, plots) in laid.iter().enumerate() {
                    assert_eq!(plots.len() as u32, option.counts[zone].iter().sum::<u32>());
                }
                let all: Vec<&Placement> = laid.iter().flatten().collect();
                for (i, plot) in all.iter().enumerate() {
                    assert!(
                        all[i + 1..].iter().all(|other| !placements_overlap(plot, other)),
                        "{types:?} at {}: two plots overlap",
                        option.offset
                    );
                }
                checked += 1;
            }
        }
        println!("{checked} baked arrangements laid out");
    }

    // A tidied pair layout holds exactly what the packing found, keeps every plot in its own
    // zone, and never puts two on top of each other. Checked on a spread of offsets, straight
    // along and diagonal, rather than all of them: each one is a packing and they add up.
    #[test]
    fn tidy_pair_layouts_keep_their_zones() {
        let sizes = PairSizes::of("Heat Furnace", "Cooling Unit");
        let offsets = [Offset::new(2, 0), Offset::new(5, 0), Offset::new(8, 0), Offset::new(3, 3), Offset::new(6, 4)];
        for types in [&["Farmland"][..], &["Woodland"][..], &["Woodland", "Farmland"][..]] {
            for offset in offsets {
                for option in pair_options(sizes, types, offset) {
                    let total: u32 = option.counts.concat().iter().sum();
                    let Some(tidy) = tidy_pair_layout(sizes, types, offset, &option.counts) else {
                        // A packing this full has no room to tidy into; its own layout stands.
                        assert!(total > 12, "{types:?} at {offset} {:?} should tidy", option.counts);
                        continue;
                    };
                    let (a, b) = pair_coverage(sizes, offset);
                    let (dx, dy) = (offset.dx as f64, offset.dy as f64);
                    let buildings = [
                        building_rect(sizes.first),
                        Rect { x1: dx, y1: dy, x2: dx + sizes.second, y2: dy + sizes.second },
                    ];
                    let all: Vec<&Placement> = tidy.iter().flatten().collect();
                    for (zone, plots) in tidy.iter().enumerate() {
                        let wanted: u32 = option.counts[zone].iter().sum();
                        assert_eq!(plots.len() as u32, wanted, "{types:?} at {offset} zone {zone}");
                        for placement in plots {
                            let rect = placement.rect();
                            let expected = [(true, false), (true, true), (false, true)][zone];
                            assert_eq!((rect.overlaps(&a), rect.overlaps(&b)), expected, "{types:?} at {offset} zone {zone}: {placement:?}");
                            assert!(!buildings.iter().any(|building| rect.overlaps(building)), "{placement:?} on a building");
                            assert_eq!(placement.x, placement.x.round(), "{placement:?} off the tile grid");
                            assert_eq!(placement.y, placement.y.round(), "{placement:?} off the tile grid");
                        }
                    }
                    for (i, placement) in all.iter().enumerate() {
                        assert!(
                            all[i + 1..].iter().all(|other| !placements_overlap(placement, other)),
                            "{types:?} at {offset}: two plots overlap"
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;

    /// A joint ILP across every owned building's shared capacity AND a facility ownership cap is a
    /// packing+knapsack combinatorial cliff: it hangs once `owned` reaches ~10 with a
    /// `facility_owned` around 100, even though the same shape at owned=1..8 solves in ~150-190ms
    /// each, and a much looser cap of 1000 stays fast at any owned count (the cap only bites,
    /// combinatorially, once it's tight enough to force real trade-offs). `solve_building_packing`
    /// assigns buildings one at a time (`solve_one_building_layout`, always a fast capacity-1
    /// binary ILP) to sidestep this cliff. Runs on a background thread with a generous timeout so
    /// a regression fails loudly instead of hanging the whole test suite.
    #[test]
    fn building_packing_stays_fast_at_realistic_owned_counts() {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let weighted: Vec<(&str, &str, f64)> = vec![
                ("Farmland", "Cool", 1.0),
                ("Woodland", "Cool", 1.3),
                ("Starfall Hammock", "Cool", 0.8),
                ("Tidewhisper Sandcastle", "Cool", 1.6),
                ("Tidewhisper Sandcastle", "Freeze", 2.0),
            ];
            let facility_owned: Vec<(&str, u32)> = vec![
                ("Farmland", 100),
                ("Woodland", 100),
                ("Starfall Hammock", 100),
                ("Tidewhisper Sandcastle", 100),
            ];
            let start = std::time::Instant::now();
            let (mode_counts, placements, layouts) =
                solve_building_packing("Cooling Unit", &["Cool", "Freeze"], &weighted, 20, &facility_owned);
            let elapsed = start.elapsed();
            let _ = tx.send((elapsed, mode_counts, placements, layouts));
        });
        let (elapsed, mode_counts, placements, layouts) = rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("solve_building_packing hung at owned=20 -- the packing+knapsack cliff regressed");
        println!("owned=20 took {:?}, mode_counts={:?}", elapsed, mode_counts);
        assert!(elapsed.as_secs_f64() < 5.0, "took too long: {:?}", elapsed);
        let total_units: u32 = mode_counts.values().sum();
        assert!(total_units <= 20, "can't configure more than 20 owned Cooling Units, got {:?}", mode_counts);
        assert!(!placements.is_empty(), "expected some coverage to be assigned");
        let total_layouts: usize = layouts.values().map(|v| v.len()).sum();
        assert_eq!(total_layouts as u32, total_units, "one layout per assigned building instance");
    }

    /// Same scenario but with a much larger owned count and a tighter facility cap, well beyond
    /// what any real player is likely to reach; guards against the cliff reappearing at scale
    /// now that each individual building solve is O(1) work, not the whole batch's.
    #[test]
    fn building_packing_stays_fast_at_large_owned_count() {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let weighted: Vec<(&str, &str, f64)> = vec![
                ("Farmland", "Cool", 1.0),
                ("Woodland", "Cool", 1.3),
                ("Starfall Hammock", "Cool", 0.8),
                ("Tidewhisper Sandcastle", "Cool", 1.6),
                ("Tidewhisper Sandcastle", "Freeze", 2.0),
            ];
            let facility_owned: Vec<(&str, u32)> =
                vec![("Farmland", 50), ("Woodland", 30), ("Starfall Hammock", 20), ("Tidewhisper Sandcastle", 20)];
            let start = std::time::Instant::now();
            let (mode_counts, _placements, _layouts) =
                solve_building_packing("Cooling Unit", &["Cool", "Freeze"], &weighted, 60, &facility_owned);
            let elapsed = start.elapsed();
            let _ = tx.send((elapsed, mode_counts));
        });
        let (elapsed, mode_counts) = rx
            .recv_timeout(std::time::Duration::from_secs(20))
            .expect("solve_building_packing hung at owned=60");
        println!("owned=60 took {:?}, mode_counts={:?}", elapsed, mode_counts);
        assert!(elapsed.as_secs_f64() < 10.0, "took too long: {:?}", elapsed);
    }
}
