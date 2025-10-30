#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod types;
pub mod config;
pub mod api;

// Temporarily disable complex modules to get basic compilation working
// pub mod utils;
// pub mod amms;
// pub mod state_space;
pub mod security;
pub mod wallet;
// pub mod queue;  // Has complex dependencies, needs coordination
// // pub mod cache;  // Has dependencies on other modules, needs coordination
// pub mod database;  // Disabled until we set up offline sqlx or use runtime queries
// pub mod services;

pub use types::*;
pub use config::*;
