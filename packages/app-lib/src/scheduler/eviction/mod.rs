//! Destructive swap-out for L1 (spec §7). tokio cannot suspend or page out a
//! task — it can only let one finish, panic or abort, and `spawn_blocking`
//! cannot be aborted at all — so swap-out is evict-and-rebuild: the data is
//! dropped and the service is rebuilt from its entry point later. Data can be
//! swapped out; tasks cannot.
//!
//! [`criteria`] decides what may be swapped out; [`EvictionSlot`] runs the rebuild
//! protocol for one swappable service.
//!
//! The rebuild protocol, one slot per tag:
//!
//! 1. Eviction starts only from `Live`, under the slot's rebuild lock.
//! 2. The slot moves to `Evicting`.
//! 3. Every descendant task in the slot's `JoinSet` is aborted.
//! 4. The `JoinSet` is drained until `len() == 0`; a task that panicked counts
//!    as terminated.
//! 5. The service object is dropped, which returns its memory.
//! 6. The slot moves to `Evicted`.
//! 7. A rebuild takes the same rebuild lock, so at most one rebuild runs and
//!    never while an eviction is draining (no double instance).
//! 8. The slot moves to `Rebuilding` and calls the service's rebuild entry.
//! 9. Success registers the new descendants, returns to `Live` and resets the
//!    failure counters — the only reset.
//! 10. Failure counts towards `REBUILD_ATTEMPTS_TOTAL`; the next try waits
//!     1 s, 4 s, then 16 s. Nine failures in all, or 30 minutes since the
//!     first, revoke the slot: it stays `Revoked`, its work runs with L2
//!     semantics, and it reports a user-visible degraded state.

pub mod criteria;
mod slot;

pub use slot::*;

#[cfg(test)]
mod no_double_instance;
