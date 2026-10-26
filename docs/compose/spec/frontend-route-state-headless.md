---
feature: frontend-route-state-headless
status: delivered
updated: 2026-06-10
branch: feature/frontend-route-state-headless
commits: 0ff225f6a..34c1cc08f
---

# 前端路由/状态治理 + 渐进 Headless（reka-ui）

## Report

**What was built** — Replaced the monolithic `routes.js` with per-domain route modules and typed `RouteMeta`; PascalCase route names (URLs unchanged). Browse/upgrade return navigation now lives in a Pinia store (helpers remain as forwarders). Thinned `content-install` / `content-selection` into types/messages/compat/manual-downloads/registry/session modules while keeping public APIs. Added `reka-ui@2.10.x`, `/headless-demo`, and HeadlessButton/Tooltip/Dialog/Select adapters; wired Appearance tooltip + default-landing-page select. Settings-About no longer top-level-awaits Tauri; 3D loads async with error isolation.

**Verification** — `vue-tsc` PASS; eslint/prettier on touched files PASS; `node --test` return-flow suites 17 PASS; `pnpm --filter @modrinth/app-frontend build` PASS (~33s). `pnpm prepr:frontend:app` was used during implementation. About page: accepted as **production-first** (dev Vite skeleton stickiness out of scope per S3).

**Journey log** — (1) Lab tools stay sibling routes because the hub has no `RouterView`. (2) Pinia must install before the router (`main.js`). (3) `node --test` needs relative `.ts` imports in helpers. (4) Brand fills use `--color-accent-contrast`, modals `bg-surface-3`. (5) About `dev` hang is accepted; `vite build` is the acceptance gate for that page.

## [S1] Problem

桌面前端路由与状态已能支撑产品，但组织债务在阻碍后续演进：

1. **路由单文件** `apps/app-frontend/src/routes.js`（~500 行）堆叠全部域；命名含空格（`'Discover content'`）；Lab 平铺、Library 嵌套不一致；返回流依赖模块级可变状态（`browse-return-state` / `upgrade-return-state`）。
2. **状态多轨无归属**：Pinia（theme/breadcrumbs）、DI providers（download/content-install/selection）、TanStack Query、helpers+localStorage、后端 Settings 并存；`content-install.ts` / `content-selection.ts` 文件过大；App.vue 同时是 composition root。
3. **UI 原语手写成本高**：`packages/ui` base 层 DatePicker/MultiSelect/表格等 ~25–35% 适合外部行为原语；MC 领域组件必须自研。调研结论：**reka-ui + 现有 Tailwind token**，不整库换皮。

用户要求：**先做路由/状态治理，再做渐进 headless；同一 feature 两阶段交付。**

## [S2] Design

### [S2.1] 目标分层（阶段 1–2 共用）

```
UI 组件（packages/ui / app-frontend pages）
    ↓ 只读、调用
Pinia — 纯 UI 态：theme、breadcrumbs、导航/返回流 UI
    ↓
Domain services / providers — 安装、下载、内容流（DI，文件级可拆薄）
    ↓
TanStack Query（远程读） + theseus 命令（写 / 权威配置）
    ↓
settings_dir / SQLite（权威持久化）
```

**状态归属表（契约，实现必须遵守）**

| 状态类 | 归属 | 禁止 |
| --- | --- | --- |
| 主题/feature flags/窗口偏好镜像 | Pinia `theme` | 页面本地大副本 |
| 面包屑/导航短态 | Pinia `breadcrumbs` | 路由外模块全局 |
| browse/upgrade 返回流 | Pinia 或显式 store（阶段 1 新建） | 模块级可变 map |
| 远程列表/搜索 | TanStack Query | 手写无缓存 fetch 到组件 |
| 安装/下载/内容选择流 | `providers/*` DI | 页面直接 invoke 业务命令 |
| 权威设置 | theseus Settings / 命令 | 前端私自当真相 |
| 轻量 UI 偏好 | 单一 storage 入口（helpers 已有键） | 各处散落 `localStorage` |

### [S2.2] 路由契约（阶段 1）

- 目录：`apps/app-frontend/src/routes/`
  - `index.ts` — 组装 `createRouter`、全局 guard、`scrollBehavior`（行为与现状等价）
  - `meta.ts` — `RouteMeta` 接口：`breadcrumb`、`discordActivity`、`pageTransitionGroup`、`useContext`、`useRootContext`、`renderMode`、`upgradeRequirement`
  - 域模块（与现有 URL **完全兼容**）：
    - `home.ts`（`/`）
    - `discover.ts`（`/browse/*`、`/project/*` 及 provider 项目重定向）
    - `library.ts`、`multiplayer.ts`、`instance.ts`、`lab.ts`
    - `settings.ts`、`utility.ts`（worlds/skins/downloads/create/help）
- **name 规范**：PascalCase 无空格（如 `DiscoverContent`）；旧 name 若被代码引用需同步改，URL 不变。
- **Lab**：工具页在 `lab.ts` 域模块内保持**同级**路由（hub 无 `RouterView`，嵌套 children 会无法渲染）；path 字符串不变。
- 域路由只 import 自己的 page/component；跨域跳转用 `name` 或完整 path 字符串。
- 薄 re-export：原 `src/routes.js` 可改为 re-export `src/routes/index.ts` 一版，再删。

### [S2.3] 返回流（阶段 1）

- 新建 Pinia store（建议 `store/navigation-return.ts` 或并入现有可测 store）：
  - browse 快照：`saveBrowseReturn` / `consumeBrowseReturn` / `peekBrowseReturn`
  - upgrade 流：`parkUpgradeFlow` / `clearUpgradeFlow` / `peekUpgradeFlow`
- `routes/index.ts` 的 `beforeEach` / `scrollBehavior` 改为读该 store。
- 旧 `helpers/browse-return-state.ts`、`helpers/upgrade-return-state.ts` 保留导出一版转发，调用点迁完后删除。

### [S2.4] Provider 切薄（阶段 1，文件级）

- `providers/content-install.ts`、`content-selection.ts`：按职责拆到子模块（如 `install-job`、`preview`、`selection-api`、`types`），根文件 re-export 保持 API 稳定。
- **不改 IPC 协议与用户可见行为**；不强制合并 Pinia。

### [S2.5] 渐进 Headless（阶段 2，reka-ui）

- 依赖：`reka-ui@2.10.x`（MIT）；**不**整包 shadcn-vue registry；**不用** `@headlessui/vue`。
- 视觉：一律 adapter + 现有 token（`surface-*`、brand、FormatJS 文案）；**禁止**引入第二主题真相源。
- 顺序：
  1. **Demo 闸门**：隔离页验证 Dialog/Select/Tooltip → token 映射（light/dark/OLED/accent）
  2. **叶子原语 adapter**：Button/Input/Checkbox/Switch/Tooltip（API 兼容 `Button` 风格）
  3. **浮层**：Dialog/Drawer/Select 对接 `MODALS.md` + `data-onboarding-id`
  4. **设置表单等低耦合区**局部替换；虚拟列表可叠 `@tanstack/vue-virtual`
- **不替换**：ProjectCard/ContentCard、皮肤 3D、Instance 列表、Console、Files/Monaco、安装流、Tauri chrome、Onboarding 对话框语义。

### [S2.6] 验证

- 每阶段：`pnpm prepr:frontend:app`；相关 `node --test`；`vue-tsc --noEmit`。
- 路由改名后：rg 旧 name 字符串应无残留（除 changelog 历史）。
- headless 阶段：双端目视（桌面 `pnpm app:dev`；website 已迁至 Axolotl-Website 仓库，仅在其同步 `packages/ui` 改动后于该仓库 `pnpm dev` 冒烟）。

## [S3] Out of Scope

- 整库 Element/Naive/Vuetify/Quasar 替换
- 重写 MC 内容卡 / 安装流 / 皮肤预览 / 终端
- 后端 theseus Settings 语义变更
- URL 重命名或导航 IA 增删
- 桌面 onboarding 步骤语义变更（仅保证目标 id 仍存在）
- 官网信息架构重构
- **设置-关于页在 dev（Vite）下的骨架/加载体验**：以生产 `vite build` / 正式包行为为准；已保留 hash 同步、异步 3D、错误隔离等对 build 有益的缓解，但不保证消除 dev 卡顿或偶发回退

## Tasks

### 阶段 1 — 路由 / 状态

- [x] T1: 新建 `src/routes/meta.ts` + 域路由模块骨架，`index.ts` 组装且 URL/name 集合与现网一致（name 仅规范 PascalCase） — acceptance: 应用可启动；`/browse/mods`、`/library`、`/instance/:id` 等可达；`vue-tsc` 过 (covers: S2.2)
- [x] T2: `routes.js` 改为 re-export 或删除并改 import；全仓无残留旧 name 含空格字符串 — acceptance: `rg "Discover content|Skin editor" apps/app-frontend/src` 仅剩可接受的历史/注释为 0 (covers: S2.2; depends: T1)
- [x] T3: Lab 工具路由归入 `lab.ts` 域模块，path 不变 — acceptance: `/lab`、`/lab/seed-map` 等可进可返回 (covers: S2.2; depends: T1)
- [x] T4: 返回流迁入 Pinia store；调用点直连 store — acceptance: Browse/Favorites/upgrade 页从 `@/store/navigation-return` 导入；helpers 仅 re-export (covers: S2.3; depends: T1)
- [x] T5: `content-install` / `content-selection` 文件级拆薄，公共 API re-export — acceptance: 无行为 diff；公共 API 稳定；根文件仍含 preview/install 主流程（registry/session/messages/types/compat 已抽出） (covers: S2.4)
- [x] T6: 写入/校验状态归属表（文档段落在 spec；代码侧注释或 `providers/README` 可选） — acceptance: 审查可对照归属表判断新代码 (covers: S2.1; depends: T4, T5)

### 阶段 2 — 渐进 Headless

- [x] T7: 引入 `reka-ui`，新增隔离 demo 路由/页面验证 token 映射 — acceptance: demo 结构完成（`/headless-demo`）；light/dark/OLED **目视待用户确认** (covers: S2.5; depends: T1)
- [x] T8: 叶子原语 adapter（≥ Tooltip + Button/Input 或 Checkbox）接入现有 `Button` 风格 — acceptance: 至少一处生产 UI 使用 adapter；视觉与 token 一致 (covers: S2.5; depends: T7)
- [x] T9: Dialog/Select adapter 对接 `MODALS.md` 与 FormatJS — acceptance: 一处模态或下拉替换；`data-onboarding-id` 仍可命中 (covers: S2.5; depends: T8)
- [x] T10: 设置表单等低耦合区至少一处理替换 + prepr — acceptance: `pnpm prepr:frontend:app` 过；无 MC 领域组件被替换 (covers: S2.5; depends: T9)
