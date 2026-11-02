//! The one startup reconciliation entry. Work that a previous session could
//! leave half done is repaired here before any background loop starts:
//! telemetry's multi-step writes (each step atomic or idempotent, but not one
//! transaction). The shutdown loss list points here too.
//!
//! Running games are tracked in memory only (`ProcessManager`), so a process
//! exit an L0 event could not report leaves nothing behind for a restart to
//! repair: the old `processes` table is retired upstream, and the spec's
//! `AppClosed` compensation has no persisted row left to repair. The loss is
//! bounded to the current session; a restart starts with no process records.
//! Interrupted install jobs are recovered by `install::recover_interrupted_jobs`
//! before this entry runs.

use crate::State;

pub(crate) async fn at_startup(state: &State) {
    if let Err(error) = crate::telemetry::reconcile(state).await {
        tracing::warn!(%error, "Telemetry startup reconciliation failed");
    }
}
