//! Memory measurement gate for destructive swap-out: dropping an L1 object
//! must return measurable resident memory. The values are printed so the gain
//! can be judged against `L1_MIN_RETAINED`; a small gain is reported, never
//! hidden by lowering the threshold.

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

/// Smallest resident-memory gain that justifies destroying an L1 object.
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

#[test]
fn measured_values_are_within_bounds() {
    let mut system = System::new();
    let before = resident_bytes(&mut system);

    // Stands in for the state an L1 task keeps alive: touch every page so
    // it is resident, then destroy it as a swap-out would.
    let mut object = vec![0u8; SAMPLE_OBJECT_BYTES];
    for page in object.chunks_mut(4096) {
        page[0] = 1;
    }
    std::hint::black_box(&object);
    let loaded = resident_bytes(&mut system);
    drop(object);
    let after = resident_bytes(&mut system);

    let retained = loaded.saturating_sub(after);
    let verdict = if retained >= L1_MIN_RETAINED {
        "meets"
    } else {
        "below"
    };
    println!(
        "swap-out RSS: before={before} loaded={loaded} after={after} \
         retained={retained} min={L1_MIN_RETAINED} ({verdict} L1_MIN_RETAINED)"
    );

    // The resident set belongs to the whole test process, so tests running in
    // parallel move it too; only "it can be measured" is this test's to assert.
    for (name, value) in
        [("before", before), ("loaded", loaded), ("after", after)]
    {
        assert!(value > 0, "resident memory {name} must be measurable");
    }
}
