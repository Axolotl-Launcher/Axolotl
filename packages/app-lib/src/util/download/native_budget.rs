//! Per-authority physical connection admission; transfer weights use the shared download semaphore.

use crate::util::download::constants::{
    NATIVE_BUDGET_AUTHORITY_BATCH_CONTAINS_ENOUGH_PERMITS,
    NATIVE_BUDGET_GLOBAL_BATCH_CONTAINS_ENOUGH_PERMITS,
    NATIVE_BUDGET_REGULAR_BATCH_CONTAINS_ENOUGH_PERMITS,
};
use crate::util::fetch::{DownloadRoute, ProxyPolicy};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, TryAcquireError};

use super::constants::{
    MAX_CONNECTIONS_PER_AUTHORITY, MAX_PHYSICAL_DOWNLOAD_CONNECTIONS,
};
static PHYSICAL_CONNECTIONS: LazyLock<Arc<Semaphore>> = LazyLock::new(|| {
    Arc::new(Semaphore::new(MAX_PHYSICAL_DOWNLOAD_CONNECTIONS))
});

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

fn regular_budget(route: &DownloadRoute) -> Option<Arc<Semaphore>> {
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

fn budget(route: &DownloadRoute) -> Option<Arc<Semaphore>> {
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

pub(crate) async fn acquire(
    route: &DownloadRoute,
) -> Result<NativeBudgetPermit, tokio::sync::AcquireError> {
    acquire_with_global(route, &PHYSICAL_CONNECTIONS).await
}

async fn acquire_with_global(
    route: &DownloadRoute,
    global: &Arc<Semaphore>,
) -> Result<NativeBudgetPermit, tokio::sync::AcquireError> {
    let regular = match regular_budget(route) {
        Some(budget) => {
            if budget.available_permits() == 0 {
                super::h2_pool::evict_idle_connections(Some(
                    &super::proxy_context::authority_key(
                        &crate::util::fetch::url_authority(&route.url).unwrap(),
                        route.proxy,
                    ),
                ))
                .await;
            }
            Some(budget.acquire_owned().await?)
        }
        None => None,
    };
    let authority = match budget(route) {
        Some(budget) => {
            if budget.available_permits() == 0 {
                super::h2_pool::evict_idle_connections(Some(
                    &super::proxy_context::authority_key(
                        &crate::util::fetch::url_authority(&route.url).unwrap(),
                        route.proxy,
                    ),
                ))
                .await;
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
    let regular_budget = regular_budget(route);
    let count = count.min(if regular_budget.is_some() {
        MAX_CONNECTIONS_PER_AUTHORITY - 1
    } else {
        MAX_CONNECTIONS_PER_AUTHORITY
    });
    let mut regular = match regular_budget {
        Some(budget) => {
            if budget.available_permits() < count {
                super::h2_pool::evict_idle_connections(Some(
                    &super::proxy_context::authority_key(
                        &crate::util::fetch::url_authority(&route.url).unwrap(),
                        route.proxy,
                    ),
                ))
                .await;
            }
            Some(budget.acquire_many_owned(count as u32).await?)
        }
        None => None,
    };
    let mut authority = match budget(route) {
        Some(budget) => {
            if budget.available_permits() < count {
                super::h2_pool::evict_idle_connections(Some(
                    &super::proxy_context::authority_key(
                        &crate::util::fetch::url_authority(&route.url).unwrap(),
                        route.proxy,
                    ),
                ))
                .await;
            }
            Some(
                budget
                    .acquire_many_owned(
                        count.min(MAX_CONNECTIONS_PER_AUTHORITY) as u32,
                    )
                    .await?,
            )
        }
        None => None,
    };
    if PHYSICAL_CONNECTIONS.available_permits() < count {
        super::h2_pool::evict_idle_connections(None).await;
    }
    let mut global = PHYSICAL_CONNECTIONS
        .clone()
        .acquire_many_owned(count as u32)
        .await?;
    Ok((0..count.min(MAX_CONNECTIONS_PER_AUTHORITY))
        .map(|_| NativeBudgetPermit {
            _regular: regular.as_mut().map(|permit| {
                permit
                    .split(1)
                    .expect(NATIVE_BUDGET_REGULAR_BATCH_CONTAINS_ENOUGH_PERMITS)
            }),
            _global: global
                .split(1)
                .expect(NATIVE_BUDGET_GLOBAL_BATCH_CONTAINS_ENOUGH_PERMITS),
            _authority: authority.as_mut().map(|permit| {
                permit.split(1).expect(
                    NATIVE_BUDGET_AUTHORITY_BATCH_CONTAINS_ENOUGH_PERMITS,
                )
            }),
        })
        .collect())
}

pub(crate) fn try_acquire(
    route: &DownloadRoute,
) -> Result<NativeBudgetPermit, TryAcquireError> {
    let regular = regular_budget(route)
        .map(|budget| budget.try_acquire_owned())
        .transpose()?;
    let mut permit = try_acquire_recovery(route)?;
    permit._regular = regular;
    Ok(permit)
}

pub(crate) fn try_acquire_recovery(
    route: &DownloadRoute,
) -> Result<NativeBudgetPermit, TryAcquireError> {
    Ok(NativeBudgetPermit {
        _regular: None,
        _global: PHYSICAL_CONNECTIONS.clone().try_acquire_owned()?,
        _authority: match budget(route) {
            Some(budget) => Some(budget.try_acquire_owned()?),
            None => None,
        },
    })
}

pub(crate) fn available(route: &DownloadRoute) -> usize {
    budget(route)
        .map(|budget| budget.available_permits())
        .unwrap_or(MAX_CONNECTIONS_PER_AUTHORITY)
        .min(PHYSICAL_CONNECTIONS.available_permits())
}

pub(crate) fn in_use(route: &DownloadRoute) -> usize {
    budget(route).map_or(0, |budget| {
        MAX_CONNECTIONS_PER_AUTHORITY.saturating_sub(budget.available_permits())
    })
}

/// Admission changes with active transfers; it must not permanently reduce a
/// file's growth ceiling when that file starts during a busy batch.
pub(crate) fn capacity(route: &DownloadRoute) -> usize {
    MAX_CONNECTIONS_PER_AUTHORITY - usize::from(regular_budget(route).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::fetch::DownloadRouteSource;

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
