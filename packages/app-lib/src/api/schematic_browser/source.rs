//! Source abstraction and registry.
//!
//! Each upstream site implements [`SchematicBrowseSource`]. The command layer
//! only ever talks to the trait, so adding a source never touches commands.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::error::SourceResult;
use super::types::{
    DownloadPlan, GenerateSize, ListQuery, ListResult, SchematicDetail,
    SourceCapabilities, SourceId, SourceStatus,
};

/// One browsable upstream site.
#[async_trait]
pub trait SchematicBrowseSource: Send + Sync {
    fn id(&self) -> SourceId;

    fn capabilities(&self) -> SourceCapabilities;

    /// Read one page of the source's browse listing.
    async fn list(
        &self,
        query: &ListQuery,
        cancel: &CancellationToken,
    ) -> SourceResult<ListResult>;

    /// Read one item's detail.
    async fn detail(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> SourceResult<SchematicDetail>;

    /// Resolve how to download one item.
    ///
    /// For generator-backed items this may perform the upstream generation
    /// call, which is why it takes the size arguments.
    async fn download_plan(
        &self,
        id: &str,
        size: Option<GenerateSize>,
        cancel: &CancellationToken,
    ) -> SourceResult<DownloadPlan>;

    /// Cheap liveness probe used by the settings page and by tests.
    async fn health_check(&self) -> SourceStatus;
}

type Registry = HashMap<SourceId, Arc<dyn SchematicBrowseSource>>;

static REGISTRY: LazyLock<Registry> = LazyLock::new(|| {
    let mut registry: Registry = HashMap::new();
    for source in super::sources::all() {
        registry.insert(source.id(), source);
    }
    registry
});

/// Look up one source.
pub fn get(id: SourceId) -> Option<Arc<dyn SchematicBrowseSource>> {
    REGISTRY.get(&id).cloned()
}

/// Every registered source, in stable order.
pub fn all() -> Vec<Arc<dyn SchematicBrowseSource>> {
    SourceId::all().iter().filter_map(|id| get(*id)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_source_id_is_registered() {
        for id in SourceId::all() {
            assert!(get(*id).is_some(), "source {id:?} missing from registry");
        }
    }

    #[test]
    fn registry_preserves_declared_order() {
        let ids: Vec<_> = all().iter().map(|source| source.id()).collect();
        assert_eq!(ids, SourceId::all().to_vec());
    }
}
