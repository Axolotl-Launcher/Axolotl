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
- **Rust 编译并行数**：dev、CI、release 和依赖缓存预热统一使用 4 个 Cargo 编译任务，macOS ARM64 使用 2 个。`pnpm app:dev`、`pnpm app:build` 和应用的 `tauri` 脚本会自动选择；本地在 macOS ARM64 上直接运行 `cargo` 时需设置 `CARGO_BUILD_JOBS=2`。可通过 `CARGO_BUILD_JOBS` 显式覆盖默认值。

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

启动器把设置、数据库、日志与缓存放在同一个数据目录下，Windows 上是 `%APPDATA%\red.ghs.axolotl`，其中按更新通道分为 `release\app.db` 与 `beta\app.db`。

**本地构建与已安装的正式版共用这份数据目录。** 本地跑一次桌面应用，改动的就是正在使用的那份数据库，因此：

- 迁移是单向的，也不要手工改 `_sqlx_migrations` 表。
- 某个构建写入新迁移后，比它旧的构建（包括已安装的正式版）将因 sqlx 拒绝打开而无法启动，报错形如 `migration <版本号> was previously applied but is missing in the resolved migrations`。
- 需要回退时用下面的降级脚本，不要手工处理。

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
