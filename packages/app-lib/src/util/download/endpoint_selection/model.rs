use crate::util::download::constants::{
    DIAGNOSTICS_DNS, MODEL_FASTLY_RULE, MODEL_HISTORY, MODEL_VERIFIED,
};
use crate::util::download_dns::DnsSource;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CandidateSource {
    Dns(DnsSource),
    Verified,
    History,
    FastlyRule(u32),
}

impl CandidateSource {
    pub(super) fn as_str(&self) -> &'static str {
        match self {
            Self::Dns(_) => DIAGNOSTICS_DNS,
            Self::Verified => MODEL_VERIFIED,
            Self::History => MODEL_HISTORY,
            Self::FastlyRule(_) => MODEL_FASTLY_RULE,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct EndpointCandidate {
    pub(super) ip: IpAddr,
    pub(super) source: CandidateSource,
    pub(super) expires_at: Instant,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Protocol {
    Http1,
    H2,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct NetworkContext {
    pub(crate) config_scope: Arc<str>,
    pub(crate) epoch: u64,
}

/// Never use this connection identity as a native_budget or breaker key.
/// Those retain the existing authority/configuration identity across epochs.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PoolKey {
    pub(crate) authority: Arc<str>,
    pub(crate) network: NetworkContext,
    pub(crate) generation: u64,
    pub(crate) endpoint: SocketAddr,
    pub(crate) protocol: Protocol,
}

#[derive(Clone, Debug)]
pub(crate) struct EndpointLease {
    pub(crate) pool: PoolKey,
    pub(super) source: CandidateSource,
    pub(crate) expires_at: Instant,
}

#[derive(Clone, Debug)]
pub(super) struct ProbeTarget {
    pub(super) url: url::Url,
    pub(super) size: u64,
    /// Unfinished half-open interval that triggered recovery.
    pub(super) region: std::ops::Range<u64>,
    pub(super) protocol: Protocol,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProbeFailure {
    Ineligible,
    Cancelled,
    BudgetExhausted,
    Timeout,
    Transport,
    InvalidCertificate,
    Redirect,
    ServerCooldown {
        status: u16,
        retry_after: std::time::Duration,
    },
    TargetMissing,
    HttpStatus,
    RangeUnsupported,
    InvalidRange,
    InvalidEncoding,
    WrongPeer,
    WrongProtocol,
    Truncated,
}

impl ProbeFailure {
    pub(super) fn reason(self) -> super::super::diagnostics::EndpointReason {
        use super::super::diagnostics::EndpointReason;
        match self {
            Self::InvalidCertificate => EndpointReason::InvalidCertificate,
            Self::RangeUnsupported => EndpointReason::RangeUnsupported,
            Self::InvalidRange => EndpointReason::InvalidRange,
            Self::InvalidEncoding => EndpointReason::InvalidEncoding,
            Self::Truncated => EndpointReason::Truncated,
            Self::BudgetExhausted => EndpointReason::BudgetExhausted,
            Self::ServerCooldown { .. } => EndpointReason::ServerCooldown,
            Self::Cancelled => EndpointReason::Cancelled,
            Self::Ineligible => EndpointReason::RuleIneligible,
            Self::Timeout => EndpointReason::InsufficientSample,
            _ => EndpointReason::NoGain,
        }
    }
}

/// A measurement is attributable to one connection and one protocol. An IP
/// alone does not identify a sample when several H2 streams share a socket.
#[derive(Clone, Debug)]
pub(super) struct ThroughputSample {
    pub(super) ip: IpAddr,
    pub(super) protocol: Protocol,
    pub(super) connection_id: u64,
    pub(super) bytes: u64,
    pub(super) elapsed: std::time::Duration,
    pub(super) observed_at: Instant,
    pub(super) complete_object: bool,
    pub(super) contaminated: bool,
}
