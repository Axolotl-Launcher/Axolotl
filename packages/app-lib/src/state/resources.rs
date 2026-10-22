//! Durable identities and business associations in the launcher database.
//!
//! Search and metadata reads never call `ensure_project`. Only a selected
//! project or a persisted business reference creates a durable resource.

pub use super::instances::adapters::sqlite::resource_rows::ResourceStore;
