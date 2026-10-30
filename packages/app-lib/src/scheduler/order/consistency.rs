//! T4.7: the run-time order check against the generated ranks. The checker
//! tests run on a small table of their own; the replay of the annotated
//! nesting paths needs the generated registry and skips when it is absent
//! (it is produced by `node scripts/axolotl/sched-resource-inventory.mjs`).

#![cfg(debug_assertions)]

use super::instrument::enabled::Holder;
use super::instrument::enabled::probe::Probe;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn lines(out: &Path) -> Vec<Value> {
    std::fs::read_to_string(out)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn kinds(out: &Path, kind: &str) -> Vec<Value> {
    lines(out)
        .into_iter()
        .filter(|line| line["kind"] == kind)
        .collect()
}

fn holder() -> Holder {
    Holder::Thread(std::thread::current().id())
}

/// Two resources of family S8 (`a` below `b`) and one of S5.
fn small_probe(dir: &Path, exceptions: Value) -> (Probe, PathBuf) {
    let out = dir.join("violations.json");
    let registry = json!({ "items": [
        { "ident": "t::a", "rank": 1, "scopeClass": "S8" },
        { "ident": "t::b", "rank": 2, "scopeClass": "S8" },
        { "ident": "t::permit", "rank": 3, "scopeClass": "S5" },
    ]});
    let policy = json!({ "orderExceptions": exceptions });
    (Probe::new(registry, policy, out.clone()), out)
}

#[test]
fn an_ascending_pair_is_reported_once_and_is_no_violation() {
    let dir = tempfile::tempdir().unwrap();
    let (probe, out) = small_probe(dir.path(), json!([]));
    for _ in 0..2 {
        let a = probe.hold(holder(), "t::a");
        probe.touch(holder(), "t::b");
        probe.release(holder(), a);
    }
    assert!(kinds(&out, "violation").is_empty());
    let pairs = kinds(&out, "pair");
    assert_eq!(pairs.len(), 1, "a pair is reported once per process");
    assert_eq!(pairs[0]["held"], "t::a");
    assert_eq!(pairs[0]["acquired"], "t::b");
}

#[test]
fn a_descending_pair_within_a_family_lands_in_the_violation_list() {
    let dir = tempfile::tempdir().unwrap();
    let (probe, out) = small_probe(dir.path(), json!([]));
    let _b = probe.hold(holder(), "t::b");
    probe.touch(holder(), "t::a");
    let violations = kinds(&out, "violation");
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0]["held"], "t::b");
    assert_eq!(violations[0]["acquired"], "t::a");
    assert_eq!(violations[0]["family"], "S8");
}

#[test]
fn a_listed_exception_is_a_pair_not_a_violation() {
    let dir = tempfile::tempdir().unwrap();
    let (probe, out) = small_probe(
        dir.path(),
        json!([{ "held": "t::b", "acquired": "t::a" }]),
    );
    let _b = probe.hold(holder(), "t::b");
    probe.touch(holder(), "t::a");
    assert!(kinds(&out, "violation").is_empty());
    assert_eq!(kinds(&out, "pair")[0]["exception"], true);
}

#[test]
fn other_families_and_released_ranks_do_not_constrain() {
    let dir = tempfile::tempdir().unwrap();
    let (probe, out) = small_probe(dir.path(), json!([]));
    let permit = probe.hold(holder(), "t::permit");
    probe.touch(holder(), "t::a");
    probe.release(holder(), permit);
    let b = probe.hold(holder(), "t::b");
    probe.release(holder(), b);
    probe.touch(holder(), "t::a");
    assert!(kinds(&out, "violation").is_empty());
    assert!(kinds(&out, "pair").is_empty());
}

#[test]
fn holders_are_tracked_separately() {
    let dir = tempfile::tempdir().unwrap();
    let (probe, out) = small_probe(dir.path(), json!([]));
    let _b = probe.hold(holder(), "t::b");
    let other = std::thread::spawn(|| std::thread::current().id())
        .join()
        .unwrap();
    probe.touch(Holder::Thread(other), "t::a");
    assert!(kinds(&out, "violation").is_empty());
}

#[test]
fn an_unregistered_ident_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let (probe, out) = small_probe(dir.path(), json!([]));
    probe.touch(holder(), "t::missing");
    assert_eq!(kinds(&out, "unregistered").len(), 1);
}

/// The nested pairs the static check found on annotated paths, replayed
/// against the generated ranks and the policy's exceptions.
#[test]
fn the_annotated_nesting_paths_hold_under_the_generated_ranks() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let Ok(registry) = std::fs::read_to_string(
        root.join(".agents/plans/sched-resource-registry.json"),
    ) else {
        eprintln!(
            "skipped: run node scripts/axolotl/sched-resource-inventory.mjs first"
        );
        return;
    };
    let policy =
        std::fs::read_to_string(root.join("scripts/axolotl/sched-policy.json"))
            .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("violations.json");
    let probe = Probe::new(
        serde_json::from_str(&registry).unwrap(),
        serde_json::from_str(&policy).unwrap(),
        out.clone(),
    );
    let paths: [&[&'static str]; 6] = [
        &[
            "theseus::api::loader_metadata::MetadataCache.value",
            "theseus::api::loader_metadata::MetadataCache.failure",
        ],
        &[
            "theseus::state::instances::watcher::FileWatcher.watcher",
            "theseus::state::instances::watcher::FileWatcher.manual_import_directory",
        ],
        &[
            "theseus::state::instances::watcher::FileWatcher.watcher",
            "theseus::state::instances::watcher::FileWatcher.instance_ids",
        ],
        &[
            "theseus::state::instances::watcher::FileWatcher.instance_ids",
            "theseus::state::instances::watcher::FileWatcher.manual_import_directory",
        ],
        &[
            "theseus::util::fetch::policy::RANGE_SPLITTING_PROTOCOL_FAILURES",
            "theseus::util::fetch::policy::RANGE_SPLITTING_SUPPORTED",
        ],
        &[
            "theseus_gui::api::mod_translation::TaskHandle.event_gate",
            "theseus_gui::api::mod_translation::TaskHandle.snapshot",
        ],
    ];
    for path in paths {
        let held: Vec<_> = path[..path.len() - 1]
            .iter()
            .map(|ident| probe.hold(holder(), ident))
            .collect();
        probe.touch(holder(), path[path.len() - 1]);
        for rank in held {
            probe.release(holder(), rank);
        }
    }
    assert!(kinds(&out, "unregistered").is_empty(), "{:?}", lines(&out));
    assert!(kinds(&out, "violation").is_empty(), "{:?}", lines(&out));
    assert_eq!(kinds(&out, "pair").len(), paths.len());
}
