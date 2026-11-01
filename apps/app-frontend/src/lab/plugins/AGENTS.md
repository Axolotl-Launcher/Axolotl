# Lab Plugins

Read the [lab plugin standard](../../../../../standards/frontend/LAB_PLUGINS.md) before changing the plugin host or adding a plugin.

## Boundaries

- `types.ts` owns the public contract. Changing a field there changes what every plugin can rely on; update the standard in the same change.
- `validation.ts` holds the discovery rules and must stay importable without Vite globs, so its tests run in the node test project.
- `registry.ts` is the only file that touches `import.meta.glob`. Keep discovery there so validation stays testable.
- `packages/<id>/` holds plugins. A plugin's folder name must equal `manifest.id`; the route `/lab/p/<id>` depends on it.
- A plugin may not add routes, lab registry entries or hub cards by hand. The host derives all three from the manifest, and a hand-written entry would double up.
- Plugin copy uses message descriptors registered in both `en-US` and `zh-CN`, like every other user-visible string. Do not hard-code English inside a plugin component.

## Verification

`pnpm --filter @modrinth/app-frontend test` covers the validation, state, slot ordering and card mapping unit tests. The user performs visual acceptance for a rendered plugin page.
