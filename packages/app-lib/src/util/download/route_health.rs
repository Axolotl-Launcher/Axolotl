//! Route health state, authority memory, and probe bookkeeping.

use super::route_policy;
use crate::util::download::constants::{
    DIAGNOSTICS_OTHER, ROUTE_HEALTH_CURSEFORGE, ROUTE_HEALTH_LOADER,
    ROUTE_HEALTH_MINECRAFT, ROUTE_HEALTH_MODRINTH,
};
use crate::util::fetch::{DownloadRoute, ProxyPolicy, ResourceClass};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Instant;
use tokio::sync::Notify;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ResourceFamily {
    Minecraft,
    Loader,
    Modrinth,
    CurseForge,
    Other,
}
impl ResourceFamily {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Minecraft => ROUTE_HEALTH_MINECRAFT,
            Self::Loader => ROUTE_HEALTH_LOADER,
            Self::Modrinth => ROUTE_HEALTH_MODRINTH,
            Self::CurseForge => ROUTE_HEALTH_CURSEFORGE,
            Self::Other => DIAGNOSTICS_OTHER,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct RouteHealthKey {
    pub(crate) family: ResourceFamily,
    pub(crate) authority: String,
    pub(crate) scope: String,
}
#[derive(Clone, Debug, Default)]
pub(crate) struct RouteHealth {
    pub(crate) success_samples: u32,
    pub(crate) ttfb_ms: Option<f64>,
    pub(crate) throughput_bps: Option<f64>,
    pub(crate) consecutive_failures: u32,
    pub(crate) cooldown_until: Option<Instant>,
}
pub(crate) static ROUTE_HEALTH: LazyLock<
    Mutex<HashMap<RouteHealthKey, RouteHealth>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) struct TaskProbeGuard {
    pub(crate) state: Arc<TaskProbeState>,
    pub(crate) family: ResourceFamily,
    pub(crate) notify: Arc<Notify>,
    pub(crate) armed: bool,
}
impl TaskProbeGuard {
    pub(crate) fn disarm(&mut self) {
        self.armed = false;
    }
}
impl Drop for TaskProbeGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let mut families = self.state.families.lock();
        if let Some(entry) = families.get_mut(&self.family)
            && entry
                .in_flight
                .as_ref()
                .is_some_and(|v| Arc::ptr_eq(v, &self.notify))
        {
            entry.in_flight = None;
            entry.last_probed = None;
        }
    }
}
pub(crate) static ROUTE_EFFECTIVE_AUTHORITIES: LazyLock<
    Mutex<HashMap<String, String>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum TaskProbeKey {
    Job(Uuid, u64),
    Anonymous(u64),
}
#[derive(Default)]
pub(crate) struct TaskProbeState {
    pub(crate) families: Mutex<HashMap<ResourceFamily, FamilyProbeState>>,
}
impl TaskProbeState {
    pub(crate) fn has_in_flight(&self) -> bool {
        self.families.lock().values().any(|f| f.in_flight.is_some())
    }
}
#[derive(Default)]
pub(crate) struct FamilyProbeState {
    pub(crate) last_probed: Option<Instant>,
    pub(crate) in_flight: Option<Arc<Notify>>,
}
pub(crate) static TASK_PROBE_STATES: LazyLock<
    Mutex<HashMap<TaskProbeKey, Arc<TaskProbeState>>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) fn resource_family(
    route: &DownloadRoute,
    resource: ResourceClass,
) -> ResourceFamily {
    match resource {
        ResourceClass::MinecraftLibrary
            if route_policy::is_loader_repository(&route.url, resource) =>
        {
            ResourceFamily::Loader
        }
        ResourceClass::Metadata
        | ResourceClass::MinecraftAsset
        | ResourceClass::MinecraftLibrary
        | ResourceClass::Java => ResourceFamily::Minecraft,
        ResourceClass::Loader => ResourceFamily::Loader,
        ResourceClass::Modrinth | ResourceClass::Modpack => {
            ResourceFamily::Modrinth
        }
        ResourceClass::CurseForge => ResourceFamily::CurseForge,
        ResourceClass::Other => ResourceFamily::Other,
    }
}
pub(crate) fn route_health_key(
    route: &DownloadRoute,
    resource: ResourceClass,
) -> Option<RouteHealthKey> {
    Some(RouteHealthKey {
        family: resource_family(route, resource),
        authority: crate::util::fetch::range_splitting_authority(route)?,
        scope: super::proxy_context::scope(route.proxy),
    })
}
pub(crate) fn persisted_route_health(
    key: &RouteHealthKey,
    proxy: ProxyPolicy,
) -> RouteHealth {
    crate::util::download::native_reputation::get(
        key.family.as_str(),
        &key.authority,
        proxy,
    )
    .map(|p| RouteHealth {
        success_samples: p.success_samples,
        ttfb_ms: p.ttfb_ms,
        throughput_bps: p.throughput_bps,
        consecutive_failures: p.consecutive_failures,
        cooldown_until: None,
    })
    .unwrap_or_default()
}
pub(crate) fn effective_route_authority(
    route: &DownloadRoute,
) -> Option<String> {
    let authority = crate::util::fetch::url_authority(&route.url)?;
    ROUTE_EFFECTIVE_AUTHORITIES
        .lock()
        .get(&super::proxy_context::authority_key(
            &route.url,
            route.proxy,
        ))
        .cloned()
        .or(Some(authority))
}
pub(crate) fn remember_effective_route_authority(
    route: &DownloadRoute,
    final_url: &str,
) {
    let (Some(original), Some(effective)) = (
        crate::util::fetch::url_authority(&route.url),
        crate::util::fetch::url_authority(final_url),
    ) else {
        return;
    };
    let mut map = ROUTE_EFFECTIVE_AUTHORITIES.lock();
    if original == effective {
        map.remove(&super::proxy_context::authority_key(
            &route.url,
            route.proxy,
        ));
    } else {
        map.insert(
            super::proxy_context::authority_key(&route.url, route.proxy),
            effective,
        );
    }
}
pub(crate) fn forget_effective_route_authority(
    route: &DownloadRoute,
    failed_url: &url::Url,
) {
    let Some(failed) = crate::util::fetch::url_authority(failed_url.as_str())
    else {
        return;
    };
    let mut map = ROUTE_EFFECTIVE_AUTHORITIES.lock();
    if map.get(&super::proxy_context::authority_key(
        &route.url,
        route.proxy,
    )) == Some(&failed)
    {
        map.remove(&super::proxy_context::authority_key(
            &route.url,
            route.proxy,
        ));
    }
}

use super::constants::ROUTE_HEALTH_EWMA_ALPHA as EWMA_ALPHA;

/// Updates a metric using the same weighted moving average for every route.
pub(crate) fn update_ewma(current: &mut Option<f64>, sample: f64) {
    *current = Some(current.map_or(sample, |current| {
        current * (1.0 - EWMA_ALPHA) + sample * EWMA_ALPHA
    }));
}

/// Stable scope identifier for task-level route probes.
pub(crate) fn probe_scope(family: &str, authorities: &mut Vec<String>) -> u64 {
    use std::hash::{Hash, Hasher};
    authorities.sort_unstable();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    family.hash(&mut hasher);
    authorities.hash(&mut hasher);
    hasher.finish()
}
