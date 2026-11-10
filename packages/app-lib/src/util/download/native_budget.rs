//! Per-authority physical connection admission; transfer weights use the shared download semaphore.

use crate::util::fetch::{DownloadRoute, ProxyPolicy};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, TryAcquireError};

use super::constants::{
    FORCED_DOWNLOAD_CONCURRENCY_DEFAULT, FORCED_DOWNLOAD_CONCURRENCY_MAX,
    MAX_CONNECTIONS_PER_AUTHORITY, MAX_PHYSICAL_DOWNLOAD_CONNECTIONS,
};
static PHYSICAL_CONNECTIONS: LazyLock<Arc<Semaphore>> = LazyLock::new(|| {
    Arc::new(Semaphore::new(MAX_PHYSICAL_DOWNLOAD_CONNECTIONS))
});
static FORCED_CONNECTIONS: LazyLock<Arc<Semaphore>> = LazyLock::new(|| {
    Arc::new(Semaphore::new(FORCED_DOWNLOAD_CONCURRENCY_DEFAULT))
});
static FORCED_CAPACITY: AtomicUsize =
    AtomicUsize::new(FORCED_DOWNLOAD_CONCURRENCY_DEFAULT);
static FORCED_TARGET: AtomicUsize =
    AtomicUsize::new(FORCED_DOWNLOAD_CONCURRENCY_DEFAULT);

pub(crate) fn set_forced_capacity(target: usize) {
    let target = target.clamp(1, FORCED_DOWNLOAD_CONCURRENCY_MAX);
    FORCED_TARGET.store(target, Ordering::Release);
    crate::state::grow_semaphore(&FORCED_CONNECTIONS, &FORCED_CAPACITY, target);
}

pub(crate) fn forced_capacity_needs_shrink() -> bool {
    FORCED_CAPACITY.load(Ordering::Acquire)
        > FORCED_TARGET.load(Ordering::Acquire)
}

pub(crate) async fn shrink_forced_capacity() {
    super::h2_pool::evict_idle_connections(None).await;
    crate::state::shrink_semaphore(
        &FORCED_CONNECTIONS,
        &FORCED_CAPACITY,
        &FORCED_TARGET,
    )
    .await;
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct AuthorityKey {
    authority: String,
    proxy: ProxyPolicy,
}

static AUTHORITY_BUDGETS: LazyLock<
    Mutex<HashMap<AuthorityKey, Arc<Semaphore>>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));
static REGULAR_BUDGETS: LazyLock<Mutex<HashMap<AuthorityKey, Arc<Semaphore>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) struct NativeBudgetPermit {
    _regular: Option<OwnedSemaphorePermit>,
    _authority: Option<OwnedSemaphorePermit>,
    _global: OwnedSemaphorePermit,
}

fn forced_connections() -> bool {
    crate::State::get_if_initialized()
        .is_some_and(|state| state.force_multiconnection_downloads())
}

fn regular_budget(
    route: &DownloadRoute,
    forced: bool,
) -> Option<Arc<Semaphore>> {
    if forced {
        return None;
    }
    let clients = super::proxy_context::clients()?;
    if clients.endpoints.mode()
        != super::endpoint_selection::SelectionMode::Auto
        || !super::endpoint_selection::EndpointSelector::eligible_network(
            &clients,
            route.proxy,
        )
        || !url::Url::parse(&route.url)
            .is_ok_and(|url| super::modrinth_cdn::is_current_url(&url))
    {
        return None;
    }
    let authority = crate::util::fetch::url_authority(&route.url)?;
    let key = AuthorityKey {
        authority: super::proxy_context::authority_key(&authority, route.proxy),
        proxy: route.proxy,
    };
    let mut budgets = REGULAR_BUDGETS.lock();
    if budgets.len() >= 256 {
        budgets.retain(|_, budget| Arc::strong_count(budget) > 1);
    }
    Some(
        budgets
            .entry(key)
            .or_insert_with(|| {
                Arc::new(Semaphore::new(MAX_CONNECTIONS_PER_AUTHORITY - 1))
            })
            .clone(),
    )
}

fn budget(
    route: &DownloadRoute,
    forced: bool,
    forced_budget: &Arc<Semaphore>,
) -> Option<Arc<Semaphore>> {
    // Forced downloads share this pool across authorities and fetch semaphores.
    // The existing physical pool also continues to bound all download traffic.
    if forced {
        return Some(Arc::clone(forced_budget));
    }
    let authority = crate::util::fetch::url_authority(&route.url)?;
    let key = AuthorityKey {
        authority: super::proxy_context::authority_key(&authority, route.proxy),
        proxy: route.proxy,
    };
    let mut budgets = AUTHORITY_BUDGETS.lock();
    if budgets.len() >= 256 {
        budgets.retain(|_, budget| Arc::strong_count(budget) > 1);
    }
    Some(
        budgets
            .entry(key)
            .or_insert_with(|| {
                Arc::new(Semaphore::new(MAX_CONNECTIONS_PER_AUTHORITY))
            })
            .clone(),
    )
}

async fn evict_idle_for_budget(route: &DownloadRoute, forced: bool) {
    let authority = if forced {
        None
    } else {
        crate::util::fetch::url_authority(&route.url).map(|authority| {
            super::proxy_context::authority_key(&authority, route.proxy)
        })
    };
    super::h2_pool::evict_idle_connections(authority.as_deref()).await;
}

pub(crate) async fn acquire(
    route: &DownloadRoute,
) -> Result<NativeBudgetPermit, tokio::sync::AcquireError> {
    acquire_with_global(route, &PHYSICAL_CONNECTIONS).await
}

async fn acquire_with_global(
    route: &DownloadRoute,
    global: &Arc<Semaphore>,
) -> Result<NativeBudgetPermit, tokio::sync::AcquireError> {
    let forced = forced_connections();
    let regular_budget = regular_budget(route, forced);
    let authority_budget = budget(route, forced, &FORCED_CONNECTIONS);
    let regular = match regular_budget {
        Some(budget) => {
            if budget.available_permits() == 0 {
                evict_idle_for_budget(route, false).await;
            }
            Some(budget.acquire_owned().await?)
        }
        None => None,
    };
    let authority = match authority_budget {
        Some(budget) => {
            if budget.available_permits() == 0 {
                evict_idle_for_budget(route, forced).await;
            }
            Some(budget.acquire_owned().await?)
        }
        None => None,
    };
    if global.available_permits() == 0 {
        super::h2_pool::evict_idle_connections(None).await;
    }
    let global = global.clone().acquire_owned().await?;
    Ok(NativeBudgetPermit {
        _regular: regular,
        _authority: authority,
        _global: global,
    })
}

pub(crate) async fn acquire_many(
    route: &DownloadRoute,
    count: usize,
) -> Result<Vec<NativeBudgetPermit>, tokio::sync::AcquireError> {
    acquire_many_with_mode(
        route,
        count,
        forced_connections(),
        &PHYSICAL_CONNECTIONS,
        &FORCED_CONNECTIONS,
    )
    .await
}

async fn acquire_many_with_mode(
    route: &DownloadRoute,
    count: usize,
    forced: bool,
    global_budget: &Arc<Semaphore>,
    forced_budget: &Arc<Semaphore>,
) -> Result<Vec<NativeBudgetPermit>, tokio::sync::AcquireError> {
    let regular_budget = regular_budget(route, forced);
    let authority_budget = budget(route, forced, forced_budget);
    let count = count.min(if forced {
        FORCED_DOWNLOAD_CONCURRENCY_MAX
    } else if regular_budget.is_some() {
        MAX_CONNECTIONS_PER_AUTHORITY - 1
    } else {
        MAX_CONNECTIONS_PER_AUTHORITY
    });
    if count == 0 {
        return Ok(Vec::new());
    }
    if regular_budget
        .as_ref()
        .is_some_and(|pool| pool.available_permits() == 0)
    {
        evict_idle_for_budget(route, false).await;
    }
    if authority_budget
        .as_ref()
        .is_some_and(|pool| pool.available_permits() == 0)
    {
        evict_idle_for_budget(route, forced).await;
    }
    if global_budget.available_permits() == 0 {
        super::h2_pool::evict_idle_connections(None).await;
    }
    // Await one connection only so queued requests survive live limit changes.
    let regular = match &regular_budget {
        Some(pool) => Some(pool.clone().acquire_owned().await?),
        None => None,
    };
    let authority = match &authority_budget {
        Some(pool) => Some(pool.clone().acquire_owned().await?),
        None => None,
    };
    let global = global_budget.clone().acquire_owned().await?;
    let mut permits = vec![NativeBudgetPermit {
        _regular: regular,
        _authority: authority,
        _global: global,
    }];
    for _ in 1..count {
        let regular = match regular_budget
            .as_ref()
            .map(|pool| pool.clone().try_acquire_owned())
            .transpose()
        {
            Ok(permit) => permit,
            Err(_) => break,
        };
        let authority = match authority_budget
            .as_ref()
            .map(|pool| pool.clone().try_acquire_owned())
            .transpose()
        {
            Ok(permit) => permit,
            Err(_) => break,
        };
        let Ok(global) = global_budget.clone().try_acquire_owned() else {
            break;
        };
        permits.push(NativeBudgetPermit {
            _regular: regular,
            _authority: authority,
            _global: global,
        });
    }
    Ok(permits)
}

pub(crate) fn try_acquire(
    route: &DownloadRoute,
) -> Result<NativeBudgetPermit, TryAcquireError> {
    let forced = forced_connections();
    let regular = regular_budget(route, forced)
        .map(|budget| budget.try_acquire_owned())
        .transpose()?;
    let mut permit = try_acquire_recovery_with_mode(route, forced)?;
    permit._regular = regular;
    Ok(permit)
}

pub(crate) fn try_acquire_recovery(
    route: &DownloadRoute,
) -> Result<NativeBudgetPermit, TryAcquireError> {
    try_acquire_recovery_with_mode(route, forced_connections())
}

fn try_acquire_recovery_with_mode(
    route: &DownloadRoute,
    forced: bool,
) -> Result<NativeBudgetPermit, TryAcquireError> {
    Ok(NativeBudgetPermit {
        _regular: None,
        _global: PHYSICAL_CONNECTIONS.clone().try_acquire_owned()?,
        _authority: match budget(route, forced, &FORCED_CONNECTIONS) {
            Some(budget) => Some(budget.try_acquire_owned()?),
            None => None,
        },
    })
}

pub(crate) fn available(route: &DownloadRoute) -> usize {
    let forced = forced_connections();
    budget(route, forced, &FORCED_CONNECTIONS)
        .map(|budget| budget.available_permits())
        .unwrap_or_else(|| capacity_with_mode(route, forced))
        .min(PHYSICAL_CONNECTIONS.available_permits())
}

pub(crate) fn in_use(route: &DownloadRoute) -> usize {
    let forced = forced_connections();
    budget(route, forced, &FORCED_CONNECTIONS).map_or(0, |budget| {
        let capacity = if forced {
            FORCED_CAPACITY.load(Ordering::Acquire)
        } else {
            MAX_CONNECTIONS_PER_AUTHORITY
        };
        capacity.saturating_sub(budget.available_permits())
    })
}

/// Admission changes with active transfers; it must not permanently reduce a
/// file's growth ceiling when that file starts during a busy batch.
pub(crate) fn capacity(route: &DownloadRoute) -> usize {
    capacity_with_mode(route, forced_connections())
}

fn capacity_with_mode(route: &DownloadRoute, forced: bool) -> usize {
    if forced {
        FORCED_TARGET.load(Ordering::Acquire)
    } else {
        MAX_CONNECTIONS_PER_AUTHORITY
            - usize::from(regular_budget(route, false).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::fetch::DownloadRouteSource;

    #[tokio::test]
    async fn forced_mode_can_fill_the_global_budget_beyond_one_authority_cap() {
        let route = route();
        let global =
            Arc::new(Semaphore::new(MAX_PHYSICAL_DOWNLOAD_CONNECTIONS));
        let forced = Arc::new(Semaphore::new(FORCED_DOWNLOAD_CONCURRENCY_MAX));
        let permits = acquire_many_with_mode(
            &route,
            MAX_PHYSICAL_DOWNLOAD_CONNECTIONS,
            true,
            &global,
            &forced,
        )
        .await
        .unwrap();
        assert_eq!(permits.len(), FORCED_DOWNLOAD_CONCURRENCY_MAX);
        assert!(forced.clone().try_acquire_owned().is_err());
        drop(permits);
        assert_eq!(forced.available_permits(), FORCED_DOWNLOAD_CONCURRENCY_MAX);
        assert_eq!(
            global.available_permits(),
            MAX_PHYSICAL_DOWNLOAD_CONNECTIONS
        );
    }

    #[tokio::test]
    async fn forced_mode_shares_its_total_budget_across_authorities() {
        let global =
            Arc::new(Semaphore::new(MAX_PHYSICAL_DOWNLOAD_CONNECTIONS));
        let forced = Arc::new(Semaphore::new(FORCED_DOWNLOAD_CONCURRENCY_MAX));
        let first = route();
        let mut second = first.clone();
        second.url = "https://forced-second.invalid/file".into();
        let mut first_permits =
            acquire_many_with_mode(&first, 40, true, &global, &forced)
                .await
                .unwrap();
        let second_permits = acquire_many_with_mode(
            &second,
            FORCED_DOWNLOAD_CONCURRENCY_MAX - first_permits.len(),
            true,
            &global,
            &forced,
        )
        .await
        .unwrap();
        assert_eq!(forced.available_permits(), 0);
        let pending =
            acquire_many_with_mode(&second, 1, true, &global, &forced);
        tokio::pin!(pending);
        assert!(futures::poll!(pending.as_mut()).is_pending());
        drop(first_permits.pop());
        let replacement = pending.await.unwrap();
        assert_eq!(replacement.len(), 1);
        drop((first_permits, second_permits, replacement));
        assert_eq!(forced.available_permits(), FORCED_DOWNLOAD_CONCURRENCY_MAX);
        assert_eq!(
            global.available_permits(),
            MAX_PHYSICAL_DOWNLOAD_CONNECTIONS
        );
    }

    #[tokio::test]
    async fn live_resize_keeps_active_permits_and_queued_batches_satisfiable() {
        let global = Arc::new(Semaphore::new(256));
        let forced = Arc::new(Semaphore::new(256));
        let current = AtomicUsize::new(256);
        let target = AtomicUsize::new(72);
        let route = route();
        let active =
            acquire_many_with_mode(&route, 256, true, &global, &forced)
                .await
                .unwrap();
        let pending =
            acquire_many_with_mode(&route, 256, true, &global, &forced);
        tokio::pin!(pending);
        assert!(futures::poll!(pending.as_mut()).is_pending());
        let shrink = crate::state::shrink_semaphore(&forced, &current, &target);
        tokio::pin!(shrink);
        assert!(futures::poll!(shrink.as_mut()).is_pending());
        drop(active);
        let replacement =
            tokio::time::timeout(std::time::Duration::from_secs(1), pending)
                .await
                .unwrap()
                .unwrap();
        assert!(!replacement.is_empty());
        drop(replacement);
        tokio::time::timeout(std::time::Duration::from_secs(1), shrink)
            .await
            .unwrap();
        assert_eq!(current.load(Ordering::Acquire), 72);
        assert_eq!(forced.available_permits(), 72);
        crate::state::grow_semaphore(&forced, &current, 256);
        let expanded =
            acquire_many_with_mode(&route, 256, true, &global, &forced)
                .await
                .unwrap();
        assert_eq!(expanded.len(), 256);
        drop(expanded);
        assert_eq!(forced.available_permits(), 256);
    }

    #[tokio::test]
    async fn ordinary_batch_keeps_the_authority_cap_and_skips_forced_pool() {
        let global =
            Arc::new(Semaphore::new(MAX_PHYSICAL_DOWNLOAD_CONNECTIONS));
        let forced = Arc::new(Semaphore::new(FORCED_DOWNLOAD_CONCURRENCY_MAX));
        let mut route = route();
        route.url = "https://ordinary-batch.invalid/file".into();
        let permits = acquire_many_with_mode(
            &route,
            MAX_PHYSICAL_DOWNLOAD_CONNECTIONS,
            false,
            &global,
            &forced,
        )
        .await
        .unwrap();
        assert_eq!(permits.len(), MAX_CONNECTIONS_PER_AUTHORITY);
        assert_eq!(forced.available_permits(), FORCED_DOWNLOAD_CONCURRENCY_MAX);
        assert_eq!(
            budget(&route, false, &forced).unwrap().available_permits(),
            0
        );
        drop(permits);
        assert_eq!(
            global.available_permits(),
            MAX_PHYSICAL_DOWNLOAD_CONNECTIONS
        );
    }

    #[tokio::test]
    async fn global_physical_budget_bounds_multiple_authorities_and_releases_after_drop()
     {
        let global = Arc::new(Semaphore::new(2));
        let mut first = route();
        first.url = "https://global-first.invalid/file".into();
        let mut second = first.clone();
        second.url = "https://global-second.invalid/file".into();
        let a = acquire_with_global(&first, &global).await.unwrap();
        let b = acquire_with_global(&second, &global).await.unwrap();
        let pending = acquire_with_global(&first, &global);
        tokio::pin!(pending);
        assert!(futures::poll!(pending.as_mut()).is_pending());
        drop(a);
        let c = pending.await.unwrap();
        drop((b, c));
        assert_eq!(global.available_permits(), 2);
    }

    fn route() -> DownloadRoute {
        DownloadRoute {
            url: "https://budget.example/file".to_string(),
            source: DownloadRouteSource::Official,
            is_mirror: false,
            allow_sensitive_headers: true,
            supports_range: true,
            proxy: ProxyPolicy::Direct,
        }
    }

    #[tokio::test]
    async fn authority_budget_is_bounded() {
        let route = route();
        let mut permits = Vec::new();
        for _ in 0..MAX_CONNECTIONS_PER_AUTHORITY {
            permits.push(acquire(&route).await.unwrap());
        }
        assert!(matches!(
            try_acquire(&route),
            Err(TryAcquireError::NoPermits)
        ));
        drop(permits);
        assert!(try_acquire(&route).is_ok());
    }
}
