//! Tests for the exact planner (`aniimax::exact`). Each scenario checks that the plan is proven
//! optimal, passes the independent re-check, never earns less than the heuristic planner, and,
//! where worked out by hand, earns exactly what the arithmetic in the comments says.

use aniimax::data::{load_all_data, load_aniimo_requirements};
use aniimax::exact::{check_plan, net_rates, solve_exact, to_production_plan, ExactPlan, Goal, LevelUp, PACE_UNIT};
use aniimax::models::{AniimoSetup, FacilityCounts, ModuleLevels, ProductionItem};
use aniimax::optimizer::find_production_plan;
use std::path::Path;
use std::time::Duration;

fn load_items() -> Option<Vec<ProductionItem>> {
    let data_dir = Path::new("data");
    data_dir.exists().then(|| load_all_data(data_dir).expect("Failed to load data"))
}

/// Solves `counts`/`modules` exactly and checks what every scenario should hold.
fn solve_and_check(items: &[ProductionItem], counts: &FacilityCounts, modules: &ModuleLevels) -> ExactPlan {
    let plan = solve_exact(items, "coins", counts, modules, Goal::Earn { floors: &[] }, Some(Duration::from_secs(60)), None)
        .expect("exact plan");
    assert!(plan.proven_optimal, "not proven optimal: {plan:?}");
    let recomputed = check_plan(&plan, items, "coins", counts, modules, None).expect("plan passes its re-check");
    assert!((recomputed - plan.rate_per_second).abs() < 1e-6);
    let heuristic = find_production_plan(items, "coins", counts, modules, false).map_or(0.0, |p| p.rate_per_second);
    assert!(
        plan.rate_per_second >= heuristic - 1e-6,
        "exact {} below heuristic {heuristic}",
        plan.rate_per_second
    );
    plan
}

// Six Farmland at level 2 with nothing else: wheat (5 per 240s, sells for 1) earns 6 x 5/240 =
// 0.125/sec; potato (2 per 640s, sells for 8, seed 1) earns 6 x 15/640 = 0.1406; quick_wheat needs
// Ecological Module 1, which isn't owned. Potato wins.
#[test]
fn exact_picks_the_best_crop_on_its_own() {
    let Some(items) = load_items() else { return };
    let counts = FacilityCounts::only(&[("Farmland", 6, 2)]);
    let plan = solve_and_check(&items, &counts, &ModuleLevels::default());
    assert!((plan.rate_per_second - 6.0 * 15.0 / 640.0).abs() < 1e-9, "got {}", plan.rate_per_second);
    assert_eq!(plan.units.get("potato"), Some(&6));
}

// rice_drink (Carousel Mill) needs milled_rice made at a Carousel Mill too; with two Mills, one
// makes each. 3 rice plots -> 0.000625 rice_drink/sec x (1860 - 2 x 12 seed) = 1.1475; 4 Wells with
// level-1 Aniimo make 0.014222 fresh_water/sec, rice_drink uses 0.01 and the rest sells:
// 0.004222 x 46 = 0.19422. Total 1.34172.
#[test]
fn exact_matches_the_hand_worked_rice_drink_plan() {
    let Some(items) = load_items() else { return };
    let counts = FacilityCounts::only(&[("Farmland", 3, 3), ("Well", 4, 2), ("Carousel Mill", 2, 4)]);
    let plan = solve_and_check(&items, &counts, &ModuleLevels::default());
    let expected = 3.0 * 18.0 / 2400.0 / 18.0 / 2.0 * (1860.0 - 24.0) + (4.0 * 8.0 / 2250.0 - 0.01) * 46.0;
    assert!((plan.rate_per_second - expected).abs() < 1e-6, "got {}, expected {expected}", plan.rate_per_second);
    assert_eq!(plan.units.get("rice_drink"), Some(&1));
    assert_eq!(plan.units.get("milled_rice"), Some(&1));
}

// A mid-game setup with an Aniimo setup applied and a Cooling Unit, so environment coverage and
// whole processor units both matter: the exact plan is proven, re-checks, and beats or ties the
// heuristic planner; its rows cover every owned unit exactly once.
#[test]
fn exact_handles_environments_and_aniimo_speeds() {
    let Some(mut items) = load_items() else { return };
    load_aniimo_requirements(Path::new("data")).unwrap().apply(AniimoSetup::Best, &mut items);
    let counts = FacilityCounts::only(&[
        ("Farmland", 14, 4),
        ("Woodland", 8, 3),
        ("Mine", 4, 2),
        ("Well", 1, 1),
        ("Cooling Unit", 1, 1),
        ("Carousel Mill", 1, 2),
        ("Crafting Table", 1, 3),
        ("Jukebox Dryer", 1, 3),
        ("Claw Game Cooker", 1, 3),
        ("Phonolfactory Table", 1, 2),
        ("Dewy House", 1, 1),
        ("Tidewhisper Sandcastle", 1, 1),
    ]);
    let modules = ModuleLevels { ecological_module: 2, kitchen_module: 2, resource_detector: 1, crafting_module: 2 };
    let plan = solve_and_check(&items, &counts, &modules);
    let shown = to_production_plan(&plan, &items, "coins", &counts);
    for facility in ["Farmland", "Woodland", "Mine", "Crafting Table", "Jukebox Dryer"] {
        let rows: u32 = shown.coin_items.iter().filter(|s| s.facility == facility).map(|s| s.facility_count).sum();
        assert_eq!(rows, counts.get_count(facility), "{facility} rows: {:?}", shown.coin_items);
    }
    let streams: f64 = shown.income_streams.iter().map(|s| s.rate_per_second).sum();
    assert!((streams - plan.rate_per_second).abs() < 1e-6, "income streams add to {streams}");
}

// "Prioritize byproducts": the most Wood Blocks the Woodland can make is found first, and the
// earning plan must still make that much.
#[test]
fn exact_keeps_prioritized_byproducts_at_their_maximum() {
    let Some(items) = load_items() else { return };
    let counts = FacilityCounts::only(&[("Woodland", 4, 2), ("Jukebox Dryer", 1, 2)]);
    let modules = ModuleLevels::default();
    let most = solve_exact(&items, "coins", &counts, &modules, Goal::MostOf("Wood Blocks"), None, None).unwrap();
    assert!(most.proven_optimal && most.objective > 0.0);
    let floors = vec![("Wood Blocks".to_string(), most.objective)];
    let plan = solve_exact(&items, "coins", &counts, &modules, Goal::Earn { floors: &floors }, None, None).unwrap();
    assert!(plan.proven_optimal);
    let made: f64 = plan
        .recipe_rates
        .iter()
        .filter_map(|(name, rate)| {
            let item = items.iter().find(|i| &i.name == name)?;
            item.byproduct.as_ref().filter(|(r, _)| r == "Wood Blocks").map(|(_, amount)| rate * *amount as f64)
        })
        .sum();
    assert!(made >= most.objective * (1.0 - 1e-5), "made {made}, most {}", most.objective);
    let unfloored = solve_exact(&items, "coins", &counts, &modules, Goal::Earn { floors: &[] }, None, None).unwrap();
    assert!(plan.rate_per_second <= unfloored.rate_per_second + 1e-9);
}

fn level_up(cost: &[(&str, f64)], stock: &[(&str, f64)]) -> LevelUp {
    let list = |l: &[(&str, f64)]| l.iter().map(|(n, a)| (n.to_string(), *a)).collect();
    LevelUp { cost: list(cost), stock: list(stock) }
}

/// Solves a level-up in both stages and checks the plan: the soonest level-up, then the most
/// coins at that pace. Returns the plan and the level-up time in seconds.
fn solve_level_up(items: &[ProductionItem], counts: &FacilityCounts, level_up: &LevelUp) -> (ExactPlan, f64) {
    let modules = ModuleLevels::default();
    let fastest = solve_exact(items, "coins", counts, &modules, Goal::LevelUp(level_up), None, None).expect("level-up plan");
    assert!(fastest.proven_optimal && fastest.objective > 0.0);
    let pace = fastest.objective;
    let plan = solve_exact(items, "coins", counts, &modules, Goal::EarnWhileLevelingUp(level_up, pace), None, None)
        .expect("earning plan");
    assert!(plan.proven_optimal);
    assert!(plan.pace.unwrap() >= pace * (1.0 - 2e-4), "pace {:?} below {pace}", plan.pace);
    check_plan(&plan, items, "coins", counts, &modules, Some(level_up)).expect("plan passes its re-check");
    // Then spare Bench and Kiln time, without giving up pace or coins.
    let stocked = solve_exact(items, "coins", counts, &modules, Goal::StockUp(level_up, pace, plan.rate_per_second), None, None)
        .expect("stocked plan");
    assert!(stocked.proven_optimal);
    assert!(stocked.rate_per_second >= plan.rate_per_second * (1.0 - 2e-3));
    check_plan(&stocked, items, "coins", counts, &modules, Some(level_up)).expect("stocked plan passes its re-check");
    (stocked, PACE_UNIT / pace)
}

// With the items already in stock, a level-up is only coins: the time is the coins still needed
// over the best coin rate.
#[test]
fn exact_level_up_with_items_in_stock_is_only_coins() {
    let Some(items) = load_items() else { return };
    let counts = FacilityCounts::only(&[("Farmland", 6, 2), ("Woodland", 3, 2), ("Mine", 2, 2)]);
    let best = solve_exact(&items, "coins", &counts, &ModuleLevels::default(), Goal::Earn { floors: &[] }, None, None).unwrap();
    let cost = [("coins", 69000.0), ("rough_lumber", 290.0), ("coarse_sifted_ore", 360.0)];
    let stock = [("coins", 9000.0), ("rough_lumber", 300.0), ("coarse_sifted_ore", 400.0)];
    let (plan, seconds) = solve_level_up(&items, &counts, &level_up(&cost, &stock));
    let expected = 60000.0 / best.rate_per_second;
    assert!((seconds - expected).abs() < 1e-4 * expected, "took {seconds}s, expected {expected}s");
    assert!((plan.rate_per_second - best.rate_per_second).abs() < 1e-6);
}

// The items come from Wood Blocks and Mineral Sand through the Woodworking Bench and Chimney
// Kiln, so the plan must run them, and has to give up some coins to make the byproducts.
#[test]
fn exact_level_up_makes_its_items_from_byproducts() {
    let Some(items) = load_items() else { return };
    let counts = FacilityCounts::only(&[
        ("Farmland", 6, 2),
        ("Woodland", 3, 2),
        ("Mine", 2, 2),
        ("Woodworking Bench", 1, 1),
        ("Chimney Kiln", 1, 1),
    ]);
    let cost = level_up(&[("coins", 69000.0), ("rough_lumber", 290.0), ("coarse_sifted_ore", 360.0)], &[]);
    let (plan, seconds) = solve_level_up(&items, &counts, &cost);
    assert!(plan.units.contains_key("rough_lumber") && plan.units.contains_key("coarse_sifted_ore"), "{plan:?}");
    let net = net_rates(&plan, &items);
    assert!(net["rough_lumber"] * seconds >= 290.0 * (1.0 - 2e-4));
    assert!(net["coarse_sifted_ore"] * seconds >= 360.0 * (1.0 - 2e-4));
    assert!(plan.rate_per_second * seconds >= 69000.0 * (1.0 - 2e-4));

    // Wood Blocks in stock cut the time.
    let stocked = level_up(&[("coins", 69000.0), ("rough_lumber", 290.0), ("coarse_sifted_ore", 360.0)], &[("wood_block", 2320.0)]);
    let (_, sooner) = solve_level_up(&items, &counts, &stocked);
    assert!(sooner < seconds, "{sooner}s with Wood Blocks in stock, {seconds}s without");

    // Without the Bench the items can't be made at all.
    let no_bench = FacilityCounts::only(&[("Farmland", 6, 2), ("Woodland", 3, 2), ("Mine", 2, 2), ("Chimney Kiln", 1, 1)]);
    let fastest = solve_exact(&items, "coins", &no_bench, &ModuleLevels::default(), Goal::LevelUp(&cost), None, None).unwrap();
    assert!(fastest.objective < 1e-9, "pace {} without a Bench", fastest.objective);
}

// Standard Planks are made from Rough Lumber on the same Woodworking Bench, so with one Bench the
// two recipes take turns on it.
#[test]
fn exact_level_up_takes_turns_on_one_bench() {
    let Some(items) = load_items() else { return };
    let counts = FacilityCounts::only(&[
        ("Farmland", 6, 2),
        ("Woodland", 3, 2),
        ("Mine", 2, 2),
        ("Woodworking Bench", 1, 2),
        ("Chimney Kiln", 1, 2),
    ]);
    let cost = level_up(&[("coins", 680000.0), ("standard_planks", 320.0), ("sintered_ore_brick", 350.0)], &[]);
    let (plan, _) = solve_level_up(&items, &counts, &cost);
    for name in ["rough_lumber", "standard_planks", "coarse_sifted_ore", "sintered_ore_brick"] {
        assert_eq!(plan.units.get(name), Some(&1), "{name}: {plan:?}");
    }
    let shown = to_production_plan(&plan, &items, "coins", &counts);
    let planks = shown.coin_items.iter().find(|s| s.item_name.as_deref() == Some("standard_planks")).unwrap();
    assert!(planks.reason.contains("takes turns with rough_lumber"), "{}", planks.reason);
    let bench_busy: f64 =
        shown.coin_items.iter().filter(|s| s.facility == "Woodworking Bench").filter_map(|s| s.busy_units).sum();
    assert!(bench_busy <= 1.0 + 1e-6, "Bench busy {bench_busy}");
}

// Mineral Sand is plentiful here while Wood Blocks set the pace, so the Kiln turns the spare sand
// into Coarse-Sifted Ore and the ore is ready before the Rough Lumber.
#[test]
fn exact_level_up_processes_spare_byproducts() {
    let Some(items) = load_items() else { return };
    let counts = FacilityCounts::only(&[
        ("Woodland", 1, 1),
        ("Mine", 6, 3),
        ("Woodworking Bench", 1, 1),
        ("Chimney Kiln", 1, 1),
    ]);
    let cost = level_up(&[("coins", 1000.0), ("rough_lumber", 100.0), ("coarse_sifted_ore", 100.0)], &[]);
    let (plan, seconds) = solve_level_up(&items, &counts, &cost);
    let net = net_rates(&plan, &items);
    assert!((net["rough_lumber"] * seconds - 100.0).abs() < 0.05, "lumber {}", net["rough_lumber"] * seconds);
    assert!(net["coarse_sifted_ore"] * seconds > 150.0, "ore {}", net["coarse_sifted_ore"] * seconds);
}
