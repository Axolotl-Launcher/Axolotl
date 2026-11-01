# K0.26 关停行为逐项对比表（T0.12）

- **任务**：T0.12 保留 abort 语义（阶段 0 最后一个任务）
- **采集时间**：2026-10-29（改造后）
- **基线文件**：[sched-shutdown-baseline.txt](./sched-shutdown-baseline.txt)（改造前，7 passed）
- **改造后命令**：`cargo nextest run -p theseus shutdown`
- **改造后结果**：**exit=0，9 tests run: 9 passed, 2338 skipped**
- **判据（K0.26 / R28 / R33）**：过滤串 `shutdown` 与测试模块路径分段前缀逐段一致；nextest exit≠4 且命中数 >0。

## 1. 逐项对比（每行一个测试名）

断言条数按各测试函数体内的 `assert!` / `assert_eq!` / `assert_ne!` 计数（改造前后均为实测）。

| # | 测试名 | 基线（改造前） | 改造后 | 断言条数（前→后） | 一致性 |
| --- | --- | --- | --- | --- | --- |
| 1 | `api::multiplayer::tests::queued_provider_claim_is_rejected_after_shutdown_begins` | PASS | PASS | 3 → 3 | 一致 |
| 2 | `api::servers::lifecycle::tests::shutdown_all_returns_directly_without_running_servers` | PASS | PASS | 1 → 1 | 一致 |
| 3 | `api::servers::lifecycle::tests::shutdown_all_uses_configured_stop_commands` | PASS | PASS | 3 → 3 | 一致 |
| 4 | `api::servers::lifecycle::tests::shutdown_all_cannot_be_blocked_by_a_stuck_pty_write` | PASS | PASS | 2 → 2 | 一致 |
| 5 | `resources::refresh::tests::shutdown_cancels_the_active_check_and_preserves_already_saved_results` | PASS | PASS | 4 → 4 | 一致 |
| 6 | `state::tunnel::tests::shutdown_closes_all_owned_connections_and_preserves_other_listeners` | PASS | PASS | 6 → 6 | 一致 |
| 7 | `state::tunnel::tests::shutdown_notifies_the_server_once_for_listener_and_each_connection` | PASS | PASS | 2 → 2 | 一致 |
| 8 | `state::shutdown::tests::abort_all_stops_a_registered_task` | —（不存在） | PASS | — → 6 | **新增** |
| 9 | `state::shutdown::tests::abort_all_stops_what_a_tracker_alone_would_leave_running` | —（不存在） | PASS | — → 5 | **新增** |

**结论**：基线 7 条**逐条通过、断言条数不变**；改造后多出 2 条 T0.12 新增测试，**未改动、未放宽、未删除任何既有断言**。故「关停行为与改造前逐项一致」成立（第 8/9 行是新增覆盖，不是行为变更）。

## 2. T0.12 自己的验证命令（checklist 原文过滤串）

```powershell
cargo nextest run -p theseus state::shutdown
```

- **exit=0**，命中 **2** 个测试（`Starting 2 tests across 2 binaries`，`2 tests run: 2 passed`）。
- 即：checklist K0.26 原文给出的过滤串 `state::shutdown` **在交付后成为可命中的正确命令**（改造前它命中 0 / exit=4，属命令设计错误 —— 裁决 R40）。
- 命中测试名：`state::shutdown::tests::abort_all_stops_a_registered_task`、`state::shutdown::tests::abort_all_stops_what_a_tracker_alone_would_leave_running`。

## 3. abort 语义的落点

| 位置 | 改动 |
| --- | --- |
| `packages/app-lib/src/state/shutdown.rs`（新增，208 行） | `BackgroundTasks` = `Mutex<Registry{next_task_id, tracked: HashMap<BackgroundTaskId, AbortHandle>}>` + `tokio_util::task::TaskTracker`；`track()` 取 `handle.abort_handle()` 后把 `JoinHandle` 交给 tracker；`abort_all()` 先 `abort()` 再清空注册表 |
| `packages/app-lib/src/state/mod.rs:204` | 字段由 `Mutex<Vec<JoinHandle<()>>>` 换成 `BackgroundTasks` |
| `packages/app-lib/src/state/mod.rs:1220`（`State::shutdown`） | `for task in …drain(..) { task.abort() }` → `self.background_tasks.abort_all()`（**不等待**，与改造前同为「abort 后立即返回」） |
| 注册点 5 处：`state/mod.rs:708/716/723/754`、`telemetry.rs:65` | `.lock().push(handle)` → `.track(handle)` |

**并存方式**：`TaskTracker` 只做存活登记（`wait()` 用），它**没有 abort**；真正的取消能力保留在 `HashMap<BackgroundTaskId, AbortHandle>` 上。两者由同一个 `BackgroundTasks` 持有，与 T0.7 的 `Supervisor`（`JoinSet` + `tracked` 表）同构。

**未改变关停行为的两点刻意取舍**：

1. **不调用 `tracker.close()`**：`close()` 会让其后 `track()` panic，属行为变更；阶段 0 不加。
2. **不调用 `tracker.wait()`**：常驻循环此时**还没有 cancel token**（阶段 2/3 才引入），`wait()` 会一直等到各循环自然退出 → 把「abort 后立即退出」变成「可能永久等待」（即设计 v8 R-B5 指出的挂死）。有界等待归 **T3.1**（`L0_SOFT_SHUTDOWN_WINDOW` / `L0_HARD_SHUTDOWN_CAP`）。

## 4. 验证退出码汇总

| 命令 | 退出码 | 命中测试数 |
| --- | --- | --- |
| `cargo fmt --all --check` | 0 | — |
| `cargo check -p theseus` | 0 | — |
| `cargo check -p theseus --tests` | 0 | — |
| `cargo nextest run -p theseus state::shutdown` | 0 | **2**（2 passed） |
| `cargo nextest run -p theseus shutdown` | 0 | **9**（9 passed；基线 7 + 新增 2） |
