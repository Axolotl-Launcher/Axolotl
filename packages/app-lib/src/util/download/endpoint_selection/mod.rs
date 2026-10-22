//! Modrinth file endpoint selection, independent of URL routing and DNS truth.

mod candidates;
mod clients;
mod coordinator;
mod fastly_rules;
pub(crate) mod metrics;
mod model;
mod policy;
mod probe;
mod store;
pub(crate) mod tuning;

pub(crate) use coordinator::{EndpointSelector, SelectionMode};
pub(crate) use model::{EndpointLease, PoolKey, Protocol};
pub(crate) use probe::invalid_certificate;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "tests/fixtures.rs"]
mod fixtures;

#[cfg(test)]
#[path = "tests/benchmark.rs"]
mod benchmark;
