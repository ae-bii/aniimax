//! Exact 2D geometric packing for environment-building coverage (Heat Furnace/Cooling Unit/
//! Sunlamp).
//!
//! ## Coverage geometry
//! - Every environment building is a 2x2 footprint ([`BUILDING_SIZE`]).
//! - It radiates coverage as a square of side `2 * `[`COVERAGE_RADIUS`] centered on its own exact
//!   center (not its corner).
//! - A facility is covered only if its own footprint overlaps that coverage square by a real
//!   area; a corner-only touch is not enough. Since every footprint is snapped to the
//!   quarter-grid ([`GRID_STEP`]), any nonzero overlap between two quarter-grid-aligned rectangles
//!   is automatically at least 0.25x0.25, so this rule falls out for free from only ever
//!   generating candidate positions on that grid; no separate minimum-area check is needed.
//! - Facilities can't overlap the building itself or each other.
//! - Facility footprints: Farmland/Dewy House 2x2, Woodland 4x4, Starfall Hammock/Tidewhisper
//!   Sandcastle/Grass Blossom Mat 5x5; see [`ENVIRONMENT_GATED_FACILITIES`].
//!
//! ## Candidate generation is a bounded heuristic, not exhaustive
//!
//! Sweeping every quarter-grid position for every facility type would generate thousands of
//! candidates per type; intractable for the MILP branch & bound this feeds into (see
//! `crate::optimizer::solve_facility_allocation`). Instead, [`candidate_positions`] finds the
//! single best quarter-grid alignment for each facility type on its own, plus a handful of
//! half-space variants (restricting that same search to one half of the region, split through the
//! building's center) so the ILP has enough raw material to reconstruct mixed layouts when two or
//! more types share one building's coverage. Candidate **generation** is this bounded, tuned
//! subset; candidate **selection** among them (which is what actually decides how many of each
//! type get used) is exact integer optimization.

use std::collections::HashMap;

const EPS: f64 = 1e-6;

/// Environment building footprint (Heat Furnace/Cooling Unit/Sunlamp; all identical size).
pub const BUILDING_SIZE: f64 = 2.0;
/// Coverage radiates this far from the building's exact center in every direction, i.e. total
/// coverage span is `2 * COVERAGE_RADIUS` (a 9x9 square).
pub const COVERAGE_RADIUS: f64 = 4.5;
/// Facilities snap to this fine grid; also the smallest possible nonzero coverage overlap.
pub const GRID_STEP: f64 = 0.25;

/// Every facility type whose environment-gated items are capacity-bound by owned environment
/// buildings, with its fixed square footprint side length.
pub const ENVIRONMENT_GATED_FACILITIES: &[(&str, f64)] = &[
    ("Farmland", 2.0),
    ("Woodland", 4.0),
    ("Starfall Hammock", 5.0),
    ("Tidewhisper Sandcastle", 5.0),
    ("Grass Blossom Mat", 5.0),
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

fn building_rect() -> Rect {
    Rect { x1: 0.0, y1: 0.0, x2: BUILDING_SIZE, y2: BUILDING_SIZE }
}

fn coverage_rect() -> Rect {
    let c = BUILDING_SIZE / 2.0;
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

/// Finds the single best quarter-grid alignment (offset) for tiling `size`-square facilities
/// around the fixed building/coverage geometry, optionally restricted to one `half`, and returns
/// every valid position from that best alignment (valid = doesn't overlap the building, overlaps
/// the coverage square by positive area).
fn best_grid_positions(size: f64, half: Option<HalfSpace>) -> Vec<(f64, f64)> {
    let building = building_rect();
    let coverage = coverage_rect();
    let mut best: Vec<(f64, f64)> = Vec::new();

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
                    if rect.overlaps(&building) {
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
                best = positions;
            }
        }
    }
    best
}

/// Every candidate position worth offering the packing solver for one facility type: its own
/// best full grid, plus its best grids restricted to each half of the region (so the solver can
/// reconstruct "Hybrid"-style splits when sharing coverage with another type); deduplicated.
pub fn candidate_positions(size: f64) -> Vec<(f64, f64)> {
    let split = BUILDING_SIZE / 2.0; // the building's own center coordinate on each axis
    let mut all: Vec<(f64, f64)> = best_grid_positions(size, None);
    for half in [
        HalfSpace::Left(split),
        HalfSpace::Right(split),
        HalfSpace::Bottom(split),
        HalfSpace::Top(split),
    ] {
        for pos in best_grid_positions(size, Some(half)) {
            if !all.contains(&pos) {
                all.push(pos);
            }
        }
    }
    all
}

/// Every candidate [`Placement`] for one facility type; see [`candidate_positions`].
pub fn candidate_placements(facility: &str, size: f64) -> Vec<Placement> {
    candidate_positions(size)
        .into_iter()
        .map(|(x, y)| Placement { facility: facility.to_string(), x, y, size })
        .collect()
}

/// `true` if two placements' footprints overlap by positive area (so at most one of them,
/// across all K identical buildings sharing this coverage, can occupy that overlap at once).
pub fn placements_overlap(a: &Placement, b: &Placement) -> bool {
    a.rect().overlaps(&b.rect())
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
/// any single quarter-grid cell at once by `capacity`. This is a cell-based set-packing
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
pub fn solve_packing(weighted_types: &[(&str, f64)], building_count: u32) -> Option<PackingSolution> {
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
        for placement in candidate_placements(facility, size) {
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
        for placement in candidate_placements(facility, size) {
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
            let (layout, value) = solve_one_building_layout(entries, &remaining_owned);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn building_packing_with_multiple_modes_is_fast_and_correct() {
        // Cooling Unit sharing Cool + Freeze across 4 candidate facility types; mirrors a
        // realistic worst case from `crate::optimizer::solve_facility_allocation`'s decoupled
        // packing step. Confirms both speed (this hung for 20+ seconds when it was still combined
        // with the continuous item-rate LP in one MILP) and that mode capacity is respected.
        let weighted: Vec<(&str, &str, f64)> = vec![
            ("Farmland", "Cool", 1.0),
            ("Woodland", "Cool", 1.3),
            ("Starfall Hammock", "Cool", 0.8),
            ("Tidewhisper Sandcastle", "Cool", 1.6),
            ("Tidewhisper Sandcastle", "Freeze", 2.0),
        ];
        // Generous owned counts (well above what one building could ever cover) so this test
        // exercises the packing's own geometric limits, not the ownership cap.
        let facility_owned: Vec<(&str, u32)> =
            vec![("Farmland", 100), ("Woodland", 100), ("Starfall Hammock", 100), ("Tidewhisper Sandcastle", 100)];
        let start = std::time::Instant::now();
        let (mode_counts, placements, layouts) =
            solve_building_packing("Cooling Unit", &["Cool", "Freeze"], &weighted, 1, &facility_owned);
        let elapsed = start.elapsed();
        println!("building packing took {:?}, mode_counts={:?}", elapsed, mode_counts);
        assert!(elapsed.as_secs_f64() < 2.0, "took too long: {:?}", elapsed);

        let total_units: u32 = mode_counts.values().sum();
        assert!(total_units <= 1, "can't configure more than 1 owned Cooling Unit combined, got {:?}", mode_counts);
        assert!(!placements.is_empty(), "expected some coverage to be assigned");
        let total_layouts: usize = layouts.values().map(|v| v.len()).sum();
        assert_eq!(total_layouts as u32, total_units, "one layout per assigned building instance");
    }

    #[test]
    fn multi_type_sharing_gives_a_sensible_non_zero_split() {
        // Farmland + Starfall Hammock, both weighted so neither trivially dominates, one
        // building: confirm both types can actually get placed (not one starving the other).
        let solution = solve_packing(&[("Farmland", 1.0), ("Starfall Hammock", 1.0)], 1)
            .expect("packing should be feasible");
        let farmland = solution.covered.iter().find(|(n, _)| n == "Farmland").map(|(_, c)| *c).unwrap_or(0);
        let hammock = solution.covered.iter().find(|(n, _)| n == "Starfall Hammock").map(|(_, c)| *c).unwrap_or(0);
        assert!(farmland > 0, "expected some Farmland placed, got {:?}", solution.covered);
        assert!(hammock > 0, "expected some Starfall Hammock placed, got {:?}", solution.covered);
    }

    #[test]
    fn building_packing_layouts_match_aggregate_and_stay_non_overlapping() {
        let weighted: Vec<(&str, &str, f64)> = vec![
            ("Farmland", "Cool", 1.0),
            ("Woodland", "Cool", 1.3),
            ("Starfall Hammock", "Cool", 1.6),
        ];
        let facility_owned: Vec<(&str, u32)> = vec![("Farmland", 100), ("Woodland", 100), ("Starfall Hammock", 100)];
        let owned = 4;
        let (mode_counts, placements, layouts) =
            solve_building_packing("Cooling Unit", &["Cool"], &weighted, owned, &facility_owned);

        let total_units: u32 = mode_counts.values().sum();
        assert_eq!(total_units, owned, "every owned building should find some Cool coverage worth assigning");
        let cool_layouts = &layouts["Cool"];
        assert_eq!(cool_layouts.len() as u32, owned, "expected one layout per building instance");

        // Aggregate totals must match exactly: the per-building layouts are a partition of the
        // aggregate `placements`, not an approximation; nothing gained or lost when summing them.
        let mut recombined: Vec<(Placement, u32)> = Vec::new();
        for building in cool_layouts {
            for placement in building {
                match recombined.iter_mut().find(|(p, _)| p == placement) {
                    Some((_, c)) => *c += 1,
                    None => recombined.push((placement.clone(), 1)),
                }
            }
        }
        let total_recombined: u32 = recombined.iter().map(|(_, c)| c).sum();
        let total_aggregate: u32 = placements["Cool"].iter().map(|(_, c)| c).sum();
        assert_eq!(total_recombined, total_aggregate, "layouts should not lose or invent placements vs. the aggregate");

        // Every individual building's own layout must be genuinely non-overlapping.
        for building in cool_layouts {
            for i in 0..building.len() {
                for j in (i + 1)..building.len() {
                    assert!(
                        !placements_overlap(&building[i], &building[j]),
                        "a single building's layout has overlapping placements: {:?} / {:?}",
                        building[i],
                        building[j]
                    );
                }
            }
        }
    }

    #[test]
    fn farmland_alone_matches_hand_verified_geometry() {
        // Farmland's true max coverage around one building is 32 positions.
        let positions = candidate_positions(2.0);
        // best_grid_positions(None) alone (the unrestricted full grid) should already reach 32;
        // the half-space variants only ADD candidates, never remove them, so the full pool must
        // be at least 32.
        assert!(
            positions.len() >= 32,
            "expected at least 32 candidate Farmland positions, got {}",
            positions.len()
        );
    }

    #[test]
    fn woodland_alone_matches_expected_max() {
        let positions = best_grid_positions(4.0, None);
        assert_eq!(positions.len(), 12, "Woodland's unrestricted best grid should be exactly 12");
    }

    #[test]
    fn packing_solve_is_fast_enough_for_the_worst_realistic_case() {
        // Worst case: all 6 environment-gated types simultaneously profitable and needing the
        // same mode, with a handful of owned buildings.
        let weighted_types: Vec<(&str, f64)> = ENVIRONMENT_GATED_FACILITIES
            .iter()
            .enumerate()
            .map(|(i, &(name, _))| (name, 1.0 + i as f64 * 0.37)) // distinct weights, no ties
            .collect();

        let start = std::time::Instant::now();
        let solution = solve_packing(&weighted_types, 5).expect("packing should be feasible");
        let elapsed = start.elapsed();

        println!("packing solve took {:?}, covered = {:?}", elapsed, solution.covered);
        assert!(
            elapsed.as_secs_f64() < 1.0,
            "packing solve took too long: {:?} (target: well under 1s)",
            elapsed
        );
        assert!(!solution.covered.is_empty());
    }

    #[test]
    fn no_two_positions_from_the_same_best_grid_overlap() {
        for &(name, size) in ENVIRONMENT_GATED_FACILITIES {
            let placements = candidate_placements(name, size);
            // Spot-check the unrestricted grid specifically (a subset of `placements`, but built
            // fresh here so this test doesn't depend on `candidate_positions`'s internal order).
            let grid = best_grid_positions(size, None);
            for i in 0..grid.len() {
                for j in (i + 1)..grid.len() {
                    let a = Rect::new(grid[i].0, grid[i].1, size);
                    let b = Rect::new(grid[j].0, grid[j].1, size);
                    assert!(!a.overlaps(&b), "{name}'s own best grid has overlapping positions {:?}/{:?}", grid[i], grid[j]);
                }
            }
            assert!(!placements.is_empty(), "{name} should have at least one candidate placement");
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
