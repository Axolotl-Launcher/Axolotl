# oiloil-ui-ux-guide

一套 **风格中性** 的 UI/UX 咨询 Skill。它不是一个"特定审美的助手"，而是一个 **耐心的访谈者**：

- 不预设你的产品该长什么样
- 不强加单一风格的字体 / 色彩 / 圆角偏好
- 先听你的产品、品牌、参考、约束，再给选项
- 给出的选项是平等并列的，不带"推荐星标"——除非你主动问"你觉得呢"

## 适用场景

- **新项目**：通过对话定下设计系统（颜色 / 字体 / 圆角 / 间距 / 阴影 / 动效），生成 `design-spec.md`
- **已有项目**：评审现有 UI，按 `P0 / P1 / P2` 给修复清单
- **单一面型**：给一种页面类型（dashboard / form / 长文等）的"该做 / 不该做"规则

## 三种模式（默认 `design`）

未指定模式时进入 `design`。其他模式需要显式触发。

| 模式     | 用途                                                                      | 默认？   |
| -------- | ------------------------------------------------------------------------- | -------- |
| `design` | 通过对话定制项目专属的 design spec，最终在项目根目录生成 `design-spec.md` | 默认     |
| `guide`  | 给定页面类型，输出"该做 / 不该做"规则                                     | 显式触发 |
| `review` | 评审现有界面，输出 `P0 / P1 / P2` 修复清单                                | 显式触发 |

**Skill 不会被轻量问题触发**——"这两个蓝色哪个好看"这种问题它会直接答，不会启动整套对话流程。

## 核心范式：UX Hard Rules vs Style Lens

旧版本把"现代极简"风格的 token 偏好当成全局硬规则（禁止 Inter、禁止纯黑、禁止圆角 >12px、强制 OKLCH 等）。这不通用——同样的规则用在儿童产品、奢侈品、游戏、品牌强烈的项目上反而是限制。

新版本把规则严格分成两层：

### UX Hard Rules（10 条，跨风格不可妥协）

任务优先 / 状态闭环 / Affordance / 错误预防 / 反馈闭环 / 一致性 / CRAP / 间距规整 / 提示分层 L0–L3 / UI 文案纪律——这些是感知与认知层面的事实，不是审美。

### Style Lens（8 个风格家族，项目自选）

| Family           | 一句话                            | 参考                                     |
| ---------------- | --------------------------------- | ---------------------------------------- |
| `modern-minimal` | 留白 + 排版 + 克制色彩 + 锐利网格 | Linear, Vercel, Notion                   |
| `editorial`      | 长文友好、衬线标题、宽松版心      | Medium, Substack, NYT                    |
| `brutal`         | 原始、等宽、硬阴影、刻意粗糙      | 独立 maker 站点、Vercel brutal templates |
| `playful`        | 大圆角、饱和色、弹性动效、插画    | Duolingo, MailChimp, Notion 早期         |
| `premium-luxury` | 优雅衬线、留白即价值、缓慢动效    | Aesop, Hermès, Apple Music               |
| `tech-cyberpunk` | 暗色优先、霓虹强调、等宽密集      | GitHub dark, Vercel docs, Cursor         |
| `warm-content`   | 暖色中性、舒适阅读、柔软表面      | Are.na, Notion light, Craft              |
| `brand-driven`   | 所有 token 来自现有品牌资产       | 项目自身的 brand book                    |

每个家族都有自己的字体推荐 / 色彩倾向 / 圆角范围 / 动效语汇 / 反 AI 缺陷。**这些规则是家族内部的，不会跨家族适用**——`modern-minimal` 不喜欢 Inter，`tech-cyberpunk` 拥抱 Geist Mono，`playful` 允许 bounce。Skill 不会用一个家族的规则去批评另一个家族的项目。

## `design` 模式的对话节奏

```
Phase 0  Scan code (silent, 强制)
   ↓
Phase 1  Listen — 开放式提问，不抛推荐
   ↓
Phase 2  Style family — 用户已说清就确认，否则展示 2–4 个并列家族
   ↓
Phase 3  Visual choices — 每个 token 给 2–3 个选项，无星标
   ↓
Phase 4  Full preview — 在 dashboard / marketing / content / form / pricing 多个面型上预览，可切 dark mode + viewport
   ↓
Phase 5  Output design-spec.md
```

**硬性前置**：进入 `design` 之后，Skill 会先静默扫一遍代码（Tailwind / theme / CSS 变量 / UI 框架 / 关键 UI 文件），形成对现有 design tokens 一致性的事实判断（不是好坏判决），再以一段总结开口。**不会在没看代码的情况下问任何问题**。

完整流程见 [设计访谈流程](references/design-interview.md)。

## 可视化预览模板

`references/design-preview-template.html` 是一个 **风格中性的静态预览模板**——它的外壳（chrome）刻意保持灰白色调，不抢戏。每次迭代只重写 JSON config，用户刷新浏览器即可。

模板支持：

- **Compare 模式** — 多个候选 token 集并排展示，**渲染同一个真实面型**（dashboard / marketing / content / form / pricing），方便横向对比
- **Full 模式** — 完整设计系统应用到上述 5 个面型上，带 **viewport 切换**（desktop / tablet / mobile）和 **dark mode 切换**

## 跨工具支持（Codex / Claude Code / Cursor / Windsurf）

- `AGENTS.md` 作为跨工具共享指令入口
- 仓库采用 `.agents/skills/` 发现 Skills；各工具的实际支持以其加载规则为准。
- `SKILL.md` 是 Skill 行为的真实定义

## 安装

### 用 `skills` CLI 一键安装到多个 Agent

```bash
npx skills add oil-oil/ui-ux-guide --list
npx skills add oil-oil/ui-ux-guide -a codex -a claude-code -a cursor -a windsurf
# 全局
npx skills add oil-oil/ui-ux-guide -g -a codex -a claude-code -a cursor -a windsurf
```

### 手动复制

```bash
git clone https://github.com/oil-oil/ui-ux-guide ~/.codex/skills/oiloil-ui-ux-guide
```

## 触发方式

两种方式：

1. 显式点名 — `请使用 $oiloil-ui-ux-guide 帮我把这个项目的设计规范定下来。`
2. 描述任务 — "帮我定一套这个项目的颜色和字体" / "评审这个仪表盘" / "给我表单页的 UX 规则"

只描述任务、没指定模式时，Skill 默认进入 `design`。轻量问题（"这个按钮颜色对吗"）不会触发完整流程。

## 推荐提示词模板

### `design`（默认 — 定制项目设计规范）

```text
请使用 $oiloil-ui-ux-guide 帮我把这个项目的设计规范定下来。
背景：[一句话产品 / 目标用户]
要求：先扫一遍现有的 design tokens 再开始问我；
我希望你听完我的回答再给选项，不要一上来就推荐风格。
最终输出 design-spec.md 到项目根目录。
```

### `review`（评审现有界面）

```text
请使用 $oiloil-ui-ux-guide 的 review 模式。
背景：Web 管理后台，目标用户为首次完成配置的新用户。
请输出 P0/P1/P2 问题清单 + 每个问题的可执行修复 + 验收检查点。
注意：不要按某种风格家族评判我们 — 我们目前还没定型。
```

### `guide`（先出规则）

```text
请使用 $oiloil-ui-ux-guide 的 guide 模式。
页面类型：B 端长表单（8 个字段）。
请输出该做 / 不该做规则，覆盖：CTA 层级、状态、affordance、错误预防、提示分层、间距。
要求：纯要点，不要长段落。
```

## 当前仓库结构

此 Skill 位于 `.agents/skills/oiloil-ui-ux-guide/`：

- [SKILL.md](SKILL.md)：触发范围和各模式规则。
- [evals/evals.json](evals/evals.json)：12 个自包含行为评估案例；校验 JSON 与引用不代表已经运行模型评估。
- `references/`：模式流程、模板、检查清单和风格家族。

## 参考文档

- skills CLI（跨 Agent 分发）：<https://github.com/vercel-labs/skills>
- Cursor 规则与 `AGENTS.md`：<https://docs.cursor.com/context/rules-for-ai>
- Windsurf `AGENTS.md` 支持：<https://docs.windsurf.com/windsurf/cascade/memories>

## 许可证

上游项目声明 Apache License 2.0；许可证信息见 [上游仓库](https://github.com/oil-oil/ui-ux-guide)。
