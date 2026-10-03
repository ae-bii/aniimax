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

// Six Farmland at level 2 with nothing else: watering takes a quarter off both, so wheat (5 per
// 180s, sells for 1) earns 6 x 5/180 = 0.1667/sec and potato (2 per 480s, sells for 8, seed 1)
// earns 6 x 15/480 = 0.1875; quick_wheat needs Ecological Module 1, which isn't owned. Potato wins.
#[test]
fn exact_picks_the_best_crop_on_its_own() {
    let Some(items) = load_items() else { return };
    let counts = FacilityCounts::only(&[("Farmland", 6, 2)]);
    let plan = solve_and_check(&items, &counts, &ModuleLevels::default());
    assert!((plan.rate_per_second - 6.0 * 15.0 / 480.0).abs() < 1e-9, "got {}", plan.rate_per_second);
    assert_eq!(plan.units.get("potato"), Some(&6));
}

// rice_drink (Carousel Mill) needs milled_rice made at a Carousel Mill too; with two Mills, one
// makes each. 3 rice plots, watered, make a rice drink every 1200s: 0.000833/sec x (1860 - 2 x 12
// seed) = 1.53; 4 Wells with level-1 Aniimo make 0.014222 fresh_water/sec, the drinks use 16 each
// and the rest sells at 46.
#[test]
fn exact_matches_the_hand_worked_rice_drink_plan() {
    let Some(items) = load_items() else { return };
    let counts = FacilityCounts::only(&[("Farmland", 3, 3), ("Well", 4, 2), ("Carousel Mill", 2, 4)]);
    let plan = solve_and_check(&items, &counts, &ModuleLevels::default());
    let drinks = 3.0 * 18.0 / 1800.0 / 18.0 / 2.0;
    let expected = drinks * (1860.0 - 24.0) + (4.0 * 8.0 / 2250.0 - 16.0 * drinks) * 46.0;
    assert!((plan.rate_per_second - expected).abs() < 1e-6, "got {}, expected {expected}", plan.rate_per_second);
    assert_eq!(plan.units.get("rice_drink"), Some(&1));
    assert_eq!(plan.units.get("milled_rice"), Some(&1));
    // The rice seeds come off the rice drink they end up in.
    let shown = to_production_plan(&plan, &items, "coins", &counts);
    let drink = shown.income_streams.iter().find(|s| s.item_name == "rice_drink").expect("rice drink sold");
    assert!((drink.rate_per_second - drinks * (1860.0 - 24.0)).abs() < 1e-9, "rice drink earns {}", drink.rate_per_second);
}

// A mid-game setup with an Aniimo setup applied and a Cooling Unit, so environment coverage and
// whole processor units both matter: the exact plan is proven, re-checks, and beats or ties the
// heuristic planner; its rows cover every owned unit exactly once.
#[test]
fn exact_handles_environments_and_aniimo_speeds() {
    let Some(mut items) = load_items() else { return };
    load_aniimo_requirements(Path::new("data")).unwrap().apply(&AniimoSetup::Best(aniimax::models::AniimoLevels::all(aniimax::models::MAX_ANIIMO_LEVEL)), &mut items);
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
    // Seed costs follow a crop into what it's made into, so nothing the plan chose to sell loses
    // money.
    for stream in &shown.income_streams {
        assert!(stream.rate_per_second > 0.0, "{} earns {}", stream.item_name, stream.rate_per_second);
    }
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

// A crop grows without its environment building, just slower: chestnut (Warm) grows in 3000s
// rather than 2400s at 80% speed, which watering brings to 2400s and 1800s, and it still beats
// every crop that wants no environment.
#[test]
fn exact_grows_crops_without_their_environment() {
    let Some(items) = load_items() else { return };
    let counts = FacilityCounts::only(&[("Woodland", 4, 4)]);
    let plan = solve_and_check(&items, &counts, &ModuleLevels::default());
    assert!((plan.rate_per_second - 4.0 * 653.0 / 2400.0).abs() < 1e-9, "got {}", plan.rate_per_second);
    assert_eq!(plan.units.get("chestnut__uncovered"), Some(&4));

    // A Heat Furnace covers all four plots, and palm bark (Scorching, 673 a harvest over 1800s
    // once watered) is the best thing to put under it.
    let warmed = FacilityCounts::only(&[("Woodland", 4, 4), ("Heat Furnace", 1, 1)]);
    let covered = solve_and_check(&items, &warmed, &ModuleLevels::default());
    assert!((covered.rate_per_second - 4.0 * 673.0 / 1800.0).abs() < 1e-9, "got {}", covered.rate_per_second);
    assert_eq!(covered.units.get("palm_bark"), Some(&4));

    // Adequate crops still need their Sunlamp: walnut is worth more than chestnut but can't grow
    // without one.
    assert_eq!(plan.units.get("walnut__uncovered"), None);
    assert_eq!(plan.units.get("walnut"), None);
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
    // Nothing that sells for coins is left over: spare clay from a Mine kept for its Mineral Sand
    // still sells.
    for (name, spare) in net_rates(&stocked, items) {
        let sells = items.iter().any(|i| i.name == name && i.sell_currency == "coins" && i.sell_value > 0.0);
        assert!(!sells || spare < 1e-7, "{name}: {spare}/sec left unsold");
    }
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

    // Items in stock cut the time. Wood Blocks and Mineral Sand wouldn't: the Woodland and Mine
    // make plenty of both, and what sets the pace is the Bench and Kiln time to work them up.
    let stocked = level_up(
        &[("coins", 69000.0), ("rough_lumber", 290.0), ("coarse_sifted_ore", 360.0)],
        &[("rough_lumber", 145.0), ("coarse_sifted_ore", 180.0)],
    );
    let (_, sooner) = solve_level_up(&items, &counts, &stocked);
    assert!(sooner < seconds, "{sooner}s with half the items in stock, {seconds}s without");

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

/// Every item plus the Harvest Moon Festival's.
fn load_items_with_season() -> Option<Vec<ProductionItem>> {
    let mut items = load_items()?;
    let text = std::fs::read_to_string("data/harvest_moon_festival.csv").expect("season data");
    let mut season = aniimax::data::parse_season(&text).expect("season parses");
    aniimax::models::apply_watering(&mut season);
    items.extend(season);
    Some(items)
}

// Moonray Wheat is treated as unlimited, so season seeds cost nothing a plan counts and all six
// plots grow Moondew Radish or Waxing Moon Pepper (8 a batch, 74 coins each), far more per plot
// than potato.
#[test]
fn exact_season_crops_fill_every_plot() {
    let Some(items) = load_items_with_season() else { return };
    let counts = FacilityCounts::only(&[("Farmland", 6, 2)]);
    let plan = solve_and_check(&items, &counts, &ModuleLevels::default());
    let grow = items.iter().find(|i| i.name == "moondew_radish").unwrap().production_time;
    let expected = 6.0 * 8.0 * 74.0 / grow;
    assert!((plan.rate_per_second - expected).abs() < 1e-9, "got {}, expected {expected}", plan.rate_per_second);
}

// With 60 Moonray Wheat a day for seeds (4 a planting), the season crops are planted 15 times a
// day, which one plot covers (15 x 1800s), worth 8880 coins a day against a potato plot's 2700;
// the other five plots grow potato.
#[test]
fn test_exact_season_currency_budget_caps_seeds() {
    let Some(mut items) = load_items_with_season() else { return };
    for terms in items.iter_mut().filter_map(|i| i.season.as_mut()) {
        terms.currency_per_day = Some(60.0);
    }
    let counts = FacilityCounts::only(&[("Farmland", 6, 2)]);
    let plan = solve_and_check(&items, &counts, &ModuleLevels::default());
    let time = |name: &str| items.iter().find(|i| i.name == name).unwrap().production_time;
    let expected = 15.0 * 8.0 * 74.0 / PACE_UNIT + 5.0 * (2.0 * 8.0 - 1.0) / time("potato");
    assert!((plan.rate_per_second - expected).abs() < 1e-9, "got {}, expected {expected}", plan.rate_per_second);

    // The re-check catches a plan spending more than the budget.
    let mut over = plan.clone();
    let crop = over.recipe_rates.keys().find(|n| n.starts_with("moondew") || n.starts_with("waxing")).unwrap().clone();
    *over.recipe_rates.get_mut(&crop).unwrap() *= 1.5;
    let err = check_plan(&over, &items, "coins", &counts, &ModuleLevels::default(), None).unwrap_err();
    assert!(err.contains("budget"), "{err}");
}

// Points as a priority: every season crop sold raw counts 1, which beats cooking 16 of them into
// something worth 8 at most, so the most points is all six plots sold raw. The coin plan that has
// to keep that many points is then the same as the best coin plan.
#[test]
fn exact_season_points_are_a_priority() {
    let Some(items) = load_items_with_season() else { return };
    let counts = FacilityCounts::only(&[("Farmland", 6, 2), ("Simmering Pot", 1, 1), ("Tidewhisper Sandcastle", 1, 1)]);
    let modules = ModuleLevels::default();
    let points = aniimax::models::SEASON_POINTS;
    let most = solve_exact(&items, points, &counts, &modules, Goal::Earn { floors: &[] }, None, None).unwrap();
    assert!(most.proven_optimal);
    let grow = items.iter().find(|i| i.name == "moondew_radish").unwrap().production_time;
    assert!((most.rate_per_second - 6.0 * 8.0 / grow).abs() < 1e-9, "{} points a second", most.rate_per_second);
    check_plan(&most, &items, points, &counts, &modules, None).expect("points plan passes its re-check");
    let floors = vec![(points.to_string(), most.rate_per_second)];
    let plan = solve_exact(&items, "coins", &counts, &modules, Goal::Earn { floors: &floors }, None, None).unwrap();
    assert!(plan.proven_optimal);
    check_plan(&plan, &items, "coins", &counts, &modules, None).expect("coin plan passes its re-check");
    let kept = aniimax::exact::target_rate(&plan, &items, points);
    // Floors leave 0.01% of slack for the solver's tolerances.
    assert!(kept >= most.rate_per_second * (1.0 - 1.01e-4), "kept {kept} of {} points a second", most.rate_per_second);
}

// A goal counts each item from its first batch, so what makes a priority is split by item with
// its wait; the rates still add up to the priority's own rate, for a byproduct and for points.
#[test]
fn exact_target_streams_add_up_to_the_rate() {
    let Some(items) = load_items_with_season() else { return };
    let counts = FacilityCounts::only(&[("Farmland", 6, 2), ("Woodland", 4, 2), ("Jukebox Dryer", 1, 2)]);
    let plan = solve_and_check(&items, &counts, &ModuleLevels::default());
    for target in ["Wood Blocks", aniimax::models::SEASON_POINTS] {
        let streams = aniimax::exact::target_streams(&plan, &items, target);
        assert!(!streams.is_empty(), "nothing makes {target}");
        let total: f64 = streams.iter().map(|(rate, _)| rate).sum();
        let rate = aniimax::exact::target_rate(&plan, &items, target);
        assert!((total - rate).abs() < 1e-12, "{target}: streams {total}, rate {rate}");
        assert!(streams.iter().all(|&(_, lead)| lead > 0.0), "{target}: a stream with no wait");
    }
}

// Three plots of Sugarcane want Scorching, and one Heat Furnace covers all three, so a plan with
// two Furnaces owned sets up just the one: the second would earn nothing and need its own Aniimo.
#[test]
fn exact_uses_the_fewest_environment_buildings() {
    let Some(items) = load_items() else { return };
    let counts = FacilityCounts::only(&[("Farmland", 3, 5), ("Heat Furnace", 2, 1)]);
    let plan = solve_and_check(&items, &counts, &ModuleLevels::default());
    assert_eq!(plan.units.get("sugarcane"), Some(&3), "units {:?}", plan.units);
    let buildings: u32 = plan.environment.iter().map(|e| e.count).sum::<u32>() + 2 * plan.pairs.iter().map(|p| p.count).sum::<u32>();
    assert_eq!(buildings, 1, "environment {:?}, pairs {:?}", plan.environment, plan.pairs);
    // What the plan reports earning leaves the tie-break out.
    let recomputed = check_plan(&plan, &items, "coins", &counts, &ModuleLevels::default(), None).unwrap();
    assert!((plan.objective - recomputed).abs() < 1e-9, "objective {} vs {recomputed}", plan.objective);
}

/// The data's items reworked for `crew` (see `aniimax::models::crew_variants`).
fn crew_items(crew: &aniimax::models::Crew) -> Option<Vec<ProductionItem>> {
    let items = load_items()?;
    let requirements = load_aniimo_requirements(Path::new("data")).unwrap();
    let steps = aniimax::data::load_grower_steps(Path::new("data")).unwrap();
    Some(aniimax::models::crew_variants(items, crew, &requirements, &steps))
}

/// A roster of `(count, [(ability, level)])`, with the game's environment buildings and no
/// personality bonuses.
fn crew_of(members: &[(u32, &[(&str, u32)])]) -> aniimax::models::Crew {
    aniimax::models::Crew {
        members: members
            .iter()
            .map(|(count, abilities)| aniimax::models::RosterAniimo {
                count: *count,
                abilities: abilities.iter().map(|(a, l)| (a.to_string(), *l)).collect(),
                personalities: Vec::new(),
            })
            .collect(),
        residents: ["Tidewhisper Sandcastle", "Dewy House", "Nimbus Bed", "Starfall Hammock", "Floral Windmill"]
            .into_iter()
            .map(String::from)
            .collect(),
        environment: [("Heat Furnace", "Fire"), ("Cooling Unit", "Ice"), ("Sunlamp", "Light")]
            .into_iter()
            .map(|(b, a)| (b.to_string(), a.to_string()))
            .collect(),
        personalities: Default::default(),
    }
}

fn solve_with_crew(counts: &FacilityCounts, crew: aniimax::models::Crew) -> (ExactPlan, Vec<ProductionItem>) {
    let items = crew_items(&crew).expect("data");
    let mut counts = counts.clone();
    counts.set_crew(crew);
    let modules = ModuleLevels::default();
    let plan = solve_exact(&items, "coins", &counts, &modules, Goal::Earn { floors: &[] }, Some(Duration::from_secs(60)), None)
        .expect("exact plan");
    assert!(plan.proven_optimal, "not proven optimal");
    check_plan(&plan, &items, "coins", &counts, &modules, None).expect("plan passes its re-check");
    (plan, items)
}

// Four Mines, but only one level-4 Earth Aniimo to work them: it has one day to spread over the
// four, so the plan makes exactly a quarter of what four such Aniimo would.
#[test]
fn crew_one_aniimo_works_one_day() {
    if load_items().is_none() {
        return;
    }
    let counts = FacilityCounts::only(&[("Mine", 4, 3)]);
    let (one, _) = solve_with_crew(&counts, crew_of(&[(1, &[("Earth", 4)])]));
    let (four, _) = solve_with_crew(&counts, crew_of(&[(4, &[("Earth", 4)])]));
    assert!(four.rate_per_second > 0.0);
    assert!((one.rate_per_second * 4.0 - four.rate_per_second).abs() < 1e-9, "one {} vs four {}", one.rate_per_second, four.rate_per_second);
}

// A Heat Furnace needs a Fire Aniimo on it all day. Without one, Sugarcane (which wants Scorching)
// can only grow uncovered; with one, it's covered and the plan says who staffs the Furnace.
// Farmland jobs need Earth, Grass and Dark, so the roster has someone for those too.
#[test]
fn crew_staffs_environment_buildings() {
    if load_items().is_none() {
        return;
    }
    let counts = FacilityCounts::only(&[("Farmland", 3, 5), ("Heat Furnace", 1, 1)]);
    let farmer: &[(&str, u32)] = &[("Earth", 1), ("Grass", 1), ("Dark", 1)];
    let (without, _) = solve_with_crew(&counts, crew_of(&[(1, farmer)]));
    assert!(without.environment.is_empty() && without.staffing.is_empty(), "no one to staff it: {:?}", without.environment);
    let (with, _) = solve_with_crew(&counts, crew_of(&[(1, farmer), (1, &[("Fire", 1)])]));
    assert_eq!(with.environment.iter().map(|e| e.count).sum::<u32>(), 1);
    assert_eq!(with.staffing.len(), 1);
    assert_eq!(with.staffing[0].1, 1, "staffed by the Fire Aniimo");
    assert!((with.staffing[0].2 - 1.0).abs() < 1e-9);
    assert!(with.rate_per_second > without.rate_per_second);
}

// Nobody to sow or reap: no crop can be grown at all.
#[test]
fn crew_crops_need_their_jobs_done() {
    if load_items().is_none() {
        return;
    }
    let counts = FacilityCounts::only(&[("Farmland", 3, 5)]);
    let (plan, _) = solve_with_crew(&counts, crew_of(&[(2, &[("Fire", 3)])]));
    assert!(plan.recipe_rates.is_empty(), "grew {:?}", plan.recipe_rates);
}


// Watering only speeds crops up: with nobody to water them they still grow, at their unwatered
// time. Six level-2 plots of potato (2 per batch, sells for 8, seed 1) take 640s unwatered and
// 480s watered.
#[test]
fn crew_without_water_grows_unwatered() {
    if load_items().is_none() {
        return;
    }
    let counts = FacilityCounts::only(&[("Farmland", 6, 2)]);
    let farmer: &[(&str, u32)] = &[("Earth", 1), ("Grass", 1), ("Dark", 1)];
    let (dry, _) = solve_with_crew(&counts, crew_of(&[(1, farmer)]));
    let (wet, _) = solve_with_crew(&counts, crew_of(&[(1, farmer), (1, &[("Water", 1)])]));
    assert!((dry.rate_per_second - 6.0 * 15.0 / 640.0).abs() < 1e-9, "unwatered {}", dry.rate_per_second);
    assert!((wet.rate_per_second - 6.0 * 15.0 / 480.0).abs() < 1e-9, "watered {}", wet.rate_per_second);
}

// Each roster member able to work a recipe gets a copy of it, timed for that member; the recipe
// itself stays for its price, can't be made as it is, and takes the fastest member's time.
#[test]
fn crew_copies_are_timed_per_member() {
    let crew = crew_of(&[(1, &[("Earth", 1)]), (1, &[("Earth", 4)])]);
    let Some(items) = crew_items(&crew) else { return };
    let rocks: Vec<&ProductionItem> = items.iter().filter(|i| aniimax::models::base_item_name(&i.name) == "rock").collect();
    let copy = |member: usize| rocks.iter().find(|i| i.crew == Some(member)).expect("a copy per member");
    assert_eq!(copy(0).name, "rock__by0");
    assert!(copy(1).production_time < copy(0).production_time, "the level-4 Aniimo is faster");
    let original = rocks.iter().find(|i| i.crew.is_none()).expect("the recipe itself stays");
    assert_eq!(original.facility_level, u32::MAX);
    assert!((original.production_time - copy(1).production_time).abs() < 1e-9);
}

// A resident facility keeps its Aniimo all day: two Sandcastles need two Leisure Aniimo.
#[test]
fn crew_residents_take_a_whole_aniimo() {
    if load_items().is_none() {
        return;
    }
    let counts = FacilityCounts::only(&[("Tidewhisper Sandcastle", 2, 1)]);
    let sandcastles = |plan: &ExactPlan, items: &[ProductionItem]| -> u32 {
        plan.units
            .iter()
            .filter(|(name, _)| items.iter().any(|i| &i.name == *name && i.facility == "Tidewhisper Sandcastle"))
            .map(|(_, &n)| n)
            .sum()
    };
    let (one, items) = solve_with_crew(&counts, crew_of(&[(1, &[("Leisure", 2)])]));
    assert_eq!(sandcastles(&one, &items), 1);
    let (two, items) = solve_with_crew(&counts, crew_of(&[(2, &[("Leisure", 2)])]));
    assert_eq!(sandcastles(&two, &items), 2);
}
