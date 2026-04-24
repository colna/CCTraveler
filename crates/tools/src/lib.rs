pub mod analyze;
pub mod definitions;
pub mod executor;
pub mod export;
pub mod scrape;
pub mod search;
// v0.2 modules
pub mod train;
pub mod flight;
pub mod route;
pub mod geo;
// v0.3 modules
pub mod cache;
pub mod distance;
pub mod metrics;
pub mod monitor;
pub mod notifier;
pub mod planner;
pub mod scheduler;
pub mod wiki;

// Re-export key types
pub use definitions::all_tool_specs;
pub use executor::{store_scraped_hotel, TravelerToolExecutor};

// Re-export runtime traits for convenience
pub use runtime::{ToolExecutor, ToolSpec};
