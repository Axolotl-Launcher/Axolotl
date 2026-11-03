//! The run-time half of the order check, for instrumented test runs only:
//! debug builds with `SCHED_ORDER_GUARD=1`. Annotated acquisition sites call
//! [`hold`] for a guard they keep and [`touch`] for one dropped within its
//! statement. Ranks, families and the listed exceptions are read at run time
//! from the generated registry and the scheduler policy, so no rank is
//! written in code; everywhere else both calls do nothing.
//!
//! Every distinct nested pair a process sees, and every violation, is
//! appended as one JSON object per line to
//! `.agents/plans/sched-order-violations.json` (override the path with
//! `SCHED_ORDER_VIOLATIONS`, the registry with `SCHED_ORDER_REGISTRY`).
//! Holders are tokio tasks, or threads outside a task. Only the annotated
//! sites report, so nesting through unannotated code is not seen.

/// Releases a [`hold`] when dropped.
#[derive(Debug)]
#[must_use = "the rank is released when this is dropped"]
pub struct HeldRank {
    #[cfg(debug_assertions)]
    entry: Option<(enabled::Holder, super::Rank)>,
}

/// Records `ident` as held by the current task until the result drops,
/// after checking it against what the task already holds.
pub fn hold(ident: &'static str) -> HeldRank {
    #[cfg(debug_assertions)]
    {
        HeldRank {
            entry: enabled::acquire(ident, true),
        }
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = ident;
        HeldRank {}
    }
}

/// Checks `ident` against what the current task holds without holding it.
pub fn touch(ident: &'static str) {
    #[cfg(debug_assertions)]
    enabled::acquire(ident, false);
    #[cfg(not(debug_assertions))]
    let _ = ident;
}

impl Drop for HeldRank {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        if let Some((holder, rank)) = self.entry.take() {
            enabled::release(holder, rank);
        }
    }
}

#[cfg(debug_assertions)]
pub(super) mod enabled {
    use super::super::{Rank, descends_past};
    use serde_json::{Value, json};
    use std::collections::{HashMap, HashSet};
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::{LazyLock, Mutex};

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub enum Holder {
        Task(tokio::task::Id),
        Thread(std::thread::ThreadId),
    }

    impl Holder {
        fn current() -> Self {
            tokio::task::try_id().map_or_else(
                || Self::Thread(std::thread::current().id()),
                Self::Task,
            )
        }

        fn describe(self) -> String {
            match self {
                Self::Task(id) => format!("task {id}"),
                Self::Thread(id) => format!("{id:?}"),
            }
        }
    }

    pub struct Table {
        ranks: HashMap<&'static str, Rank>,
        exceptions: HashSet<(&'static str, &'static str)>,
        out: PathBuf,
        run: String,
    }

    impl Table {
        pub fn from_json(
            registry: &Value,
            policy: &Value,
            out: PathBuf,
        ) -> Option<Self> {
            let leak = |text: &str| -> &'static str {
                Box::leak(text.to_owned().into_boxed_str())
            };
            let ranks = registry["items"]
                .as_array()?
                .iter()
                .filter_map(|item| {
                    let ident = leak(item["ident"].as_str()?);
                    let rank = Rank {
                        rank: u32::try_from(item["rank"].as_u64()?).ok()?,
                        family: leak(item["scopeClass"].as_str()?),
                        ident,
                    };
                    Some((ident, rank))
                })
                .collect();
            let exceptions = policy["orderExceptions"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|exception| {
                    Some((
                        leak(exception["held"].as_str()?),
                        leak(exception["acquired"].as_str()?),
                    ))
                })
                .collect();
            Some(Self {
                ranks,
                exceptions,
                out,
                run: format!(
                    "{}-{}",
                    std::process::id(),
                    crate::scheduler::l0::control::now_millis()
                ),
            })
        }

        fn allowed(&self, held: &Rank, acquired: &Rank) -> bool {
            self.exceptions.contains(&(held.ident, acquired.ident))
        }

        fn append(&self, line: Value) {
            if let Some(parent) = self.out.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.out)
            {
                let _ = writeln!(file, "{line}");
            }
        }
    }

    fn repo_path(relative: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(relative)
    }

    fn load() -> Option<Table> {
        if std::env::var("SCHED_ORDER_GUARD").as_deref() != Ok("1") {
            return None;
        }
        // The report is append-only on purpose: several test binaries run at
        // once, and truncating from each process would erase its siblings'
        // lines. Every line is stamped with this process's `run`, so a reader
        // filters to the run it asked about instead of sharing a truncation.
        let path = |name: &str, default: &str| {
            std::env::var_os(name)
                .map(PathBuf::from)
                .unwrap_or_else(|| repo_path(default))
        };
        let read = |file: PathBuf| -> Option<Value> {
            serde_json::from_str(&std::fs::read_to_string(&file).ok()?).ok()
        };
        let registry = path(
            "SCHED_ORDER_REGISTRY",
            ".agents/plans/sched-resource-registry.json",
        );
        let Some(registry) = read(registry.clone()) else {
            tracing::warn!(
                path = %registry.display(),
                "SCHED_ORDER_GUARD is set but the resource registry cannot be read; the order is not checked"
            );
            return None;
        };
        let policy = read(repo_path("scripts/axolotl/sched-policy.json"))
            .unwrap_or(Value::Null);
        Table::from_json(
            &registry,
            &policy,
            path(
                "SCHED_ORDER_VIOLATIONS",
                ".agents/plans/sched-order-violations.json",
            ),
        )
    }

    static TABLE: LazyLock<Option<Table>> = LazyLock::new(load);

    /// Ranks each holder has, plus the nested pairs this process reported.
    #[derive(Default)]
    struct Tracking {
        held: HashMap<Holder, Vec<Rank>>,
        /// Nested pairs already written; a pair and a violation are separate
        /// findings, so they keep separate sets and one can never suppress
        /// the other.
        reported: HashSet<(&'static str, &'static str)>,
        violations: HashSet<(&'static str, &'static str)>,
        /// Idents already reported as missing from the registry.
        unregistered: HashSet<&'static str>,
    }

    static TRACKING: LazyLock<Mutex<Tracking>> =
        LazyLock::new(Default::default);

    pub fn acquire(ident: &'static str, keep: bool) -> Option<(Holder, Rank)> {
        let table = TABLE.as_ref()?;
        observe(table, &TRACKING, Holder::current(), ident, keep)
    }

    pub fn release(holder: Holder, rank: Rank) {
        forget(&TRACKING, holder, rank);
    }

    fn observe(
        table: &Table,
        tracking: &Mutex<Tracking>,
        holder: Holder,
        ident: &'static str,
        keep: bool,
    ) -> Option<(Holder, Rank)> {
        let Some(rank) = table.ranks.get(ident).copied() else {
            // Report an unregistered ident once per process, not on every
            // acquisition: an annotated site whose identifier drifted out of
            // the registry would otherwise append a line per lock take.
            let first = tracking
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .unregistered
                .insert(ident);
            if first {
                tracing::warn!(
                    ident,
                    "Annotated resource is not in the generated registry; the order is not checked for it"
                );
                table.append(json!({
                    "run": table.run,
                    "kind": "unregistered",
                    "acquired": ident,
                }));
            }
            return None;
        };
        let mut lines = Vec::new();
        {
            let mut tracking =
                tracking.lock().unwrap_or_else(|e| e.into_inner());
            let held = tracking.held.get(&holder).cloned().unwrap_or_default();
            if let Some(violation) =
                descends_past(&held, rank, |held, acquired| {
                    table.allowed(held, acquired)
                })
            {
                tracing::error!(
                    held = violation.held.ident,
                    acquired = ident,
                    "Managed resource acquired against its family order"
                );
                // One line per distinct descending pair, like the recorded
                // pairs: a violation in a loop must not grow the file per
                // iteration. Kept apart from `reported`, which holds the
                // plain pairs: a pair already logged must not hide the
                // violation of the same two idents.
                if tracking.violations.insert((violation.held.ident, ident)) {
                    lines.push(json!({
                        "run": table.run,
                        "kind": "violation",
                        "family": rank.family,
                        "held": violation.held.ident,
                        "acquired": ident,
                        "holder": holder.describe(),
                    }));
                }
            }
            for outer in held.iter().filter(|outer| outer.family == rank.family)
            {
                if tracking.reported.insert((outer.ident, ident)) {
                    lines.push(json!({
                        "run": table.run,
                        "kind": "pair",
                        "family": rank.family,
                        "held": outer.ident,
                        "acquired": ident,
                        "exception": table.allowed(outer, &rank),
                    }));
                }
            }
            if keep {
                tracking.held.entry(holder).or_default().push(rank);
            }
        }
        for line in lines {
            table.append(line);
        }
        keep.then_some((holder, rank))
    }

    fn forget(tracking: &Mutex<Tracking>, holder: Holder, rank: Rank) {
        let mut tracking = tracking.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(held) = tracking.held.get_mut(&holder) {
            if let Some(index) = held.iter().rposition(|held| *held == rank) {
                held.remove(index);
            }
            if held.is_empty() {
                tracking.held.remove(&holder);
            }
        }
    }

    #[cfg(test)]
    pub(in crate::scheduler::order) mod probe {
        //! Drives the instrumentation with its own table and tracking, so the
        //! tests neither need `SCHED_ORDER_GUARD` nor share state.
        use super::*;

        pub struct Probe {
            pub table: Table,
            tracking: Mutex<Tracking>,
        }

        impl Probe {
            pub fn new(registry: Value, policy: Value, out: PathBuf) -> Self {
                Self {
                    table: Table::from_json(&registry, &policy, out).unwrap(),
                    tracking: Mutex::default(),
                }
            }

            pub fn hold(&self, holder: Holder, ident: &'static str) -> Rank {
                observe(&self.table, &self.tracking, holder, ident, true)
                    .unwrap()
                    .1
            }

            pub fn touch(&self, holder: Holder, ident: &'static str) {
                observe(&self.table, &self.tracking, holder, ident, false);
            }

            pub fn release(&self, holder: Holder, rank: Rank) {
                forget(&self.tracking, holder, rank);
            }
        }
    }

    #[cfg(test)]
    mod table_tests {
        use super::*;
        use serde_json::json;

        fn out() -> PathBuf {
            std::env::temp_dir()
                .join(format!("sched-instrument-{}", std::process::id()))
                .join("violations.json")
        }

        #[test]
        fn a_registry_without_items_disables_the_table() {
            assert!(
                Table::from_json(&json!({}), &json!({}), out()).is_none(),
                "a registry without items cannot drive the guard"
            );
            assert!(
                Table::from_json(&json!({ "items": [] }), &json!({}), out())
                    .is_some(),
                "an empty item list is a valid, empty table"
            );
        }

        #[test]
        fn an_item_without_a_rank_or_scope_is_skipped() {
            let table = Table::from_json(
                &json!({ "items": [
                    { "ident": "t::kept", "rank": 1, "scopeClass": "S8" },
                    { "ident": "t::no_rank", "scopeClass": "S8" },
                    { "ident": "t::no_scope", "rank": 2 },
                    { "rank": 3, "scopeClass": "S8" }
                ] }),
                &json!({}),
                out(),
            )
            .unwrap();
            assert!(table.ranks.contains_key("t::kept"));
            assert_eq!(table.ranks.len(), 1, "incomplete items are dropped");
        }

        #[test]
        fn a_holder_describes_itself_for_the_violation_record() {
            let thread = Holder::Thread(std::thread::current().id());
            assert!(!thread.describe().is_empty());
        }
    }
}
