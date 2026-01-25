//! Tests for production optimization algorithms.

use aniimax::data::load_all_data;
use aniimax::models::{FacilityCounts, ModuleLevels};
use aniimax::optimizer::{calculate_efficiencies, find_best_production_path};
use std::path::Path;

fn default_facility_counts() -> FacilityCounts {
    FacilityCounts {
        farmland: (1, 3),
        woodland: (1, 3),
        mineral_pile: (1, 3),
        carousel_mill: (1, 3),
        jukebox_dryer: (1, 3),
        crafting_table: (1, 3),
        dance_pad_polisher: (1, 3),
        aniipod_maker: (1, 3),
    }
}

fn default_module_levels() -> ModuleLevels {
    ModuleLevels::default()
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
fn test_calculate_efficiencies_coupons() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        return;
    }

    let items = load_all_data(data_dir).expect("Failed to load data");
    let counts = default_facility_counts();
    let modules = default_module_levels();

    let efficiencies = calculate_efficiencies(&items, "coupons", &counts, &modules);

    for eff in &efficiencies {
        assert_eq!(eff.item.sell_currency, "coupons");
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
    let counts_level_1 = FacilityCounts {
        farmland: (1, 1),
        woodland: (1, 1),
        mineral_pile: (1, 1),
        carousel_mill: (1, 1),
        jukebox_dryer: (1, 1),
        crafting_table: (1, 1),
        dance_pad_polisher: (1, 1),
        aniipod_maker: (1, 1),
    };

    // Level 3 for all
    let counts_level_3 = FacilityCounts {
        farmland: (1, 3),
        woodland: (1, 3),
        mineral_pile: (1, 3),
        carousel_mill: (1, 3),
        jukebox_dryer: (1, 3),
        crafting_table: (1, 3),
        dance_pad_polisher: (1, 3),
        aniipod_maker: (1, 3),
    };

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
    let counts_single = FacilityCounts {
        farmland: (1, 3),
        woodland: (1, 3),
        mineral_pile: (1, 3),
        carousel_mill: (1, 3),
        jukebox_dryer: (1, 3),
        crafting_table: (1, 3),
        dance_pad_polisher: (1, 3),
        aniipod_maker: (1, 3),
    };

    // Multiple facilities
    let counts_multi = FacilityCounts {
        farmland: (4, 3),
        woodland: (2, 3),
        mineral_pile: (2, 3),
        carousel_mill: (2, 3),
        jukebox_dryer: (2, 3),
        crafting_table: (2, 3),
        dance_pad_polisher: (2, 3),
        aniipod_maker: (2, 3),
    };

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
