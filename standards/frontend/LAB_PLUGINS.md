# 实验室插件

英文版：[LAB_PLUGINS.en.md](LAB_PLUGINS.en.md)。适用范围：`apps/app-frontend/src/lab/plugins/`。

插件是一段被编译进启动器的 TypeScript 代码，不是运行时下载的。装插件的全部含义就是"放一个文件夹，重新构建"——这换来两件事：插件代码走正常的类型检查和评审，启动器也永远加载不了自己没有构建过的代码。

## 目录

```
apps/app-frontend/src/lab/plugins/
  types.ts                契约：manifest、tool、slots、context
  validation.ts           校验规则，可脱离 Vite 单独引用
  registry.ts             发现 ./packages/*/index.ts 并整批校验
  state.ts                启用/停用与故障状态，持久化
  slots.ts                按渲染顺序汇总插槽贡献
  plugin-tools.ts         把有工具页的插件转成实验室卡片
  context.ts              交给插件的存储与日志
  use-lab-plugin-text.ts  解析文案（描述符或纯字符串）
  packages/<id>/          插件本体
```

`packages/<id>/index.ts` 是启动器唯一会找的文件，默认导出必须是 `LabPlugin`。文件夹名必须等于 `manifest.id`，因为 id 同时是插件页面的路由参数（`/lab/p/<id>`）。

所以加插件不需要动任何其他地方：不进注册表、不加路由、不改实验室页面、不配封面图。卡片、路由、开关全部从 manifest 推出来。

## 契约

```ts
import { defineLabPlugin } from '@/lab/plugins/types'

export default defineLabPlugin({
    manifest: { ... },
    tool: { ... },   // 可选
    slots: [ ... ],  // 可选
})
```

`tool` 和 `slots` 至少要有一个。什么都不渲染的插件会被拒绝。

### `manifest`

| 字段          | 类型                                     | 必填 | 作用                                                     |
| ------------- | ---------------------------------------- | ---- | -------------------------------------------------------- |
| `id`          | `string`                                 | 是   | 短横线小写标识，必须等于文件夹名，用作路由参数和存储分区 |
| `name`        | `LabPluginText`                          | 是   | 卡片标题、页面标题、插件列表行、错误提示                 |
| `description` | `LabPluginText`                          | 是   | 名字下面那句话                                           |
| `version`     | `string`                                 | 是   | 语义化版本，如 `1.0.0`，只用于显示                       |
| `author`      | `string`                                 | 否   | 显示在版本号后面                                         |
| `category`    | `'creation' \| 'maintenance' \| 'world'` | 否   | 实验室分类筛选，默认 `creation`                          |
| `icon`        | Vue 组件                                 | 否   | 没有封面图时的卡片图标                                   |
| `permissions` | `LabPluginPermission[]`                  | 否   | 在插件页展示给用户，见「权限」                           |

### `tool`

| 字段          | 类型            | 必填 | 作用                                                                 |
| ------------- | --------------- | ---- | -------------------------------------------------------------------- |
| `title`       | `LabPluginText` | 是   | 卡片标题，通常与 `manifest.name` 是同一个描述符                      |
| `description` | `LabPluginText` | 是   | 卡片说明                                                             |
| `icon`        | Vue 组件        | 否   | 覆盖 `manifest.icon`                                                 |
| `category`    | 同上            | 否   | 覆盖 `manifest.category`                                             |
| `component`   | Vue 组件        | 是   | `/lab/p/<id>` 的正文，宿主用错误边界包住它；页面标题以下的空间全归它 |

### `slots`

数组，每一项：

| 字段        | 类型                | 必填 | 作用                                           |
| ----------- | ------------------- | ---- | ---------------------------------------------- |
| `slot`      | `LabPluginSlotName` | 是   | 填哪个位置，未知名字会拒绝整个插件             |
| `component` | Vue 组件            | 是   | 渲染在该位置，以 props 接收上下文              |
| `order`     | `number`            | 否   | 越小越靠前，相同则按插件 id 排序，保证顺序稳定 |

## 插槽

插槽是插件触碰"不属于自己的界面"的唯一方式。位置和排版由启动器决定，插件只负责往里面放一个组件，不会把周围页面搞乱。

| 插槽               | 位置                         | Props                        |
| ------------------ | ---------------------------- | ---------------------------- |
| `lab.hub.footer`   | 实验室页面底部，工具列表下方 | 无                           |
| `instance.actions` | 实例页操作栏，设置按钮之后   | `{ instance: GameInstance }` |

插槽组件抛错时会被换成一行提示，插件随即停用，操作栏其余部分照常工作。

新增一个插槽要动三处：`LAB_PLUGIN_SLOTS` 加名字、`LabPluginSlotContexts` 加 props 类型、宿主位置插一行 `<LabPluginSlot name="..." />`。已有插件不受影响。

## 上下文

插件组件用 `useLabPluginContext()` 拿自己的上下文：

```ts
import { useLabPluginContext } from '@/lab/plugins/types'

const { pluginId, storage, log } = useLabPluginContext()
```

| 成员                          | 作用                                                     |
| ----------------------------- | -------------------------------------------------------- |
| `pluginId`                    | 当前插件的 manifest id                                   |
| `storage.read(key, fallback)` | 读本插件命名空间下的 JSON，缺失或读不出时返回 `fallback` |
| `storage.write(key, value)`   | 写入可序列化的值，容量或可用性问题会被吞掉               |
| `storage.remove(key)`         | 删掉一个键                                               |
| `log(message, detail?)`       | 带插件 id 前缀的 `console.info`                          |

在插件之外调用会抛错——没有 id 和存储的插件不可能行为正确。

## 发现与校验

`registry.ts` 在构建期用 glob 一次性拿到 `./packages/*/index.ts`，`validation.ts` 逐个检查，不合格就**拒绝这个文件夹并记下原因**，而不是让构建失败。

会被拒绝的情况：

- 没有默认导出，或默认导出不是对象
- `manifest` 缺失，或 `id` 缺失 / 不是短横线小写 / 与文件夹名不一致
- `name` 或 `description` 缺失或为空
- `version` 不是语义化版本
- 有 `tool` 但缺 `title`、`description` 或 `component`
- 有 `slots` 但插槽名未知，或 `order` 不是数字
- `tool` 和 `slots` 都没有
- id 重复（先发现的文件夹生效）

拒绝记录显示在 `/lab/plugins` 的「被跳过的文件夹」里。它们永远不会弄挂启动器，所以写了一半的插件是可见的，而不是静默消失。

## 启用、停用与故障

`state.ts` 在 `axolotl-lab-plugin-state` 下存两份数据：停用列表 `disabled` 和故障记录 `failures`。不在停用列表里就是启用。

- 关掉插件，卡片和它的插槽贡献立刻消失
- 插件抛错会被自动停用，异常信息被记下来
- 重新打开会清掉那条故障记录
- 存储读不出来时按"全部启用"处理，避免一条坏数据把插件全藏起来

## 权限

`manifest.permissions` 是一份**告知清单**，显示在插件页：

| 权限             | 含义                   |
| ---------------- | ---------------------- |
| `instances:read` | 读取实例列表或实例详情 |
| `files:read`     | 读取用户选择的文件     |
| `files:write`    | 写入用户选择的文件     |
| `network`        | 发起网络请求           |
| `clipboard`      | 读写剪贴板             |

**权限是声明，不是强制。** 插件运行在启动器进程里，没有沙箱；这份清单告诉用户"它打算碰什么"，拦不住任何东西。真正的控制手段是代码评审——这也是插件必须编译进启动器、而不是由用户安装的原因。

## 文案

`LabPluginText` 可以是纯字符串，也可以是 `defineMessage` 的描述符。仓库内的插件一律用描述符，并把 id 加进 `apps/app-frontend/src/locales/en-US/index.json` 和 `zh-CN/index.json`，这样文案和其他界面一样有翻译。第三方插件传纯字符串，界面照原样显示。

`pnpm axolotl:i18n-check` 会检查：键是否两边都有、是否为空、中英是否完全相同、ICU 参数是否匹配。

## 新增插件：八步

1. **定 id。** 小写短横线，如 `schematic-batch-rename`。它是文件夹名、路由参数和存储分区，改一次等于换了个插件。
2. **生成骨架。** 在仓库根目录跑 `pnpm axolotl:new-lab-plugin schematic-batch-rename "批量重命名投影"`，会写出 `packages/<id>/index.ts` 和起始组件，并打印需要补的文案 id。
3. **决定怎么露脸。** 有 `tool` 就多一张卡片和一个页面；有 `slots` 就往现成位置塞组件；两个都要也行，但不能都没有。
4. **写 manifest。** `name`、`description`、`version` 必填，`category` 决定归到哪个筛选，`permissions` 是你告诉用户它会碰什么。
5. **用现成组件搭界面。** `Button`、`Card`、`StyledInput`、`TagItem`、`EmptyState`、`Toggle`，图标从 `@modrinth/assets` 取。现成组件都不合适时再用原生标签加工具类。
6. **补文案。** 每个 `defaultMessage` 在中英两个 locale 里都要有，键名 `app.lab.plugins.<id>.<name>`。和组件同一次改完，别留到后面。
7. **跑门禁。** `pnpm --filter @modrinth/app-frontend lint`、`pnpm --filter @modrinth/app-frontend exec vitest run --project unit src/lab/plugins`、`pnpm axolotl:check-agents`、`pnpm axolotl:i18n-check`。
8. **打开看。** `pnpm app:dev`，确认卡片在、页面能进，然后把开关关一次再开一次，把停用路径也走一遍。

不要手动往 `registry.ts`、`routes/lab.ts` 或 `Lab.vue` 里添加条目——手写的条目会和自动发现重复。

## 常见错误

按实际发生频率排：

- **文件夹名和 `manifest.id` 对不上。** 文件夹才是身份，不一致会被拒（`manifest.id "x" does not match folder "y"`），表现为插件没出现。
- **忘了加文案。** 英文正常、中文显示成一串键名，或者 `i18n-check` 直接红。
- **什么都不渲染。** 既没有 `tool` 也没有 `slots`，故意拒绝——否则它只是占着文件夹。
- **去改共享文件。** 契约确实需要新字段时改 `types.ts` 没问题；为了一个插件去改 `registry.ts`、`routes/lab.ts`、`Lab.vue`，通常说明它该用插槽。
- **自己写样式表。** 会在另一套主题下崩。用 `var(--color-text-primary)`、`bg-surface-2`、`border-surface-5` 这类变量。
- **以为权限会拦。** 不会，见「权限」。
- **在 `onMounted` 里干重活。** 工具页一打开就卡住整个实验室。放到按钮后面。
- **把大块数据塞进存储。** `storage` 是 `localStorage`，几 MB 还和其他部分共用。只放 id、路径、小配置。

## 交给智能体

给智能体派活时，把这三样说清楚就够了：插件要做什么（用户视角）、在哪儿出现（工具页 / 哪个插槽）、要不要持久化什么。

那边有一份专门的技能：[`.agents/skills/lab-plugin/SKILL.md`](../../.agents/skills/lab-plugin/SKILL.md)，写了步骤、验收标准和几个明确的"不要做"。派活时直接指向它，比贴一大段说明省事。

## 示例插件

- `packages/motd-preview/` —— 有工具页，用了插件自己的存储，不声明权限。
- `packages/copy-instance-id/` —— 只有插槽，往实例操作栏加一个按钮。

## 边界

- **没有生命周期钩子。** 插件只在它的工具页或插槽挂载期间存在。
- **不能运行时安装。** 用户不重新构建就加不了插件。
- **没有后台任务。** 长任务只能活在发起它的组件里。
- **不能自带样式表。** 样式来自共享的 Tailwind 预设。
- **权限只是告知。** 启动器不阻止插件调用 capabilities 已放行的 Tauri 命令，所以插件代码和普通代码一样要评审。
