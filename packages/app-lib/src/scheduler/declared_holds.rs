//! A test that a task's declaration does not lie about what it acquires.
//!
//! The order guard records every nested resource pair it observes into
//! `sched-order-violations.json` when a test binary runs under
//! `SCHED_ORDER_GUARD=1`. This module reads that report and checks each pair
//! against the declaration of the task kinds that ran, so a declaration that
//! omits a resource the code actually acquires fails the suite instead of
//! silently weakening the order.
//!
//! The report is optional: without `SCHED_ORDER_GUARD=1` there is nothing to
//! check and the test says so rather than passing silently.

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use serde::Deserialize;

    use crate::scheduler::contract::TaskKind;
    use crate::scheduler::tasks::{ResourceRegistry, declaration};

    #[derive(Debug, Deserialize)]
    struct ReportLine {
        kind: String,
        #[serde(default)]
        held: String,
        #[serde(default)]
        acquired: String,
        #[serde(default)]
        exception: bool,
    }

    fn report_path() -> Option<PathBuf> {
        if let Ok(explicit) = std::env::var("SCHED_ORDER_VIOLATIONS") {
            return Some(PathBuf::from(explicit));
        }
        std::env::var("SCHED_ARTIFACT_DIR")
            .ok()
            .map(|dir| PathBuf::from(dir).join("sched-order-violations.json"))
    }

    async fn read_report(path: &PathBuf) -> Vec<ReportLine> {
        let Ok(bytes) = tokio::fs::read(path).await else {
            return Vec::new();
        };
        String::from_utf8_lossy(&bytes)
            .lines()
            .filter_map(|line| serde_json::from_str::<ReportLine>(line).ok())
            .collect()
    }

    fn kinds() -> Vec<TaskKind> {
        vec![
            TaskKind::InstallJob(
                crate::install::model::InstallJobKind::InstallContent,
            ),
            TaskKind::ContentRefresh,
            TaskKind::PlanExecution,
            TaskKind::Backup,
            TaskKind::Export,
            TaskKind::ServerSupervision,
            TaskKind::Maintenance,
        ]
    }

    /// If a task kind declares one end of an observed nesting, it must declare
    /// the other end too. Pairs where neither ident is declared belong to code
    /// no task kind covers, and are not evidence that a declaration lies.
    #[tokio::test]
    async fn a_declared_hold_never_nests_an_undeclared_resource() {
        if std::env::var("SCHED_ORDER_GUARD").as_deref() != Ok("1") {
            eprintln!(
                "skipped: set SCHED_ORDER_GUARD=1 with SCHED_ARTIFACT_DIR to record pairs"
            );
            return;
        }
        let Some(path) = report_path() else {
            eprintln!("skipped: the order guard has no report path");
            return;
        };
        let lines = read_report(&path).await;
        if lines.is_empty() {
            eprintln!(
                "skipped: no pairs at {}; the instrumented run did not reach them",
                path.display()
            );
            return;
        }

        let mut failures = Vec::new();
        for kind in kinds() {
            let declared = declaration(kind).held;
            let declared: std::collections::BTreeSet<&str> =
                declared.iter().map(String::as_str).collect();
            for line in lines.iter().filter(|line| line.kind == "pair") {
                if line.exception {
                    continue;
                }
                let owns_held = declared.contains(line.held.as_str());
                let owns_acquired = declared.contains(line.acquired.as_str());
                match (owns_held, owns_acquired) {
                    (true, false) => failures.push(format!(
                        "{kind:?} holds {} but nests {} without declaring it",
                        line.held, line.acquired
                    )),
                    (false, true) => failures.push(format!(
                        "{kind:?} holds {} but nests it inside undeclared {}",
                        line.acquired, line.held
                    )),
                    _ => {}
                }
            }
        }
        failures.sort_unstable();
        failures.dedup();
        assert!(
            failures.is_empty(),
            "declarations lie about nesting:\n{}",
            failures.join("\n")
        );
    }

    /// A declared ident that the generator does not register is a typo that
    /// would never be checked at run time.
    #[tokio::test]
    async fn declarations_only_name_registered_resources() {
        let path = crate::scheduler::tasks::default_registry_path();
        let Ok(registry) = ResourceRegistry::load(&path).await else {
            eprintln!(
                "skipped: no generated registry at {}; run the inventory script",
                path.display()
            );
            return;
        };
        let known: HashMap<&str, u32> = registry
            .items
            .iter()
            .map(|item| (item.ident.as_str(), item.rank))
            .collect();
        for kind in kinds() {
            for ident in declaration(kind).held {
                assert!(
                    known.contains_key(ident.as_str()),
                    "{kind:?} declares {ident}, which the generator does not register"
                );
            }
        }
    }
}
