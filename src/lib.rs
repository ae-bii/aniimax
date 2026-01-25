//! # Aniimax
//!
//! A command-line tool and library for optimizing production paths in Aniimo Homeland.
//!
//! This crate provides functionality to calculate the most efficient way to produce
//! a target amount of in-game currency (coins or coupons) based on:
//!
//! - Available production items and their recipes
//! - Production times and yields
//! - Energy consumption
//! - Number of available facilities for parallel production
//! - Facility levels
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
//!     models::FacilityCounts,
//!     display::display_results,
//! };
//! use std::path::Path;
//!
//! // Load production data
//! let items = load_all_data(Path::new("data")).unwrap();
//!
//! // Define facility counts and levels: (count, level)
//! let counts = FacilityCounts {
//!     farmland: (4, 3),        // 4 farmlands at level 3
//!     woodland: (2, 2),
//!     mineral_pile: (1, 1),
//!     carousel_mill: (2, 2),
//!     jukebox_dryer: (1, 1),
//!     crafting_table: (1, 1),
//!     dance_pad_polisher: (1, 1),
//!     aniipod_maker: (1, 1),
//! };
//!
//! // Calculate efficiencies for coins
//! let efficiencies = calculate_efficiencies(&items, "coins", &counts);
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

pub mod data;
pub mod display;
pub mod models;
pub mod optimizer;
pub mod wasm;
