use super::model::{
    EndpointCandidate, ProbeFailure, ProbeTarget, Protocol, ThroughputSample,
};
use super::policy::{self, ProbeBudget};
use crate::util::download::constants::MAX_CERTIFICATE_DIAGNOSTIC_CHARS;
use crate::util::download::constants::{
    DIAGNOSTICS_CERTIFICATE, IDENTITY_ENCODING, MODRINTH_DATA_PREFIX,
    PROBE_CERTNOTVALIDFORNAME, PROBE_X_CACHE, PROBE_X_SERVED_BY,
};
use crate::util::download::native_budget::NativeBudgetPermit;
use crate::util::fetch::DownloadRoute;
use parking_lot::Mutex;
use reqwest::{StatusCode, Version, header};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

static NEXT_PROBE_CONNECTION: AtomicU64 = AtomicU64::new(1);

/// Field order is intentional: drop the client/pool before its physical
/// admission permit. Responses are local to a method borrowing this session.
pub(super) struct ProbeSession {
    client: reqwest::Client,
    _permit: NativeBudgetPermit,
    candidate: EndpointCandidate,
    target: ProbeTarget,
    bytes_received: Arc<AtomicU64>,
    trials: Arc<super::tuning::TrialRegistry>,
}

#[derive(Debug)]
pub(super) struct ProbeObservation {
    pub(super) source: super::model::CandidateSource,
    pub(super) body_elapsed: Duration,
    pub(super) sample: ThroughputSample,
    pub(super) remote_addr: std::net::SocketAddr,
    pub(super) ttfb: Duration,
    pub(super) x_served_by: String,
    pub(super) cache_status: String,
}

impl ProbeSession {
    pub(super) fn open(
        candidate: EndpointCandidate,
        target: ProbeTarget,
        route: &DownloadRoute,
        clients: &crate::util::fetch::DownloadClients,
        bytes_received: Arc<AtomicU64>,
    ) -> Result<Self, ProbeFailure> {
        #[cfg(test)]
        let fixture = clients.endpoints.probe_fixture.lock().as_ref().and_then(
            |fixture| {
                fixture
                    .candidates
                    .iter()
                    .find(|(address, _)| address.ip() == candidate.ip)
                    .cloned()
            },
        );
        #[cfg(test)]
        let public_candidate =
            super::candidates::is_public_ip(candidate.ip) || fixture.is_some();
        #[cfg(not(test))]
        let public_candidate = super::candidates::is_public_ip(candidate.ip);
        if !super::EndpointSelector::eligible_network(clients, route.proxy)
            || !crate::util::download::modrinth_cdn::is_current_url(&target.url)
            || !target.url.path().starts_with(MODRINTH_DATA_PREFIX)
            || target.url.query().is_some()
            || target.size == 0
            || target.region.start >= target.region.end
            || target.region.end > target.size
            || candidate.expires_at <= Instant::now()
            || !public_candidate
        {
            return Err(ProbeFailure::Ineligible);
        }
        let permit =
            crate::util::download::native_budget::try_acquire_recovery(route)
                .map_err(|_| ProbeFailure::BudgetExhausted)?;
        #[cfg(test)]
        let mut target = target;
        #[cfg(test)]
        if let Some((address, _)) = &fixture {
            target.url.set_port(Some(address.port())).unwrap();
        }
        let builder = probe_client_builder(&candidate, &target);
        #[cfg(test)]
        let builder = if let Some((_, root)) = fixture {
            builder.add_root_certificate(root)
        } else {
            builder
        };
        let client = builder.build().map_err(|_| ProbeFailure::Transport)?;
        Ok(Self::with_client(
            candidate,
            target,
            client,
            permit,
            bytes_received,
            clients.endpoints.trials.clone(),
        ))
    }

    fn with_client(
        candidate: EndpointCandidate,
        target: ProbeTarget,
        client: reqwest::Client,
        permit: NativeBudgetPermit,
        bytes_received: Arc<AtomicU64>,
        trials: Arc<super::tuning::TrialRegistry>,
    ) -> Self {
        Self {
            client,
            _permit: permit,
            candidate,
            target,
            bytes_received,
            trials,
        }
    }

    pub(super) async fn validate_endpoint(
        &self,
        budget: &Mutex<ProbeBudget>,
        cancellation: &CancellationToken,
        deadline: Instant,
    ) -> Result<ProbeObservation, ProbeFailure> {
        self.request(
            policy::VALIDATION_BYTES,
            true,
            budget,
            cancellation,
            deadline.min(Instant::now() + policy::VALIDATION_TIMEOUT),
        )
        .await
    }

    pub(super) async fn sample_endpoint(
        &self,
        budget: &Mutex<ProbeBudget>,
        cancellation: &CancellationToken,
        deadline: Instant,
    ) -> Result<ProbeObservation, ProbeFailure> {
        self.request(
            policy::SAMPLE_BYTES,
            false,
            budget,
            cancellation,
            deadline.min(Instant::now() + policy::SAMPLE_TIMEOUT),
        )
        .await
    }

    async fn request(
        &self,
        wanted: u64,
        require_complete: bool,
        budget: &Mutex<ProbeBudget>,
        cancellation: &CancellationToken,
        deadline: Instant,
    ) -> Result<ProbeObservation, ProbeFailure> {
        let operation = async {
            if deadline <= Instant::now()
                || self.candidate.expires_at <= Instant::now()
            {
                return Err(ProbeFailure::Timeout);
            }
            let length = budget.lock().reserve(
                wanted.min(self.target.region.end - self.target.region.start),
                Instant::now(),
            );
            if length == 0 {
                return Err(ProbeFailure::BudgetExhausted);
            }
            let started_at = Instant::now();
            let trial_revision = self.trials.snapshot(started_at).0;
            let mut contaminated =
                crate::util::download::local_resources::network_pressure()
                    .blocks_expansion();
            let connection_id =
                NEXT_PROBE_CONNECTION.fetch_add(1, Ordering::Relaxed);
            let mut response = tokio::time::timeout_at(
                deadline.into(),
                self.client
                    .get(self.target.url.clone())
                    .header(
                        header::RANGE,
                        format!(
                            "bytes={}-{}",
                            self.target.region.start,
                            self.target.region.start + length - 1
                        ),
                    )
                    .header(header::ACCEPT_ENCODING, IDENTITY_ENCODING)
                    .send(),
            )
            .await
            .map_err(|_| ProbeFailure::Timeout)?
            .map_err(classify_transport)?;
            let ttfb = started_at.elapsed();
            match response.status() {
                StatusCode::OK => return Err(ProbeFailure::RangeUnsupported),
                StatusCode::NOT_FOUND => {
                    return Err(ProbeFailure::TargetMissing);
                }
                StatusCode::TOO_MANY_REQUESTS
                | StatusCode::SERVICE_UNAVAILABLE => {
                    return Err(ProbeFailure::ServerCooldown {
                        status: response.status().as_u16(),
                        retry_after: crate::util::fetch::retry_after(&response)
                            .unwrap_or(policy::COOLDOWN),
                    });
                }
                status if status.is_redirection() => {
                    return Err(ProbeFailure::Redirect);
                }
                StatusCode::PARTIAL_CONTENT => {}
                _ => return Err(ProbeFailure::HttpStatus),
            }
            validate_headers(
                response.headers(),
                self.target.region.start,
                length,
                self.target.size,
            )?;
            let remote_addr =
                response.remote_addr().ok_or(ProbeFailure::WrongPeer)?;
            if remote_addr.ip() != self.candidate.ip {
                return Err(ProbeFailure::WrongPeer);
            }
            let expected = match self.target.protocol {
                Protocol::Http1 => Version::HTTP_11,
                Protocol::H2 => Version::HTTP_2,
            };
            if response.version() != expected {
                return Err(ProbeFailure::WrongProtocol);
            }
            let x_served_by =
                bounded_header(response.headers(), PROBE_X_SERVED_BY);
            let cache_status =
                bounded_header(response.headers(), PROBE_X_CACHE);
            let body_started = Instant::now();
            let mut received = 0u64;
            let mut complete = false;
            loop {
                let chunk =
                    tokio::time::timeout_at(deadline.into(), response.chunk())
                        .await;
                match chunk {
                    Err(_) if !require_complete && received > 0 => break,
                    Err(_) => return Err(ProbeFailure::Timeout),
                    Ok(Err(error)) => {
                        return Err(if error.is_timeout() {
                            ProbeFailure::Timeout
                        } else {
                            ProbeFailure::Truncated
                        });
                    }
                    Ok(Ok(None)) => {
                        complete = true;
                        break;
                    }
                    Ok(Ok(Some(chunk))) => {
                        contaminated |= crate::util::download::local_resources::network_pressure().blocks_expansion()
                            || self.trials.snapshot(Instant::now()).0 != trial_revision;
                        self.bytes_received
                            .fetch_add(chunk.len() as u64, Ordering::Relaxed);
                        received = received.saturating_add(chunk.len() as u64);
                        if received > length {
                            return Err(ProbeFailure::InvalidRange);
                        }
                    }
                }
            }
            if complete && received != length {
                return Err(ProbeFailure::Truncated);
            }
            if require_complete && !complete {
                return Err(ProbeFailure::Truncated);
            }
            let observed_at = Instant::now();
            let complete_object = complete
                && received
                    == self.target.region.end - self.target.region.start;
            Ok(ProbeObservation {
                source: self.candidate.source.clone(),
                body_elapsed: observed_at.saturating_duration_since(body_started),
                sample: ThroughputSample {
                    ip: self.candidate.ip,
                    protocol: self.target.protocol,
                    connection_id,
                    bytes: received,
                    // Complete unfinished regions compare total completion cost. A
                    // cold handshake must not disappear into a body-only
                    // rate inferred from a millisecond-long cached response.
                    elapsed: observed_at.saturating_duration_since(
                        if complete_object {
                            started_at
                        } else {
                            body_started
                        },
                    ),
                    observed_at,
                    complete_object,
                    contaminated: contaminated
                        || crate::util::download::local_resources::network_pressure().blocks_expansion()
                        || self.trials.snapshot(observed_at).0 != trial_revision,
                },
                remote_addr,
                ttfb,
                x_served_by,
                cache_status,
            })
        };
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(ProbeFailure::Cancelled),
            result = operation => result,
        }
    }
}

fn probe_client_builder(
    candidate: &EndpointCandidate,
    target: &ProbeTarget,
) -> reqwest::ClientBuilder {
    let builder = crate::util::fetch::file_reqwest_client_builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .danger_accept_invalid_certs(false)
        .https_only(true)
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        // A session has exactly one outstanding request and no idle sockets.
        .pool_max_idle_per_host(0)
        .resolve(
            crate::util::download::modrinth_cdn::HOST,
            std::net::SocketAddr::new(
                candidate.ip,
                target.url.port_or_known_default().unwrap_or(443),
            ),
        );
    match target.protocol {
        Protocol::Http1 => builder.http1_only(),
        Protocol::H2 => builder.http2_prior_knowledge(),
    }
}

pub(crate) fn invalid_certificate(error: &reqwest::Error) -> bool {
    let detail = format!("{error:?}").to_ascii_lowercase();
    detail.contains(DIAGNOSTICS_CERTIFICATE)
        || detail.contains(PROBE_CERTNOTVALIDFORNAME)
}

fn classify_transport(error: reqwest::Error) -> ProbeFailure {
    if error.is_timeout() {
        return ProbeFailure::Timeout;
    }
    if invalid_certificate(&error) {
        ProbeFailure::InvalidCertificate
    } else if error.is_body() {
        ProbeFailure::Truncated
    } else {
        ProbeFailure::Transport
    }
}

fn validate_headers(
    headers: &header::HeaderMap,
    start: u64,
    length: u64,
    total: u64,
) -> Result<(), ProbeFailure> {
    if headers
        .get(header::CONTENT_ENCODING)
        .is_some_and(|value| value != IDENTITY_ENCODING)
    {
        return Err(ProbeFailure::InvalidEncoding);
    }
    let Some(end) =
        start.checked_add(length).and_then(|end| end.checked_sub(1))
    else {
        return Err(ProbeFailure::InvalidRange);
    };
    if length == 0 || end >= total {
        return Err(ProbeFailure::InvalidRange);
    }
    let expected = format!("bytes {start}-{end}/{total}");
    if headers
        .get(header::CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        != Some(expected.as_str())
        || headers
            .get(header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            != Some(length)
    {
        return Err(ProbeFailure::InvalidRange);
    }
    Ok(())
}

fn bounded_header(headers: &header::HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .chars()
        .filter(|ch| !ch.is_control())
        .take(MAX_CERTIFICATE_DIAGNOSTIC_CHARS)
        .collect()
}

#[cfg(test)]
#[path = "tests/probe.rs"]
mod tests;
