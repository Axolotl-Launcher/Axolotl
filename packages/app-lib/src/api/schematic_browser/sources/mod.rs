//! Source adapters, one file per upstream site.

pub mod buildschematics;
pub mod mc_schematics;
pub mod reden;

use std::sync::Arc;

use super::source::SchematicBrowseSource;

/// Every compiled-in source, in registry order.
pub fn all() -> Vec<Arc<dyn SchematicBrowseSource>> {
    vec![
        Arc::new(reden::RedenSource),
        Arc::new(buildschematics::BuildSchematicsSource),
        Arc::new(mc_schematics::McSchematicsSource),
    ]
}
