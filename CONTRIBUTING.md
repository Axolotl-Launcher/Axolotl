# 贡献指南 (Contributing)

感谢你对 Axolotl Launcher 及其相关内容感兴趣！在提交代码前，请先阅读以下指南。

我们希望所有参与者都能在友善、包容的环境中协作。请先阅读[行为准则 (CODE_OF_CONDUCT.md)](CODE_OF_CONDUCT.md)，了解社区期望和问题报告方式。

## 本地开发

### 环境要求

- **Node.js**：以 [`.nvmrc`](.nvmrc) 为准
- **pnpm**：以根目录 [`package.json`](package.json) 的 `packageManager` 为准
- **Rust**：以 [`rust-toolchain.toml`](rust-toolchain.toml) 为准
- [Tauri v2 系统依赖](https://v2.tauri.app/start/prerequisites/)
- **Linux 链接器**：[`.cargo/config.toml`](.cargo/config.toml) 让 Linux 构建使用 [mold](https://github.com/rui314/mold) 链接，需要 `ld.mold` 在 `PATH` 中，且 C 编译器为 GCC 12.1+ 或 Clang。用发行版的包管理器安装即可（Debian/Ubuntu：`sudo apt install mold`；Fedora：`sudo dnf install mold`；Arch：`sudo pacman -S mold`），`nix develop` 已自带。Windows 使用 Rust 工具链自带的 rust-lld，无需额外安装。
- **macOS 链接器**：使用 LLD，通过 `brew install lld@21` 安装，并在启动开发或构建命令前运行 `export PATH="$(brew --prefix lld@21)/bin:$PATH"`，确保 `ld64.lld` 在 `PATH` 中。CI 和 release 工作流会自动安装并验证链接器。
- **Rust 编译并行数**：dev、CI、release 和依赖缓存预热统一使用 4 个 Cargo 编译任务，macOS ARM64 使用 2 个。`pnpm app:dev`、`pnpm app:build` 和应用包的 `tauri`、`test`、`lint`、`fix` 脚本会自动选择；本地在 macOS ARM64 上直接运行 `cargo` 时需设置 `CARGO_BUILD_JOBS=2`。可通过 `CARGO_BUILD_JOBS` 显式覆盖默认值。

### 启动开发环境

1. 初始化 Git 子模块（cubiomes 是构建期的编译依赖，缺失会导致 Rust 编译失败），启用 Corepack 并安装依赖：
   ```powershell
   git submodule update --init --recursive
   corepack enable
   pnpm install --frozen-lockfile
   ```
2. 启动开发服务器：
   ```powershell
   pnpm app:dev
   ```

### 常用检查命令

在提交代码前，建议运行以下命令确保代码符合规范：

```powershell
# 品牌名和文本合规检查
pnpm axolotl:brand-guard
# Agent 指令与 Skills 完整性检查
pnpm axolotl:check-agents
# 国际化词条检查
pnpm axolotl:i18n-check
# 前端格式化及 lint
pnpm prepr:frontend:app
# Rust 格式化检查
cargo fmt --all --check
# Rust 基础检查
cargo check --package theseus_gui --features updater
```

### Scheduler 检查产物

scheduler 生成器与后续门禁通过 `SCHED_ARTIFACT_DIR` 共享同一轮检查的产物。每轮创建独立的系统临时目录，不使用 `.agents/`；显式指定的 `--out`（生成器）或 `--dir`（门禁）优先于环境变量。

```powershell
$env:SCHED_ARTIFACT_DIR = node -e 'process.stdout.write(require("node:fs").mkdtempSync(require("node:path").join(require("node:os").tmpdir(), "axolotl-scheduler-")))'
node scripts/axolotl/sched-resource-inventory.mjs
# 在同一环境中运行 desktop CI 的 Check scheduler resource gates 所列全部门禁。
```

Bash 中可用 `export SCHED_ARTIFACT_DIR="$(mktemp -d)"` 准备目录。CI 在 runner 的临时目录下创建独立目录。`sched-level-predicate-check.mjs --write` 的报告也写入所选产物目录。

调试构建启用 `SCHED_ORDER_GUARD=1` 时，插桩从同一个 `SCHED_ARTIFACT_DIR` 读取 `sched-resource-registry.json`，并写入 `sched-order-violations.json`；也可分别用 `SCHED_ORDER_REGISTRY` 和 `SCHED_ORDER_VIOLATIONS` 指定路径。检查及回放完成、输出检查完毕后，清理本轮创建的临时目录并取消环境变量。

### Rust 覆盖率

CI 的 `coverage` 作业用 [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) 在 Linux 上测量 workspace 的行覆盖率，`scripts/axolotl/rust-coverage.mjs` 是本地与 CI 共用的唯一入口。产物在 `target/coverage/` 下（`lcov.info`、`total.txt`、`summary.txt`、`html/`），CI 把它整个作为 `rust-coverage` 工件上传，保留 7 天。

本地先准备工具与构建前置条件（子模块缺失时 `apps/app` 的构建脚本会失败；跳过 Blockbench 皮肤编辑器可以省下一次前端构建）：

```powershell
cargo install cargo-llvm-cov --version 0.9.1 --locked
rustup component add llvm-tools-preview
git submodule update --init --recursive
$env:AXOLOTL_SKIP_BLOCKBENCH_BUILD = "1"
```

运行方式：

```powershell
# 完整测量，跑完自动打开 HTML 报告
pnpm coverage:rust
# 完整测量，只产出报告与 lcov，不打开浏览器
pnpm coverage:rust:lcov
# 只测一个 crate，迭代更快
cargo llvm-cov -p theseus --html --open
```

`theseus_gui` 会启用 `theseus/tauri`，而该特性会把 `theseus` 测试需要的一组辅助函数编译掉，Cargo 又按单次调用统一特性，所以整条命令不能一次跑完整个 workspace：脚本先测除壳以外的所有 crate（`theseus` 不带 `tauri`），再单独测壳。两轮 profiling 数据会在生成报告时合并，同一行只要任一轮执行过就算已覆盖。

插桩构建与报告都写在 `target/` 下（`target/llvm-cov-target/`、`target/coverage/`），已被忽略，不会出现在 `git status` 中。

口径说明：指标是 `cargo llvm-cov report` 的 TOTAL 行覆盖率，只统计当前操作系统上编译得到的代码，因此 Linux 的 CI 数字不包含 `cfg(windows)` / `cfg(target_os = "macos")` 分支。`cargo-llvm-cov` 默认把 `tests/`、`examples/`、`benches/` 目录与 `tests.rs`、`*_tests.rs`、`*-tests.rs` 这类测试专用文件排除在分母之外；写在源码里的内联 `#[cfg(test)] mod tests` 计入分母，测试执行时会被命中。按当前源码结构（内联测试约占可报告行的 35%），75% 的行覆盖率大约对应 61% 的生产代码覆盖率，改 Rust 代码时请以这个口径判断目标，不要用排除文件的方式把缺口藏起来。

首次实测基线是 **59.97%（159990/266762 行）**，因此 CI 现在用棘轮门槛 `COVERAGE_MIN_LINES = 59`（`.github/workflows/axolotl-ci.yml` 的 `coverage` 作业）：它只防回退，不是目标值。目标是 75%，补测试推进后要同步调高这个数，**不允许为了让某个改动过关而下调**。门槛接入 `Desktop CI result` 汇总闸，不达标会阻断合并。覆盖率作业失败时，`rust-coverage` 工件仍会照常上传，先看 `total.txt` 和 HTML 报告定位缺口；常见热点见 `.agents/plans/` 下的基线记录。

### 构建缓存与磁盘空间

Rust 编译产物位于 `target` 目录，首次完整构建可能占用数 GB 空间。Turbo 仅缓存前端输出，不会缓存 `target/**`。桌面应用的 Tauri 构建任务已明确关闭 Turbo 缓存。

如需释放本地开发缓存，可以删除以下目录：

```powershell
Remove-Item -Recurse -Force .turbo\cache
Remove-Item -Recurse -Force target\debug
```

此操作不会删除 `target\installer-test` 中单独生成的安装包。下次启动开发模式时需要重新编译 Rust 依赖。

### 应用数据目录与数据库迁移

启动器原始数据目录在 Windows 上是 `%APPDATA%\red.ghs.axolotl`。启动器内部数据库统一位于原始数据目录的 `databases/`；Release 与 Beta 共用业务数据。自定义内容目录不会搬走这些数据库。实例备份仓库的 `backup.db` 仍保存在各自仓库中，以支持独立携带。

持久化按数据职责划分，新增数据应先选择权威来源，再决定是否需要数据库事务：

| 数据 | 权威来源 | 故障边界 |
| --- | --- | --- |
| 实例身份、内容集、资源来源与归属、共享文件引用 | `system.db` 协调的实例与共享存储关系库 | 核心事务必须整体提交；未知关系暂停对应修改与清理 |
| 启动器设置、个性化、同步开关、资源偏好 | `settings/*.json` | 按领域读写；运行时不建立 SQL 配置副本 |
| 账户元数据与当前账户 | `accounts.json` | 凭据使用系统密钥库；写入失败仅保留在本次会话内存 |
| 遥测身份、心跳及有界待发送队列 | `state/telemetry.json` | 不可用仅影响遥测 |
| 截图身份、分组、排序、皮肤关系 | `state/media.json`；皮肤图片在 `media/skins/` | 截图本体仍在游戏的 `screenshots/`，列表扫描目录属性 |
| 世界展示选择、资源收藏、实例置顶、分享记录、加入历史 | `state/` 下分领域 JSON | 单份文档损坏不清空其他领域；分享删除凭据不写入 JSON |
| 实例分组与成员关系 | `instance_groups.json` | 沿用现有独立分组文档 |
| 共享选项、服务器列表、快捷栏、命令历史及投影检查点 | 内容目录的 `synced-options/` 中不可变文件与发布日志；原始目录的 `state/sync.json` 标识权威位置 | 原生文件保留未知字段；运行中不覆盖受保护的实例文件 |
| 文件操作恢复依据 | 每实例 `.axolotl/operations/` 中的独立记录与附件 | 外部关联实例改存原始目录的 `state/instances/`，不写外部游戏目录 |
| 游戏时长、可选任务历史 | 独立的 `records.db`、`operations-history.db` | 独立打开；不可用不阻止实例加载或启动 |
| 哈希、解析结果、远端展示与资源缓存 | `derived.db` 及分渠道缓存库 | 可重建；不承担资源身份、所有权或恢复职责 |

旧 `app.db`、`release/app.db`、`beta/app.db` 以及实验多库布局只作为导入来源。导入遵循“读取快照 → 写入新来源 → 校验 → 写接管标记 → 切换调用”。已经接管的来源缺失或损坏时，不重新打开旧库复活已删除的数据。旧文件保留供人工恢复参考，不自动删除，也不双向同步。

`src/state/store/database_baseline.json` 冻结为历史布局导入定义，不能修改它或已经使用的迁移来适配新结构。当前核心使用独立的精简定义。新格式与启动器发布版本解耦，使用局部协议和扩展记录演进；旧读者保留未知数据，跳过不理解的记录，暂停相应修改、恢复或清理。这里的版本往返适用于采用此存储契约的构建；此前只认识旧数据库的启动器不会读取新文档。

核心关系库使用持久协调库和 `DELETE/FULL` 日志设置。写事务先用 `BEGIN IMMEDIATE` 按固定顺序预留资源；不要改成 WAL，也不要先写附加库再申请协调库。与核心关系一起生效的设置、同步及恢复记录，必须在 SQL 决策提交前持久化准备文件，再按决策发布；不能用“先提交 SQL，再尽力写 JSON”代替。纯文档状态使用所属领域的串行文档事务。现有设置原子 API 通过发布决策协调文档，不在 SQL 中保存配置值；不因为现有数据库方便而增加配置表。

正常情况下数据库是实例关系的权威。原始目录 `state/instances/` 保存带修订号、校验和及分片的实例恢复副本；完全托管实例还在自身 `.axolotl/recovery/` 保存镜像。核心关系不可用时，完整且可理解的副本可提供只读浏览和已有本地运行时的启动上下文；不会自动重建一个可写核心，也不执行安装、关系修改或垃圾清理。

首次从旧版数据库接管到当前存储体系是启动前迁移：必须等待核心关系与旧操作记录的接管收据落盘后才继续初始化，允许长时间运行，并在启动界面报告迁移阶段和进度。中断后的启动继续未完成的类别；已接管的旧来源不再读取。该首次迁移不受日常记录恢复的 15 秒预算限制。

文件恢复分为“恢复记录”和“执行后续操作”。定位、解析、协议校验、选择备份与重建记录合计最多等待 15 秒；损坏、超时或无法恢复时允许启动，保留原件并在后台继续尝试。哈希、下载、复制、回滚和重试不计入这 15 秒。晚到的结果在发布或执行前重新核对修订号和实例运行状态；游戏运行期间不得回放会修改实例文件的操作。fail-ok 不表示恢复已完成，不得因此删除备份或把未知引用当成不存在。

更新前及周期备份写入 `Backups/databases/<id>/`，包含核心库、已接管的用户文档、实例恢复副本、操作日志和共享同步来源；缓存不是恢复必需项。可选领域缺失或不可读时应明确记录排除状态。恢复前关闭启动器，按完整集合恢复核心与相应发布记录，不单独替换 `system.db`；自定义内容目录还必须与同步来源描述保持一致。不要将缺失或未知引用视为空集进行文件清理。

维护层的显式离线恢复入口为 `state::store::maintenance::restore_snapshot(root, bundle, content_root)`；它要求启动器已关闭并独占原始协调库，验证快照后保留旧文件、重绑定引用并发布恢复收据。启动流程只继续已经明确发起并写入收据的恢复。该入口尚未接入桌面恢复界面，不应以手工逐个替换文件代替这一协议。

存在未完成且依赖绝对路径的文件恢复操作时，应先完成恢复再移动内容目录；不要批量替换日志中的任意字符串。恢复到不同目录同样需要确认日志记录的文件根仍有效，无法解释的记录保留并暂停相应修改。

**本地构建与已安装的正式版共用这份数据目录。** 启动桌面开发进程可能接管正在使用的数据；开发和恢复验证必须隔离。下面的 SQLx 降级脚本仅用于旧 `app.db`，不用于新核心或文档体系；不要手工修改 `_sqlx_migrations` 或套用旧脚本处理新库。

隔离数据目录有两种方式，推荐在开发时始终使用其中一种：

```powershell
# 构建期：该构建写入 red.ghs.axolotl-<后缀>，与正式版互不影响
$env:AXOLOTL_DATA_DIR_SUFFIX = "pr538"
pnpm app:build

# 运行时：把整个数据目录搬到指定路径（便携模式也用这个变量）
$env:THESEUS_CONFIG_DIR = "$PWD\.dev-data"
pnpm app:dev
```

如果数据库已经被较新的构建升级，用降级脚本回退（**运行前必须关闭启动器**；默认只做演练，执行时会先写一份备份）：

```powershell
# 列出已应用的迁移
node scripts/axolotl/downgrade-app-db.mjs --list

# 演练：显示将移除哪些迁移记录、删除哪些列
node scripts/axolotl/downgrade-app-db.mjs --to 20260903120000

# 实际执行
node scripts/axolotl/downgrade-app-db.mjs --to 20260903120000 --apply
```

`--to` 是**阈值而非单条**：它会移除**该版本及其之后**的全部迁移记录。请填你确认可以回退到的最老版本。

脚本默认在正式的数据目录里找库；如果那个库属于带后缀的构建，用 `--suffix` 指过去：

```powershell
node scripts/axolotl/downgrade-app-db.mjs --suffix pr538 --to 20260903120000 --apply
```

恢复备份的方法：关闭启动器，删除 `app.db` 及其 `-wal`/`-shm` 边车文件，再把 `app.db.before-downgrade-<时间戳>` 改名回 `app.db`。

脚本的几条硬性约束，遇到时请照提示处理而不是绕过：

- 只认识登记在案的迁移结构。遇到未登记的迁移会**拒绝执行**，因为删掉记录却留下它建的 schema 对象，会让重新安装该构建时因对象重复而失败。开发与 CI 中应补齐登记，不得用 `--allow-unmapped` 绕过。
- 若某个已登记迁移对应的列在库里不存在，说明登记的表结构与数据库不符（版本号被复用、列被改名等），脚本同样拒绝执行。
- 迁移维护的完整约束以 [app-lib 指令](packages/app-lib/AGENTS.md#database-migration-safety) 为准。降级脚本使用 `REVERTIBLE_SCHEMA` 登记新增列、表以及独立索引和触发器；数据迁移也要登记空对象 `{}`。修改迁移或映射后运行 `node scripts/axolotl/downgrade-app-db.test.mjs`。

该脚本依赖 Node 内置的 `node:sqlite`，并要求 Windows（解析默认数据目录需要 `APPDATA`；其他平台请用 `--db` 指定路径）。

## 仓库范围

Axolotl 的产品改动主要位于：

- `apps/app-frontend`
- `apps/app`
- `packages/app-lib`
- 上述包所需的共享 UI 与资源包

本仓库**不包含** Modrinth 网站、Labrinth API 或其运营服务源码。桌面端保留对 Modrinth 公共 API 的客户端兼容；如果需要参考上游实现，请仅手动挑选与 Axolotl 产品相关的改动，避免直接合并无关代码。

## 发布新版本

发布流程由 [`.github/workflows/axolotl-release.yml`](.github/workflows/axolotl-release.yml) 自动完成。版本号以 Git 标签为准，必须符合[语义化版本格式](https://semver.org/lang/zh-CN/)。

打标签并推送到远端即可触发发布工作流：

```powershell
git tag -a v1.2.3 -m "Axolotl Launcher 1.2.3"
git push origin v1.2.3
```

预发布版本请使用带后缀的标签（如 `v1.2.3-beta.1`）。

### 可选 Release notice

在 [`.github/release-notices.json`](.github/release-notices.json) 的 `notices` 中，以完整标签（包含 `v`）为键提交中英文提示。每项包含 `type`、`zh-CN` 和 `en-US`；`type` 支持 `note`、`tip`、`important`、`warning` 和 `caution`，文本支持 Markdown，多行内容在 JSON 字符串中使用 `\n`。编辑器可通过同目录的 JSON Schema 提供补全和校验。

notice 会显示在 GitHub Release 的发布日期之后、更新内容之前，并随 Release 正文进入更新清单和 CNB 镜像。没有对应标签的条目时不会显示；格式错误、缺失语言或空白文案会使发布预检失败。普通更新条目仍只在 `apps/app-frontend/src/announcements/catalog.ts` 维护，notice 不代替该版本的公告。

请在打标签前提交 notice 配置。Workflow 读取标签对应的文件，之后仅修改 `main` 或重跑同一标签不会读取新的内容。可以在本地预览：

```powershell
node scripts/axolotl/create-release-notes.mjs v1.10.0-beta.4 release-preview.md
```

预览要求公告目录中已有对应版本；检查后删除临时的 `release-preview.md`。运行 `node --test scripts/axolotl/create-release-notes.test.mjs` 可检查生成规则。

**自动发布工作流执行步骤**：

1. 将标签版本写入桌面应用构建配置。
2. 在 GitHub 托管的 Windows、macOS 和 Linux runner 上并行构建安装包。
3. 使用仓库 Secrets 中的 Tauri 私钥生成签名更新包。
4. 生成并校验包含全部桌面平台的 `latest.json`。
5. 校验成功后将草稿 Release 转为正式发布。
6. GitHub Release 发布完成后，将安装包、更新清单和 Release 信息镜像到 CNB，并同步镜像到 Gitlink、OSS。

源码分支和标签也由 GitHub Actions 在 push 和删除事件后直接同步到 CNB。CNB 仓库不运行定时同步或标签流水线，避免在 GitHub 构建尚未完成时占用 CNB 构建时长。

> 注意：自动更新公钥已固化在客户端中，私钥只保存在 GitHub Actions Secrets 中，切勿提交到仓库。
