//! Tests for data loading functionality.

use aniimax::data::load_all_data;
use std::path::Path;

#[test]
fn test_load_all_data() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        // Skip test if data directory doesn't exist (e.g., in CI)
        return;
    }

    let items = load_all_data(data_dir).expect("Failed to load data");
    assert!(!items.is_empty(), "Should load at least some items");

    // Check that we have items from different facilities
    let facilities: Vec<&str> = items.iter().map(|i| i.facility.as_str()).collect();
    assert!(facilities.contains(&"Farmland"), "Should have Farmland items");
}

#[test]
fn test_loaded_items_have_valid_data() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }

    let items = load_all_data(data_dir).expect("Failed to load data");

    for item in &items {
        assert!(!item.name.is_empty(), "Item name should not be empty");
        assert!(!item.facility.is_empty(), "Facility should not be empty");
        assert!(item.production_time > 0.0, "Production time should be positive");
        assert!(item.yield_amount > 0, "Yield should be positive");
        assert!(item.facility_level > 0, "Facility level should be positive");
    }
}

// The higher-tier Mine items are Mineral Sand-heavy: most of each batch is byproduct, with only a
// small sellable yield (quartz_ore yields 4 against 86 Mineral Sand). That inversion makes the
// `yield`/`byproduct_yield` columns easy to transpose by accident, so this locks the real values
// in. Byproduct also scales purely with facility level, not with the specific item, which is the
// other half of what's asserted here.
#[test]
fn test_mine_yields_are_not_swapped_with_byproduct() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");

    let get = |name: &str| {
        items
            .iter()
            .find(|i| i.name == name && i.facility == "Mine")
            .unwrap_or_else(|| panic!("{name} should exist in Mine data"))
    };

    let quartz_ore = get("quartz_ore");
    assert_eq!(quartz_ore.yield_amount, 4, "quartz_ore's real sellable yield is 4, not 86");
    assert_eq!(quartz_ore.byproduct, Some(("Mineral Sand".to_string(), 86)));

    let gem = get("gem");
    assert_eq!(gem.yield_amount, 2, "gem's real sellable yield is 2, not 122");
    assert_eq!(gem.byproduct, Some(("Mineral Sand".to_string(), 122)));

    // Sellable yield falls as tier rises while byproduct climbs; if a future edit transposes the
    // columns for any row, one of these two orderings breaks.
    let rock = get("rock");
    assert!(
        rock.yield_amount > quartz_ore.yield_amount && quartz_ore.yield_amount > gem.yield_amount,
        "sellable yield should fall with tier: rock > quartz_ore > gem"
    );
    let sand = |i: &aniimax::models::ProductionItem| i.byproduct.as_ref().map(|(_, n)| *n).unwrap_or(0);
    assert!(
        sand(rock) < sand(quartz_ore) && sand(quartz_ore) < sand(gem),
        "Mineral Sand byproduct should climb with tier: rock < quartz_ore < gem"
    );
}

#[test]
fn test_currency_types() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }

    let items = load_all_data(data_dir).expect("Failed to load data");

    for item in &items {
        // "aniimo_exp" and "aniipods": Growth items and Aniipods, which the player keeps rather
        // than selling. "none": a level-up material, made for RV level-ups.
        assert!(
            ["coins", "none", "aniimo_exp", "aniipods"].contains(&item.sell_currency.as_str()),
            "Currency should be 'coins', 'aniimo_exp', 'aniipods' or 'none', got: {}",
            item.sell_currency
        );
    }
}

// Data-entry checks. These catch transcription and formatting mistakes when new game data is
// added, before they quietly skew the optimizer.

/// Ingredients referenced by a recipe that no facility in the data produces yet. When the facility
/// that makes one of these is added, remove it from this list; when a new recipe references
/// something not yet in the data, add it here deliberately.
const KNOWN_MISSING_INGREDIENTS: &[&str] = &[];

fn load_items() -> Option<Vec<aniimax::models::ProductionItem>> {
    let data_dir = Path::new("data");
    data_dir.exists().then(|| load_all_data(data_dir).expect("Failed to load data"))
}

#[test]
fn test_every_ingredient_is_produced_somewhere() {
    let Some(items) = load_items() else { return };
    let mut names: std::collections::HashSet<&str> = items.iter().map(|i| i.name.as_str()).collect();
    // Wood Blocks and Mineral Sand come from Woodland and Mine byproducts.
    names.extend(aniimax::models::BYPRODUCT_ITEMS.iter().map(|(_, item)| *item));

    let mut missing: Vec<String> = items
        .iter()
        .filter_map(|i| i.raw_materials.as_ref())
        .flatten()
        .filter(|m| !names.contains(m.as_str()))
        .cloned()
        .collect();
    missing.sort();
    missing.dedup();

    let unexpected: Vec<&String> =
        missing.iter().filter(|m| !KNOWN_MISSING_INGREDIENTS.contains(&m.as_str())).collect();
    assert!(
        unexpected.is_empty(),
        "these ingredients aren't produced by any facility (check the spelling, or add them to \
         KNOWN_MISSING_INGREDIENTS): {unexpected:?}"
    );

    let now_produced: Vec<&&str> =
        KNOWN_MISSING_INGREDIENTS.iter().filter(|m| names.contains(**m)).collect();
    assert!(
        now_produced.is_empty(),
        "these are produced now; remove them from KNOWN_MISSING_INGREDIENTS: {now_produced:?}"
    );
}

#[test]
fn test_item_names_are_unique_and_snake_case() {
    let Some(items) = load_items() else { return };
    let mut seen = std::collections::HashMap::new();
    for item in &items {
        assert!(
            !item.name.is_empty()
                && item.name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
            "item names must be lowercase with underscores (e.g. `milled_rice`), got {:?} at {}",
            item.name,
            item.facility
        );
        if let Some(other) = seen.insert(item.name.as_str(), item.facility.as_str()) {
            panic!("{} is listed twice (at {} and {})", item.name, other, item.facility);
        }
        for m in item.raw_materials.iter().flatten() {
            assert!(
                m.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "{}'s ingredient {m:?} should be lowercase with underscores",
                item.name
            );
        }
    }
}

#[test]
fn test_item_values_are_in_range() {
    let Some(items) = load_items() else { return };
    const ENVIRONMENTS: &[&str] = &["Warm", "Scorching", "Cool", "Freeze", "Adequate"];
    const MODULES: &[&str] = &["ecological_module", "kitchen_module", "resource_detector", "crafting_module"];

    for item in &items {
        let name = &item.name;
        let level_up_material = item.sell_currency == "none";
        assert!(level_up_material || item.sell_value > 0.0, "{name} has no sell value");
        assert!(item.yield_amount > 0, "{name} has no yield");
        assert!(item.production_time > 0.0, "{name} has no grow time or workload");
        assert!((1..=10).contains(&item.facility_level), "{name} has facility level {}", item.facility_level);
        if let Some(env) = &item.environment {
            assert!(ENVIRONMENTS.contains(&env.as_str()), "{name} has unknown environment {env:?}");
        }
        if let Some((module, level)) = &item.module_requirement {
            assert!(MODULES.contains(&module.as_str()), "{name} needs unknown module {module:?}");
            assert!((1..=10).contains(level), "{name} needs {module} level {level}");
        }
        match (&item.raw_materials, &item.required_amount) {
            (Some(mats), Some(amounts)) => {
                assert_eq!(
                    mats.len(),
                    amounts.len(),
                    "{name} lists {} ingredients but {} amounts",
                    mats.len(),
                    amounts.len()
                );
                assert!(amounts.iter().all(|&a| a > 0), "{name} has an ingredient amount of 0");
            }
            (None, None) => {}
            _ => panic!("{name} has ingredients without amounts, or amounts without ingredients"),
        }
    }
}

// A quick variant is the same item grown or gathered faster: it sells for the same price, costs the
// same seed, yields more per batch, and is always gated by a module.
#[test]
fn test_quick_variants_match_their_base_item() {
    let Some(items) = load_items() else { return };
    for quick in items.iter().filter(|i| i.name.starts_with("quick_")) {
        let base_name = &quick.name["quick_".len()..];
        let base = items
            .iter()
            .find(|i| i.name == base_name)
            .unwrap_or_else(|| panic!("{} has no base item named {base_name}", quick.name));
        assert_eq!(quick.facility, base.facility, "{} and {base_name} should share a facility", quick.name);
        assert_eq!(quick.sell_value, base.sell_value, "{} should sell for the same as {base_name}", quick.name);
        assert_eq!(quick.cost, base.cost, "{} should cost the same seed as {base_name}", quick.name);
        assert!(quick.yield_amount > base.yield_amount, "{} should yield more than {base_name}", quick.name);
        assert!(
            quick.facility_level >= base.facility_level,
            "{} unlocks before {base_name}",
            quick.name
        );
        assert!(quick.module_requirement.is_some(), "{} should need a module", quick.name);
    }
}

// Every Aniimo-worked recipe (anything with a workload) needs an ability and minimum level in
// aniimo_requirements.csv, so the Minimum and Best Aniimo setups can price it. The file's facility
// column must match where the recipe is made, and nothing may be listed that isn't in the data.
#[test]
fn test_aniimo_requirements_cover_every_worked_recipe() {
    let Some(items) = load_items() else { return };
    let reqs = aniimax::data::load_aniimo_requirements(Path::new("data")).expect("Failed to load requirements");
    const ABILITIES: &[&str] = &[
        "Fire", "Grass", "Water", "Earth", "Lightning", "Ice", "Wind", "Dark", "Light", "Hauling",
        "Artisanship", "Leisure", "Perfumery",
    ];

    for item in items.iter().filter(|i| i.workload.is_some()) {
        let (ability, level) = reqs
            .get(&item.name)
            .unwrap_or_else(|| panic!("{} has no row in aniimo_requirements.csv", item.name));
        assert!(ABILITIES.contains(&ability), "{} needs unknown ability {ability:?}", item.name);
        assert!((1..=3).contains(&level), "{} needs ability level {level}", item.name);
    }

    let text = std::fs::read_to_string("data/aniimo_requirements.csv").unwrap();
    for line in text.lines().skip(1).filter(|l| !l.trim().is_empty()) {
        let cols: Vec<&str> = line.split(',').map(str::trim).collect();
        let item = items
            .iter()
            .find(|i| i.name == cols[0])
            .unwrap_or_else(|| panic!("aniimo_requirements.csv lists {}, which isn't in the data", cols[0]));
        assert_eq!(cols[1], item.facility, "{} is made at {}, not {}", cols[0], item.facility, cols[1]);
        assert!(item.workload.is_some(), "{} has a fixed grow time; it doesn't need an Aniimo row", cols[0]);
    }
}

// Every crop and tree needs its Aniimo jobs in grower_steps.csv so the plan can count the Aniimo
// that tend Farmland and Woodland, and nothing may be listed that isn't grown there.
#[test]
fn test_grower_steps_cover_every_crop_and_tree() {
    let Some(items) = load_items() else { return };
    let steps = aniimax::data::load_grower_steps(Path::new("data")).expect("Failed to load grower steps");
    for item in items.iter().filter(|i| i.facility == "Farmland" || i.facility == "Woodland") {
        let jobs = steps.get(&item.name);
        assert!(jobs.iter().any(|j| j.step == "Sowing"), "{} has no Sowing step in grower_steps.csv", item.name);
        assert!(jobs.iter().all(|j| j.workload > 0.0 && (1..=3).contains(&j.min_level)), "{} has a bad step", item.name);
    }

    let text = std::fs::read_to_string("data/grower_steps.csv").unwrap();
    for line in text.lines().skip(1).filter(|l| !l.trim().is_empty()) {
        let cols: Vec<&str> = line.split(',').map(str::trim).collect();
        let item = items
            .iter()
            .find(|i| i.name == cols[0])
            .unwrap_or_else(|| panic!("grower_steps.csv lists {}, which isn't in the data", cols[0]));
        assert_eq!(cols[1], item.facility, "{} is grown at {}, not {}", cols[0], item.facility, cols[1]);
    }
}

// Every recipe listed as not yet checked in game exists, at the facility the list says.
#[test]
fn test_unverified_list_names_real_recipes() {
    let Some(items) = load_items() else { return };
    let listed = aniimax::data::load_unverified(Path::new("data")).expect("Failed to load unverified.csv");
    assert!(!listed.is_empty());
    for (name, facility) in &listed {
        let item = items
            .iter()
            .find(|i| &i.name == name && &i.facility == facility)
            .unwrap_or_else(|| panic!("unverified.csv lists {name} at {facility}, which isn't in the data"));
        assert_eq!(&item.facility, facility);
    }
}

// Every processor recipe takes its facility's own ability: a Crafting Table recipe needs
// Artisanship, a Blazing Stove one Fire, and so on. The web's facility list is where those
// abilities are written down, so the two are checked against each other. Crops are exempt; each
// of their jobs (sowing, watering, reaping) has an ability of its own, in grower_steps.csv.
#[test]
fn test_recipes_take_their_facility_ability() {
    let js = std::fs::read_to_string("web/facility-config.js").expect("the web facility list");
    let mut ability_of: Vec<(String, String)> = Vec::new();
    for entry in js.split("name: '").skip(1) {
        let Some(facility) = entry.split('\'').next() else { continue };
        // Only up to the end of this entry, so a facility without an ability doesn't borrow the
        // next one's.
        let entry = entry.split("\n    }").next().unwrap_or(entry);
        if let Some(rest) = entry.split("ability: '").nth(1) {
            if let Some(ability) = rest.split('\'').next() {
                ability_of.push((facility.to_string(), ability.to_string()));
            }
        }
    }
    assert!(ability_of.len() > 10, "read {} facility abilities", ability_of.len());

    let csv = std::fs::read_to_string("data/aniimo_requirements.csv").expect("aniimo_requirements.csv");
    let mut checked = 0;
    for line in csv.lines().skip(1).filter(|l| !l.trim().is_empty()) {
        let cols: Vec<&str> = line.split(',').map(str::trim).collect();
        let [name, facility, ability, ..] = cols[..] else { continue };
        let Some((_, wanted)) = ability_of.iter().find(|(f, _)| f == facility) else { continue };
        assert_eq!(ability, wanted, "{name} at the {facility} needs {wanted}, not {ability}");
        checked += 1;
    }
    assert!(checked > 100, "only checked {checked} recipes");
}
