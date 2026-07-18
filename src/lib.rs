//! # Aniimax
//!
//! A command-line tool and library for optimizing production paths in Aniimo Homeland.
//!
//! This crate provides functionality to calculate the most efficient way to produce
//! a target amount of in-game currency (coins or Bud Tickets) based on:
//!
//! - Available production items and their recipes
//! - Production times and yields
//! - Energy consumption
//! - Number of available facilities for parallel production
//! - Facility levels
//! - Item upgrade module levels
//!
//! ## Modules
//!
//! - [`models`] - Core data structures for production items, paths, and efficiencies
//! - [`data`] - CSV data loading functionality
//! - [`optimizer`] - Production optimization algorithms
//! - [`display`] - Output formatting and display utilities
//!
//! ## Example Usage
//!
//! ```no_run
//! use aniimax::{
//!     data::load_all_data,
//!     optimizer::{calculate_efficiencies, find_best_production_path},
//!     models::{FacilityCounts, ModuleLevels},
//!     display::display_results,
//! };
//! use std::path::Path;
//!
//! // Load production data
//! let items = load_all_data(Path::new("data")).unwrap();
//!
//! // Define facility counts and levels: (name, count, level)
//! let counts = FacilityCounts::from_pairs(&[
//!     ("Farmland", 4, 3),        // 4 farmlands at level 3
//!     ("Woodland", 2, 2),
//!     ("Mineral Pile", 1, 1),
//!     ("Carousel Mill", 2, 2),
//!     ("Jukebox Dryer", 1, 1),
//!     ("Crafting Table", 1, 1),
//!     ("Nimbus Bed", 1, 1),      // Produces Wool and Petals
//! ]);
//!
//! // Define module levels (0 = not unlocked)
//! let modules = ModuleLevels::default();
//!
//! // Calculate efficiencies for coins
//! let efficiencies = calculate_efficiencies(&items, "coins", &counts, &modules);
//!
//! // Find the best path to make 5000 coins
//! if let Some(path) = find_best_production_path(&efficiencies, 5000.0, false, 0.0, &counts) {
//!     display_results(&path, &efficiencies, false);
//! }
//! ```
//!
//! ## Optimization Modes
//!
//! The optimizer supports two modes:
//!
//! 1. **Time Optimization** (default): Finds the fastest way to reach your currency goal,
//!    considering parallel production with multiple facilities.
//!
//! 2. **Energy Optimization**: Finds the most energy-efficient production path,
//!    useful when energy is a limited resource.

pub mod coverage;
pub mod data;
pub mod display;
pub mod models;
pub mod optimizer;
pub mod wasm;

use serde::{Deserialize, Deserializer};

/// Custom deserializer for optional f64 that handles empty strings.
/// Returns None for empty strings, Some(value) for valid floats.
pub fn deserialize_optional_f64<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        trimmed
            .parse::<f64>()
            .map(Some)
            .map_err(serde::de::Error::custom)
    }
}
