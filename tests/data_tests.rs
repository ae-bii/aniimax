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

// quartz/quick_quartz/gem are Mineral Sand-heavy recipes: most of each batch is Mineral Sand
// byproduct, with only a small sellable quartz/gem yield (e.g. quartz yields 4, with 63 Mineral
// Sand as byproduct). Locks in these values so a future data edit can't silently swap the
// `yield`/`byproduct_yield` columns in mineral_pile.csv.
#[test]
fn test_mineral_pile_quartz_and_gem_yields_are_not_swapped_with_byproduct() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }
    let items = load_all_data(data_dir).expect("Failed to load data");

    let get = |name: &str| {
        items
            .iter()
            .find(|i| i.name == name && i.facility == "Mineral Pile")
            .unwrap_or_else(|| panic!("{name} should exist in Mineral Pile data"))
    };

    let quartz = get("quartz");
    assert_eq!(quartz.yield_amount, 4, "quartz's real sellable yield is 4, not 63");
    assert_eq!(quartz.byproduct, Some(("Mineral Sand".to_string(), 63)));

    let quick_quartz = get("quick_quartz");
    assert_eq!(quick_quartz.yield_amount, 6, "quick_quartz's real sellable yield is 6, not 63");
    assert_eq!(quick_quartz.byproduct, Some(("Mineral Sand".to_string(), 63)));
    assert!(
        quick_quartz.yield_amount > quartz.yield_amount,
        "quick_quartz should out-yield plain quartz, same as every other quick_* variant"
    );

    let gem = get("gem");
    assert_eq!(gem.yield_amount, 2, "gem's real sellable yield is 2, not 102");
    assert_eq!(gem.byproduct, Some(("Mineral Sand".to_string(), 102)));
}

#[test]
fn test_currency_types() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }

    let items = load_all_data(data_dir).expect("Failed to load data");

    for item in &items {
        assert!(
            item.sell_currency == "coins" || item.sell_currency == "bud_tickets",
            "Currency should be 'coins' or 'bud_tickets', got: {}",
            item.sell_currency
        );
    }
}
