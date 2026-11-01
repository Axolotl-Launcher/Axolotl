# Scheduler verification — 2026-11-01

This file records the evidence behind the scheduler PR chain (#993, #995,
#996, #997). Commands were run from the worktree named in each section.
`CARGO_INCREMENTAL=0` avoids the rustc 1.95 incremental-compilation ICE; the
phase-3 tree uses its own target directory so cargo cannot reuse a phase-2
artifact from a shared target.

## Phase 2 — `feat/sched-phase2`

- `cargo check -p theseus --tests` → 0.
- `cargo test -p theseus --lib -- scheduler::` → 29 passed.
- `cargo test -p theseus --lib -- install::critical_section` → 5 passed.
- `cargo test -p theseus --lib -- resources` → 317 passed.
- `cargo test -p theseus --lib -- state::instances::commands` → 180 passed.
- `cargo test -p theseus --lib -- install::runner` → 92 passed (the
  `database is locked` flake can appear under parallel load; it passes alone).
- `cargo fmt --all --check` → 0.
- `node scripts/axolotl/downgrade-app-db.test.mjs` → 0 (includes the
  failed-migration-only case).
- Scheduler gates: 11 scripts exit 0; `sched-recall-gap-gate` exits 1 (the
  spec's accepted per-family fallback); `node --test sched-gates.test.mjs
  sched-resource-fixtures.test.mjs` → 34/34.

## Phase 3 — `feat/sched-phase3` (independent target)

- `cargo check -p theseus --tests` → 0.
- `cargo check --package theseus_gui --features updater` → 0 (cubiomes
  submodule present for the check; `AXOLOTL_SKIP_BLOCKBENCH_BUILD=1`).
- `cargo test -p theseus --lib -- scheduler::` → 43 passed.
- `state::process` → 34 passed (12 process tests + 4 stopped-detector outcome tests + 18 L0 separation tests).
- `install::critical_section` → 5 passed.
- `telemetry::reconcile` → 2 passed.
- `install::runner` → 92 passed.
- `cargo fmt --all --check` → 0.
- Scheduler gates: 12 scripts exit 0; `sched-recall-gap-gate` exits 1
  (accepted fallback); gate tests → 36/36.
- Cross-runtime round trip: 22.7 µs average over 1000 trips (debug build).
- K3.7: `rg -c "生产线程" .agents/plans/sched-governance-table.md` → 2.

## K3.3 — the 17 L0-1 separation tests

Each name below asserts the L0 detector does not perform the access and the
reaction side does (`state::process::l0_separation_tests`):

1. `l0_process_monitor_does_not_touch_shared_resources`
2. `l0_playtime_events_are_queued_not_written_item_2`
3. `l0_playtime_events_are_queued_not_written_item_3`
4. `l0_monitor_loop_body_has_no_await_on_shared_resources`
5. `l0_does_not_await_child_output_tasks`
6. `l0_playtime_events_are_queued_not_written_item_6`
7. `l0_post_exit_file_io_is_dispatched_item_7`
8. `l0_post_upgrade_record_runs_at_l3`
9. `l0_post_exit_file_io_is_dispatched_item_9`
10. `l0_post_exit_file_io_is_dispatched_item_10`
11. `l0_does_not_await_event_emission`
12. `l0_has_no_detached_spawn_item_12`
13. `l0_has_no_detached_spawn_item_13`
14. `l0_does_not_call_discord_rpc`
15. `l0_multiplayer_notify_is_dispatched`
16. `l0_post_exit_file_io_is_dispatched_item_16`
17. `l0_post_exit_hook_runs_at_l2`

`finished_event_implies_log_buffer_has_last_line` covers the V7 ordering
(pumps finish before `Finished`).

## K0.7 — frozen golden set

`scripts/axolotl/sched-resource-golden.json` is the frozen set;
`sched-resource-golden.sha256` records its hash. The checker rejects an
inline/file disagreement and an edited set whose recorded hash was not
updated; `sched-gates.test.mjs` covers the negative case.

## Declared boundaries

- The L0 supervisor (JoinSet, panic/cancel reporting, `abort_all`) runs in
  the driver. The application exit path that calls the executor's shutdown
  is the phase-7 exit barrier (#1003); this PR provides and tests the API but
  does not rewire the exit sequence.
- The 17 tests are source-level separation checks. The runtime split is
  exercised by the process tests and the desktop CI build; a full
  subprocess/Tauri acceptance harness is not part of this phase.
- The scheduler gate step is blocking from this chain onward. The text
  checks for `enter(`/tag forgeability remain auxiliary as the spec marks
  them (K2.9); the type system carries the primary guarantee.

## K0.22 — L1 swap-out memory measurement

`cargo test -p theseus scheduler::budget::measured_values_are_within_bounds
-- --nocapture`:

```
swap-out RSS: before=8339456 loaded=75538432 after=8454144
retained=67084288 min=1048576 (meets L1_MIN_RETAINED)
```

## Tracked phase-0 evidence

- `sched-shutdown-baseline.txt`, `sched-shutdown-comparison.md` — K0.26.
- `sched-order-coarse-screen-report.md` — K0.24/K0.25.
- `sched-l0-members.md` — K0.12.
- `sched-governance-table.md` — K3.7 (two production threads).
