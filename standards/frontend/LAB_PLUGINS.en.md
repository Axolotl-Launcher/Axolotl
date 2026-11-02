# Lab Plugins

Chinese version: [LAB_PLUGINS.md](LAB_PLUGINS.md). This file is the English translation and may lag behind it.

How to add a plugin to the desktop app's Lab, and what the launcher guarantees a plugin. Applies to `apps/app-frontend/src/lab/plugins/`.

A plugin is a TypeScript package compiled into the launcher. It is not downloaded at runtime: a plugin folder plus a rebuild is the whole installation story. That keeps plugins inside normal code review and type checking, and it means the launcher can never load code it did not build.

## Folder Layout

```
apps/app-frontend/src/lab/plugins/
  types.ts            contract: manifest, tool, slots, context
  validation.ts       discovery rules, importable without Vite globs
  registry.ts         discovers ./packages/*/index.ts and validates the batch
  state.ts            enabled/disabled and error state, persisted
  slots.ts            resolves slot contributions in render order
  plugin-tools.ts     turns plugins with a tool page into lab hub cards
  context.ts          storage and log handed to a plugin
  use-lab-plugin-text.ts  resolves descriptor-or-string text
  packages/<id>/      the plugins themselves
```

`packages/<id>/index.ts` is the only file the launcher looks for. Its default export must be a `LabPlugin`. The folder name must equal `manifest.id`, because the id is also the route parameter of the plugin's tool page (`/lab/p/<id>`).

Adding a plugin therefore changes nothing else: no registry edit, no route, no lab hub edit, no cover image. The hub lists it, the router reaches it, and the plugin page toggles it, all from the manifest.

## The Contract

```ts
import { defineLabPlugin } from '@/lab/plugins/types'

export default defineLabPlugin({
    manifest: { ... },
    tool: { ... },      // optional
    slots: [ ... ],     // optional
})
```

At least one of `tool` and `slots` is required. A plugin that renders nothing is rejected.

### `manifest`

| Field         | Type                                     | Required | What it does                                                                                            |
| ------------- | ---------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------- |
| `id`          | `string`                                 | yes      | Kebab-case identity. Must equal the folder name. Becomes the route parameter and the storage namespace. |
| `name`        | `LabPluginText`                          | yes      | Card title, page heading, plugin list row, error messages.                                              |
| `description` | `LabPluginText`                          | yes      | One sentence under the name.                                                                            |
| `version`     | `string`                                 | yes      | Semantic version, e.g. `1.0.0`. Shown next to the name. Used only for display.                          |
| `author`      | `string`                                 | no       | Shown after the version.                                                                                |
| `category`    | `'creation' \| 'maintenance' \| 'world'` | no       | Which lab hub filter the card belongs to. Defaults to `creation`.                                       |
| `icon`        | Vue `Component`                          | no       | Fallback card icon when the plugin has no cover image.                                                  |
| `permissions` | `LabPluginPermission[]`                  | no       | Disclosed on the plugin page. See [Permissions](#permissions).                                          |

### `tool`

| Field         | Type            | Required | What it does                                                                                                     |
| ------------- | --------------- | -------- | ---------------------------------------------------------------------------------------------------------------- |
| `title`       | `LabPluginText` | yes      | Card title. Usually the same descriptor as `manifest.name`.                                                      |
| `description` | `LabPluginText` | yes      | Card description.                                                                                                |
| `icon`        | Vue `Component` | no       | Overrides `manifest.icon`.                                                                                       |
| `category`    | category        | no       | Overrides `manifest.category`.                                                                                   |
| `component`   | Vue `Component` | yes      | Body of `/lab/p/<id>`. The host renders it inside the error boundary; it owns everything below the page heading. |

### `slots`

Array of contributions. Each contribution:

| Field       | Type                | Required | What it does                                                     |
| ----------- | ------------------- | -------- | ---------------------------------------------------------------- |
| `slot`      | `LabPluginSlotName` | yes      | Which launcher surface to fill. Unknown names reject the plugin. |
| `component` | Vue `Component`     | yes      | Rendered in the slot. Receives the slot context as props.        |
| `order`     | `number`            | no       | Lower renders first. Ties sort by plugin id, so order is stable. |

## Slots

Slots are the only way a plugin touches UI it does not own. The launcher decides where each slot is and how it is laid out, so a plugin can add a control without breaking the surrounding page.

| Slot               | Where                                               | Props                        |
| ------------------ | --------------------------------------------------- | ---------------------------- |
| `lab.hub.footer`   | Bottom of the lab hub, below the tool list          | none                         |
| `instance.actions` | Instance page action row, after the settings button | `{ instance: GameInstance }` |

A slot component that throws is replaced by a one-line notice and the plugin is turned off. The rest of the row keeps working.

Adding a slot means adding its name to `LAB_PLUGIN_SLOTS`, its props to `LabPluginSlotContexts`, and one `<LabPluginSlot name="..." />` at the host location. Existing plugins are unaffected.

## Context

Plugin components read their context with `useLabPluginContext()`:

```ts
import { useLabPluginContext } from '@/lab/plugins/types'

const { pluginId, storage, log } = useLabPluginContext()
```

| Member                        | What it does                                                                                           |
| ----------------------------- | ------------------------------------------------------------------------------------------------------ |
| `pluginId`                    | The running plugin's manifest id.                                                                      |
| `storage.read(key, fallback)` | Reads a JSON value namespaced to this plugin. Returns `fallback` when the key is absent or unreadable. |
| `storage.write(key, value)`   | Stores a JSON-serialisable value. Swallows quota and availability errors.                              |
| `storage.remove(key)`         | Deletes one key.                                                                                       |
| `log(message, detail?)`       | `console.info` with the plugin id prefixed.                                                            |

Calling `useLabPluginContext()` outside a plugin render throws, because a plugin without an id and storage cannot behave correctly.

## Discovery and Validation

`registry.ts` globs `./packages/*/index.ts` eagerly at build time. `validation.ts` then checks each module, and rejects the folder with a reason instead of failing the build:

- default export missing or not an object
- `manifest` missing, or `id` missing / not kebab-case / different from the folder
- `name` or `description` missing or empty
- `version` not a semantic version
- `tool` present but missing `title`, `description` or `component`
- `slots` present but holding an unknown slot name or a non-numeric `order`
- neither `tool` nor `slots`
- duplicate id (first folder wins)

Rejections are listed on `/lab/plugins` under "Skipped folders". They never break the launcher, so a half-written plugin is visible instead of silent.

## Enable, Disable and Failures

`state.ts` stores two lists under `axolotl-lab-plugin-state`: `disabled` ids and `failures`. A plugin is enabled unless it is in `disabled`.

- Toggling a plugin off hides its card and its slot contributions immediately.
- A plugin that throws is disabled automatically and its message is stored.
- Turning it back on clears the stored failure.
- Unreadable storage falls back to "everything enabled", so a corrupted entry cannot hide every plugin.

## Permissions

`manifest.permissions` is a disclosure list shown on the plugin page:

| Permission       | Meaning                                     |
| ---------------- | ------------------------------------------- |
| `instances:read` | Reads the instance list or instance details |
| `files:read`     | Reads files the user selects                |
| `files:write`    | Writes files the user selects               |
| `network`        | Makes network requests                      |
| `clipboard`      | Reads or writes the clipboard               |

**Permissions are declared, not enforced.** A plugin runs un-sandboxed in the launcher process, so the list tells the user what to expect; it does not stop anything. Review is the actual control, which is why plugins are compiled in rather than installed by users.

## Text and Localization

`LabPluginText` is either a plain string or a message descriptor from `defineMessage`. Plugins shipped in this repository pass descriptors and add their ids to `apps/app-frontend/src/locales/en-US/index.json` and `zh-CN/index.json`, so the copy is translated like the rest of the app. Third-party plugins pass plain strings and are shown as written.

## Adding a Plugin, Step by Step

1. Pick an id. Kebab-case, lowercase, no spaces: `schematic-batch-rename`. The folder will be named exactly this and the route becomes `/lab/p/schematic-batch-rename`, so changing the id later breaks links and stored settings.
2. Scaffold it:

   ```powershell
   pnpm axolotl:new-lab-plugin schematic-batch-rename "Batch rename schematics"
   ```

   This writes `packages/schematic-batch-rename/index.ts` and a starter tool component, and prints the locale ids it used.

3. Decide how the plugin appears. A tool page (`tool`) gets a card in the lab hub and a page at `/lab/p/<id>`. A slot (`slots`) instead puts a component into an existing surface such as the instance action row. A plugin may do both; it must do at least one.
4. Describe it in the manifest. `name`, `description` and `version` are required; `category` decides which hub filter it lands under; `permissions` is what the plugin tells the user it touches.
5. Build the UI out of the shared primitives (`Button`, `Card`, `StyledInput`, `TagItem`, `EmptyState`, `Toggle`, icons from `@modrinth/assets`). Reach for a raw `div` with Tailwind utilities only when no primitive fits.
6. Localize everything the user sees. Add every `defaultMessage` pair to both `apps/app-frontend/src/locales/en-US/index.json` and `zh-CN/index.json`, keyed `app.lab.plugins.<id>.<name>`. `pnpm axolotl:i18n-check` fails if a key is missing, empty, identical in both languages, or has a broken ICU argument.
7. Run the gates: `pnpm --filter @modrinth/app-frontend lint`, `pnpm --filter @modrinth/app-frontend exec vitest run --project unit src/lab/plugins`, `pnpm axolotl:check-agents`, `pnpm axolotl:i18n-check`.
8. Open the launcher with `pnpm app:dev`, confirm the card appears, open the page, and toggle the plugin off and on once so the disabled path is exercised too.

Do not add the plugin to `src/lab/registry.ts`, to `routes/lab.ts` or to the hub page. The host derives all three from the manifest, and a hand-written entry duplicates the card.

## Pitfalls

These are the mistakes that actually happen, in rough order of frequency.

- **Folder name and `manifest.id` drift apart.** The folder is the identity. A mismatch rejects the plugin with `manifest.id "x" does not match folder "y"` and the plugin silently never appears.
- **Locale keys are forgotten.** The plugin renders fine in English and shows raw key names in Chinese, or `i18n-check` fails the branch. Add both files in the same edit as the component.
- **A plugin that renders nothing.** Neither `tool` nor `slots` means the plugin is rejected on purpose, because it would occupy a folder and do nothing.
- **Writing into shared files.** Editing `types.ts` is fine when the contract genuinely needs a new field, but editing `registry.ts`, `routes/lab.ts` or `Lab.vue` to make one plugin work usually means the plugin should use an existing slot instead.
- **Custom styles.** A plugin with its own CSS breaks in the other theme. Use the tokens (`var(--color-text-primary)`, `bg-surface-2`, `border-surface-5`) so light and dark both work.
- **Assuming permissions are enforced.** They are labels for the user. A plugin runs with the launcher's full privileges; the review of the diff is the actual control.
- **Long work on mount.** The tool page mounts on navigation. Starting an expensive job in `onMounted` makes the whole lab feel slow; put it behind an explicit button.
- **Storing big data.** `storage` is `localStorage`, a few megabytes shared with everything else. Keep ids, paths and small preferences there, not file contents.

## Working with Agents

Agents get a dedicated skill: [`.agents/skills/lab-plugin/SKILL.md`](../../.agents/skills/lab-plugin/SKILL.md). It carries the steps, the gates and an explicit do-not list, and it is deliberately short so it costs little context.

Point an agent at that skill plus `src/lab/plugins/types.ts` and the work usually lands in one pass. Two things still need a human: the gate output, because an agent that cannot paste command and exit code has not run them, and the screen, because a plugin can pass every check and still be laid out wrong.

## Reference Plugins

- `packages/motd-preview/` — a tool page with plugin-scoped storage, no permissions.
- `packages/copy-instance-id/` — a slot-only plugin that adds one button to the instance action row.

## Limits

- No lifecycle hooks. A plugin exists only while its tool page or slot is mounted.
- No runtime install. Users cannot add a plugin without rebuilding the launcher.
- No background work. Long jobs must live inside the component that started them.
- Styles come from the shared Tailwind preset; a plugin may not ship its own stylesheet.
- Permissions are disclosure only. Nothing in the launcher blocks a plugin from calling a Tauri command the app's capabilities already allow, which is why plugin code is reviewed like any other code.
