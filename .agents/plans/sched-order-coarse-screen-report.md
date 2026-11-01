# t1 粗筛收尾报告（T0.11 交付物）

> **任务**：`tasks.md` T0.11「t1 粗筛收尾 + OrderGuard」。
> **产物角色**：本文件是 T0.11 的**粗筛报告**（K0.24 的证据载体），承载
> 「3 相容 / 1 冲突已修正 / 未覆盖」的入档、**未覆盖部分的 rank 相互顺序补齐**，
> 以及 `V_struct > 0` 的回退决策记录（K0.25）。
> **口径来源**：`spec.md` §6.3.2 / §11.8（R2、R16、R25、R32）、`scheduler-design-v8.md`
> §4.9 / §4.5 / §8.10、`scheduler-survey-t1-state-tasks-locks.md` §3.5。
> **仓库锚**：分支 `xinvxueyuan/dev`，app-lib **1.10.3**。

---

## 0. 一句话结论

粗筛计数**按设计原文「3 相容 / 1 冲突已修正 / 未覆盖」入档**（§1）；但**该计数及其修正所用的
rank 号（5 / 6）是在已被取代的 rank 表下算出的**（v8 自己声明「不写具体 rank 数字」）。
本报告**以当前生成物**（`sched-resource-registry.json`，**R = 197**）重做粗筛（§2），实测：

- **合规（升序）**：安装路径的 DB permit → 内容锁（13 → 27）、**备份与恢复三处（2 → 27 → 1）**、
  **启动实例（2 → 27）**、删除快照（1 → 27）；安装路径全程为 28 → 13 → 27，**其中 28 → 13 即 C-1 的冲突**；
- **已核实冲突 11 处**（降序，**全部**可经「调整代码获取顺序」解决 → `V_struct = 0`），见 §2.3；
- **回退判据**：`V_struct = 0` 且 `V_code = 11 <= 20` → **不触发**「分族 + 族内全序」回退（§4.9 判据 (2)）；
- **未覆盖部分已补齐**：由**生成物的排序键**给出确定性全序（§3(a)），代码侧顺序事实由
  审计 + `OrderGuard` 补齐（§3(b)）。

**我未改动任何生产代码**：11 处冲突的修复是「调整获取顺序」，属行为变更，须 lead 裁决（§5）。

---

## 1. 粗筛计数入档（设计原文）

计数出处 = `scheduler-design-v8.md:338`（**不在** t1 文档内，见 `tasks.md:302` 的锚点更正）：

> **t1 粗筛现状（维持 v7）**：**3 相容 / 1 冲突（已修正：`install_db_semaphore` → rank 5、
> `instance_locks` → rank 6，使 `queue_resource_plan` 的 `runner.rs:320→321→349` 变为升序）/ 未覆盖**。

| 类别 | 计数（入档） | 内容 |
| --- | --- | --- |
| **已知相容** | **3** | 安装路径 ①、备份路径 ②、跨业务共享 ④（v6 §4.6 的检查项 ①②④） |
| **已知冲突（v6/v8 已修正）** | **1** | `queue_resource_plan` 的 rank 3/6 顺序与代码的 320→321→349 相反 → v6 重排 |
| **未覆盖** | — | rank 3/4/7–40 的相互顺序（t1 只给 7 类业务的**重叠关系**，不给每一对的获取顺序） |

**该计数与修正用的 rank 号在 1.10.3 下已失效**（见 §2.1 的登记锚点表）：
`install_db_semaphore` 现为 **rank 13**（非 5）、`instance_locks` **已不在登记表内**、
`install_admission_locks` 现为 **rank 28**。

---

## 2. 以当前生成物重做粗筛（1.10.3 实测）

### 2.1 生成物与登记锚点

- 生成器实跑：`node scripts/axolotl/sched-resource-inventory.mjs` → 退出码 **0**，
  `G=305 (lock-class=247) R=197 E=108 getpoints=600 recall-gap=39 (genuine=2) crates=9`（**采集时值 G=303 / E=106**；**当前实测 = 305 / 108**，R=197 未变，结论不受影响）。
- rank 表完整性（脚本核对）：**197 项、rank 1..197 稠密、无重复**；排序键 =
  `(scope_class, crate, module_path, identifier)` 字典序。
- 7 类业务的登记锚点（**全部在 R 内**）：

| 业务类 | 登记锚点（registry id） | rank | scope_class |
| --- | --- | --- | --- |
| 安装准入 | `state::State::install_admission_locks` | **28** | S6 |
| 安装执行 | `state::State::install_execution_locks` | **29** | S6 |
| 备份维护 | `api::instance::backup::INSTANCE_MAINTENANCE_LOCKS` | **2** | S3 |
| 内容锁 D | `state::InstanceLockManager::locks` | **27** | S6 |
| 实例删除 | `state::State::synced_options_lock`（删除路径的独占点） | **91** | S7 |
| 启动实例 | `INSTANCE_MAINTENANCE_LOCKS`（复用） | **2** | S3 |
| DB 写者 | `state::State::install_db_semaphore` | **13** | S5 |

### 2.2 实测获取顺序（逐路径）

| 路径 | 实测获取序列（rank） | 结论 |
| --- | --- | --- |
| 安装 `queue_resource_plan` | `admission::lock`(28) → `acquire_install_db_permit`(13) → `lock_instance_content_exclusive`(27) | **28 → 13 降序**（C-1） |
| 备份 `create_snapshot_inner` | `lock_instance_maintenance`(2) → `lock_instance_content_exclusive`(27) → `REPOSITORY_LOCK`(1) | 升序 ✅ |
| 备份恢复 `restore` 两处（`:2804/`:2813、`:3471/`:3480） | 同上（2 → 27 → 1） | 升序 ✅ |
| 删除快照 `delete_snapshot_inner` | `REPOSITORY_LOCK`(1) → 内容锁(27) | 升序 ✅ |
| **实例删除** `remove_instance_with_policy` → `remove_instance_files_and_state` | 维护锁(2) → [`begin_instance_deletion` 内 1，**函数内即释放**] → `lock_synced_options`(91) → `lock_instance_content`(27) | **91 → 27 降序**（C-3） |
| **启动实例** `run_with_extra_launch_args_inner` | 维护锁(2) → `launcher/mod.rs:1145` 内容锁(27) | 升序 ✅ |
| **安装执行** `admission::execution_slot` | `install_execution_locks`(29) → `install_job_semaphore`(14) | **29 → 14 降序**（C-4） |

> 口径：`begin_instance_deletion`（`backup.rs:2646`）的 `REPOSITORY_LOCK` 守卫在**该函数内**
> 释放，与后续获取不重叠，故不计为冲突；`remove_instance_with_policy` 的维护锁守卫持有到函数末。

### 2.3 已核实冲突清单（11 处，全部为「可调代码顺序」类）

| # | 位置 | 持有 → 获取（rank） | 同时持有 | 审计可见性 |
| --- | --- | --- | --- | --- |
| **C-1** | `install/runner.rs:314` → `:315` | `install_admission_locks`(28) → `install_db_semaphore`(13) | 是（`_admission`/`_writer` 绑定到函数末） | getpoints 可见 |
| **C-2** | `install/runner/admission.rs:114` → `:115` | `install_execution_locks`(29) → `install_job_semaphore`(14) | 是（元组返回） | **盲区**（间接） |
| **C-3** | `state/instances/commands/remove_instance.rs:56` → `:57` | `synced_options_lock`(91) → `InstanceLockManager::locks`(27) | 是 | **盲区**（间接） |
| **C-4** | `api/multiplayer.rs:140` → `:141`（同型 `:241→:242`、`:250→:251`） | `MULTIPLAYER_OPERATION`(69) → `ACTIVE_PROVIDER`(68) | 是 | 审计命中 |
| **C-5** | `api/loader_metadata.rs:177` → `:188` | `MetadataCache::value`(63) → `MetadataCache::failure`(62) | 是（注释明写「keep the lock through the fetch」） | 审计命中 |
| **C-6** | `resources/cache/mod.rs:422` → `:426` | `ResourceCache::gate`(82) → `ResourceCache::file_lock`(81) | 是（`_guard` 到函数末） | 审计命中 |
| **C-7** | `api/minecraft_skins.rs:1293` → `:1297` | `SKIN_CHANGE_FLUSH_LOCK`(67) → `PENDING_SKIN_CHANGE`(66) | 是（循环体内反复取） | 审计命中 |
| **C-8** | `state/mod.rs:881` → `:884` | `curseforge_api_key_update`(90) → `curseforge_api_key`(89) | 是 | 审计命中 |
| **C-9** | `state/mod.rs:892` → `:900` | `configured_http_client_update`(88) → `configured_http_client`(87) | 是 | 审计命中 |
| **C-10** | `state/mod.rs:910` → `:926` | `configured_http_client_update`(88) → `configured_http_client`(87) | 是 | 审计命中 |
| **C-11** | `util/download/endpoint_selection/coordinator.rs:1306` → `:1325`（`#[cfg(test)]` 分支） | `EndpointCoordinator::state`(100) → `probe_fixture`(99) | 是 | 审计命中 |

**C-8..C-10 的建模性质（须与 lead 一起判）**：这三处是**同一个字段的更新锁与其数据锁**
（如 `curseforge_api_key_update` 与 `curseforge_api_key`）。生成器按「实际可获取的字段」
（裁决 R17）各自成条 → 两个独立 rank → 强制一个固定先后。若判定这类**成对锁**应视为
**一条资源**（或应给「更新锁 → 数据锁」这一模式一个**声明式例外**），则 C-8..C-10 属
**建模口径问题**而非代码顺序问题 —— 该判定会同时影响 R 的构成与全部 rank。

**「已修正」那一处的现况**：v6/v8 称 `queue_resource_plan` 已重排为升序。在**当前** rank 表下
该断言**不成立**：原冲突对（`install_db_semaphore` vs 内容锁）确已升序（13 → 27），
但**同一函数内的准入锁与 DB permit 构成新的降序对**（C-1，28 → 13）。

---

## 3. 「未覆盖」部分的 rank 相互顺序补齐

**(a) 顺序本身已是确定的、全序的、可生成的**（「未定义」不再成立）：

- 生成器按 `(scope_class, crate, module_path, identifier)` 字典序排序
  （`sched-resource-inventory.mjs:618-626`），再以 `index + 1` 赋 rank（`:716-733`）；
- 实测 **197 项、rank 1..197 稠密、无重复** → 对**任意**两项资源，(a, b) 的先后**可判定、可复现**。

**(b) 代码侧的顺序事实由「审计 + OrderGuard」补齐**：

- 审计面：`sched-order-annotation-check.mjs --audit-order` 逐函数检查获取点 rank 是否升序
  （数据源 = 生成器的 600 个获取点，92 文件、392 函数）；
- 实测 **48 处降序候选**（生产 36 / 测试 12）；守卫存活期三角化后：
  **held-across 18 / needs-review 21 / released-before 9**；
- **审计是启发式的，不是判据**：`resources/fingerprints.rs:150→:196` 被判为候选，但
  `:160` 已释放 `paths` 守卫、`:196` 才取 `reads` → **假阳性**（守卫不重叠）；
- **精确判据 = debug `OrderGuard`**（§4），它只看真实持有时刻的 rank 栈。

**审计的已声明盲区（重要）**：生成器的获取点扫描面**不含间接获取** —— `lock_instance_maintenance(...)`、
`admission::lock(...)`、`lock_instance_content(...)` 等**包装函数**内部的 `.lock()` 不在
getpoints 内，因此 C-2/C-3 在 48 个候选里**不出现**，是逐条读代码核实的。这与设计
「未接入者不可见」的已声明限制一致，也是 T4.1「每个获取点二选一（接入或登记排除）」存在的原因。

---

## 4. 回退决策记录（K0.25）

**判据（`design-v8.md:417-427`，按序判定、命中即停）**：V = 违反总数，V_code = 可经
「调整代码获取顺序」解决的违反数，V_struct = 必须「改变资源归属或拆分资源」才能解决的违反数。

- 实测 **V_code = 11**（§2.3 的 C-1..C-11：全部是「先取的锁未在该函数内释放」的纯顺序问题）；
- 实测 **V_struct = 0**（未发现任何需要拆资源 / 改归属才能解决的违反）；
- 命中判据 **(2)**：`V_struct = 0 且 V_code <= 20` → **全序成立**，把 V_code 处代码调整列为收尾工作。

**结论：不触发回退**（不切换到「分族 + 族内全序」），**不记录回退代价**。
**11 处 V_code 是未决工作项**，须 lead 裁决修复方式（§5）。

---

## 5. 需 lead 裁决的点

1. **C-1..C-11 的修复方式**（我未改动任何生产代码）：11 处均「调整获取顺序」即可解决，
   但**都会改变持锁时长/竞争窗口**（例：C-1 若先取 DB permit 再取准入锁，DB permit 会在等待
   准入锁期间被持有 → 放大全局串行点）。**属行为变更，须裁决**。
2. **C-1 的根因含 rubric 层面**：`install_admission_locks` 因 `PER_KEY_LOCK_TOKEN` 判为
   **S6**（rank 28），`install_db_semaphore` 因 `SEMAPHORE_TOKEN` 判为 **S5**（rank 13），
   S5 整体先于 S6。若认为「准入许可」应先于「DB 许可」，需**改 rubric 顺序** ——
   **R12** 明确「改排序键属 R2 的破坏性操作 → 必须评审」。本任务处于**注解存在之前**
   （R16 允许直接重生成），但改 rubric 仍须裁决。
3. **C-8..C-10 的成对锁建模**（见 §2.3）：是否把「更新锁 + 数据锁」并作一条资源，
   或给该模式声明式例外。影响 R 构成与全部 rank → 须裁决。
4. **生成器扫描面的间接获取盲区**（C-2/C-3 不可见）：本阶段扩展包装函数识别，
   或**显式登记为已知盲区**留给 T4.1 的「接入或登记排除」。**建议后者**（R24 已把
   「别名解析 + enum 变体字段」列为待办，包装函数解析属同类工作）。
5. **审计的 48 个候选是否长期入档**：本报告只落三分类计数（三角化脚本为临时产物
   `.agents/tmp/t011/`，非受跟踪交付物）。若需留存请指定路径。

---

## 6. 窗口关闭点（R2 / R16）

**当前注解数 = 0**（`apps/**` 与 `packages/**` 内严格形态 `sched-order:` **0 命中**）→
树仍处「**注解存在之前**」，**R16 适用**：修生成器并重生成 rank **不属破坏性操作**。

**窗口关闭点 = 第一个 `// sched-order: <module_path::IDENT>` 注解进入源码的那一刻**
（由任务 **T4.1** 引入）。标注位置三处，互为对照：

1. `packages/app-lib/src/scheduler/order.rs` 模块文档「# The window this task closes」一节；
2. `scripts/axolotl/sched-order-annotation-check.mjs` 头部「The window」一节 + 运行时输出
   `window: open (no annotation exists: ruling R16 applies)`（**由树本身判定，不硬编码条数**）；
3. 本文件 §6。

**关闭后的处置（A 类停手条件）**：rank↔ident 指纹校验失败 → **停止该阶段 + 记录失败项与
差异清单 + 标记「受阻」**，**不得自动重排**（`spec.md` §11.8 R2；`tasks.md` §0.1.3 第 1 条；
`RUNBOOK.md:231` 4.6）。检查器在那一刻**行为切换**（两个分支均已实测）：

- 注解 0 + 指纹不符 → 退出码 1，输出「the tree still holds no annotation, so ruling R16 applies」；
- 注解 > 0 + 指纹不符 → 退出码 1，输出「**A-class stop condition (spec 11.8 R2)**」。

---

## 7. 实跑证据（本报告全部数字的来源）

| 命令 | 退出码 | 实测 |
| --- | --- | --- |
| `node scripts/axolotl/sched-resource-inventory.mjs` | 0 | `G=305 R=197 E=108 getpoints=600 recall-gap=39 crates=9`（采集时值 G=303 / E=106；当前 305 / 108） |
| `cargo fmt --all --check` | 0 | — |
| `cargo check -p theseus` | 0 | `Finished dev profile` |
| `cargo nextest run -p theseus scheduler::order::order_guard` | **0** | **6 tests run: 6 passed**（非 nextest 的 exit 4、非「0 命中」） |
| `node scripts/axolotl/sched-order-annotation-check.mjs` | **0** | 注解 0、指纹 `82aa067f…` 匹配、窗口 open |
| `node scripts/axolotl/sched-order-annotation-check.mjs --audit-order` | 0 | 600 获取点 / 92 文件 / 392 函数 / 48 降序候选 |
| `node scripts/axolotl/sched-resource-consistency-check.mjs` | 0 | 判据 1/2/5/6 全 PASS（含判据 6） |
| **负向 A**：临时 registry（`install_db_semaphore.rank` 13→99，指纹不动） | **1** | `rank_fingerprint: the recorded fingerprint does not describe the shipped rank table` |
| **负向 B**：`--scan-dir` 指向含 `// sched-order: state::State::does_not_exist` 的临时目录 | **1** | `annotation names an identifier that is not in R` |
| **正对照**：`--scan-dir` 指向含真实标识符注解的临时目录 | 0 | 窗口 closed、两项 PASS |
| `cargo check -p theseus --release` | **0** | release 侧 `imp`（`#[cfg(not(debug_assertions))]`）编译通过、**order.rs 零告警** → 证明 release 下 `enter` 恒返回 `None`、`scope`/`scope_async` 直通，无 rank 栈 / 无环境查询 / 无比较分支 |

> **负向夹具的重建方式**（临时目录 `.agents/tmp/t011/` 已在验证后清理，不在交付物内）：
> **A** = 复制 `sched-resource-registry.json`，把 `state::State::install_db_semaphore` 的 `rank`
> 由 13 改成 99（`rank_fingerprint` 不动），再以 `--registry <副本>` 运行；
> **B** = 建一个只含 `// sched-order: state::State::does_not_exist` 的 `.rs` 文件，
> 以 `--scan-dir <该目录>` 运行；正对照把该标识符换成 R 内的真实标识符。
> 三条命令都不触碰真实产物（`--registry` / `--scan-dir` 即为此提供的覆盖点）。

---

## 8. 明确不构成结论的项

- 本报告**不是**覆盖率声明：判据 1/2 只证明「登记表与生成结果一致」。
- 48 个降序候选**不是** 48 处冲突：审计不含守卫释放分析，已核实的假阳性见 §3(b)。
- 11 处已核实冲突**未被修复**，也**未触发回退**；修复属行为变更，见 §5。
- **本报告修订过一次**：初稿把 `create_snapshot_inner` 记为「2 → 1 降序」，复核后
  发现该函数实为 `2 → 27 → 1`（**升序**），系漏看中间的内容锁 —— 已更正，
  备份/恢复路径由「冲突」改判为「相容」。**这一更正同时说明：不读全文的启发式审计会同时
  产生假阳性与假阴性**，故 §2.3 的每一条都经逐行读码核实，且以 file:line 钉住。
- 调用链**未逐层展开**：只核实到函数内部的获取点（如 `queue_resource_plan` 的上游、
  `create_snapshot` 的下游）。
