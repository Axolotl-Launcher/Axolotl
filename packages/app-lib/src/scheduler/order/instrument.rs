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
        reported: HashSet<(&'static str, &'static str)>,
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
            table.append(json!({
                "run": table.run,
                "kind": "unregistered",
                "acquired": ident,
            }));
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
                lines.push(json!({
                    "run": table.run,
                    "kind": "violation",
                    "family": rank.family,
                    "held": violation.held.ident,
                    "acquired": ident,
                    "holder": holder.describe(),
                }));
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
}
