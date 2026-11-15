---
name: launcher-settings
description: Use when adding, changing, removing or versioning a launcher setting, when editing the settings documents (default / row / deployment / local layers), or when a `settings` column or `DOMAINS` entry has to change. Activates for "改默认值", "新增设置", "删除设置", "schema_version", "settings.json", "部署配置", "settings 列". Not for reading a setting at runtime (that is ordinary business code calling `Settings::get`).
---

# Launcher Settings

A setting is composed per document domain, and per key inside that domain:

```
build default  →  compatibility projection  →  deployment documents  →  local documents
```

Each layer answers only what the ones below it do not. The compatibility row contributes a domain only while that domain has no local document. A deployment's value sits above the row and below the local document: it is what the launcher shows until the user changes that key, and the user's own change wins from then on.

The active database system keeps configuration in JSON. Its configuration tables are volatile projections shared by the pool's connections, rebuilt at startup; they are not persistent business tables. Historical app.db rows are used only during the one-time import. Read [the storage contract](../../../CONTRIBUTING.md#应用数据目录与数据库迁移) before changing that boundary.

## The four layers

- **Build default**: `impl Default for Settings` in `packages/app-lib/src/state/settings.rs`. Keep the fresh volatile projection consistent with it. Historical migration replay tests describe the legacy importer and must retain explicit historical expectations when current defaults change.
- **Compatibility projection**: `configuration.settings` in shared memory. `version` tracks historical settings transformations during import; it is not an application release gate. `personalized_ads`, a `#[serde(skip)]` field, is persisted separately in `settings/internal.json` through `state/store/configuration.rs`.
- **Deployment documents**: read from `axolotl-launcher` under the platform configuration directory on Linux, or from the directory `AXOLOTL_SETTINGS_CONFIG_DIR` names anywhere; an empty value means no deployment. The launcher only reads this directory. A document is `<domain>.json` holding `{"schema_version": N, "data": { … }}`, and a file this build cannot read is left untouched rather than overwritten.
- **Local documents**: `<data directory>/settings/<domain>.json`, written by the launcher. A document holds only the keys that differ from the build default and from what a deployment answers, so a key absent from it is one the lower layers still answer.

`DOMAINS` in `packages/app-lib/src/state/documents/settings.rs` is the single table of which domain carries which keys and which version of that shape this build reads. A key missing from `DOMAINS` fails `the_domains_carry_every_key_the_settings_serialize`.

## Adding, changing and removing a setting

A new setting needs its field on `Settings`, its document domain and key, and any mapping required by the volatile compatibility projection. Do not add a persistent database column or a release migration solely to store configuration. If a compatibility column is needed, change the volatile `configuration` definition and its row mapping; durable definitions in `database_baseline.json` remain stable. Removing a setting removes its runtime and frontend references while preserving document keys an older build does not understand.

**Never rewrite historical migrations or drop their legacy columns.** Import still replays them on a temporary snapshot. Removing a setting means deleting the field, the `DOMAINS` key, the ranges and whitelists that name it, the frontend type, the settings page row, the search index entry and the locale strings. A migration genuinely needed by the legacy importer must be forward-only and include its downgrade mapping as required by the app-lib instructions.

A new setting is not finished at the backend. Check each place a setting can be named, and decide explicitly rather than by omission: the `AppSettings` type in `apps/app-frontend/src/helpers/settings.ts`, the demo backend mock, the deep-link key lists (the handler in `packages/app-lib/src/api/handler.rs`, the deep-link abilities, and the tools preference whitelist), the settings page row, `settings-search-index.ts`, and every locale file. A setting left out of one of these is reachable from some entry points and not from others.

## Changing a default value

Change `impl Default for Settings` and the fresh volatile projection together. Do not rewrite explicit local or deployment overrides to change a default, and do not use a business database migration for this purpose.

A local document stores only overrides, so a missing key follows the new build default while a stored value keeps the user's choice. Test both cases after reopening the database pool; the projection is recreated on each start.

## Changing a document's `schema_version`

Bump the version of a domain only when the set of keys that domain carries changes — a key added, removed or renamed. Changing a default value, a range, or any other behaviour does not change the shape and does not bump it. The number is the `DOMAINS` entry's third field.

The version records the shape of the last write, so:

- reading, handing over rows into documents, and pruning never rewrite it;
- a write refuses to lower a document whose version is higher than the one this build knows, because that drops keys it cannot see. Only an operation the user explicitly starts for this purpose (`sanitise`, `import`) may replace such a document.

## Writing settings

Business code reads through `Settings::get` and writes through the existing atomic store API. On the active database system, `configuration::commit_settings` syncs staged JSON before committing its durable token; startup finishes publication if the process stops between SQL commit and publication. Do not replace this with an ordinary transaction commit followed by a document write: the volatile row cannot recover that write after restart. The legacy database path retains `store_cut_over` for historical callers.

Only affected domains are stated, and a deployment's key is written locally only when the write names it. Preserve unknown keys; refuse a targeted write to a newer or unreadable settings document, while whole-row startup writes leave that domain untouched. First import hands legacy rows over once, skipping deployment-owned keys. Later starts recover committed stages and hydrate the projection from JSON.

## Verifying a settings change

- `cargo test -p theseus --lib settings::` — defaults, historical importer expectations, ranges, and the write path.
- `cargo test -p theseus --lib documents::` — the layers, `DOMAINS`, handover, and the document version rules.
- `cargo test -p theseus --lib state::store::configuration::` — restart hydration, staged commit recovery, and preservation of unknown data.
- `node scripts/axolotl/downgrade-app-db.test.mjs` — mandatory whenever a migration or its `REVERTIBLE_SCHEMA` entry changes; it replays the upgrade, the downgrade and the re-upgrade.
- `cargo fmt --all --check` and `cargo check -p theseus --tests` for the crate itself.
