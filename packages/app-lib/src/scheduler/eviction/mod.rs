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
//!    as terminated. The drain is bounded by [`DRAIN_TIMEOUT`]: a descendant
//!    in `spawn_blocking` cannot be aborted, and waiting forever would hold
//!    the rebuild lock and never return the memory. Tasks still alive at the
//!    deadline are detached and logged.
//! 5. The service object is dropped, which returns its memory.
//! 6. The slot moves to `Evicted`.
//! 7. The slot's rebuild lock serialises callers. It is **not** held while
//!    the rebuild entry runs (an entry may read the slot it rebuilds), so
//!    exclusion comes from the `Rebuilding` state: one caller wins it, the
//!    rest return `Busy`. A caller that finds `Rebuilding` set by an attempt
//!    whose future was dropped (panic or cancel) settles that attempt as a
//!    failure instead of leaving the slot stuck.
//! 8. The slot moves to `Rebuilding` and calls the service's rebuild entry.
//! 9. Success registers the new descendants, returns to `Live` and resets the
//!    failure counters — the only reset.
//! 10. Failure counts towards `REBUILD_ATTEMPTS_TOTAL`; the next try waits
//!     1 s, 4 s, then 16 s. Nine failures in all, or 30 minutes since the
//!     first, revoke the slot: it stays `Revoked`. The revocation is
//!     evaluated when the next rebuild is attempted (there is no timer), and
//!     `level()`/`degraded()` report it: the slot's level becomes L2 and the
//!     degraded flag is set. Nothing in this module changes a `Tag`; the
//!     caller that owns the tag reads these two accessors.

pub mod criteria;
mod slot;

pub use slot::*;

#[cfg(test)]
mod no_double_instance;
