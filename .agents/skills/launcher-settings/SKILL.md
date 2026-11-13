---
name: launcher-settings
description: Use when adding, changing, removing or versioning a launcher setting, when editing the settings documents (default / row / deployment / local layers), or when a `settings` column or `DOMAINS` entry has to change. Activates for "改默认值", "新增设置", "删除设置", "schema_version", "settings.json", "部署配置", "settings 列". Not for reading a setting at runtime (that is ordinary business code calling `Settings::get`).
---

# Launcher Settings

A setting is composed per document domain, and per key inside that domain:

```
build default  →  database row  →  deployment documents  →  local documents
```

Each layer answers only what the ones below it do not. The row contributes a domain only while that domain has no local document, so a domain answered by a document is answered by it alone — a key the document leaves out reads as the build default, not as the row. A deployment's value sits above the row and below the local document: it is what the launcher shows until the user changes that key, and the user's own change wins from then on.

## The four layers

- **Build default**: `impl Default for Settings` in `packages/app-lib/src/state/settings.rs`. It must equal, field for field, the row a fresh database produces after replaying every migration; `default_matches_the_row_a_fresh_database_creates` asserts exactly that, so changing one side alone fails the test.
- **Row**: the `settings` table. `version` (which settings migrations still have to run) and `personalized_ads` (a `#[serde(skip)]` field) are answered by the row alone and never appear in a document.
- **Deployment documents**: read from `axolotl-launcher` under the platform configuration directory on Linux, or from the directory `AXOLOTL_SETTINGS_CONFIG_DIR` names anywhere; an empty value means no deployment. The launcher only reads this directory. A document is `<domain>.json` holding `{"schema_version": N, "data": { … }}`, and a file this build cannot read is left untouched rather than overwritten.
- **Local documents**: `<data directory>/settings/<domain>.json`, written by the launcher. A document holds only the keys that differ from the build default and from what a deployment answers, so a key absent from it is one the lower layers still answer.

`DOMAINS` in `packages/app-lib/src/state/documents/settings.rs` is the single table of which domain carries which keys and which version of that shape this build reads. A key missing from `DOMAINS` fails `the_domains_carry_every_key_the_settings_serialize`.

## Adding, changing and removing a setting

A new setting needs all three: the field on `Settings`, the column in a new forward migration, and the key in the `DOMAINS` entry of the domain that owns it. Removing one needs the mirror image — but the column stays.

**Never drop the column.** Historical migrations keep referring to it: `20260802121000` writes `max_concurrent_writes`, so replaying it after a `DROP COLUMN` fails, and the downgrade script has no mapping that can add a column back. Removing a setting means deleting the field, the `DOMAINS` key, the ranges and whitelists that name it, the frontend type, the settings page row, the search index entry and the locale strings; the column remains, unused, until a future rebuild of the table handles it deliberately.

A new setting is not finished at the backend. Check each place a setting can be named, and decide explicitly rather than by omission: the `AppSettings` type in `apps/app-frontend/src/helpers/settings.ts`, the demo backend mock, the deep-link key lists (the handler in `packages/app-lib/src/api/handler.rs`, the deep-link abilities, and the tools preference whitelist), the settings page row, `settings-search-index.ts`, and every locale file. A setting left out of one of these is reachable from some entry points and not from others.

Migrations follow the repository's existing naming: a 14-digit timestamp prefix, an underscore, and a kebab-case description, with a version greater than every migration already present.

## Changing a default value

Change `impl Default for Settings` **and** write a forward migration that brings existing rows to the same value. Never change a column default only in one place: the replay test compares the two, and a local database that already applied the old migration will never see a changed `CREATE TABLE`.

Understand what such a migration rewrites. A local document stores only the keys that differ from the default, so a row value equal to the old default cannot be told apart from a value the user chose and that happened to equal it. Bringing rows from the old default to the new one therefore moves every user who never wrote an override, and leaves anyone who did. That is the intended behaviour — say so in the commit message — rather than a bug to work around by leaving rows alone.

## Changing a document's `schema_version`

Bump the version of a domain only when the set of keys that domain carries changes — a key added, removed or renamed. Changing a default value, a range, or any other behaviour does not change the shape and does not bump it. The number is the `DOMAINS` entry's third field.

The version records the shape of the last write, so:

- reading, handing over rows into documents, and pruning never rewrite it;
- a write refuses to lower a document whose version is higher than the one this build knows, because that drops keys it cannot see. Only an operation the user explicitly starts for this purpose (`sanitise`, `import`) may replace such a document.

## Writing settings

Business code never reads or writes the `settings` row directly; it calls `Settings::get`, which composes the layers and caches the result. A write goes to the row first and then, after the unit commits, `store_cut_over` states the affected domains in their local documents — only the domains carrying the keys the write named, and never a key a deployment answers unless that write named it. A write that changes nothing leaves no file behind, and a domain with nothing to record keeps being answered by the row.

Startup hands the rows over to local documents once, skipping every key a deployment answers; that is what stops a deployment's value from being frozen behind a copy of an older row value.

## Verifying a settings change

- `cargo test -p theseus --lib settings::` — the build default against a replayed database, ranges, and the write path.
- `cargo test -p theseus --lib documents::` — the layers, `DOMAINS`, handover, and the document version rules.
- `node scripts/axolotl/downgrade-app-db.test.mjs` — mandatory whenever a migration or its `REVERTIBLE_SCHEMA` entry changes; it replays the upgrade, the downgrade and the re-upgrade.
- `cargo fmt --all --check` and `cargo check -p theseus --tests` for the crate itself.
