//! Tests for data models and structures.

use aniimax::models::{FacilityCounts, ProductionItem};

fn default_facility_counts() -> FacilityCounts {
    FacilityCounts::from_pairs(&[
        ("Farmland", 4, 3),
        ("Woodland", 2, 2),
        ("Mineral Pile", 1, 1),
        ("Carousel Mill", 2, 2),
        ("Jukebox Dryer", 1, 1),
        ("Crafting Table", 1, 1),
        ("Nimbus Bed", 1, 1),
    ])
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
fn test_facility_counts_add_tier_sums_count_and_takes_max_level() {
    let mut counts = FacilityCounts::new();
    counts.add_tier("Farmland", 5, 3);
    counts.add_tier("Farmland", 4, 5);

    assert_eq!(counts.get_count("Farmland"), 9, "get_count should sum every tier regardless of level");
    assert_eq!(counts.get_level("Farmland"), 5, "get_level should report the highest owned tier");
}

#[test]
fn test_facility_counts_capacity_at_level_respects_tiers() {
    let mut counts = FacilityCounts::new();
    counts.add_tier("Farmland", 5, 3);
    counts.add_tier("Farmland", 4, 5);

    // A higher-level tier can always run a lower-level recipe too, so every level <= 3 sees the
    // full 9; level 4-5 recipes can only use the 4 units actually upgraded that far.
    assert_eq!(counts.capacity_at_level("Farmland", 1), 9);
    assert_eq!(counts.capacity_at_level("Farmland", 3), 9);
    assert_eq!(counts.capacity_at_level("Farmland", 4), 4);
    assert_eq!(counts.capacity_at_level("Farmland", 5), 4);
    assert_eq!(counts.capacity_at_level("Farmland", 6), 0);

    assert!(counts.can_produce("Farmland", 5));
    assert!(!counts.can_produce("Farmland", 6));
}

#[test]
fn test_facility_counts_set_replaces_tiers_while_add_tier_accumulates() {
    let mut counts = FacilityCounts::new();
    counts.add_tier("Farmland", 5, 3);
    counts.set("Farmland", 2, 4); // replaces the tier(s) set above, not additive

    assert_eq!(counts.get_count("Farmland"), 2);
    assert_eq!(counts.get_level("Farmland"), 4);
    assert_eq!(counts.tiers("Farmland"), vec![(2, 4)]);
}

#[test]
fn test_facility_counts_set_tiers_replaces_wholesale() {
    let mut counts = FacilityCounts::new();
    counts.set_tiers("Farmland", vec![(5, 3), (4, 5)]);

    assert_eq!(counts.get_count("Farmland"), 9);
    assert_eq!(counts.tiers("Farmland"), vec![(5, 3), (4, 5)]);
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
        workload: None,
        byproduct: None,
        environment: None,
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
        workload: None,
        byproduct: None,
        environment: None,
    };

    assert_eq!(item.name, "wheatmeal");
    assert_eq!(item.raw_materials, Some(vec!["wheat".to_string()]));
    assert_eq!(item.required_amount, Some(vec![2]));
}
