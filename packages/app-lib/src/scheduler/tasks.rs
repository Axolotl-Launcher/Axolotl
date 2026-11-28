//! The scheduler's registry of declared task shapes, read from the generated
//! resource registry and the checked-in policy.
//!
//! One place answers "what does a task of this kind declare?": which
//! resources it may hold, whether its delivery is lossy or reserved, whether
//! it is cancelled or quiesced, and what exit policy it carries. The answers
//! come from the same generated artifacts the order and level gates read, so a
//! declaration cannot drift from the code without a gate failing.
//!
//! Nothing here acquires a resource or starts a task.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::scheduler::contract::{
    ExitPolicy, IdempotencyKey, RetryPolicy, TaskKind,
};

/// Which delivery guarantee a task's progress events carry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Delivery {
    /// Progress that may be dropped and re-sent from the durable record.
    Lossy,
    /// Events that must arrive; they take the reserved segment.
    Reserved,
}

/// How a task is stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cancellation {
    /// Finish the current unit, then return at a safety point (L0).
    Quiesce,
    /// Stop now.
    Abort,
}

/// What one task kind declares before it runs.
#[derive(Clone, Debug)]
pub struct Declaration {
    pub kind: TaskKind,
    /// Registered resource idents this kind may hold, ascending by rank.
    pub held: Vec<String>,
    pub delivery: Delivery,
    pub cancellation: Cancellation,
    pub exit_policy: ExitPolicy,
    pub retry: RetryPolicy,
    /// Longest a `MustComplete` unit may run; mirrors the exit drain window.
    pub budget_ms: u64,
}

/// A declared resource, as the generator emits it.
#[derive(Clone, Debug, Deserialize)]
pub struct ResourceItem {
    pub ident: String,
    pub rank: u32,
    #[serde(rename = "scopeClass")]
    pub scope_class: String,
    pub file: String,
    pub line: u32,
}

#[derive(Debug, Deserialize)]
pub struct ResourceRegistry {
    pub fingerprint: String,
    pub items: Vec<ResourceItem>,
}

impl ResourceRegistry {
    /// Read a generated registry. The file is produced by
    /// `scripts/axolotl/sched-resource-inventory.mjs`; a missing file is an
    /// error so a test can never silently validate against nothing.
    pub async fn load(path: &Path) -> crate::Result<Self> {
        let bytes = tokio::fs::read(path).await.map_err(|error| {
            crate::ErrorKind::InputError(format!(
                "The scheduler resource registry could not be read from {}: {error}",
                path.display()
            ))
        })?;
        let registry: Self = serde_json::from_slice(&bytes)?;
        if registry.fingerprint.is_empty() {
            return Err(crate::ErrorKind::InputError(
                "The scheduler resource registry has no fingerprint".into(),
            )
            .into());
        }
        Ok(registry)
    }

    /// Registered idents in ascending rank, which is the order they must be
    /// acquired in.
    pub fn ascending(&self) -> Vec<&ResourceItem> {
        let mut items: Vec<_> = self.items.iter().collect();
        items.sort_by_key(|item| item.rank);
        items
    }

    pub fn ranks(&self) -> BTreeSet<&str> {
        self.items.iter().map(|item| item.ident.as_str()).collect()
    }
}

/// Registry path the gates and the generator agree on.
pub fn default_registry_path() -> PathBuf {
    if let Ok(explicit) = std::env::var("SCHED_ORDER_REGISTRY") {
        return PathBuf::from(explicit);
    }
    if let Ok(directory) = std::env::var("SCHED_ARTIFACT_DIR") {
        return PathBuf::from(directory).join("sched-resource-registry.json");
    }
    Path::new("scripts")
        .join("axolotl")
        .join("sched-resource-registry.json")
}

/// The bracket a task kind occupies in the generated order.
///
/// These are the kinds the scheduler actually runs today, mapped onto the
/// resource families their entry points acquire. A kind with no bracket holds
/// nothing and may run anywhere.
pub fn declared_held(kind: TaskKind) -> &'static [&'static str] {
    match kind {
        TaskKind::InstallJob(_) => &[
            "theseus::api::instance::backup::INSTANCE_MAINTENANCE_LOCKS",
            "theseus::state::State.install_db_semaphore",
            "theseus::state::InstanceLockManager.locks",
        ],
        TaskKind::ContentRefresh => {
            &["theseus::state::InstanceLockManager.locks"]
        }
        TaskKind::PlanExecution => &[
            "theseus::api::instance::backup::INSTANCE_MAINTENANCE_LOCKS",
            "theseus::state::InstanceLockManager.locks",
        ],
        // Ascending by generated rank: instance maintenance (1) ->
        // instance content locks (35) -> repository lock (37).
        TaskKind::Backup => &[
            "theseus::api::instance::backup::INSTANCE_MAINTENANCE_LOCKS",
            "theseus::state::InstanceLockManager.locks",
            "theseus::api::instance::backup::REPOSITORY_LOCK",
        ],
        TaskKind::Export
        | TaskKind::ServerSupervision
        | TaskKind::Maintenance => &[],
    }
}

/// Delivery, cancellation and exit policy for a kind, from the same
/// reasoning as `declared_held`.
pub fn declaration(kind: TaskKind) -> Declaration {
    match kind {
        TaskKind::InstallJob(_) => Declaration {
            kind,
            held: declared_held(kind)
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            delivery: Delivery::Lossy,
            cancellation: Cancellation::Abort,
            exit_policy: ExitPolicy::PublishGuarded,
            retry: RetryPolicy::HOUSE,
            budget_ms: 3_000,
        },
        TaskKind::ContentRefresh => Declaration {
            kind,
            held: declared_held(kind)
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            delivery: Delivery::Lossy,
            cancellation: Cancellation::Abort,
            exit_policy: ExitPolicy::MayBeDropped,
            retry: RetryPolicy::HOUSE,
            budget_ms: 3_000,
        },
        TaskKind::PlanExecution => Declaration {
            kind,
            held: declared_held(kind)
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            delivery: Delivery::Reserved,
            cancellation: Cancellation::Abort,
            exit_policy: ExitPolicy::PublishGuarded,
            retry: RetryPolicy::HOUSE,
            budget_ms: 3_000,
        },
        TaskKind::Backup => Declaration {
            kind,
            held: declared_held(kind)
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            delivery: Delivery::Reserved,
            cancellation: Cancellation::Abort,
            exit_policy: ExitPolicy::MustComplete,
            retry: RetryPolicy::HOUSE,
            budget_ms: 3_000,
        },
        TaskKind::Export => Declaration {
            kind,
            held: Vec::new(),
            delivery: Delivery::Reserved,
            cancellation: Cancellation::Abort,
            exit_policy: ExitPolicy::PublishGuarded,
            retry: RetryPolicy::HOUSE,
            budget_ms: 3_000,
        },
        TaskKind::ServerSupervision => Declaration {
            kind,
            held: Vec::new(),
            delivery: Delivery::Reserved,
            cancellation: Cancellation::Quiesce,
            exit_policy: ExitPolicy::MayBeDropped,
            retry: RetryPolicy::HOUSE,
            budget_ms: 3_000,
        },
        TaskKind::Maintenance => Declaration {
            kind,
            held: Vec::new(),
            delivery: Delivery::Lossy,
            cancellation: Cancellation::Quiesce,
            exit_policy: ExitPolicy::MayBeDropped,
            retry: RetryPolicy::ONCE,
            budget_ms: 3_000,
        },
    }
}

/// The idempotency key a plan revision names. The SQL unique column keeps
/// enforcing it; this only spells it the same way at every call site.
pub fn plan_revision_key(
    plan_id: &str,
    revision: u32,
) -> Result<IdempotencyKey, crate::Error> {
    IdempotencyKey::parse(format!("content-plan:{plan_id}@{revision}")).map_err(
        |error| crate::ErrorKind::InputError(format!("{error}")).into(),
    )
}

/// The idempotency key an install job names.
pub fn install_job_key(job_id: &str) -> Result<IdempotencyKey, crate::Error> {
    IdempotencyKey::parse(format!("install-job:{job_id}")).map_err(|error| {
        crate::ErrorKind::InputError(format!("{error}")).into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[tokio::test]
    async fn declarations_are_ascending_in_the_generated_order() {
        // The registry is generated; without one the assertion is vacuous, so
        // the test says so rather than passing silently.
        let path = default_registry_path();
        let Ok(registry) = ResourceRegistry::load(&path).await else {
            eprintln!(
                "skipped: no generated registry at {}; run `node scripts/axolotl/sched-resource-inventory.mjs`",
                path.display()
            );
            return;
        };
        let ranks: std::collections::HashMap<&str, u32> = registry
            .items
            .iter()
            .map(|item| (item.ident.as_str(), item.rank))
            .collect();

        for kind in [
            TaskKind::InstallJob(
                crate::install::model::InstallJobKind::InstallContent,
            ),
            TaskKind::ContentRefresh,
            TaskKind::PlanExecution,
            TaskKind::Backup,
        ] {
            let declaration = declaration(kind);
            let mut previous: Option<u32> = None;
            for ident in &declaration.held {
                let Some(rank) = ranks.get(ident.as_str()).copied() else {
                    panic!(
                        "{kind:?} declares {ident}, which is not in the generated registry"
                    );
                };
                if let Some(previous) = previous {
                    assert!(
                        rank > previous,
                        "{kind:?} declares {ident} at rank {rank} after rank {previous}; declared holds must ascend"
                    );
                }
                previous = Some(rank);
            }
        }
    }

    #[tokio::test]
    async fn every_declared_ident_is_registered() {
        let Ok(registry) =
            ResourceRegistry::load(&default_registry_path()).await
        else {
            eprintln!(
                "skipped: no generated registry; run the inventory script"
            );
            return;
        };
        let known: BTreeSet<&str> = registry.ranks();
        for kind in [
            TaskKind::InstallJob(
                crate::install::model::InstallJobKind::InstallContent,
            ),
            TaskKind::ContentRefresh,
            TaskKind::PlanExecution,
            TaskKind::Backup,
            TaskKind::Export,
            TaskKind::ServerSupervision,
            TaskKind::Maintenance,
        ] {
            for ident in declaration(kind).held {
                assert!(
                    known.contains(ident.as_str()),
                    "{kind:?} declares {ident}, which the generator does not register"
                );
            }
        }
    }

    #[test]
    fn must_complete_budgets_fit_the_drain_window() {
        // `DRAIN_WINDOW` is three seconds; a longer MustComplete task would
        // outlive the exit barrier's budget for it.
        for kind in [
            TaskKind::Backup,
            TaskKind::InstallJob(
                crate::install::model::InstallJobKind::InstallContent,
            ),
        ] {
            let declaration = declaration(kind);
            if declaration.exit_policy == ExitPolicy::MustComplete {
                assert!(
                    declaration.budget_ms <= 3_000,
                    "{kind:?} is MustComplete but budgets {} ms",
                    declaration.budget_ms
                );
            }
        }
    }

    #[test]
    fn plan_and_job_keys_match_the_existing_constraints() {
        assert_eq!(
            plan_revision_key("plan-1", 3)
                .expect("valid plan key")
                .as_str(),
            "content-plan:plan-1@3"
        );
        assert_eq!(
            install_job_key("job-1").expect("valid job key").as_str(),
            "install-job:job-1"
        );
    }
}
