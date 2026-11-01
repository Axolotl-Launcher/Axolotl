# L0 成员表 + L0-1 的 17 项共享访问逐项核对表（T0.4 交付物）

> **定位**：本文件是 **T0.4**（`tasks.md:172-188`）的两份交付物之一，对应 **D11**（L0 不持跨级共享资源，17 项分离）。它**不改产品代码**，只登记裁决与实测位置。
> **仓库基线**：`C:\dev\external\Axolotl`，分支 `xinvxueyuan/dev`，app-lib **1.10.3**（instance 重构 #986 之后）。
> **回读方式（本文件所有 file:line 均亲自回读当前工作树）**：用 DSH 的 `read` 工具按 `offset`/`limit` 逐区间读取 `packages/app-lib/src/state/process.rs`、`packages/app-lib/src/api/servers/lifecycle.rs`、`packages/app-lib/src/state/mod.rs`、`packages/app-lib/src/logger.rs`、`packages/app-lib/src/api/logs/crash_analysis.rs`、`apps/app/src/main.rs`；用 `grep` 工具定位 `#[cfg(test)]` 边界与调用点。**未使用任何转引**；凡与旧设计文档不一致者，一律记入 §4。
> **权威来源**：`tasks.md:172-188`（T0.4）、`spec.md:241-305`（§5）、`checklist.md:45-47`（K0.12/K0.13/K0.14）、`scheduler-design-v8.md:61-81`（§3 维持 v7）与 `§4.6`、`scheduler-design-v5.md:232-274`（17 项清单与 L0 成员表原始形态）、`scheduler-design-v3.md:356-421`（L0-1 逐行核实与 L0-5 修正）、`scheduler-design-v6.md:146-227`（§3.4/§3.6）。

> **表格约定（K0.12 机械判据的承载方式）**：**只有 §1 的成员表使用「序号」首列**，其余所有表（§1.2 / §2 / §3 / §4 / §5 / §6）**一律不使用数字首列**。这样 `rg -c "^\| *[0-9]+ *\|" .agents/plans/sched-l0-members.md` = **5**，与 K0.12 的「只数带序号的成员行」口径**恰好一致**，无需修改 checklist。

---

## 1. L0 成员表（5 类，逐项五列裁决）

**口径**：`spec.md:253` 定义 L0 成员 **5 类**。下表每行 = 一类成员，五列齐全（为何属 L0 / 是否持跨级共享资源 / 是否可 quiesce / 内存上界 / 与 R1–R30 的对应），另附「位置」列。

| # | 成员（类别） | 位置（本次回读） | 为何属 L0 | 是否持跨级共享资源 | 是否可 quiesce | 内存上界 | 与 R1–R30 的对应 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 进程监控（Minecraft 进程收割者） | `state/process.rs:544`（spawn 点）、`:1206`（`async fn sequential_process_manager`） | 唯一的游戏进程收割者；它死则实例状态永远停在 running（`spec.md:253`；`scheduler-design-v3.md:406`） | **是 —— 17 项**（`spec.md:255`；逐项见 §3）。**分离前 bypass 必须覆盖 L0**（`scheduler-design-v5.md:1114` 第 10 条） | **可**：安全点在监控循环自身的 await 边界（`process.rs:1306` 的 `sleep(50ms)`）；响应 quiesce、永不响应 cancel（`spec.md:272`、`spec.md:274`） | **O(1)** + 每实例 1 个 `Process` 条目；**不含** `LogRingBuffer`（那是数据，4 MiB/实例） | **R4**（`scheduler-design-v3.md:461`；`scheduler-design-v6.md:182`「R4/R10 → L0」） |
| 2 | 崩溃日志收集（崩溃/日志收集） | `logger.rs:138`（`state` 字段）、`:176`（构造）；`api/logs/crash_analysis.rs:635`（`spawn_blocking`） | 用户点名的「自身错误收集程序」；故障可观测性的最后防线（`spec.md:253`；`scheduler-design-v3.md:407`） | **部分**：日志写锁（`logger.rs:190`/`:197`）是**跨级共享**但登记为**叶子原语**（`spec.md:263`）；`crash_analysis.rs:635` 的 `spawn_blocking` **必须移出 L0** | **可**（安全点 = 日志写入单元之间；不跨 await） | 日志轮转缓冲有界（`logger.rs:139-140` 的 `normal_max_bytes`/`warn_error_max_bytes`）；崩溃事件入 **L0 私有有界队列**（容量**待标定**，见 §5） | **非 R1–R30**（一次性 spawn，`scheduler-design-v3.md:407`） |
| 3 | 监督者自身（Supervisor） | 新建 `scheduler/supervisor.rs`（**尚不存在**，本次回读确认无此文件） | 派发与准入的根；它死则无 tag 可签发（`scheduler-design-v3.md:408`） | **否（改造后）**：registry 只以**叶子原语**形式访问（`DashMap` shard 锁），不进入全序（`spec.md:263` 的「registry shard 锁」） | **可**（它是 quiesce 的**发起方**，自身也须响应） | registry ≤ `max_entries` × 约 96 B（**待实测标定**，`scheduler-design-v3.md:408` 原文） | **新增**（不在 R1–R30 内） |
| 4 | DB 会话维护（`finish_database_session`） | `state/mod.rs:265`（fn）、`:267`（3s 超时）；**唯一调用点** `apps/app/src/main.rs:1074` | 关停收尾最后一环，决定 DB 是否被标记为干净关闭（`spec.md:253`） | **是（时序上独占）**：持 `self.pool.close()`（`:267`），但它**在所有其它级别静默之后**运行，此刻无竞争者 → 不构成「别人等它」。**该前提必须写进关停序列**（`scheduler-design-v6.md:184-227` §3.6） | **可**（本身就在关停序列内，且取 `min(3s, remaining)`） | **O(1)**（仅 DB 文件名 + 一次 `pool.close()`） | **非 R1–R30**（关停路径，非常驻任务，`scheduler-design-v3.md:409`） |
| 5 | servers 监控（服务器进程收割者） | `api/servers/lifecycle.rs:315`（spawn 点）、`:602-653`（`async fn monitor_server_process`） | 服务器进程的唯一收割者（`spec.md:253`；`scheduler-design-v3.md:410`） | **部分（已核实）**：`process.child` 的 tokio Mutex 在 `:609-623` 的**不跨 await** 作用域内（`:610` 取锁 → `:611-622` 无 `.await` → `:623` 释放）；`SERVER_PROCESSES`（`:43`）为叶子原语（`spec.md:263`）。**但检测后的 5 项副作用违反分离原则**（`spec.md:259`） | **可**（安全点 = 循环自身的 `:608` `sleep(250ms)` 边界） | **O(1)** | **R10**（`scheduler-design-v3.md:467`；`scheduler-design-v6.md:182`） |

### 1.1 五列齐备自查（K0.12）

| 列 | 进程监控 | 崩溃日志收集 | 监督者自身 | DB 会话维护 | servers 监控 |
| --- | --- | --- | --- | --- | --- |
| 为何属 L0 | 有 | 有 | 有 | 有 | 有 |
| 是否持跨级共享资源 | 有（17 项） | 有（叶子 + 移出项） | 有（改造后为叶子） | 有（时序独占） | 有（部分已核实） |
| 是否可 quiesce | 有 | 有 | 有 | 有 | 有 |
| 内存上界 | 有（O(1) + 每实例条目） | 有（有界缓冲；队列容量待标定） | 有（待实测标定） | 有（O(1)） | 有（O(1)） |
| 与 R1–R30 的对应 | 有（R4） | 有（非 R1–R30） | 有（新增） | 有（非 R1–R30） | 有（R10） |

**列数断言**：§1 表 = 5 行数据行 × 8 列（序号 + 成员 + 位置 + 五列裁决），**无空单元格**。唯一标注「待标定」的单元格是**已声明限制**而非空缺（见 §5）。

### 1.2 L0-5 的 5 项副作用分离（`spec.md:259`，逐项回读确认）

| 副作用 | 本次回读位置 | 分离目标 |
| --- | --- | --- |
| `read_eula_accepted` | `lifecycle.rs:627` | L2 |
| `analyze_exit_reason(&get_log_buffer(..))` | `lifecycle.rs:640` | **L2（强制）** |
| `read_manifest` | `lifecycle.rs:643` | L2 |
| `write_manifest` | `lifecycle.rs:645` | **L3** |
| `emit_server` | `lifecycle.rs:648` | L2 |

**L0-5 分离后只保留**：`sleep`（`:608`）+ `try_wait`（`:611`）+ `SERVER_PROCESSES.remove`（`:625`，DashMap 同步 O(1)）+ 事件入队（`scheduler-design-v5.md:274`）。

---

## 2. 叶子原语 6 项（L0 允许持有的共享资源；`spec.md:263`）

| 叶子原语 | 本次回读位置 | 准入条件状态 |
| --- | --- | --- |
| 日志写锁 | `logger.rs:190`（`write_event` 取锁）、`:197`（`flush` 取锁） | **条件 2（有界）不满足** → 移出或降级为已登记例外 + 最坏临界区时长（`spec.md:263`；K0.18 取值 ≤ 1 ms） |
| registry shard 锁 | 新建 `scheduler/supervisor.rs`（待实现） | 满足（有界、无 I/O） |
| L0 私有队列锁 | 新建（`scheduler/l0/` 待实现） | 满足 |
| `ProcessManager.processes` | `state/process.rs:328` | 满足（`DashMap` shard 锁）；**真实获取点见 §4 的「D-10」行** |
| `SERVER_PROCESSES` | `api/servers/lifecycle.rs:43` | 满足 |
| `LOG_BUFFERS` | `state/process.rs:221` | **条件 2（有界）不满足**（4 MiB 克隆）→ 已登记例外 + 最坏临界区时长（K0.18 取值 ≤ 10 ms） |

**准入条件**（`spec.md:261`）：①不跨 await（`with_lock` 同步闭包）②有界（O(1)、无 I/O、无大分配）③无排队语义 ④已登记（主判据是 `spec.md §6.3.1` 的谓词，叶子表是其子集 —— 消除循环定义）。

---

## 3. L0-1 的 17 项共享访问逐项核对表（K0.13 判据 = 恰 17 行）

**表格口径**：每行含四列 —— **项 / 位置（file:line）/ dispatch 分离方案 / 验收测试名**。「项」列沿用 `spec.md:255` 与 `tasks.md:177` 列出的 **17 个行区间**，**顺序也照此**；为不与 K0.12 的序号口径冲突，**该列以「第 N 项」文本形式书写，不用数字作首列**。

| 项 | 位置（file:line，本次回读确认） | dispatch 分离方案 | 验收测试名 |
| --- | --- | --- | --- |
| 第 1 项 `record_instance_play_session(&instance_id, &state.pool)`（循环前写库） | `packages/app-lib/src/state/process.rs:1280-1285` | L0 只把 **playtime 事件**推入 L0 私有**有界**队列（`try_send`，不 await、不持池）；由 **L2 `playtime-persist`**（独立 tag）消费并落库 | `l0_process_monitor_does_not_touch_shared_resources` |
| 第 2 项 `add_instance_recent_playtime(instance_id, elapsed, &state.pool)`（`update_playtime` 内写库） | `process.rs:1242-1248` | 同第 1 项（并入 L2 `playtime-persist`） | `l0_playtime_events_are_queued_not_written` |
| 第 3 项 `record_instance_daily_playtime(instance_id, started_at, ended_at, &state.pool)`（`update_playtime` 内写库） | `process.rs:1256-1263` | 同第 1 项（并入 L2 `playtime-persist`） | `l0_playtime_events_are_queued_not_written` |
| 第 4 项 循环内 `update_playtime(&mut last_updated_playtime, &instance_id, false).await`（**每 60 秒**写库；循环 `:1293-1311`，`sleep(50ms)` `:1306`，门 `:1223`） | `process.rs:1309-1310` | 循环体只剩 `try_wait`（同步）+ `sleep` + **入队**（非 await 的 `try_send`）→ 由 L2 落库 | `l0_monitor_loop_body_has_no_await_on_shared_resources` |
| 第 5 项 `for output_task in process.output_tasks.drain(..) { output_task.await }`（**L0 等待子任务退出**） | `process.rs:1315-1319` | **删除该 await**：子任务由 `JoinSet` 监管（`scheduler-design-v6.md:229` §3.7），其完成由 supervisor 观测，L0 不等待；**保留 V7 顺序保证**（见 §3.1） | `l0_does_not_await_child_output_tasks` + `finished_event_implies_log_buffer_has_last_line` |
| 第 6 项 `update_playtime(&mut last_updated_playtime, &instance_id, true).await`（循环后写库） | `process.rs:1324` | 同第 1 项（并入 L2 `playtime-persist`） | `l0_playtime_events_are_queued_not_written` |
| 第 7 项 `snapshot_crash_reports(&state.directories.game_crash_reports_dir(&game_dir)).await`（文件 I/O） | `process.rs:1326-1329` | dispatch 一个 **L2 `post-exit-io`** 任务执行；L0 只发事件 | `l0_post_exit_file_io_is_dispatched` |
| 第 8 项 `record_post_upgrade_launch_best_effort(&instance_id, clean_launch).await`（写库，失败需回滚语义） | `process.rs:1336`（函数定义 `:103`） | dispatch 成 **L3 任务** | `l0_post_upgrade_record_runs_at_l3` |
| 第 9 项 `Process::append_to_log_file(&log_path, ..)`（文件 I/O） | `process.rs:1360-1367` | 并入 L2 `post-exit-io` | `l0_post_exit_file_io_is_dispatched` |
| 第 10 项 `crate::api::logs::save_successful_mod_snapshot(&instance_id).await`（文件 I/O） | `process.rs:1369-1374` | 并入 L2 `post-exit-io` | `l0_post_exit_file_io_is_dispatched` |
| 第 11 项 `emit_process(..).await`（事件发射 / IPC） | `process.rs:1382-1393` | dispatch 成 **L2 `emit`** 任务；L0 不 await IPC | `l0_does_not_await_event_emission` |
| 第 12 项 **detached** `tokio::spawn`（playtime 发布） | `process.rs:1343`（spawn 体 `:1343-1356`） | 注册到该 tag 的 **`JoinSet`**（消除 detached 形态），或整体移入被 dispatch 的低级别任务 | `l0_has_no_detached_spawn` |
| 第 13 项 **detached** `tokio::spawn`（synced options reconcile） | `process.rs:1400-1413` | 同第 12 项（`scheduler-design-v3.md:390`：reconcile **本就**不在监控任务内，改造 = 消除 detached 形态） | `l0_has_no_detached_spawn` |
| 第 14 项 `state.discord_rpc.clear_to_default(true).await`（await 一个内部用 `spawn_blocking` 的组件） | `process.rs:1415` | 移出 L0，dispatch 成 **L2 任务** | `l0_does_not_call_discord_rpc` |
| 第 15 项 `crate::api::multiplayer::minecraft_process_finished(&instance_id).await` | `process.rs:1321` | dispatch 成 **L2 `multiplayer-notify`** | `l0_multiplayer_notify_is_dispatched` |
| 第 16 项 `crate::state::get_instance(&instance_id, &state.pool).await` | `process.rs:1432-1436` | 并入 L2 `post-exit-io` | `l0_post_exit_file_io_is_dispatched` |
| 第 17 项 `Command::new(command)` + `command.spawn()`（post-exit hook；`:1418` 注释「不等待」） | `process.rs:1447-1452`（`:1447` `Command::new`、`:1452` `command.spawn()`） | 移入 **L2 `post-exit-io`** | `l0_post_exit_hook_runs_at_l2` |

**行数断言**：上表 = **恰 17 行数据行**（不含表头与分隔行）。判据见 `checklist.md:46`。

### 3.1 V7 顺序保证（17 项分离后的前置条件；`spec.md:257`、`scheduler-design-v5.md:256-262`）

泵的完成是后续分类/快照/事件工作的**前置条件**。实现方式二选一：**(a)** 被 dispatch 的 L2 `post-exit-io` 先 await 这些句柄；**(b)** supervisor 在观测到该 tag 的**全部后代结束后**再派发后置工作。
**可断言不变量**：`Finished` 事件发出前，`LOG_BUFFERS` 已包含子进程的最后一行输出。
**本次回读的顺序证据**（`process.rs`）：`:1315-1319`（await 泵）→ `:1321`（multiplayer notify）→ `:1324`（最后一次 playtime）→ `:1326-1329`（crash 快照）→ `:1330-1335`（`clean_launch` 判定）→ `:1336`（post-upgrade 记录）→ `:1382-1393`（`emit_process`）。**L0-5 的同构代码保留了 100ms settle 等待**，注释写明目的：`lifecycle.rs:632-635`「The brief settle wait lets the output-stream tasks flush their final lines into the buffer first.」；`:639` `sleep(100ms)`；`:640` `analyze_exit_reason(&get_log_buffer(&server_id))`。

---

## 4. 文档与实际不符（逐条：文档位置 + 实测值）

> 纪律：以下为**实测发现**，本文件**不改**任何既有文档（tasks/spec/checklist/design 一律未改）。每条给出可复核命令或回读位置。编号为「D-n」，**不用数字作表格首列**（保持 K0.12 口径）。

| 编号 | 文档位置 | 文档写法 | 实测值（1.10.3 工作树） | 性质与影响 |
| --- | --- | --- | --- | --- |
| D-1 | `spec.md:253`（§5.2 L0 成员）、`tasks.md:175`（T0.4 涉及路径） | DB 会话维护 = `state/mod.rs:252-257` | **实际目标函数在 `state/mod.rs:265-271`**：`pub async fn finish_database_session(&self)` 起于 `:265`，3s 超时在 `:267`，`mark_clean_shutdown` 在 `:270`。`:252-257` 是**另一个函数** `maintain_database_backups`（`db_backup::maintain_app_db_backups`，调用点 `apps/app/src/main.rs:228`），**与关停序列无关** | **锚点漂移（指向错误函数）**。旧文档的 `state/mod.rs:249-255`（`scheduler-design-v3.md:409`/`v5:271`）同样是漂移值。**影响**：按 `252-257` 定位会把 L0-4 错认成备份维护。**修正值 = `:265-271`**（T0.12 后已位移：`finish_database_session` 现 `:265`、3s 超时 `:267`、`mark_clean_shutdown` `:270`；`maintain_database_backups` 现 `:257`） |
| D-2 | `scheduler-design-v3.md:409`、`scheduler-design-v5.md:271` | `state/mod.rs:249-255`（3s 超时在 `:251`）；唯一调用点 `apps/app/src/main.rs:1073` | fn 在 `:265`、超时在 `:267`；**唯一调用点 = `apps/app/src/main.rs:1074`**（`grep "finish_database_session"` 全仓**仅 1 处调用**，另 1 处是定义本身） | **锚点漂移**。`main.rs` 整体 +1 与本次实测一致（旧 `:1073` → 新 `:1074`），符合 tasks 提示；但 `state/mod.rs` 的漂移（+11 行）未被任何文档记录（T0.12 后已位移：`finish_database_session` 现 `:265`、3s 超时 `:267`、`mark_clean_shutdown` `:270`；`maintain_database_backups` 现 `:257`） |
| D-3 | `scheduler-design-v3.md:406`/`v5:234`/`v6:148` 等所有「17 项」表格 | L0-1 的函数体在 `state/process.rs:1206`、spawn 点在 `:544` | **一致，无漂移**：`:1206` 确为 `async fn sequential_process_manager(`，`:544` 确为 `tokio::spawn(Process::sequential_process_manager(` | 无 |
| D-4 | `spec.md:263`、`scheduler-design-v8.md:67` | `ProcessManager.processes` 在 `state/process.rs:328`；`SERVER_PROCESSES` 在 `api/servers/lifecycle.rs:43`；`LOG_BUFFERS` 在 `state/process.rs:221` | **三项全部一致**（`:328` = `processes: DashMap<Uuid, Process>,`；`:43` = `static SERVER_PROCESSES: LazyLock<DashMap<..>>`；`:221` = `static LOG_BUFFERS: LazyLock<DashMap<..>>`） | 无 |
| D-5 | `spec.md:263`、`scheduler-design-v3.md:407`/`v5:269` | 崩溃/日志收集 = `logger.rs:138`（字段）、`:176`（构造） | **一致**：`:138` = `state: std::sync::Arc<std::sync::Mutex<RotatingLogState>>`；`:176` = `state: std::sync::Arc::new(std::sync::Mutex::new(state))` | 无（注意：`:138` 是**结构体字段声明**，取锁点在 `:190`/`:197`，本文件 §2 已补登记） |
| D-6 | `spec.md:259`、`tasks.md:175` | L0-5 的 5 项副作用：`:627`/`:640`/`:643`/`:645`/`:648` | **五项全部一致**（`read_eula_accepted` `:627`、`analyze_exit_reason(&get_log_buffer(..))` `:640`、`read_manifest` `:643`、`write_manifest` `:645`、`emit_server` `:648`） | 无 |
| D-7 | `spec.md:259`、`scheduler-design-v5.md:272` | L0-5 的 `child` 锁「不跨 await」证据区间 `:609-623`（`v3:419` 写 `:611-622` 无 await） | **成立**：`:610` `let mut child = process.child.lock().await;` → `:611-622` 无 `.await` → `:623` `};` 释放 guard | 无（`:609` 是块起始行、`:623` 是块结束行，均属区间描述而非取锁行，**不构成漂移**） |
| D-8 | `tasks.md:184` 的 11 行生产命中分解 | 生产代码 11 行 = `lifecycle.rs:323/405/450/488/498/513/519/552/590/610` + `process.rs:421` | **10+1 = 11，全部命中，且 `process.rs:421` 确为 `OpenOptions::new().write(true)`**（非资源获取点） | 无（细节见 `sched-l0-resource-baseline.md` §3） |
| D-9 | `tasks.md:183`、`checklist.md:46` 的排除口径 | `**/tests/**` 目录在本命令的两个目标文件中不存在 | **一致**：两个目标文件路径下均无 `tests/` 目录 | 无 |
| D-10 | `tasks.md:181` 的**粗筛模式集**（本文件 §2 第 4 项的连带发现） | 粗筛命令模式集 = `\.acquire\(`、`\.lock\(`、`\.read\(`、`\.write\(` | **模式集不覆盖 DashMap 的 `get_mut`/`remove` 形态**：`state/process.rs:591`（`self.processes.get_mut(&id)`，位于 `fn try_wait` `:587`）由 **L0 监控循环 `:1294` 直接调用**，是 `ProcessManager.processes` 叶子原语的**真实获取点**，但不在粗筛输出中 | **口径缺口（新发现）**：粗筛是**定位器**，其模式集**不是**「L0 内资源获取点」的完备枚举。**影响**：后续 CI 脚本（`scripts/axolotl/sched-l0-resource-check.mjs`，T0.6/T0.8 批次）若直接照抄该模式集，会漏掉这一获取点。**处置建议**：把 `get_mut\(`/`remove\(` 纳入脚本扫描面，或显式登记 `process.rs:591` 为已知获取点 |

---

## 5. 未决项与已声明限制（如实登记，不掩盖）

| 项 | 状态 | 承担者 |
| --- | --- | --- |
| 崩溃事件 **L0 私有有界队列容量** | **待标定**（无实测值） | 阶段 3 交付物（`scheduler-design-v5.md:886`） |
| supervisor **registry 内存上界**（`max_entries` × 约 96 B） | **待实测标定** | 阶段 3 交付物 |
| 17 项分离**未实现、未验证** | 上述 dispatch 方案是**设计**；落地前 `spec.md:246` 的第四条不变量对 L0-1 **不成立** → `ReclaimBypass` 到 L0 是**必需**而非可选（`scheduler-design-v3.md:400`、`scheduler-design-v5.md:1114` 第 10 条） | 阶段 3 交付物 |
| 日志写锁 / `LOG_BUFFERS` 的「条件 2 不满足」 | 已登记为例外（K0.18）；最坏临界区时长：日志写锁 **≤ 1 ms**、`LOG_BUFFERS` 4 MiB 克隆 **≤ 10 ms**（`spec.md §11.6 N14`） | 阶段 0 交付物（叶子表登记） |
| 本文件所有「验收测试名」 | **测试尚不存在**（阶段 3 交付物）；命名遵循 `scheduler-design-v3.md:394` 与 `scheduler-design-v6.md:223` 的既有风格 | 阶段 3 交付物 |

---

## 6. 验收证据索引（本文件的机械判据）

| 判据 | 命令 | 期望 |
| --- | --- | --- |
| K0.12 成员表 5 类 | `rg -c "^\| *[0-9]+ *\|" .agents/plans/sched-l0-members.md` | **5**（仅 §1 成员表用序号首列；实测值见回报） |
| K0.13 逐项核对表 17 行 | 人工计数 §3 表数据行（`spec.md:255` 的 17 个区间逐条对应） | **17** |
| K0.14 L0 不持全序资源 | `node scripts/axolotl/sched-l0-resource-check.mjs`（**阶段 0 交付物，本批不创建**） | 0 |
| 粗筛定位器 | `rg -n "\.acquire\(|\.lock\(|\.read\(|\.write\(" packages/app-lib/src/state/process.rs packages/app-lib/src/api/servers/lifecycle.rs` | 退出码 0、**23 行**（口径见 `sched-l0-resource-baseline.md`） |
