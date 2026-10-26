# Desktop Application Library

This Rust crate owns desktop runtime behavior, persistence, and database migrations. Read [CONTRIBUTING.md](../../CONTRIBUTING.md#应用数据目录与数据库迁移) before starting the application or operating on an app database. Development builds share the installed app's data directory unless explicitly isolated with `THESEUS_CONFIG_DIR` or `AXOLOTL_DATA_DIR_SUFFIX`.

For behavior exposed to the frontend through a new or changed native command, follow the shell's [Tauri command completion rules](../../apps/app/AGENTS.md#tauri-command-completion). For build locks or occupied outputs, follow the root [shared Rust build resource rules](../../AGENTS.md#shared-rust-build-resources); do not allocate another target directory.

## Data Access Interface

Every record this crate keeps between runs is read and written through `packages/app-lib/src/state/store/`. **Do not reach the database on your own**: no business module may use `sqlx`, open a transaction, or query through `State::pool`. Which storage answers, and how it is asked, is the store package's business alone, so that a storage can be replaced without touching a caller.

A caller names a collection and the fields it looks a record up by, through the handle in `State::data`. Four verbs exist:

- `set(handle, key, Some(record))` writes the record under the key; passing `None` removes the record that key held.
- `get(handle, key) -> Result<Option<Record>>` reads one record; `Ok(None)` means the key holds nothing, which is not an error.
- `set_all(handle, records)` writes a whole collection as one unit.
- `get_all(handle) -> Result<Vec<(Key, Record)>>` reads a whole collection, each record together with its key.

The store package is the only place a storage is named:

- Add a collection by creating a module under `src/state/store/` that declares its `Key` and `Record` and implements the four verbs; its statements live in that module and nowhere else.
- `sqlx` may only appear inside `src/state/store/`. `Handle::pool` is visible to that package alone; using it elsewhere pins the crate to one storage.
- Signatures return the crate's own `Result`; a storage's error type must not appear in them, so a replacement storage can report the same failures.
- A record that cannot be interpreted (for example a stored port outside its type) is logged inside the store and skipped, while a collection that cannot be read at all returns `Err` for the caller to decide whether to degrade.
- Call sites that have not moved yet keep working while the collections are migrated one at a time. When you touch one, move that call onto the store package instead of adding a query beside it; new code never adds a query outside the package.

## Database Migration Safety

- Treat every migration as immutable as soon as it has been applied to any database, including a local development database. Never edit, rename, reorder, or delete an applied migration.
- Before changing an existing migration, query the active development database's `_sqlx_migrations` table for its version. If a successful row exists, stop and create a new migration with a higher version instead.
- Fix migration mistakes only with a new forward migration. Do not make an old migration appear compatible by directly changing `_sqlx_migrations` checksums.
- A historical checksum may be reconciled in application code only when the checksum is explicitly allowlisted, the existing schema is validated structurally, and a tested forward migration brings that schema to the canonical state. Unknown checksums must still fail.
- SQLite table renames do not free global index names. Rebuild migrations must drop or rename legacy indexes before creating replacement indexes.
- Migration tests must cover both a fresh database and an upgrade database containing the exact previous tables, indexes, foreign keys, representative data, malformed provider data, and missing optional data.
- After a migration test, run `PRAGMA foreign_key_check`, verify that legacy tables and indexes are gone, and verify that provider-qualified data did not change provider identity.
- Migrations must not access the network. Cache data used during migration is untrusted and must be validated or ignored without blocking the upgrade.
- Every new launcher migration under `packages/app-lib/migrations/` must update `REVERTIBLE_SCHEMA` in `scripts/axolotl/downgrade-app-db.mjs` in the same change. Use the migration's 14-digit timestamp prefix as the key; the downgrade tests reject mappings that do not correspond to an existing migration.
- Record every column and table added by the migration in that mapping so removing the migration record cannot leave schema that makes a later re-upgrade fail with duplicate objects. List tables in reverse creation/dependency order so dependent tables are dropped before their parents. A migration that only changes data and leaves no schema to remove must still be registered with an empty object (`{}`).
- If a migration adds durable schema that `REVERTIBLE_SCHEMA` cannot currently describe, extend the downgrade script and its tests in the same change. Do not use `--allow-unmapped` as an implementation or CI workaround.
- After adding or changing a launcher migration or its downgrade mapping, run `node scripts/axolotl/downgrade-app-db.test.mjs`; this is part of the CI guardrails. Add or update downgrade test coverage when introducing a new mapping shape or rollback behavior.
- Do not start a migration-watching development process while migration files are still being edited. Finish the migration and its upgrade tests first, then start the app once to apply it.
