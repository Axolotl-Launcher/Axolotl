---
name: lab-plugin
description: 在 Axolotl 实验室里新增、修改或评审插件（Lab plugin）。涉及 lab/plugins 目录、工具卡片、插槽贡献、插件文案时使用。
---

契约见 [LAB_PLUGINS.md](../../../standards/frontend/LAB_PLUGINS.md)，类型以 `apps/app-frontend/src/lab/plugins/types.ts` 为准。两者不一致时以类型为准，并报告文档过期。

## 铁律

1. 文件夹名 = `manifest.id` = 路由参数。不可改，改了就换了个插件。
2. 只动 `packages/<id>/` 和两个 locale 文件。**禁止**改 `registry.ts`、`routes/lab.ts`、`Lab.vue`；需要动它们说明该用插槽。
3. `tool` 与 `slots` 至少一个，否则插件被拒。
4. 文案与组件同一次改完，中英两个 locale 都要加。
5. 不自带样式表，用现有颜色变量与组件。

## 步骤

1. `pnpm axolotl:new-lab-plugin <id> "<中文名>"` 生成骨架。
2. 写 `index.ts` 的 manifest，再写组件；界面用 `Button`/`Card`/`StyledInput`/`TagItem`/`EmptyState`/`Toggle`。
3. 补 `app.lab.plugins.<id>.*` 的中英词条。
4. 跑门禁（见下）。
5. 报告改动的文件、门禁输出、以及**没验证的部分**。

## 门禁

```bash
pnpm --filter @modrinth/app-frontend lint
pnpm --filter @modrinth/app-frontend exec vitest run --project unit src/lab/plugins
pnpm axolotl:i18n-check
```

贴真实命令与退出码。没跑就说没跑，不要写"应该没问题"。

## 别做

- 不要发明 manifest 字段、权限值或生命周期钩子——没有这些。
- 不要把用户可见英文写死在组件里。
- 不要声称界面已验证：智能体看不见屏幕，界面必须由人确认。
- 不要在 `onMounted` 里起重任务。
