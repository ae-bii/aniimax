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

#[test]
fn test_currency_types() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }

    let items = load_all_data(data_dir).expect("Failed to load data");

    for item in &items {
        assert!(
            item.sell_currency == "coins" || item.sell_currency == "coupons",
            "Currency should be 'coins' or 'coupons', got: {}",
            item.sell_currency
        );
    }
}
