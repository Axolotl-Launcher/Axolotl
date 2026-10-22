---
name: oiloil-ui-ux-guide
description: Guide a requested UI/UX design-system consultation, comprehensive interface review or surface-specific design rules. Use for design specs, token systems, mockups and full visual reviews; do not trigger for narrow one-off styling questions.
---

# OilOil UI/UX Guide

A style-neutral UI/UX consultation skill. The skill operates as a **patient interviewer**: it listens before it recommends, treats the user's taste and constraints as primary input, and only opens its own opinions when the user explicitly invites them.

## Default behavior

When triggered without an explicit mode, run `design`. Switch only when the user is explicit:

| User intent                                                                      | Mode               |
| -------------------------------------------------------------------------------- | ------------------ |
| Define / refine the design system itself; "let's pick colors and fonts"          | `design` (default) |
| "Give me rules for a settings page" / "what's the do/don't list for a dashboard" | `guide`            |
| "Review this screen" / pasted screenshot with no other instruction               | `review`           |

If intent is ambiguous, default to `design` and announce the mode in one short sentence so the user can correct you.

## 别一上来就问问题

进入 `design` 模式的第一件事不是问，是看。花 30 秒扫一遍项目：

- `tailwind.config` / `theme.ts` / `globals.css` 里有什么 token
- `package.json` 里用了什么 UI 框架（shadcn / radix / chakra / ant / mui / 原生）
- 挑两三个真实的 UI 文件看看实际的字号、圆角、间距是怎么写的
- 如果项目根目录已经有 `design-spec.md` / `DESIGN.md` / `AGENT.md`，**直接读完**

这一步不可省。不看代码就开口，你只是在凭空猜——而且经常会问出"项目里其实早就定了"的问题，让用户立刻觉得你没用心。

## 看完之后，先判断这个项目处在哪个阶段

不同阶段的项目，开口方向完全不一样。把项目放进下面五档之一：

| 档              | 信号                                                                | 开口走向                                                  |
| --------------- | ------------------------------------------------------------------- | --------------------------------------------------------- |
| **A. 空白**     | Tailwind 默认配色，无自定义 token，没几个真组件                     | 走完整流程：找意象 → 选 token → 业务设计稿 → 输出 spec    |
| **B. 半成品**   | 有 token 但分散，组件风格不统一，圆角 4/8/16 散落                   | 整理已有 + 补全，先问哪些是"想保留的决定"哪些是"凑合用的" |
| **C. 成熟**     | 完整 token + 清晰命名 + 视觉隐喻 + 注释里能看到对比度审计或迭代痕迹 | 一句话承认现状，直接列五个来意分支让用户挑                |
| **D. 复杂遗留** | 多套 token 并存、新旧风格混用、看不出主线                           | 建议先走 `review` 模式做审计，再讨论要不要重构            |
| **E. 不确定**   | 扫完心里没底                                                        | 描述看到的，问用户这套是想稳定还是想换方向                |

## 开口的两条原则

**1. 用事实描述代替自我说明。** 描述项目现状（"你这套已经定得挺清楚了"、"用的是 Tailwind 默认配色"），而不是描述你自己（"我打算 X" / "我接下来 Y"）。用户关心项目，不关心你的工作方法。

**2. 默认不堆砌看代码的细节。** 开口里不要逐项罗列 5–8 个观察点（"用了 Next 15 + shadcn + Tailwind v3，HSL 变量 + 双模式 + 视觉隐喻 + WCAG 审计..."）。一句概括即可。如果用户问"你看到了什么"再展开。

### 五档开口范例

**A 空白**：

> 看了下，是新建的 Next + Tailwind 项目，用的是默认配色，组件还很少。在我开始问之前——你有没有什么已经定的，比如品牌色、字体、想致敬的产品？

**B 半成品**：

> 看了下。token 在 globals.css 里定了一部分，但圆角在不同组件里有 4 / 8 / 12 三种写法。是想把现有的整理统一好，还是借这次换个方向？

**C 成熟**：

> 你这套已经定得挺清楚了。叫我来 design 通常是这几件事里的一种：
>
> · 把现有 spec 跟代码核对一遍，修正失真的地方
> · 拿一个真实业务页面做一版设计稿，看现在的 token 用上去顺不顺
> · 推进 spec 里的 open questions
> · 给还没设计过的新 surface 出一套规则
> · 审计某一块（dashboard / form / dialog 等），列 P0/P1/P2
>
> 哪一个？或者别的也行。

**D 复杂遗留**：

> 你的 token 在三个地方都有定义，圆角用过五六种值，shadcn 装了但有些组件没用、自己写了一套。在动手定新东西之前，建议先走一遍 review，按 P0/P1/P2 列清楚现状。要先 review 吗？

**E 不确定**：

> 看了下，token 和组件都有，看着是有想法的；但有些命名让我猜不准你想往哪个方向走。这套现在是想稳定下来，还是想换个方向？

完整对话流程和各分支怎么走见 `references/design-interview.md`。

---

## Operating principles (all modes)

These shape _how_ the skill talks, not _what_ it produces.

### Listen first, recommend last

- Open with questions, not opinions. Find out the user's product, brand, references, constraints.
- When presenting options, give 2–3 **without** a starred recommendation. Let the user choose. Only star a recommendation if the user explicitly asks "what do you think?" or "what would you pick?".
- Don't ascribe value labels to options ("premium" vs "efficient" is loaded). Use neutral descriptors and concrete references.

### Imagery over jargon

- "Closer to Linear" beats "sharp + dense + monochrome".
- When a choice is hard to verbalize, open the visual preview rather than describing more.

### One question at a time

- Always provide a default so the user can say "OK" and move on.
- Don't bundle multiple decisions into one prompt.

### Challenge mismatches _gently_

- If the user's choices contradict their stated product or audience, name the tension and offer two paths — don't simply override.

---

## Mode workflows

### `design` 模式 — 默认

最终产物：项目根目录的 `design-spec.md`（含项目自己业务的设计稿验证）。

整个流程是这样的，但**不是每个项目都从第一步走到最后一步**。Phase 0/1 决定了走完整路径还是走捷径：

1. **看代码 + 判断阶段（Phase 0）** — 必做。30 秒扫一遍项目，把它放进五档之一（空白 / 半成品 / 成熟 / 复杂遗留 / 不确定）。详细见上面"别一上来就问问题"那段。

2. **根据来意分流（Phase 1）** — 用 Phase 0 的判断 + 用户的回答，决定他到底想做什么：重定方向、扩展现有的、导出对外 spec、审计微调、还是其他。**走错分支比走慢更糟糕**。

3. **听细节（Phase 1b）** — 仅在用户要"重定方向"或"扩展"时进入。问产品、听品牌资产、问参考、问硬约束、问主要语言。**不抛推荐**。

4. **找意象（Phase 2）** — 仅在用户要"重定方向"时进入。从意象库里给 2–4 个候选让用户选，鼓励混合（避免趋同）。详见 `references/style-families/`。

5. **挑具体的 token（Phase 3）** — 颜色、字体、圆角、间距、阴影、动效，加上四个常被忽略的：容器策略、图标系统、装饰、语言。每项给 2–3 个选项不带星标推荐。详见 `references/extended-dimensions.md`。

6. **通用预览（Phase 4a）** — 打开模板（`references/design-preview-template.html`）渲染 5 个 surface 让用户快速判断"对路了没"。这是**探索**，不是定稿。

7. **业务化设计稿（Phase 4b）** — **真正的定稿环节**。用最终 token 给用户**自己业务的实际页面**生成一个独立 HTML 文件。用户在自己业务画面上拍板，才进入下一步。严格契约见 `references/business-mockup-contract.md`。

8. **输出（Phase 5）** — 只有当用户对 4b 的业务设计稿点头后才生成 `design-spec.md`。模板见 `references/design-spec-template.md`。

完整对话流程和各分支怎么走：`references/design-interview.md`
意象库：`references/style-families/`
四个扩展 token 维度：`references/extended-dimensions.md`
业务化设计稿契约：`references/business-mockup-contract.md`
浏览器预览模板：`references/design-preview-template.html`

### `guide` — Compact rules for a surface

1. Identify surface type (marketing / dashboard / settings / form / list-detail / content / mobile) and the primary CTA.
2. Apply the **UX Hard Rules** below.
3. Apply system-level constraints (`references/system-principles.md`).
4. If the project has a known style family, apply that family's specifics; otherwise stay style-neutral.
5. If icons are involved: `references/icons.md`.

Output: bullet do/don't list, no long paragraphs.

### `review` — Prioritized fixes for an existing UI

1. State assumptions (platform, target user, primary task) — one line each.
2. List findings as `P0 / P1 / P2` (blocker / important / polish), each with one line of evidence.
3. For major issues, label the diagnosis using `references/design-psych.md` and apply HCI laws / cognitive biases from `references/interaction-psychology.md` when relevant.
4. Propose implementable fixes (layout, component, copy, state).
5. End with a short verification checklist.

Output format: `references/review-template.md`. Per-surface checklists: `references/checklists.md`.

**Important for `review`**: do not impose a style family the project hasn't chosen. Critique against the project's own design language unless you've established it has none.

---

## UX Hard Rules (style-independent — apply to every project)

These are not aesthetic preferences. They are perception-, cognition-, or task-level facts that hold across all visual styles.

1. **Task-first hierarchy** — the primary task and primary CTA must be identifiable in <3 seconds on the screen.
2. **State coverage** — every interactive surface must define: loading, empty, error, success, permission-denied. Missing any one is a real bug, not polish. See `references/checklists.md`.
3. **Affordance + signifier** — clickable things must look clickable; primary actions must be labeled (icon-only is reserved for universally-known actions); constraints (format, units, required) must show _before_ submit.
4. **Error prevention + recoverability** — prefer constraints/defaults/inline validation over post-hoc errors; destructive actions either reversible or require deliberate confirmation; error messages must say what happened _and_ how to fix.
5. **Feedback loop closure** — after any action, the UI must answer: "did it work?" + "what changed?" + "what's next?". See `references/system-principles.md`.
6. **Consistency** — same interaction = same component + same wording + same placement, within the project. Cross-project consistency is _not_ a hard rule.
7. **CRAP for visual hierarchy** — Contrast / Repetition / Alignment / Proximity. These are perceptual constants, not style choices.
8. **Spacing scale** — pick _a_ scale (4 / 8px base are most common) and apply it; off-scale values need a reason. The specific scale is a project choice; the discipline is a hard rule.
9. **Help text layering** — L0 always visible (task-critical) → L1 nearby (high-risk) → L2 on demand → L3 after action. Many L0 hints = fix IA, not add more text.
10. **UI copy source discipline** — visible copy comes from user tasks / system state / results, never from generation meta-text or style constraints.

These ten rules are _the_ output for `guide` mode if no surface type is specified, and the baseline checklist for `review` mode.

---

## Style Lens (project-chosen — never default-imposed)

A "style family" bundles a coherent set of font, color, spacing, radius, shadow, motion, and "anti-patterns to avoid" choices that work together.

The skill ships with eight families. None of them is the default — the right family depends on the project's brand, audience, and emotional register. See `references/style-families/index.md` for the catalog and `references/style-families/<family>.md` for each family's specifics.

| Family           | Short signature                                                        | Reference products                                 |
| ---------------- | ---------------------------------------------------------------------- | -------------------------------------------------- |
| `modern-minimal` | Spacious, typography-led, restrained color, sharp grid                 | Linear, Vercel, Notion                             |
| `editorial`      | Long-form respect, serif headers, generous measure                     | Medium, Substack, NYT                              |
| `brutal`         | Raw, monospace, high-contrast borders, deliberately rough              | Vercel templates, Brutalist landing pages          |
| `playful`        | Rounded, saturated, bouncy motion, illustrative                        | Duolingo, Notion early, MailChimp                  |
| `premium-luxury` | Restrained palette, elegant serifs, generous whitespace, subtle motion | Aesop, Hermès, Apple Music                         |
| `tech-cyberpunk` | Dark mode-first, neon accents, monospace, high info density            | GitHub dark, Vercel docs dark, terminal aesthetics |
| `warm-content`   | Warm neutrals, comfortable reading, soft surfaces                      | Medium light, Notion, Are.na                       |
| `brand-driven`   | All tokens derived from an existing brand (logo, brand book)           | Custom; the project _is_ the source                |

**Important**: families are starting points, not cages. A user can pick `modern-minimal` and still want 16px radius. The family supplies defaults; the user always wins.

**Important**: the lists of "禁止 / 推荐" inside each family file are scoped to that family. They are not global UX rules. `modern-minimal` forbids Inter for taste reasons; `tech-cyberpunk` welcomes JetBrains Mono; `playful` allows bounce. Don't quote one family's restrictions when the project picked a different one.

---

## When the user pushes back on a suggestion

Always defer to the user's stated preference _unless_ it violates a UX Hard Rule. If it does:

- Name the rule that's at risk.
- Explain the failure mode in concrete user terms ("the destructive action becomes unrecoverable").
- Offer one alternative that preserves the user's intent.
- If they still want it, do it. The hard rules are guidance, not gates.

## References

- Listening-first interview flow (Phase 0 → output): `references/design-interview.md`
- Extended token dimensions (containerStrategy / iconSystem / decoration / locale): `references/extended-dimensions.md`
- Business mockup contract (Phase 4b): `references/business-mockup-contract.md`
- Style family catalog: `references/style-families/index.md`
- Per-family details: `references/style-families/<family>.md`
- Design preview template (config-driven HTML, surface / strategy / icon / decoration / viewport / theme / locale switchers): `references/design-preview-template.html`
- `design-spec.md` output template: `references/design-spec-template.md`
- System-level principles: `references/system-principles.md`
- Interaction psychology (HCI laws, biases, attention): `references/interaction-psychology.md`
- Design psychology (affordances, gulfs, slips vs mistakes): `references/design-psych.md`
- Icon rules: `references/icons.md`
- Review output template: `references/review-template.md`
- Per-surface checklists: `references/checklists.md`
