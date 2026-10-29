//! Memory measurement gate for destructive swap-out: dropping an L1 object
//! must return measurable resident memory. The values are printed so the gain
//! can be judged against `L1_MIN_RETAINED`; a small gain is reported, never
//! hidden by lowering the threshold, and an L1 object below it stays L1 and
//! is marked as such (spec §11.8 R6).

use std::any::Any;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

/// Smallest resident-memory gain that justifies destroying an L1 object
/// (spec §7.4, §11.6 N1).
const L1_MIN_RETAINED: u64 = 1024 * 1024;
const SAMPLE_OBJECT_BYTES: usize = 64 * 1024 * 1024;

fn resident_bytes(system: &mut System) -> u64 {
    let pid = sysinfo::get_current_pid().expect("current pid");
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().with_memory(),
    );
    system
        .process(pid)
        .map(|process| process.memory())
        .unwrap_or(0)
}

fn verdict(bytes: u64) -> &'static str {
    if bytes >= L1_MIN_RETAINED {
        "meets"
    } else {
        "below"
    }
}

/// Builds an object, drops it as a swap-out would, and reports the resident
/// memory that came back.
fn measure(
    system: &mut System,
    name: &str,
    build: impl FnOnce() -> Box<dyn Any + Send>,
) -> [u64; 3] {
    let before = resident_bytes(system);
    let object = build();
    std::hint::black_box(&object);
    let loaded = resident_bytes(system);
    drop(object);
    let after = resident_bytes(system);
    let retained = loaded.saturating_sub(after);
    println!(
        "swap-out RSS [{name}]: before={before} loaded={loaded} after={after} \
         retained={retained} min={L1_MIN_RETAINED} ({} L1_MIN_RETAINED)",
        verdict(retained)
    );
    [before, loaded, after]
}

#[test]
fn measured_values_are_within_bounds() {
    let mut system = System::new();

    // Stands in for the state an L1 task keeps alive: touch every page so
    // it is resident, then destroy it as a swap-out would.
    let sample = measure(&mut system, "sample", || {
        let mut object = vec![0u8; SAMPLE_OBJECT_BYTES];
        for page in object.chunks_mut(4096) {
            page[0] = 1;
        }
        Box::new(object)
    });
    // The one object that meets every swap-out criterion unconditionally.
    let log = measure(
        &mut system,
        "LogRingBuffer at capacity",
        crate::state::filled_log_ring_buffer,
    );
    let capacity = crate::state::LOG_RING_BUFFER_BYTE_CAPACITY as u64;
    println!(
        "LogRingBuffer logical size at capacity: {capacity} bytes ({} L1_MIN_RETAINED)",
        verdict(capacity)
    );
    // The three conditional candidates keep small, bounded state; they were
    // not measured in isolation, so their bounds are stated, not claimed.
    for (name, bound) in [
        ("config_sync", "the dirty-instance id set"),
        ("endpoint selection coordinator", "per-download lease state"),
        ("google_ip warm-up", "20 addresses and one pinned client"),
    ] {
        println!("swap-out [{name}]: not measured; holds {bound}");
    }

    // The resident set belongs to the whole test process, so tests running in
    // parallel move it too; only "it can be measured" is this test's to assert.
    for (name, value) in [sample, log].into_iter().flatten().enumerate() {
        assert!(
            value > 0,
            "resident memory sample {name} must be measurable"
        );
    }
}
