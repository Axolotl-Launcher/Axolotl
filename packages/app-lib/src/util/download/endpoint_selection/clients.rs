use super::EndpointSelector;
use super::model::{EndpointLease, PoolKey, Protocol};
use crate::util::fetch::{DownloadClients, ProxyPolicy};
use parking_lot::Mutex;
use std::collections::VecDeque;
use url::Url;

#[derive(Default)]
pub(super) struct PinnedClients {
    entries: Mutex<VecDeque<(PoolKey, reqwest::Client)>>,
    #[cfg(test)]
    roots: Mutex<Vec<reqwest::Certificate>>,
}

impl PinnedClients {
    pub(super) fn clear(&self) {
        self.entries.lock().clear();
    }

    fn get(&self, lease: &EndpointLease) -> crate::Result<reqwest::Client> {
        {
            let mut entries = self.entries.lock();
            entries.retain(|(key, _)| {
                key.network == lease.pool.network
                    && key.generation == lease.pool.generation
            });
            if let Some(index) =
                entries.iter().position(|(key, _)| *key == lease.pool)
            {
                let entry = entries.remove(index).unwrap();
                let client = entry.1.clone();
                entries.push_back(entry);
                return Ok(client);
            }
        }
        // Never hold the cache or selector state lock while a client loads
        // platform roots. No DNS override is written into the shared resolver.
        let builder = crate::util::fetch::file_reqwest_client_builder()
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .no_proxy()
            .danger_accept_invalid_certs(false)
            .pool_max_idle_per_host(0)
            .resolve(super::super::modrinth_cdn::HOST, lease.pool.endpoint);
        #[cfg(test)]
        let builder =
            self.roots.lock().iter().fold(builder, |builder, root| {
                builder.add_root_certificate(root.clone())
            });
        let client = match lease.pool.protocol {
            Protocol::Http1 => builder.http1_only(),
            Protocol::H2 => builder.http2_prior_knowledge(),
        }
        .build()?;
        let mut entries = self.entries.lock();
        // Concurrent builders may finish together. Share the already cached
        // handle instead of creating independent active pools for one key.
        if let Some((_, existing)) =
            entries.iter().find(|(key, _)| *key == lease.pool)
        {
            return Ok(existing.clone());
        }
        entries.push_back((lease.pool.clone(), client.clone()));
        while entries.len() > 8 {
            entries.pop_front();
        }
        Ok(client)
    }

    #[cfg(test)]
    pub(crate) fn trust_test_root(&self, root: reqwest::Certificate) {
        self.roots.lock().push(root);
    }
}

impl EndpointSelector {
    pub(crate) fn client_for(
        &self,
        url: &Url,
        proxy: ProxyPolicy,
        clients: &DownloadClients,
        http1_only: bool,
    ) -> Option<(reqwest::Client, EndpointLease)> {
        if !Self::eligible_network(clients, proxy) {
            return None;
        }
        let lease = if http1_only {
            self.lease(url, Protocol::Http1)
        } else {
            self.lease(url, Protocol::H2)
                .or_else(|| self.lease(url, Protocol::Http1))
        }?;
        let client = self.pinned_clients.get(&lease).ok()?;
        self.is_current(&lease).then_some((client, lease))
    }
}
