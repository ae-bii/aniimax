//! Works out every pair packing worth offering and writes `data/pair_coverage.csv`, which the
//! library then reads with `include_str!`. The geometry never changes, so this only needs running
//! when the coverage rules, the facility footprints or the environment-gated crops change.
//!
//! Ignored by default; it takes the better part of an hour. Run it with:
//!
//! ```text
//! cargo test --release --test bake_pair_coverage -- --ignored --nocapture
//! ```

use aniimax::coverage::{
    pair_offsets, pair_options_uncached, undominated, Offset, PairOption, PairSizes,
    ENVIRONMENT_GATED_FACILITIES,
};
use aniimax::data::load_all_data;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The facility types a pair could ever have to cover: the environment-gated ones with a crop
/// wanting a temperature. The Sunlamp's Adequate is off the temperature line, so a crop that only
/// wants that is never part of a pair.
fn pair_facilities() -> Vec<&'static str> {
    let items = load_all_data(Path::new("data")).expect("game data");
    ENVIRONMENT_GATED_FACILITIES
        .iter()
        .filter(|(facility, _)| {
            items.iter().any(|item| {
                item.facility == *facility
                    && item
                        .environment
                        .as_deref()
                        .is_some_and(|e| aniimax::coverage::mode_temperature(e).is_some())
            })
        })
        .map(|(facility, _)| *facility)
        .collect()
}

/// Every non-empty subset, each ordered largest footprint first, as `types_for` in `exact.rs`
/// orders the types it asks for.
fn type_sets(facilities: &[&'static str]) -> Vec<Vec<&'static str>> {
    (1..(1u32 << facilities.len()))
        .map(|mask| {
            let mut set: Vec<&'static str> = facilities
                .iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, f)| *f)
                .collect();
            set.sort_by(|a, b| {
                let size = |f: &str| aniimax::coverage::facility_footprint(f).unwrap_or(0.0);
                size(b).partial_cmp(&size(a)).unwrap().then(a.cmp(b))
            });
            set
        })
        .collect()
}

#[test]
#[ignore = "an hour of packing; run by hand when the coverage rules change"]
fn bake_pair_coverage() {
    let facilities = pair_facilities();
    let sets = type_sets(&facilities);
    let sizes = PairSizes::of("Heat Furnace", "Cooling Unit");
    let offsets = pair_offsets(sizes);
    println!("{facilities:?}: {} type sets x {} offsets", sets.len(), offsets.len());
    let jobs: Vec<(Vec<&'static str>, Offset)> = sets
        .iter()
        .flat_map(|set| offsets.iter().map(move |&offset| (set.clone(), offset)))
        .collect();

    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let mut found: Vec<(usize, Vec<PairOption>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let (jobs, next, done) = (&jobs, &next, &done);
                scope.spawn(move || {
                    let mut mine: Vec<(usize, Vec<PairOption>)> = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some((types, offset)) = jobs.get(i) else { break };
                        mine.push((i, pair_options_uncached(sizes, types, *offset)));
                        let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                        println!("  {n}/{}", jobs.len());
                    }
                    mine
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    // Back into the order the offsets were handed out in, so the placements a player finds
    // easiest (in a row, close together) win any tie.
    found.sort_by_key(|(i, _)| *i);

    // What a plan gets from a pair is the counts in its three zones; the offset only tells the
    // player where to stand the buildings. So an arrangement another one matches or beats is the
    // same offer twice, and only the best of them are worth a column in the solve.
    let mut per_set: BTreeMap<String, Vec<PairOption>> = BTreeMap::new();
    for (i, options) in found {
        let seen = per_set.entry(jobs[i].0.join(";")).or_default();
        for option in options {
            if !seen.iter().any(|o| o.counts == option.counts) {
                seen.push(option);
            }
        }
    }
    let lines: Vec<String> = per_set
        .into_iter()
        .flat_map(|(types, options)| {
            let before = options.len();
            let mut best = undominated(options);
            best.sort_by_key(|o| (o.offset.dx, o.offset.dy, o.counts.concat()));
            println!("{types}: {before} arrangements, {} worth offering", best.len());
            best.into_iter().map(move |o| {
                let counts: Vec<String> = o.counts.concat().iter().map(u32::to_string).collect();
                format!("{types}, {}, {}, {}, {}, {}", sizes.first, sizes.second, o.offset.dx, o.offset.dy, counts.join(";"))
            })
        })
        .collect();
    let out = format!("types, first, second, dx, dy, counts\n{}\n", lines.join("\n"));
    std::fs::write("data/pair_coverage.csv", out).expect("write the table");
    println!("{} options written", lines.len());
}
