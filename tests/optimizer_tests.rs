//! Tests for production optimization algorithms.
//!
//! Scenarios use `FacilityCounts::only`, so any facility not listed is owned zero times. That
//! keeps each expected result tied to the facilities the test names, instead of shifting whenever
//! a new facility is added to the game data. Expected rates were worked out by hand from the CSVs;
//! the arithmetic is in the comments next to each assertion.

use aniimax::data::{load_all_data, load_aniimo_requirements};
use aniimax::models::{FacilityCounts, ModuleLevels, PlanStep, PlanStepStatus, ProductionPlan, Worker, Workers};
use aniimax::optimizer::{
    calculate_efficiencies, find_best_production_path, find_production_plan, time_to_reach_goal,
};
use std::path::Path;

fn default_facility_counts() -> FacilityCounts {
    FacilityCounts::only(&[
        ("Farmland", 1, 3),
        ("Woodland", 1, 3),
        ("Mine", 1, 3),
        ("Carousel Mill", 1, 3),
        ("Jukebox Dryer", 1, 3),
        ("Crafting Table", 1, 3),
    ])
}

fn default_module_levels() -> ModuleLevels {
    ModuleLevels::default()
}

fn steps_at<'a>(plan: &'a ProductionPlan, facility: &str) -> Vec<&'a PlanStep> {
    plan.coin_items.iter().filter(|s| s.facility == facility).collect()
}

fn count_of(plan: &ProductionPlan, facility: &str, item: &str) -> u32 {
    plan.coin_items
        .iter()
        .filter(|s| s.facility == facility && s.item_name.as_deref() == Some(item))
        .map(|s| s.facility_count)
        .sum()
}

fn produces(plan: &ProductionPlan, item: &str) -> bool {
    plan.coin_items
        .iter()
        .any(|s| s.item_name.as_deref() == Some(item) && s.status == PlanStepStatus::Producing)
}

fn wood_blocks_rate(plan: &ProductionPlan) -> f64 {
    plan.byproduct_rates.iter().filter(|(r, _, _)| r == "Wood Blocks").map(|(_, rate, _)| rate).sum()
}

fn assert_rate(plan: &ProductionPlan, expected: f64) {
    assert!(
        (plan.rate_per_second - expected).abs() < 1e-6,
        "expected {expected}/sec, got {}; plan: {:?}",
        plan.rate_per_second,
        plan.coin_items
    );
}

#[test]
fn test_calculate_efficiencies_coins() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }

    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = default_facility_counts();
    let modules = default_module_levels();

    let efficiencies = calculate_efficiencies(&items, "coins", &counts, &modules);

    assert!(!efficiencies.is_empty(), "Should find some coin-producing items");

    for eff in &efficiencies {
        assert_eq!(eff.item.sell_currency, "coins");
        assert!(eff.profit_per_second >= 0.0, "Profit per second should be non-negative");
        assert!(eff.total_time_per_unit > 0.0, "Total time should be positive");
    }
}

#[test]
fn test_calculate_efficiencies_filters_by_level() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }

    let items = load_all_data(data_dir).expect("Failed to load data");
    let modules = default_module_levels();

    // Level 1 only
    let counts_level_1 = FacilityCounts::only(&[
        ("Farmland", 1, 1),
        ("Woodland", 1, 1),
        ("Mine", 1, 1),
        ("Carousel Mill", 1, 1),
        ("Jukebox Dryer", 1, 1),
        ("Crafting Table", 1, 1),
    ]);

    // Level 3 for all
    let counts_level_3 = default_facility_counts();

    let eff_level_1 = calculate_efficiencies(&items, "coins", &counts_level_1, &modules);
    let eff_level_3 = calculate_efficiencies(&items, "coins", &counts_level_3, &modules);

    // Higher level should have at least as many options
    assert!(
        eff_level_3.len() >= eff_level_1.len(),
        "Higher level should unlock more or equal items"
    );

    // Level 1 efficiencies should only contain level 1 items
    for eff in &eff_level_1 {
        assert_eq!(
            eff.item.facility_level, 1,
            "Level 1 counts should only show level 1 items"
        );
    }
}

#[test]
fn test_find_best_production_path() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }

    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = default_facility_counts();
    let modules = default_module_levels();

    let efficiencies = calculate_efficiencies(&items, "coins", &counts, &modules);
    let path = find_best_production_path(&efficiencies, 1000.0, false, 0.0, &counts);

    assert!(path.is_some(), "Should find a production path");

    let path = path.unwrap();
    assert!(path.total_profit >= 1000.0, "Should meet target profit");
    assert!(path.total_time > 0.0, "Should have positive time");
    assert!(!path.steps.is_empty(), "Should have at least one step");
    assert_eq!(path.currency, "coins");
}

#[test]
fn test_find_best_production_path_energy_optimization() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }

    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = default_facility_counts();
    let modules = default_module_levels();

    let efficiencies = calculate_efficiencies(&items, "coins", &counts, &modules);

    // Time optimization
    let path_time = find_best_production_path(&efficiencies, 1000.0, false, 0.0, &counts);

    // Energy optimization
    let path_energy = find_best_production_path(&efficiencies, 1000.0, true, 0.0, &counts);

    assert!(path_time.is_some());
    assert!(path_energy.is_some());

    // Both should meet the target
    assert!(path_time.unwrap().total_profit >= 1000.0);
    assert!(path_energy.unwrap().total_profit >= 1000.0);
}

#[test]
fn test_empty_efficiencies() {
    let efficiencies = vec![];
    let counts = default_facility_counts();

    let path = find_best_production_path(&efficiencies, 1000.0, false, 0.0, &counts);

    assert!(path.is_none(), "Should return None for empty efficiencies");
}

#[test]
fn test_parallel_production_increases_efficiency() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }

    let items = load_all_data(data_dir).expect("Failed to load data");
    let modules = default_module_levels();

    // Single facility
    let counts_single = default_facility_counts();

    // Multiple facilities
    let counts_multi = FacilityCounts::only(&[
        ("Farmland", 4, 3),
        ("Woodland", 2, 3),
        ("Mine", 2, 3),
        ("Carousel Mill", 2, 3),
        ("Jukebox Dryer", 2, 3),
        ("Crafting Table", 2, 3),
    ]);

    let eff_single = calculate_efficiencies(&items, "coins", &counts_single, &modules);
    let eff_multi = calculate_efficiencies(&items, "coins", &counts_multi, &modules);

    let path_single =
        find_best_production_path(&eff_single, 5000.0, false, 0.0, &counts_single);
    let path_multi = find_best_production_path(&eff_multi, 5000.0, false, 0.0, &counts_multi);

    assert!(path_single.is_some());
    assert!(path_multi.is_some());

    // Multiple facilities should complete faster or equal
    assert!(
        path_multi.unwrap().total_time <= path_single.unwrap().total_time,
        "Multiple facilities should be faster or equal"
    );
}

// Regression tests locking in the LP-based allocation's core guarantees: grower capacity shared by
// sibling branches must not be double-counted, and idle processing capacity must be credited to a
// second item rather than left unused.

// bouquet (Crafting Table) needs 8 rose and 7 lavender, both grown on the same Farmland. Each is
// one full planting (1800s once watered), so a bouquet costs 3600 plot-seconds and 21 + 32 = 53 in seeds:
// 20 plots -> 20/3600 bouquets/sec x (1120 - 53) = 4.4458/sec, with a 10/10 split. If each branch
// assumed it had all 20 plots to itself, the rate would roughly double.
#[test]
fn test_find_coin_plan_shares_grower_capacity_across_branches() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[
        ("Farmland", 20, 5),
        ("Cooling Unit", 1, 1),
        ("Sunlamp", 1, 1),
        ("Crafting Table", 1, 4),
    ]);
    let plan = find_production_plan(&items, "coins", &counts, &ModuleLevels::default(), false)
        .expect("plan should be feasible");

    assert!(produces(&plan, "bouquet"), "bouquet should be produced, got: {:?}", plan.coin_items);
    assert_eq!(count_of(&plan, "Farmland", "rose"), 10);
    assert_eq!(count_of(&plan, "Farmland", "lavender"), 10);
    assert_rate(&plan, 20.0 / 3600.0 * (1120.0 - 53.0));

    let result = time_to_reach_goal(&plan, 1_000_000.0, 0.0).expect("goal should be reachable");
    let bouquet = result.products.iter().find(|p| p.item_name == "bouquet").unwrap();
    assert!((bouquet.rate_per_second - 20.0 / 3600.0 * (1120.0 - 53.0)).abs() < 1e-6);
}

// caramel_nut_chips needs nuts (chestnut + walnut) and maple_syrup, all three grown on Woodland;
// the plan must show every one of them, not just the first item found for that facility. Each
// caramel_nut_chips batch uses one full planting of each (7 chestnut, 6 walnut, 9 maple_syrup at
// 1800s once watered), so 12 plots split evenly 4/4/4: 12/5400 batches/sec x (3130 - 47 - 47 - 21) = 5.025.
#[test]
fn test_multi_ingredient_chain_shows_every_grower_item_it_needs() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[
        ("Woodland", 12, 4),
        ("Heat Furnace", 1, 1),
        ("Cooling Unit", 1, 1),
        ("Sunlamp", 1, 1),
        ("Jukebox Dryer", 1, 5),
        ("Claw Game Cooker", 1, 5),
    ]);
    let plan = find_production_plan(&items, "coins", &counts, &ModuleLevels::default(), false)
        .expect("plan should be feasible");

    assert!(produces(&plan, "caramel_nut_chips"), "got: {:?}", plan.coin_items);
    let woodland_steps = steps_at(&plan, "Woodland");
    for (name, reason) in [
        ("walnut", "Used for nuts"),
        ("chestnut", "Used for nuts"),
        ("maple_syrup", "Used for caramel_nut_chips"),
    ] {
        let step = woodland_steps
            .iter()
            .find(|s| s.item_name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("{name} should be grown for caramel_nut_chips, got: {woodland_steps:?}"));
        assert_eq!(step.reason, reason);
        assert_eq!(step.facility_count, 4, "expected an even 4/4/4 split, got: {woodland_steps:?}");
    }
    assert_rate(&plan, 12.0 / 5400.0 * (3130.0 - 47.0 - 47.0 - 21.0));
}

// A grown crop's row names what it's directly used for, not the chain's final product: quick_wheat
// is used for wheatmeal, and the Carousel Mill row says wheatmeal is used for premium_bread.
// well_water goes straight into premium_bread.
#[test]
fn test_grower_reason_names_the_intermediate_it_feeds() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[
        ("Farmland", 6, 2),
        ("Well", 1, 1),
        ("Carousel Mill", 1, 1),
        ("Claw Game Cooker", 1, 1),
    ]);
    let modules = ModuleLevels { ecological_module: 1, kitchen_module: 2, ..ModuleLevels::default() };
    let plan = find_production_plan(&items, "coins", &counts, &modules, false).expect("plan should be feasible");

    assert!(produces(&plan, "premium_bread"), "got: {:?}", plan.coin_items);
    // Farmland grows more wheat than one Well's water supports, so the Carousel Mill's leftover
    // wheatmeal sells directly; each row still names what it feeds first.
    let has_row = |facility: &str, item: &str, reason: &str| {
        plan.coin_items.iter().any(|s| {
            s.facility == facility && s.item_name.as_deref() == Some(item) && s.reason.starts_with(reason)
        })
    };
    assert!(has_row("Farmland", "quick_wheat", "Used for wheatmeal"), "got: {:?}", plan.coin_items);
    assert!(has_row("Carousel Mill", "wheatmeal", "Used for premium_bread"), "got: {:?}", plan.coin_items);
    assert!(has_row("Well", "well_water", "Used for premium_bread"), "got: {:?}", plan.coin_items);
}

// Two chains compete for the same Farmland and the same Jukebox Dryer: grape -> dried_grapes
// ((1210 - 56)/2400 per plot) and ginseng -> dried_ginseng ((1120 - 56)/2400 per plot). With a
// spare Dryer and a Cooling Unit available, ginseng could keep a token plot; it shouldn't. Grape
// takes all 12 plots: 12/2400 x 1154 = 5.77/sec.
#[test]
fn test_two_chains_sharing_a_grower_facility_settle_on_the_more_profitable_split() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[
        ("Farmland", 12, 6),
        ("Cooling Unit", 1, 1),
        ("Sunlamp", 1, 1),
        ("Jukebox Dryer", 2, 6),
    ]);
    let modules = ModuleLevels::default();

    let normal = find_production_plan(&items, "coins", &counts, &modules, false).expect("plan should be feasible");
    let prioritized = find_production_plan(&items, "coins", &counts, &modules, true).expect("plan should be feasible");
    assert!(
        normal.rate_per_second >= prioritized.rate_per_second - 0.001,
        "a byproduct floor can only lower or match the optimum: unconstrained {} vs prioritized {}",
        normal.rate_per_second,
        prioritized.rate_per_second
    );

    assert_eq!(count_of(&normal, "Farmland", "grape"), 12, "got: {:?}", steps_at(&normal, "Farmland"));
    assert!(
        !normal.coin_items.iter().any(|s| s.item_name.as_deref() == Some("ginseng")),
        "ginseng should be fully out-competed, not kept with a token plot"
    );
    assert_rate(&normal, 12.0 / 1800.0 * (1210.0 - 56.0));
}

// With only 4 Woodland plots, caramel_nut_chips can run 1/1/1 plus one leftover plot:
// 3/5400 x 3015 + 673/1800 (walnut on the 4th plot) = 1.537. Dropping the maple_syrup leg and
// selling nuts instead (2 chestnut + 2 walnut) is worth more: 4/3600 x (1980 - 94) = 1.5717.
// The Cooling Unit only matters for maple_syrup's Freeze, so once caramel_nut_chips is dropped it
// shouldn't be configured at all.
#[test]
fn test_single_chain_using_multiple_grower_items_settles_on_the_more_profitable_alternative() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[
        ("Woodland", 4, 4),
        ("Heat Furnace", 1, 1),
        ("Cooling Unit", 1, 1),
        ("Sunlamp", 1, 1),
        ("Jukebox Dryer", 1, 5),
        ("Claw Game Cooker", 1, 5),
    ]);
    let modules = ModuleLevels::default();

    let normal = find_production_plan(&items, "coins", &counts, &modules, false).expect("plan should be feasible");
    let prioritized = find_production_plan(&items, "coins", &counts, &modules, true).expect("plan should be feasible");
    assert!(normal.rate_per_second >= prioritized.rate_per_second - 0.001);

    assert!(
        !normal.coin_items.iter().any(|s| s.item_name.as_deref() == Some("caramel_nut_chips")),
        "caramel_nut_chips should be dropped in favor of selling nuts, got: {:?}",
        normal.coin_items
    );
    assert!(!normal.coin_items.iter().any(|s| s.item_name.as_deref() == Some("maple_syrup")));
    assert_eq!(count_of(&normal, "Woodland", "chestnut"), 2);
    assert_eq!(count_of(&normal, "Woodland", "walnut"), 2);
    assert_rate(&normal, 4.0 / 3600.0 * (1980.0 - 94.0));

    assert!(
        normal.environment_assignments.iter().all(|a| a.building != "Cooling Unit"),
        "no producing item needs Freeze, so the Cooling Unit should stay unconfigured, got: {:?}",
        normal.environment_assignments
    );
}

// A physical unit is set to one recipe and left there; it's never time-shared. One Jukebox Dryer
// has enough raw throughput for both dried_strawberries (20 Farmland) and dried_apple_slices
// (Woodland apples), so a fractional split would look feasible on paper. The plan has to pick one
// (strawberries, the far bigger stream) and send the apples somewhere else.
#[test]
fn test_find_coin_plan_processor_contention_dedicates_to_one_recipe_not_both() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[
        ("Farmland", 20, 5),
        ("Woodland", 10, 3),
        ("Cooling Unit", 1, 1),
        ("Jukebox Dryer", 1, 4),
    ]);
    let plan = find_production_plan(&items, "coins", &counts, &ModuleLevels::default(), false)
        .expect("plan should be feasible");

    let dryer_steps = steps_at(&plan, "Jukebox Dryer");
    assert!(
        dryer_steps.iter().all(|s| !s.reason.contains("% of the time")),
        "Jukebox Dryer should never describe a time-share percentage, got: {dryer_steps:?}"
    );
    let producing: Vec<&&PlanStep> =
        dryer_steps.iter().filter(|s| s.status == PlanStepStatus::Producing).collect();
    assert_eq!(producing.len(), 1, "the one Dryer should run exactly one recipe, got: {dryer_steps:?}");
    assert_eq!(producing[0].item_name.as_deref(), Some("dried_strawberries"));
    assert!(!produces(&plan, "dried_apple_slices"));

    // The losing chain's Woodland still produces something.
    let woodland_producing: u32 = steps_at(&plan, "Woodland")
        .iter()
        .filter(|s| s.status == PlanStepStatus::Producing)
        .map(|s| s.facility_count)
        .sum();
    assert_eq!(woodland_producing, 10, "got: {:?}", steps_at(&plan, "Woodland"));
}

// A processor with a single low-throughput contributor must report the units it actually needs,
// not every owned unit. 2 Woodland plots of bamboo feed well under one Crafting Table's worth of
// bamboo_ware, so 1 table produces and the other is idle. Rate: 2/2400 x (190 - 8) = 0.15167.
#[test]
fn test_find_coin_plan_solo_processor_contributor_reports_true_need_not_full_owned_count() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[("Woodland", 2, 2), ("Crafting Table", 2, 2)]);
    let plan = find_production_plan(&items, "coins", &counts, &ModuleLevels::default(), false)
        .expect("plan should be feasible");

    let table_steps = steps_at(&plan, "Crafting Table");
    let producing: Vec<&&PlanStep> =
        table_steps.iter().filter(|s| s.status == PlanStepStatus::Producing).collect();
    let idle: u32 = table_steps
        .iter()
        .filter(|s| s.status == PlanStepStatus::Idle)
        .map(|s| s.facility_count)
        .sum();
    assert_eq!(producing.len(), 1, "got: {table_steps:?}");
    assert_eq!(producing[0].item_name.as_deref(), Some("bamboo_ware"));
    assert_eq!(producing[0].facility_count, 1, "bamboo_ware needs only 1 of the 2 tables, got: {table_steps:?}");
    assert_eq!(idle, 1, "the other table should be idle, got: {table_steps:?}");
    assert_rate(&plan, 2.0 / 1800.0 * (190.0 - 8.0));
}

// Seeds needed: one seed per planting, so over the goal's total_time a grower plot needs
// ceil(total_time / cycle_time) plantings. Checked as an invariant against whatever crop the
// optimizer picks, so it doesn't depend on which crop wins.
#[test]
fn test_seed_requirements_match_ceil_of_total_time_over_cycle_time() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = default_facility_counts();
    let modules = default_module_levels();

    let plan = find_production_plan(&items, "coins", &counts, &modules, false).expect("plan should be feasible");
    let result = time_to_reach_goal(&plan, 1_000_000.0, 0.0).expect("goal should be reachable");

    assert!(
        !result.seed_requirements.is_empty(),
        "expected at least one grower crop to need seeds"
    );

    for req in &result.seed_requirements {
        let step = plan
            .coin_items
            .iter()
            .find(|s| s.facility == req.facility && s.item_name.as_deref() == Some(req.item_name.as_str()))
            .unwrap_or_else(|| panic!("no matching Producing PlanStep found for seed requirement {:?}", req));
        assert!(step.is_grower, "seed requirements should only ever cover grower facilities, got: {:?}", req);
        assert!(
            req.facility == "Farmland" || req.facility == "Woodland",
            "seeds only exist for Farmland/Woodland plots, got: {:?}",
            req
        );
        assert_eq!(step.status, PlanStepStatus::Producing);
        assert_eq!(step.facility_count, req.facility_count);

        let cycle_time = step.cycle_time.expect("a Producing grower row should always have a cycle_time");
        let expected_seeds_per_plot = (result.total_time / cycle_time).ceil() as u64;
        assert_eq!(
            req.seeds_per_plot, expected_seeds_per_plot,
            "seeds_per_plot should be ceil(total_time / cycle_time) for {:?}", req
        );
        assert_eq!(req.total_seeds, req.seeds_per_plot * req.facility_count as u64);
    }

    // No processor facility should ever appear; they aren't planted.
    let processor_names: Vec<&str> = plan
        .coin_items
        .iter()
        .filter(|s| !s.is_grower)
        .map(|s| s.facility.as_str())
        .collect();
    assert!(
        result.seed_requirements.iter().all(|r| !processor_names.contains(&r.facility.as_str())),
        "seed_requirements should never include a processor facility"
    );

    // Mine is a grower too (whole-unit rounding applies) but is mined, not planted. It's
    // producing in this scenario, so this is a real check rather than empty by construction.
    assert!(
        plan.coin_items.iter().any(|s| s.facility == "Mine" && s.status == PlanStepStatus::Producing),
        "expected Mine to be producing in this scenario"
    );
    assert!(
        result.seed_requirements.iter().all(|r| r.facility != "Mine"),
        "seed_requirements should never include Mine, got: {:?}",
        result.seed_requirements
    );
}

#[test]
fn test_find_coin_plan_infeasible_with_no_facilities() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[]);
    let plan = find_production_plan(&items, "coins", &counts, &ModuleLevels::default(), false);
    assert!(plan.is_none(), "no owned facilities should be infeasible");
}

// dried_bean_curd (Jukebox Dryer) is made from tofu, and tofu only comes from Carousel Mill. With
// Farmland and a Dryer owned but no Carousel Mill, dried_bean_curd must be unproducible: every
// facility a chain touches gets an LP constraint, including ones owned zero times, so an unowned
// intermediate facility can't act as an unlimited supply. The first plan (Carousel Mill owned)
// confirms the chain is otherwise the one the optimizer wants.
#[test]
fn test_find_coin_plan_never_produces_via_unowned_intermediate_facility() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let modules = ModuleLevels::default();

    let with_mill = FacilityCounts::only(&[("Farmland", 4, 3), ("Carousel Mill", 1, 3), ("Jukebox Dryer", 1, 3)]);
    let plan = find_production_plan(&items, "coins", &with_mill, &modules, false).expect("plan should be feasible");
    assert!(produces(&plan, "dried_bean_curd"), "got: {:?}", plan.coin_items);

    let without_mill = FacilityCounts::only(&[("Farmland", 4, 3), ("Jukebox Dryer", 1, 3)]);
    let plan = find_production_plan(&items, "coins", &without_mill, &modules, false)
        .expect("plan should still be feasible via some other item");
    let result = time_to_reach_goal(&plan, 100_000.0, 0.0).expect("goal should be reachable");
    for name in ["dried_bean_curd", "tofu"] {
        assert!(
            !plan.coin_items.iter().any(|s| s.item_name.as_deref() == Some(name)),
            "{name} must not appear with 0 Carousel Mill, got: {:?}",
            plan.coin_items
        );
        assert!(!result.products.iter().any(|p| p.item_name == name));
    }
}

#[test]
fn test_find_coin_plan_target_already_met() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = default_facility_counts();
    let modules = default_module_levels();

    let plan = find_production_plan(&items, "coins", &counts, &modules, false)
        .expect("plan should be feasible");
    let result = time_to_reach_goal(&plan, 1000.0, 5000.0)
        .expect("already-met target should be trivially feasible");
    assert_eq!(result.total_time, 0.0);
    assert_eq!(result.amount_produced, 0.0);
}

#[test]
fn test_production_plan_reports_candidates_evaluated_and_trial_solves() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[("Farmland", 5, 1), ("Mine", 1, 1)]);
    let plan = find_production_plan(&items, "coins", &counts, &ModuleLevels::default(), false)
        .expect("plan should be feasible");

    assert!(
        plan.candidates_evaluated > 0,
        "a feasible plan should have evaluated at least one candidate item"
    );
    assert!(
        plan.trial_solves > 0,
        "finding a plan always solves the facility-allocation LP at least once"
    );
}

// Growing-environment coverage: an environment-gated item's rate is capped by owned building
// coverage (a single Sunlamp covers at most 12 Woodland plots), not just by raw plot count.

// Level 4 Woodland unlocks palm_bark (Scorching), chestnut (Warm) and walnut (Adequate). With no
// environment buildings, none of them can be grown; all 14 plots fall back to an ungated item.
#[test]
fn test_environment_gated_item_unavailable_with_zero_matching_buildings() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[("Woodland", 14, 4)]);
    let plan = find_production_plan(&items, "coins", &counts, &ModuleLevels::default(), false)
        .expect("plan should be feasible");

    let woodland_steps = steps_at(&plan, "Woodland");
    assert!(
        woodland_steps.iter().all(|s| s.environment.is_none()),
        "no environment-gated item should be selected with 0 environment buildings owned, got: {woodland_steps:?}"
    );
    assert!(plan.environment_assignments.is_empty(), "got: {:?}", plan.environment_assignments);
    let total_producing: u32 = woodland_steps
        .iter()
        .filter(|s| s.status == PlanStepStatus::Producing)
        .map(|s| s.facility_count)
        .sum();
    assert_eq!(total_producing, 14, "all 14 Woodland plots should still find a non-gated fallback item");
}

// 14 Woodland plots but one Sunlamp: walnut (Adequate) caps at the Sunlamp's 12-plot Woodland
// coverage and the other 2 plots grow chestnut (Warm) uncovered, at 80% speed: 3000s a harvest
// rather than 2400s. 12 x (120 x 6 - 47)/2400 + 2 x (100 x 7 - 47)/3000.
#[test]
fn test_environment_gated_item_capped_by_single_building_coverage() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[("Woodland", 14, 4), ("Sunlamp", 1, 1)]);
    let plan = find_production_plan(&items, "coins", &counts, &ModuleLevels::default(), false)
        .expect("plan should be feasible");

    assert_eq!(count_of(&plan, "Woodland", "walnut"), 12, "got: {:?}", plan.coin_items);
    assert_eq!(count_of(&plan, "Woodland", "chestnut"), 2, "got: {:?}", plan.coin_items);
    assert_rate(&plan, 12.0 * 673.0 / 1800.0 + 2.0 * 653.0 / 2400.0);

    assert_eq!(plan.environment_assignments.len(), 1);
    let assignment = &plan.environment_assignments[0];
    assert_eq!(assignment.building, "Sunlamp");
    assert_eq!(assignment.mode, "Adequate");
    assert_eq!(assignment.units, 1);
    assert_eq!(assignment.covered, vec![("Woodland".to_string(), 12)]);
}

// At Woodland level 2 nothing needs an environment, so owned environment buildings stay
// unconfigured.
#[test]
fn test_environment_coverage_is_a_no_op_when_no_gated_item_is_unlocked() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[
        ("Woodland", 4, 2),
        ("Heat Furnace", 1, 1),
        ("Cooling Unit", 1, 1),
        ("Sunlamp", 1, 1),
    ]);
    let plan = find_production_plan(&items, "coins", &counts, &ModuleLevels::default(), false)
        .expect("plan should be feasible");
    assert!(
        plan.environment_assignments.is_empty(),
        "no environment building should be assigned when nothing needs one, got: {:?}",
        plan.environment_assignments
    );
}

// Wood Blocks / Mineral Sand as optimization targets: the LP maximizes byproduct output instead of
// profit. Byproduct per batch is fixed by facility level, so the best item is the highest-level
// one the player can actually grow.

// Level 3 Woodland with Ecological Module 2 and no environment buildings: cherry_blossom, apple
// and maple_syrup are gated, so quick_bamboo (21 Wood Blocks per 2400s) is the only level-3 option
// and beats bamboo/lemon (8). 10 plots -> 10 x 21/2400 = 0.0875/sec.
#[test]
fn test_wood_blocks_target_picks_the_best_byproduct_item() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[("Woodland", 10, 3), ("Farmland", 1, 3), ("Mine", 1, 3)]);
    let modules = ModuleLevels { ecological_module: 2, ..ModuleLevels::default() };
    let plan =
        find_production_plan(&items, "wood_blocks", &counts, &modules, false).expect("plan should be feasible");

    assert_eq!(plan.currency, "wood_blocks");
    let woodland_step = plan
        .coin_items
        .iter()
        .find(|s| s.facility == "Woodland")
        .expect("Woodland should appear in coin_items");
    assert_eq!(woodland_step.item_name.as_deref(), Some("quick_bamboo"));
    assert_eq!(woodland_step.facility_count, 10);
    assert_eq!(woodland_step.status, PlanStepStatus::Producing);

    for step in plan.coin_items.iter().filter(|s| s.facility != "Woodland") {
        assert_ne!(
            step.status,
            PlanStepStatus::Producing,
            "only Woodland should ever produce Wood Blocks, got a producing {:?}",
            step
        );
    }

    // The target IS the byproduct, so the passive byproduct_rates side channel stays empty.
    assert!(plan.byproduct_rates.is_empty(), "got: {:?}", plan.byproduct_rates);
    assert_rate(&plan, 10.0 * 21.0 / 1800.0);
}

// Same Sunlamp cap as above with Wood Blocks as the target: walnut (47 per batch) caps at 12
// plots and the last 2 grow uncovered chestnut, also 47 a batch but over 3000s.
#[test]
fn test_wood_blocks_target_respects_environment_coverage() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[("Woodland", 14, 4), ("Sunlamp", 1, 1), ("Mine", 1, 1)]);
    let plan = find_production_plan(&items, "wood_blocks", &counts, &ModuleLevels::default(), false)
        .expect("plan should be feasible");

    assert_eq!(count_of(&plan, "Woodland", "walnut"), 12, "got: {:?}", plan.coin_items);
    let total_woodland: u32 = steps_at(&plan, "Woodland")
        .iter()
        .filter(|s| s.status == PlanStepStatus::Producing)
        .map(|s| s.facility_count)
        .sum();
    assert_eq!(total_woodland, 14, "the remaining 2 plots should still produce something");
    assert!(plan.byproduct_rates.is_empty());
    assert_rate(&plan, 12.0 * 47.0 / 1800.0 + 2.0 * 47.0 / 2400.0);

    assert_eq!(plan.environment_assignments.len(), 1);
    assert_eq!(plan.environment_assignments[0].covered, vec![("Woodland".to_string(), 12)]);
}

// Mine level 4: copper_ore has the most Mineral Sand per workload (56 per 2700). With the default
// level-1 Aniimo (1 workload/sec), 5 Mines -> 5 x 56 / 2700.
#[test]
fn test_mineral_sand_target_picks_the_best_byproduct_item() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[("Mine", 5, 4), ("Farmland", 1, 3), ("Woodland", 1, 3)]);
    let plan = find_production_plan(&items, "mineral_sand", &counts, &ModuleLevels::default(), false)
        .expect("plan should be feasible");

    assert_eq!(plan.currency, "mineral_sand");
    let mine_step = plan
        .coin_items
        .iter()
        .find(|s| s.facility == "Mine")
        .expect("Mine should appear in coin_items");
    assert_eq!(mine_step.item_name.as_deref(), Some("copper_ore"));
    assert_eq!(mine_step.facility_count, 5);

    for step in plan.coin_items.iter().filter(|s| s.facility != "Mine") {
        assert_ne!(
            step.status,
            PlanStepStatus::Producing,
            "only Mine should ever produce Mineral Sand, got a producing {:?}",
            step
        );
    }
    assert!(plan.byproduct_rates.is_empty());
    assert_rate(&plan, 5.0 * 56.0 / 2700.0);
}

// The Aniimo working a facility sets its speed, relative to the level each recipe needs (see
// `models::efficiency`). Copper ore needs Earth level 3, so a level-3 Aniimo runs it at 100%, and
// the personality bonus makes that 120%. At a gathering facility a level-3 recipe also gets
// through 1.5 workload a second at 100% (`models::base_work_rate`), against the default's 1.
#[test]
fn test_aniimo_level_and_personality_bonus_speed_up_worked_facilities() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let mut items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[("Mine", 5, 4)]);
    let modules = ModuleLevels::default();

    let slow = find_production_plan(&items, "mineral_sand", &counts, &modules, false).expect("plan should be feasible");
    assert_rate(&slow, 5.0 * 56.0 / 2700.0);

    let mut workers = Workers::new();
    workers.set("Mine", Worker::new(3, true));
    workers.apply(&load_aniimo_requirements(data_dir).unwrap(), &mut items);
    let fast = find_production_plan(&items, "mineral_sand", &counts, &modules, false).expect("plan should be feasible");
    assert_rate(&fast, 5.0 * 56.0 * 1.2 * 1.5 / 2700.0);
}

// A faster Aniimo means fewer processor units for the same supply. 400 bamboo plots make
// 400 x 10/2400 = 1.667 bamboo/sec, enough for 0.1667 bamboo_ware/sec. At level 1 a Crafting Table
// finishes one 54-workload bamboo_ware every 54s, so all 5 tables stay busy. At level 3 (13.5s
// each) that supply needs 0.1667 x 13.5 = 2.25 tables' worth, so 3 tables run and 2 sit idle.
#[test]
fn test_faster_aniimo_needs_fewer_processor_units() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let mut items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[("Woodland", 400, 2), ("Crafting Table", 5, 2)]);
    let modules = ModuleLevels::default();

    let slow = find_production_plan(&items, "coins", &counts, &modules, false).expect("plan should be feasible");
    assert_eq!(count_of(&slow, "Crafting Table", "bamboo_ware"), 5, "got: {:?}", slow.coin_items);

    let mut workers = Workers::new();
    workers.set("Crafting Table", Worker::new(3, false));
    workers.apply(&load_aniimo_requirements(data_dir).unwrap(), &mut items);
    let fast = find_production_plan(&items, "coins", &counts, &modules, false).expect("plan should be feasible");
    assert_eq!(count_of(&fast, "Crafting Table", "bamboo_ware"), 3, "got: {:?}", fast.coin_items);
    assert_eq!(count_of(&fast, "Woodland", "bamboo"), 400, "got: {:?}", fast.coin_items);
    assert_rate(&fast, 400.0 / 1800.0 * (190.0 - 8.0));
}

#[test]
fn test_byproduct_target_goal_result_has_no_double_counted_byproducts() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[("Woodland", 10, 3), ("Farmland", 1, 3), ("Mine", 1, 3)]);
    let modules = ModuleLevels { ecological_module: 2, ..ModuleLevels::default() };
    let plan =
        find_production_plan(&items, "wood_blocks", &counts, &modules, false).expect("plan should be feasible");
    let result = time_to_reach_goal(&plan, 10_000.0, 0.0).expect("goal should be reachable");

    assert!(result.amount_produced > 0.0);
    assert!(
        result.byproducts.is_empty(),
        "GoalResult.byproducts should stay empty when the target IS the byproduct \
         (it would otherwise double-count amount_produced), got: {:?}",
        result.byproducts
    );
    assert_eq!(result.products.len(), 1);
    assert_eq!(result.products[0].item_name, "quick_bamboo");
}

// One Cooling Unit covers at most 32 Farmland plots, so 40 ginseng (Cool) needs 2 of the 3 owned
// units; the third stays unconfigured. 40 x (280 x 3 - 56)/2400 = 13.0667/sec.
#[test]
fn test_environment_coverage_uses_multiple_owned_buildings_when_one_is_not_enough() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[("Farmland", 40, 6), ("Cooling Unit", 3, 1)]);
    let plan = find_production_plan(&items, "coins", &counts, &ModuleLevels::default(), false)
        .expect("plan should be feasible");

    let farmland_steps = steps_at(&plan, "Farmland");
    assert_eq!(
        count_of(&plan, "Farmland", "ginseng"),
        40,
        "2 Cooling Units should cover all 40 ginseng plots, not cap at one unit's 32, got: {farmland_steps:?}"
    );
    assert!(!farmland_steps.iter().any(|s| s.status == PlanStepStatus::Idle));
    assert_rate(&plan, 40.0 * 784.0 / 1800.0);

    let cooling_units_used: u32 = plan
        .environment_assignments
        .iter()
        .filter(|a| a.building == "Cooling Unit")
        .map(|a| a.units)
        .sum();
    assert_eq!(cooling_units_used, 2, "expected exactly 2 Cooling Units to be configured");
}

// rice_drink (Carousel Mill) needs milled_rice, which is itself made at Carousel Mill. A unit
// can't switch recipes, so this takes two dedicated units: one making milled_rice, one making
// rice_drink, each shown as its own row.
// 3 rice plots -> 3 x 18/2400 rice/sec -> 0.000625 rice_drink/sec x (1860 - 2 x 12) = 1.1475.
// That needs 0.01 fresh_water/sec; 4 Wells with level-1 Aniimo make 4 x 8/2250 = 0.014222, and
// the rest sells: 0.004222 x 46 = 0.19422.
#[test]
fn test_processor_facility_dedicates_a_separate_unit_to_its_own_intermediate_step() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[("Farmland", 3, 3), ("Well", 4, 2), ("Carousel Mill", 2, 4)]);
    let plan = find_production_plan(&items, "coins", &counts, &ModuleLevels::default(), false)
        .expect("plan should be feasible");

    let mill_steps = steps_at(&plan, "Carousel Mill");
    let final_step = mill_steps
        .iter()
        .find(|s| s.item_name.as_deref() == Some("rice_drink"))
        .unwrap_or_else(|| panic!("rice_drink should be produced at Carousel Mill, got: {mill_steps:?}"));
    assert_eq!(final_step.reason, "Sells directly");
    assert_eq!(final_step.facility_count, 1);

    let intermediate_step = mill_steps
        .iter()
        .find(|s| s.item_name.as_deref() == Some("milled_rice"))
        .unwrap_or_else(|| panic!("milled_rice should get its own Carousel Mill row, got: {mill_steps:?}"));
    assert_eq!(intermediate_step.reason, "Used for rice_drink");
    assert_eq!(intermediate_step.facility_count, 1);

    assert_eq!(count_of(&plan, "Well", "fresh_water"), 4);
    // Each rice drink drinks 16 fresh water; the rest of the Wells' output sells.
    let drinks = 3.0 * 18.0 / 1800.0 / 18.0 / 2.0;
    let well_sale = (4.0 * 8.0 / 2250.0 - 16.0 * drinks) * 46.0;
    assert_rate(&plan, drinks * (1860.0 - 24.0) + well_sale);
}

// With only one Carousel Mill, rice_drink can't be made at all (it needs two dedicated units). The
// one Mill sells milled_rice directly instead: 3 x 18/2400 / 18 x (240 - 12) = 0.285/sec.
#[test]
fn test_two_hop_chain_is_infeasible_with_only_one_unit_of_its_shared_facility() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[("Farmland", 3, 3), ("Well", 4, 2), ("Carousel Mill", 1, 4)]);
    let plan = find_production_plan(&items, "coins", &counts, &ModuleLevels::default(), false)
        .expect("plan should be feasible");

    let mill_steps = steps_at(&plan, "Carousel Mill");
    assert!(
        mill_steps.iter().all(|s| s.item_name.as_deref() != Some("rice_drink")),
        "rice_drink needs two Carousel Mills and only one is owned, got: {mill_steps:?}"
    );
    let producing: Vec<&&PlanStep> =
        mill_steps.iter().filter(|s| s.status == PlanStepStatus::Producing).collect();
    assert_eq!(producing.len(), 1, "got: {mill_steps:?}");
    assert_eq!(producing[0].item_name.as_deref(), Some("milled_rice"));
    assert_eq!(producing[0].reason, "Sells directly");
    assert_eq!(producing[0].facility_count, 1);

    let well_sale = 4.0 * 8.0 / 2250.0 * 46.0;
    assert_rate(&plan, 3.0 * 18.0 / 1800.0 / 18.0 * (240.0 - 12.0) + well_sale);
}

// A player often upgrades some but not all plots. 5 Farmland at level 3 plus 4 at level 6:
// ginseng (level 6, Cool) can only use the 4 level-6 plots; the level-3 plots grow rice.
// 4 x 784/2400 + 5 x (10 x 18 - 12)/2400 = 1.30667 + 0.35.
#[test]
fn test_mixed_level_tiers_split_capacity_by_what_each_tier_can_actually_run() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let mut counts = FacilityCounts::only(&[("Cooling Unit", 3, 1)]);
    counts.add_tier("Farmland", 5, 3);
    counts.add_tier("Farmland", 4, 6);

    let plan = find_production_plan(&items, "coins", &counts, &ModuleLevels::default(), false)
        .expect("plan should be feasible");

    assert_eq!(
        count_of(&plan, "Farmland", "ginseng"),
        4,
        "ginseng needs level 6, so only the 4 level-6 plots qualify, got: {:?}",
        plan.coin_items
    );
    assert_eq!(count_of(&plan, "Farmland", "rice"), 5, "got: {:?}", plan.coin_items);
    let farmland_total: u32 = steps_at(&plan, "Farmland").iter().map(|s| s.facility_count).sum();
    assert_eq!(farmland_total, 9);
    assert_rate(&plan, 4.0 * 784.0 / 1800.0 + 5.0 * 168.0 / 1800.0);
}

// `prioritize_byproducts` puts a floor on Wood Blocks output before maximizing coins. The
// coin-optimal plan here is caramel_nut_chips (see the multi-ingredient test above), which puts 4
// plots on level-3 maple_syrup (21 Wood Blocks per batch) instead of level-4 trees (47):
// (8 x 47 + 4 x 21)/2400 = 0.19167 vs the 12 x 47/2400 = 0.235 maximum. Prioritizing drops
// maple_syrup and sells nuts from 6 chestnut + 6 walnut: 12/4800 x 1886 = 4.715 coins/sec,
// down from 5.025.
#[test]
fn test_prioritize_byproducts_forces_max_wood_blocks_rate_at_a_real_coin_cost() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[
        ("Woodland", 12, 4),
        ("Heat Furnace", 1, 1),
        ("Cooling Unit", 1, 1),
        ("Sunlamp", 1, 1),
        ("Jukebox Dryer", 1, 5),
        ("Claw Game Cooker", 1, 5),
    ]);
    let modules = ModuleLevels::default();

    let max_wood_blocks_rate = find_production_plan(&items, "wood_blocks", &counts, &modules, false)
        .expect("wood_blocks plan should be feasible")
        .rate_per_second;
    assert!((max_wood_blocks_rate - 12.0 * 47.0 / 1800.0).abs() < 1e-9);

    let normal_plan = find_production_plan(&items, "coins", &counts, &modules, false).expect("plan should be feasible");
    assert!((wood_blocks_rate(&normal_plan) - (8.0 * 47.0 + 4.0 * 21.0) / 1800.0).abs() < 1e-9);
    assert_rate(&normal_plan, 12.0 / 5400.0 * 3015.0);

    let prioritized_plan =
        find_production_plan(&items, "coins", &counts, &modules, true).expect("prioritized plan should be feasible");
    assert!(
        (wood_blocks_rate(&prioritized_plan) - max_wood_blocks_rate).abs() < 1e-9,
        "prioritized Wood Blocks ({}) should hit the max ({max_wood_blocks_rate})",
        wood_blocks_rate(&prioritized_plan)
    );
    assert!(!prioritized_plan.coin_items.iter().any(|s| s.item_name.as_deref() == Some("maple_syrup")));
    assert_rate(&prioritized_plan, 12.0 / 3600.0 * 1886.0);
}

// The Wood Blocks floor must reflect what walnut can actually get once coin-priced candidates
// have their share of a contested building. One Sunlamp is wanted by both lavender (Farmland, for
// lavender_powder) and walnut (Woodland). Pricing the floor off an isolated byproduct-only solve
// would hand walnut the whole Sunlamp and could make the real solve infeasible. The guarantees:
// prioritizing never turns a feasible plan infeasible, never lowers Wood Blocks output, and never
// raises the coin rate above the plain profit-maximizing solve.
#[test]
fn test_prioritize_byproducts_remains_feasible_and_does_not_reduce_byproduct_output_when_coverage_is_contested() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[
        ("Farmland", 28, 5),
        ("Woodland", 14, 4),
        ("Sunlamp", 1, 1),
        ("Carousel Mill", 1, 3),
    ]);
    let modules = ModuleLevels::default();

    let unprioritized =
        find_production_plan(&items, "coins", &counts, &modules, false).expect("plan should be feasible");
    // The one Mill turns out a lavender_powder every 68s, which is all 28 watered plots can keep
    // up with bar a little lavender that sells as it is; walnut takes the 4 Woodland plots the
    // Sunlamp still reaches, and uncovered chestnut the other 10 at 80% speed.
    let powder = 1.0 / 68.0;
    let lavender = 28.0 * 7.0 / 1800.0;
    let expected = powder * 650.0 + (lavender - 7.0 * powder) * 69.0 - 28.0 * 32.0 / 1800.0
        + 4.0 * (6.0 * 120.0 - 47.0) / 1800.0
        + 10.0 * (7.0 * 100.0 - 47.0) / 2400.0;
    assert!(
        (unprioritized.rate_per_second - expected).abs() < 1e-4,
        "expected {expected}/sec, got {}",
        unprioritized.rate_per_second
    );

    let prioritized = find_production_plan(&items, "coins", &counts, &modules, true)
        .expect("prioritizing byproducts should never turn a feasible plan into a reported failure");
    assert!(
        wood_blocks_rate(&prioritized) >= wood_blocks_rate(&unprioritized) - 1e-9,
        "got unprioritized={} prioritized={}",
        wood_blocks_rate(&unprioritized),
        wood_blocks_rate(&prioritized)
    );
    assert!(
        prioritized.rate_per_second <= unprioritized.rate_per_second + 1e-9,
        "got prioritized={} unprioritized={}",
        prioritized.rate_per_second,
        unprioritized.rate_per_second
    );
}

// When the target is already a byproduct, prioritizing is a no-op.
#[test]
fn test_prioritize_byproducts_is_a_no_op_when_targeting_a_byproduct_directly() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[("Woodland", 12, 4)]);
    let modules = ModuleLevels::default();

    let plan_a = find_production_plan(&items, "wood_blocks", &counts, &modules, false).expect("plan should be feasible");
    let plan_b = find_production_plan(&items, "wood_blocks", &counts, &modules, true).expect("plan should be feasible");
    assert_eq!(plan_a.rate_per_second, plan_b.rate_per_second);
}

/// A config with many environment buildings and processors. Guards the coverage solver's
/// single-building ILP against slow branch & bound on tied placements (see
/// `crate::coverage::solve_one_building_layout`). Runs on a background thread with a timeout so
/// a regression fails instead of hanging the suite.
#[test]
fn test_large_multi_facility_config_stays_fast() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let items = load_all_data(data_dir).expect("Failed to load data");
        let counts = FacilityCounts::only(&[
            ("Farmland", 5, 4),
            ("Woodland", 2, 4),
            ("Mine", 2, 4),
            ("Heat Furnace", 7, 4),
            ("Cooling Unit", 9, 4),
            ("Sunlamp", 3, 4),
            ("Crafting Table", 6, 4),
            ("Claw Game Cooker", 9, 4),
            ("Jukebox Dryer", 3, 4),
        ]);
        let modules = ModuleLevels {
            ecological_module: 2,
            kitchen_module: 2,
            resource_detector: 1,
            crafting_module: 1,
        };
        let start = std::time::Instant::now();
        let plan = find_production_plan(&items, "coins", &counts, &modules, false);
        let elapsed = start.elapsed();
        let _ = tx.send((elapsed, plan.is_some()));
    });
    let (elapsed, found_plan) = rx
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("find_production_plan hung on a large multi-facility-type config");
    println!("large multi-facility config took {:?}, found_plan={}", elapsed, found_plan);
    assert!(found_plan, "expected a feasible plan for this facility config");
    assert!(elapsed.as_secs_f64() < 2.0, "took too long: {:?} (target: well under 1s)", elapsed);
}

// With plenty of bamboo, bamboo_ware is the best use of every Crafting Table, so it should claim
// all 5 (not 1): a single item's whole-unit need is reported directly, not divided by owned count.
// 400 Woodland supply more bamboo than 5 tables can use; the surplus is sold raw.
#[test]
fn test_single_dominant_processor_item_claims_every_owned_unit() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[("Woodland", 400, 2), ("Crafting Table", 5, 2)]);
    let plan = find_production_plan(&items, "coins", &counts, &ModuleLevels::default(), false)
        .expect("plan should be feasible");

    let table_steps = steps_at(&plan, "Crafting Table");
    assert_eq!(count_of(&plan, "Crafting Table", "bamboo_ware"), 5, "got: {table_steps:?}");
    assert!(table_steps.iter().all(|s| s.status != PlanStepStatus::Idle), "got: {table_steps:?}");
}

// One Sunlamp and one Cooling Unit. Grape (Adequate) -> dried_grapes and walnut (Adequate) can
// share the single Sunlamp, leaving the Cooling Unit unused:
// 12/2400 x (1210 - 56) + 6 x 673/2400 = 5.77 + 1.6825 = 7.4525.
// Splitting instead (ginseng under the Cooling Unit -> dried_ginseng, walnut under the Sunlamp)
// only reaches 12/2400 x (1120 - 56) + 1.6825 = 7.0025. The coverage choice should find the
// shared layout rather than settle for one building per crop.
#[test]
fn test_environment_coverage_choice_does_not_settle_for_a_worse_joint_split() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = FacilityCounts::only(&[
        ("Farmland", 12, 6),
        ("Woodland", 6, 4),
        ("Cooling Unit", 1, 1),
        ("Sunlamp", 1, 1),
        ("Jukebox Dryer", 1, 6),
    ]);
    let plan = find_production_plan(&items, "coins", &counts, &ModuleLevels::default(), false)
        .expect("plan should be feasible");

    assert_eq!(count_of(&plan, "Farmland", "grape"), 12, "got: {:?}", plan.coin_items);
    assert_eq!(count_of(&plan, "Woodland", "walnut"), 6, "got: {:?}", plan.coin_items);
    assert!(produces(&plan, "dried_grapes"));
    assert!(!plan.coin_items.iter().any(|s| s.item_name.as_deref() == Some("ginseng")));
    assert_rate(&plan, 12.0 / 1800.0 * 1154.0 + 6.0 * 673.0 / 1800.0);

    assert_eq!(plan.environment_assignments.len(), 1, "got: {:?}", plan.environment_assignments);
    let sunlamp = &plan.environment_assignments[0];
    assert_eq!(sunlamp.building, "Sunlamp");
    let mut covered = sunlamp.covered.clone();
    covered.sort();
    assert_eq!(covered, vec![("Farmland".to_string(), 12), ("Woodland".to_string(), 6)]);
}

// More Farmland should never lower the best achievable rate; the coverage-choice and exclusion
// passes in `find_production_plan` exist largely to keep this true. Same contested setup as the
// test above, stepping Farmland past it.
#[test]
fn test_more_farmland_never_lowers_the_rate_with_contested_coverage() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");
    let modules = ModuleLevels::default();
    let mut previous = 0.0;
    for farmland in 12..=14 {
        let counts = FacilityCounts::only(&[
            ("Farmland", farmland, 6),
            ("Woodland", 6, 4),
            ("Cooling Unit", 1, 1),
            ("Sunlamp", 1, 1),
            ("Jukebox Dryer", 1, 6),
        ]);
        let plan = find_production_plan(&items, "coins", &counts, &modules, false)
            .expect("plan should be feasible");
        assert!(
            plan.rate_per_second >= previous - 1e-9,
            "Farmland={farmland} gave {} coins/sec, less than {previous} with one fewer plot",
            plan.rate_per_second
        );
        previous = plan.rate_per_second;
    }
}

// Regression: with a level-3 Aniimo and the personality bonus everywhere (the "Best" setup),
// prioritizing byproducts made this plan disappear entirely. The Mineral Sand floor equals the
// exact maximum a separate solve found, and floating-point noise (or an exclusion pass removing
// the chain that carried it) left the floored LP infeasible. Prioritizing must never turn a
// feasible plan into no plan: it now keeps a hair of slack and falls back to the unfloored solve.
#[test]
fn test_prioritize_byproducts_with_best_aniimo_still_finds_a_plan() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let mut items = load_all_data(data_dir).expect("Failed to load data");
    let reqs = aniimax::data::load_aniimo_requirements(data_dir).expect("Failed to load requirements");
    reqs.apply(aniimax::models::AniimoSetup::Best(aniimax::models::MAX_ANIIMO_LEVEL), &mut items);
    let counts = FacilityCounts::only(&[
        ("Farmland", 10, 5),
        ("Mine", 2, 3),
        ("Crafting Table", 1, 4),
        ("Joy Wheel Loom", 1, 1),
    ]);
    let modules = ModuleLevels { ecological_module: 8, kitchen_module: 7, resource_detector: 8, crafting_module: 7 };

    let normal = find_production_plan(&items, "coins", &counts, &modules, false).expect("plan should be feasible");
    let prioritized = find_production_plan(&items, "coins", &counts, &modules, true)
        .expect("prioritizing byproducts should never remove a feasible plan");
    assert!(prioritized.rate_per_second <= normal.rate_per_second + 1e-9);
}
