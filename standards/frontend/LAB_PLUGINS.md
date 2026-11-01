# Lab Plugins

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

| Field | Type | Required | What it does |
| --- | --- | --- | --- |
| `id` | `string` | yes | Kebab-case identity. Must equal the folder name. Becomes the route parameter and the storage namespace. |
| `name` | `LabPluginText` | yes | Card title, page heading, plugin list row, error messages. |
| `description` | `LabPluginText` | yes | One sentence under the name. |
| `version` | `string` | yes | Semantic version, e.g. `1.0.0`. Shown next to the name. Used only for display. |
| `author` | `string` | no | Shown after the version. |
| `category` | `'creation' \| 'maintenance' \| 'world'` | no | Which lab hub filter the card belongs to. Defaults to `creation`. |
| `icon` | Vue `Component` | no | Fallback card icon when the plugin has no cover image. |
| `permissions` | `LabPluginPermission[]` | no | Disclosed on the plugin page. See [Permissions](#permissions). |

### `tool`

| Field | Type | Required | What it does |
| --- | --- | --- | --- |
| `title` | `LabPluginText` | yes | Card title. Usually the same descriptor as `manifest.name`. |
| `description` | `LabPluginText` | yes | Card description. |
| `icon` | Vue `Component` | no | Overrides `manifest.icon`. |
| `category` | category | no | Overrides `manifest.category`. |
| `component` | Vue `Component` | yes | Body of `/lab/p/<id>`. The host renders it inside the error boundary; it owns everything below the page heading. |

### `slots`

Array of contributions. Each contribution:

| Field | Type | Required | What it does |
| --- | --- | --- | --- |
| `slot` | `LabPluginSlotName` | yes | Which launcher surface to fill. Unknown names reject the plugin. |
| `component` | Vue `Component` | yes | Rendered in the slot. Receives the slot context as props. |
| `order` | `number` | no | Lower renders first. Ties sort by plugin id, so order is stable. |

## Slots

Slots are the only way a plugin touches UI it does not own. The launcher decides where each slot is and how it is laid out, so a plugin can add a control without breaking the surrounding page.

| Slot | Where | Props |
| --- | --- | --- |
| `lab.hub.footer` | Bottom of the lab hub, below the tool list | none |
| `instance.actions` | Instance page action row, after the settings button | `{ instance: GameInstance }` |

A slot component that throws is replaced by a one-line notice and the plugin is turned off. The rest of the row keeps working.

Adding a slot means adding its name to `LAB_PLUGIN_SLOTS`, its props to `LabPluginSlotContexts`, and one `<LabPluginSlot name="..." />` at the host location. Existing plugins are unaffected.

## Context

Plugin components read their context with `useLabPluginContext()`:

```ts
import { useLabPluginContext } from '@/lab/plugins/types'

const { pluginId, storage, log } = useLabPluginContext()
```

| Member | What it does |
| --- | --- |
| `pluginId` | The running plugin's manifest id. |
| `storage.read(key, fallback)` | Reads a JSON value namespaced to this plugin. Returns `fallback` when the key is absent or unreadable. |
| `storage.write(key, value)` | Stores a JSON-serialisable value. Swallows quota and availability errors. |
| `storage.remove(key)` | Deletes one key. |
| `log(message, detail?)` | `console.info` with the plugin id prefixed. |

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

| Permission | Meaning |
| --- | --- |
| `instances:read` | Reads the instance list or instance details |
| `files:read` | Reads files the user selects |
| `files:write` | Writes files the user selects |
| `network` | Makes network requests |
| `clipboard` | Reads or writes the clipboard |

**Permissions are declared, not enforced.** A plugin runs un-sandboxed in the launcher process, so the list tells the user what to expect; it does not stop anything. Review is the actual control, which is why plugins are compiled in rather than installed by users.

## Text and Localization

`LabPluginText` is either a plain string or a message descriptor from `defineMessage`. Plugins shipped in this repository pass descriptors and add their ids to `apps/app-frontend/src/locales/en-US/index.json` and `zh-CN/index.json`, so the copy is translated like the rest of the app. Third-party plugins pass plain strings and are shown as written.

## Adding a Plugin

```powershell
node scripts/axolotl/new-lab-plugin.mjs my-plugin "My plugin"
```

The script writes `packages/my-plugin/index.ts` and a starter component, then prints the locale ids to add. To add one by hand, copy `packages/motd-preview/`, rename the folder, and change `manifest.id` to match.

## Reference Plugins

- `packages/motd-preview/` — a tool page with plugin-scoped storage, no permissions.
- `packages/copy-instance-id/` — a slot-only plugin that adds one button to the instance action row.

## Limits

- No lifecycle hooks. A plugin exists only while its tool page or slot is mounted.
- No runtime install. Users cannot add a plugin without rebuilding the launcher.
- No background work. Long jobs must live inside the component that started them.
- Styles come from the shared Tailwind preset; a plugin may not ship its own stylesheet.
