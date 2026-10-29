//! Cache of dependency resolution plans exchanged between preview and install.

use crate::state::DependencyResolutionPlan;
use dashmap::DashMap;
use std::sync::LazyLock;
use std::time::Instant;

#[derive(Clone)]
pub(super) struct CachedDependencyResolutionPlan {
    pub(super) plan: DependencyResolutionPlan,
    pub(super) expires_at: Instant,
}

pub(super) static DEPENDENCY_RESOLUTION_PLANS: LazyLock<
    DashMap<String, CachedDependencyResolutionPlan>,
> = LazyLock::new(DashMap::new);
