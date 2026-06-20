//! Search backends module

pub mod duckduckgo;
pub mod searxng;
pub mod wikipedia;

pub use duckduckgo::search as search_duckduckgo;
pub use searxng::search as search_searxng;
pub use wikipedia::search as search_wikipedia;