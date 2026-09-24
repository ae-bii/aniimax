//! How much could the coverage model be leaving on the table?
//!
//! A plan's arrangements of plots around an environment building come from a sweep of packings:
//! exhaustive for one building, a sweep of weightings for an overlapping pair. Neither is a proof
//! that no better arrangement exists. This measures the most that could be missing, by solving
//! each RV level twice: once as the app does, and once with every building and pair also offered
//! the box that holds all of its arrangements at once (see `coverage::pair_upper_bound`). No real
//! layout reaches that box, so the relaxed plan earns at least what the true best possibly could,
//! and the difference bounds how far the real plan can be off.
//!
//! Ignored by default; it solves each level twice with the fallback solver, which is slow enough
//! that only the early levels finish. Run it with:
//!
//! ```text
//! cargo test --release --test coverage_gap -- --ignored --nocapture
//! ```
//!
//! For every level, measure it through HiGHS instead, the way the app solves: build a second wasm
//! with `ANIIMAX_RELAX_COVERAGE=1 ./build-wasm.sh`, run each build's plans, and compare the
//! rates. Last read that way, the plan is the best any arrangement could give at every RV level
//! from 7 to 20 except 15, where it is within 0.313%.

use aniimax::data::load_all_data;
use aniimax::exact::{set_coverage_relaxed, solve_exact, Goal};
use aniimax::models::{FacilityCounts, ModuleLevels};
use std::path::Path;
use std::time::Duration;

/// `counts[i]` is the value at RV level i + 1, with the last carrying on; mirrors `atHomeLevel`.
fn at_home_level(counts: &[u32], home_level: usize) -> u32 {
    if counts.is_empty() {
        return 0;
    }
    counts[(home_level - 1).min(counts.len() - 1)]
}

fn numbers(text: &str) -> Vec<u32> {
    text.split(',').filter_map(|n| n.trim().parse().ok()).collect()
}

/// Reads `web/facility-config.js` so the levels measured are exactly the ones Simple mode sends.
fn simple_setup(home_level: usize) -> (FacilityCounts, ModuleLevels) {
    let js = std::fs::read_to_string("web/facility-config.js").expect("the web facility list");
    let mut counts = FacilityCounts::default();
    for entry in js.split("\n    {\n        name: '").skip(1) {
        let name = entry.split('\'').next().expect("a facility name").to_string();
        let owned = entry
            .split("counts: [")
            .nth(1)
            .map(|rest| at_home_level(&numbers(rest.split(']').next().unwrap_or("")), home_level))
            .unwrap_or(0);
        // `unlocks: { 1: 3, 2: 6 }` maps a facility level to the RV level that unlocks it.
        let level = entry
            .split("unlocks: {")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .map(|list| {
                list.split(',')
                    .filter_map(|pair| {
                        let (level, need) = pair.split_once(':')?;
                        let (level, need): (u32, usize) = (level.trim().parse().ok()?, need.trim().parse().ok()?);
                        (need <= home_level).then_some(level)
                    })
                    .max()
                    .unwrap_or(1)
            })
            .unwrap_or(1);
        if owned > 0 {
            counts.set(&name, owned, level);
        }
    }
    let mut modules = ModuleLevels::default();
    for (key, setter) in [
        ("ecological_module", 0),
        ("kitchen_module", 1),
        ("resource_detector", 2),
        ("crafting_module", 3),
    ] {
        let caps = js
            .split(&format!("{key}: ["))
            .nth(1)
            .map(|rest| numbers(rest.split(']').next().unwrap_or("")))
            .unwrap_or_default();
        let level = at_home_level(&caps, home_level);
        match setter {
            0 => modules.ecological_module = level,
            1 => modules.kitchen_module = level,
            2 => modules.resource_detector = level,
            _ => modules.crafting_module = level,
        }
    }
    (counts, modules)
}

#[test]
#[ignore = "solves every RV level twice; run by hand"]
fn coverage_gap() {
    let items = load_all_data(Path::new("data")).expect("game data");
    let limit = Some(Duration::from_secs(120));
    let mut worst: f64 = 0.0;
    for home_level in 7..=10 {
        let (counts, modules) = simple_setup(home_level);
        let solve = || {
            solve_exact(&items, "coins", &counts, &modules, Goal::Earn { floors: &[] }, limit, None)
        };
        set_coverage_relaxed(false);
        let Some(real) = solve() else {
            println!("RV{home_level}: no plan");
            continue;
        };
        set_coverage_relaxed(true);
        let Some(bound) = solve() else {
            println!("RV{home_level}: no relaxed plan");
            continue;
        };
        set_coverage_relaxed(false);
        // A solve that stopped on its time limit says nothing: its answer is only an incumbent,
        // which for the relaxed model can land below the real plan and is no bound at all.
        if !real.proven_optimal || !bound.proven_optimal {
            println!(
                "RV{home_level}: no reading, {} hit its time limit",
                if real.proven_optimal { "the relaxed solve" } else { "the plan" }
            );
            continue;
        }
        let gap = (bound.rate_per_second - real.rate_per_second) / real.rate_per_second.abs().max(1e-9);
        worst = worst.max(gap);
        println!(
            "RV{home_level}: plan {:.2}/s, at most {:.2}/s, so within {:.3}%",
            real.rate_per_second,
            bound.rate_per_second,
            gap * 100.0,
        );
    }
    println!("worst case: the plan is within {:.3}% of the best any arrangement could give", worst * 100.0);
}
