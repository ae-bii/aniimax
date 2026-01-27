//! Tests for data models and structures.

use aniimax::models::{FacilityCounts, ProductionItem};

fn default_facility_counts() -> FacilityCounts {
    FacilityCounts {
        farmland: (4, 3),
        woodland: (2, 2),
        mineral_pile: (1, 1),
        carousel_mill: (2, 2),
        jukebox_dryer: (1, 1),
        crafting_table: (1, 1),
        dance_pad_polisher: (1, 1),
        aniipod_maker: (1, 1),
        nimbus_bed: (1, 1),
    }
}

#[test]
fn test_facility_counts_get_count() {
    let counts = default_facility_counts();

    assert_eq!(counts.get_count("Farmland"), 4);
    assert_eq!(counts.get_count("Woodland"), 2);
    assert_eq!(counts.get_count("Mineral Pile"), 1);
    assert_eq!(counts.get_count("Carousel Mill"), 2);
    assert_eq!(counts.get_count("Unknown"), 1); // Default for unknown
}

#[test]
fn test_facility_counts_get_level() {
    let counts = default_facility_counts();

    assert_eq!(counts.get_level("Farmland"), 3);
    assert_eq!(counts.get_level("Woodland"), 2);
    assert_eq!(counts.get_level("Carousel Mill"), 2);
    assert_eq!(counts.get_level("Unknown"), 1); // Default for unknown
}

#[test]
fn test_facility_counts_can_produce() {
    let counts = default_facility_counts();

    // Farmland at level 3 can produce level 1, 2, 3 items
    assert!(counts.can_produce("Farmland", 1));
    assert!(counts.can_produce("Farmland", 2));
    assert!(counts.can_produce("Farmland", 3));
    assert!(!counts.can_produce("Farmland", 4));

    // Woodland at level 2 can produce level 1, 2 items
    assert!(counts.can_produce("Woodland", 1));
    assert!(counts.can_produce("Woodland", 2));
    assert!(!counts.can_produce("Woodland", 3));

    // Mineral Pile at level 1 can only produce level 1 items
    assert!(counts.can_produce("Mineral Pile", 1));
    assert!(!counts.can_produce("Mineral Pile", 2));
}

#[test]
fn test_production_item_creation() {
    let item = ProductionItem {
        name: "wheat".to_string(),
        facility: "Farmland".to_string(),
        raw_materials: None,
        required_amount: None,
        cost: Some(0.0),
        sell_currency: "coins".to_string(),
        sell_value: 1.0,
        production_time: 90.0,
        yield_amount: 10,
        energy: Some(809.0),
        facility_level: 1,
        module_requirement: None,
        requires_fertilizer: false,
    };

    assert_eq!(item.name, "wheat");
    assert_eq!(item.facility, "Farmland");
    assert!(item.raw_materials.is_none());
    assert_eq!(item.sell_value, 1.0);
    assert_eq!(item.yield_amount, 10);
}

#[test]
fn test_processed_item_creation() {
    let item = ProductionItem {
        name: "wheatmeal".to_string(),
        facility: "Carousel Mill".to_string(),
        raw_materials: Some(vec!["wheat".to_string()]),
        required_amount: Some(vec![2]),
        cost: None,
        sell_currency: "coins".to_string(),
        sell_value: 25.0,
        production_time: 300.0,
        yield_amount: 1,
        energy: Some(3000.0),
        facility_level: 1,
        module_requirement: None,
        requires_fertilizer: false,
    };

    assert_eq!(item.name, "wheatmeal");
    assert_eq!(item.raw_materials, Some(vec!["wheat".to_string()]));
    assert_eq!(item.required_amount, Some(vec![2]));
}
