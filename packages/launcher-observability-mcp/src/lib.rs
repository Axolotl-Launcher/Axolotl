//! Local MCP surface over the telemetry the launcher keeps in memory.
//!
//! This is a sibling of `launcher-mcp`, not a replacement: the launcher tools
//! act on the launcher, these tools only read diagnostics. The [`Source`] trait
//! keeps the crate independent of the runtime that records telemetry, so the
//! transport and the query surface can be tested without a launcher.

mod protocol;
mod server;
mod source;

pub use server::{Server, Snapshot};
pub use source::{
    CaptureStats, CounterSeries, EventRecord, HistogramSeries, Source,
    SpanRecord,
};

#[cfg(test)]
mod tests;
