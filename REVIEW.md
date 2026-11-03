# REVIEW.md — Agent 提交、推送、评审与归档纪律

> 本文件是 Axolotl 仓库中**一切 Agent 交付工作**的单一归口：提交、推送、创建 PR、撰写 commit 与 PR、回顾代码变更、自我审查、工作流复用、历史遗留存档。
> 开工前先读它。它与 [AGENTS.md](AGENTS.md) 及各子目录的 `AGENTS.md` **配合**使用，不替代它们；技术标准见 [standards/](standards/README.md)，环境搭建见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 1. 适用范围与优先级

- 适用于在本仓库中由 Agent 执行或协助的**全部**交付动作，无论该动作由主 Agent 还是被委派的子 Agent 执行。
- 优先级：操作者当次明确指令 > 本文件 > [AGENTS.md](AGENTS.md) 与各级子目录指令 > [standards/](standards/README.md)。同层冲突时，**更具体者优先**。
- **本文件不是实施授权**。它规定"怎么做"，不授权"做什么"：审查请求保持只读，技能（[.agents/skills/](.agents/skills/)）不授权实现、依赖变更、提交、消息或远端操作。
- 与操作者指令冲突时以操作者为准，并**在交付说明中记录该偏离**。

## 2. 核心原则

1. **证据优先于自述**。子 Agent、外部贡献者、工具输出的结论都是**自述而非事实**。对外报告成功前，必须用只读手段复核到可验证句柄：文件与行号、命令与退出码、远端 SHA、HTTP 状态。核不到就**明说核不到**。
2. **不信任自己的上下文**。上下文中的状态（文件内容、分支、远端、任务进度）一律视为可能过期；据此判断或汇报前先当场核实，冲突时以当场事实为准。
3. **如实归因**。不得错怪网络、环境或他人；也不得把功劳或责任错记到错误的来源上。归因要有对照实验或历史证据支撑。
4. **先审后发**。任何会落到外部系统的文本（PR 正文、review 评论、issue、Release notes、公告）必须先给操作者看过**将要发送的最终原文**；发出前若改了结构、措辞或增删条目，授权即失效，需重新送审。
5. **不确定就问一次**。需求歧义、需要业务背景、仓库与文档都查不到且猜错代价高时，先问；不要"先猜一个再改"。
6. **委派实施、只读复核**。复杂改动交由子 Agent 分工执行，主 Agent 负责规划、审查与验收；**指令文件自身**（本文件、各级 `AGENTS.md`）与发布类操作由主 Agent 亲自维护。
7. **一次改动一件事**。不夹带、不顺手、不扩大范围；发现的范围外问题**登记**而非就地修掉。

## 3. 提交（commit）

- **信息格式**：一律 [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)，类型、作用域与 `!`/`BREAKING CHANGE:` 的用法见 [AGENTS.md](AGENTS.md) 的 "Verification and Commits"。本文件不重复该表。
- **正文写"为什么"**：说明动机、被修正的错误认知、以及取舍理由。不要写"改了 A、改了 B"式的流水账——那是 `git show` 的职责。
- **提交前必跑该文件类型所属的门禁**，而不是笼统地跑一遍：
  - 改 TOML（`Cargo.toml`、`.cargo/config.toml`、`tombi.toml` 等）→ `tombi fmt --check`（仓库有该门禁，且它红掉的是**所有**分支）。
  - 改前端 → 该包的 `fix` 与 `lint`，见 [frontend development](standards/frontend/DEVELOPMENT.md)。
  - 改 Rust → `cargo fmt --all --check`，并按 [packages/app-lib/AGENTS.md](packages/app-lib/AGENTS.md) 或 [apps/app/AGENTS.md](apps/app/AGENTS.md) 跑对应测试。
  - 改 Agent 指令或技能 → `pnpm axolotl:check-agents`。
- **只暂存自己的路径**：用 `git add <具体路径>`。当工作区内可能有其他 Agent 或用户的同时改动时，**严禁** `git add -A`/`git add .`；提交前用 `git show --name-only HEAD` 核对实际入库文件与预期一致。
- **一个提交只做一件事**；文档改动与代码改动分开；**不为规范化历史而改写已推送的提交**。
- **`[skip ci]`**：仅用于**不影响桌面端**（`apps/app/`、`apps/app-frontend/` 及其构建、打包、运行时、专属依赖）的提交，且要接在 conventional 前缀之后，例如 `docs(agents): … [skip ci]`。桌面端相关提交**永不**使用。
- 提交前确认工作树中**没有**他人的在途改动被卷入；发现被卷入时先停下询问，不要替他人决定。

## 4. 尾签（trailers）

格式 `标签: 姓名 <邮箱>`，置于提交信息**正文末尾**（`BREAKING CHANGE:` 除外，它是正文的一部分）。按用途分类：

- **归属与致谢**：`Signed-off-by`（DCO 签署）、`Co-authored-by` / `Co-developed-by`（共同作者）、`Suggested-by` / `Helped-by` / `Mentored-by` / `Thanks-to`（想法与帮助）、`Based-on-patch-by`（衍生自他人补丁）。
- **来源与验证**：`Reported-by`（问题报告者，用于修复提交）、`Tested-by`、`Reviewed-by`、`Acked-by`。
- **追踪关联**：`Fixes`、`Closes`、`Resolves`、`Link`、`Cc`、`Bug`。
- **流程**：`BREAKING CHANGE:`、`Release-As`、`Change-Id`。
- **AI 参与**：`Assisted-by`（声明 AI 辅助）、`Acted-By`（人类操作者）。

使用要求：

- **优先沿用本仓库既有习惯**，不要另创一套。本仓库现状：提交正文用 `fixed #N` 关闭 issue，PR 正文用 `Fixes #N`。
- 引用他人工作时要**如实**：报告者用 `Reported-by`、共同作者用 `Co-authored-by`，不要把外部来源写成自己的。
- 邮箱与姓名必须来自**已知来源**（`git config`、对方提交记录），不得猜测或编造。

## 5. 推送（push）

前置条件（**缺一不可**）：

1. 工作树干净，且只含本次要推送的提交；
2. 相关门禁已跑过且通过（第 3 节）；
3. 已只读核实目标状态：远端当前 tip、本次提交与远端的关系。

流程：

```sh
git fetch origin main --prune
git rebase origin/main          # 有冲突先解决，不要用 merge 掩盖
git push origin HEAD:main
git fetch origin main --prune   # 回读
git rev-parse origin/main       # 必须等于本地 HEAD
```

- **必须回读比对**：只报告"已推送"而没比对远端 SHA，不算完成。
- **远端被强推**（`forced update`）时，先核实新 tip 是否仍包含本方基点、是否与本方文件重叠，再决定 rebase 还是放弃；不得盲目 `--force`。
- **推送是对外且不可逆的动作**，由主 Agent 亲自执行，不委派。
- 推送后发现问题的正确做法是**追加修复提交**，不是改写已推送历史。

## 6. 创建 PR

- **默认直接推 main**（本仓库既有习惯）。仅在以下情形开 PR：外部贡献者提交、需要独立评审、需要 CI 在合并前背书、或操作者要求。
- 本仓库**根目录没有 PR 模板**，因此正文按固定结构写：
  1. **动机**：解决什么问题，关联 issue（`Fixes #N`）；
  2. **改动**：按子系统列出，说明取舍；
  3. **验证证据**：逐条给命令与真实退出码，含变异验证（改坏必须失败、改回必须通过）；
  4. **未验证项**：如实列出没跑、没测、没复核的部分；
  5. **风险与回滚**：影响面与如何回退。
- 标题用 Conventional Commits 格式；正文用与仓库一致的措辞，**不加戏**（不加未被要求的章节、建议块、时间线、自我说明）。
- **先审后发**：任何落到 GitHub 的文本（含 review 评论、inline comment）都必须先给操作者看过最终原文。
- 合并他人 PR 前，先做**无副作用的冲突预演**（`git merge-tree --write-tree --name-only origin/main <ref>`），并核实其 CI 结论是否由本方改动造成；不要把自己的破坏记到他人头上，也不要替他人在 PR 上发言。

## 7. 回顾代码变更（review）

- **评审请求保持只读**；评审不隐含实现授权。
- 评审至少覆盖：契约与接口是否被破坏、边界与错误路径、并发与竞态、**已应用迁移的不可变性**、品牌名合规、i18n、门禁覆盖是否足够、以及"这条改动是否真的解决了声称的问题"。
- **结论必须有证据**：给出 `文件:行` 与复现命令；"看起来没问题"不是结论。
- **独立复核**他人与子 Agent 的结论（第 2.1 条）。复核方式优先选择**对照实验**：在干净基线上复跑、换一个入口复现、或做变异验证。
- **要求变异验证**：任何新增的守卫（测试、断言、门禁）都要证明它**能失败**——把被守护的对象改坏，守卫必须报错，改回必须通过。
- **区分"能通过"与"能防住"**：本地全绿不等于 CI 稳定；对已知抖动的门禁，要说明本地与 CI 的差异，不得用本地结果宣称 CI 已修好。
- 发现自己的判断有误时**当场更正**，并把更正写进归档（第 10 节）。

## 8. 自我审查提升

交付前逐条自问：

- 我实际改了哪些文件？与计划是否一致？有没有夹带他人在途改动？
- 我跑了哪些门禁？退出码是多少？有没有"跑了但没看结果"？
- 我的结论有没有反例？有没有把"本地通过"当成"CI 通过"？
- 我是否把一个来源的结论当成了事实？
- **我有哪些部分没能验证？**（这一条必须写进交付说明，不得省略）

发现错误时：更正结论 → 说明原先错在哪 → 记录到归档。**不要**悄悄改掉结论而不留痕迹。

## 9. 工作流复用

- 优先复用既有资产：[.agents/skills/](.agents/skills/) 中的项目技能、[standards/](standards/README.md) 中的技术标准、`pnpm axolotl:*` 脚本、各包既有的 `test`/`lint`/`fix`/`build`。
- **不为一次性验证新建长期脚本**；临时产物在收尾时清理，中间与归档产物写入 `.agents/plans/`（见 [AGENTS.md](AGENTS.md) 的 "Global Safeguards"）。
- 已确证可复用的流程：
  - **worktree 隔离**：并行任务各用一个独立 worktree，避免互相覆盖。
  - **同一 worktree 单一写者**：同一时刻只允许一个 Agent 写同一 worktree；`rebase`/`stash`/`checkout` 会清掉他人的在途改动，执行前必须先停掉其他写者。
  - **cargo 单条串行**：同一 target 目录并发构建会互相破坏链接，产生"假失败"。
  - **变异验证用 `try/finally` 收尾**：中途失败也不能把改坏的代码留在树里。
  - **从 git 对象恢复文件必须用 shell 重定向**（`git show <ref>:<path> > <path>`），不要用会改写换行符的写入方式。

## 10. 历史遗留存档

- 归档落点**只有** `.agents/plans/`（该目录被忽略，见 [AGENTS.md](AGENTS.md) 的 "Global Safeguards"）；**严禁** `git add -f` 或其它绕过 `.gitignore` 的手段。
- 文件名要有描述性，使后续会话能据此还原当时的工作上下文。
- 归档条目必须包含：**现象 → 证据 → 根因 → 处置 → 未决项**。证据要给可复现的命令或文件路径。
- **未决项必须显式处理**：要么关闭并写明依据，要么标注"待决策"并写清需要谁决定什么。**不得静默遗留**。
- 需要跨会话跟踪多项未决工作时，建立**台账**文件（同一目录内），逐项记录编号、状态、证据与阻塞关系，并在每轮工作后回填。
- 恢复上下文时：先读归档文件，再**当场核实**其中记录的状态是否仍然成立（第 2.2 条）。

## 11. 已确证的工具事实（本仓库）

每条都是本仓库实测得到的、会反复踩到的判据：

- **TOML 表顺序**：`tombi` 管 TOML 格式，表按字母序排列（如 `[env]` 必须在 `[target.*]` 之前）。改任何 TOML 后跑 `tombi fmt --check`。
- **typos 的排除语义**：判断排除是否生效必须用**不带路径参数**的 `typos`；要按排除规则检查某个路径需加 `--force-exclude`（显式路径会绕过 `extend-exclude`）。
- **typos 与子模块**：CI 的 `typos` 作业**不拉取子模块**（只有 `cargo-shear` 与 `clippy` 会），因此本地已初始化子模块时，bare `typos` 会在 `third-party/` 下报出与你的改动无关的错。判据只看自己的文件；**不要**为此去改 `_typos.toml` 的排除项。
- **依赖安装门槛**：[pnpm-workspace.yaml](pnpm-workspace.yaml) 设有 `minimumReleaseAge`，窗口内的新版本会被自家门槛拒绝；规避只能用**请求级** `--config.minimumReleaseAgeExclude`，不得改仓库设置。
- **turbo 的环境透传**：[turbo.jsonc](turbo.jsonc) 的任务 `env` 是封闭白名单，未声明的变量会被剥离；`RUST_*`、`NEXTEST_*` 等已在列，新增变量需同步声明。
- **前端视觉门禁**：`test:visual` 的 worker 数与单测超时上限会影响抖动表现；改这两个值时必须以"最慢单条用例实测耗时"为依据，而不是文件总耗时。
- **门禁脚本自身**：`pnpm axolotl:check-agents` 校验指令与技能的 Markdown 链接与 Skill frontmatter；[scripts/axolotl/check-agents.mjs](scripts/axolotl/check-agents.mjs) 的 `trackedGuidance` 决定哪些文件被校验，新增根级指令文件时要把它纳入。
