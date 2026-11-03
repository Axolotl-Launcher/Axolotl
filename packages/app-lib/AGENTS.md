# Desktop Application Library

This Rust crate owns desktop runtime behavior, persistence, and database migrations. Read [CONTRIBUTING.md](../../CONTRIBUTING.md#应用数据目录与数据库迁移) before starting the application or operating on an app database. Development builds share the installed app's data directory unless explicitly isolated with `THESEUS_CONFIG_DIR` or `AXOLOTL_DATA_DIR_SUFFIX`.

For behavior exposed to the frontend through a new or changed native command, follow the shell's [Tauri command completion rules](../../apps/app/AGENTS.md#tauri-command-completion). For build locks or occupied outputs, follow the root [shared Rust build resource rules](../../AGENTS.md#shared-rust-build-resources); do not allocate another target directory.

## Data Access Interface

Records this crate keeps between runs are read and written by its persistence layer, which has two halves of equal standing. `packages/app-lib/src/state/store/` owns the runtime's own records — settings, caches, the bookkeeping the application keeps about itself. `packages/app-lib/src/state/instances/adapters/sqlite/` owns the instance domain's rows — instances, content sets, entries, provider references, pack members, files, playtime, post-upgrade notices. Each half keeps its own statements: the store answers collection verbs, the instance half answers step functions that run inside the unit the caller opened.

**Do not reach the database on your own**: no business module may use `sqlx` or write a statement. Business code calls one of the two halves, and which half owns a row follows from the table that row lives in. Beginning a transaction to group several steps, or handing a pool to one of them, is how a caller reaches the persistence layer; spelling the SQL is not.

A caller names a collection and the fields it looks a record up by, through the handle in `State::data`. The verbs are centred on `get` and `set`, and qualify them the same way everywhere:

- `get(handle, key) -> Result<Option<Record>>` reads one record; `Ok(None)` means the key holds nothing, which is not an error.
- `set(handle, key, Some(record))` writes the record under the key; passing `None` removes the record that key held.
- `get_all(handle)` / `set_all(handle, records)` reach a whole collection.
- `get_scoped(handle, scope)` / `set_scoped(handle, scope, records)` reach the records one scope names.
- `atom_get` / `atom_set` / `atom_get_all` / `atom_set_all` / `atom_get_scoped` / `atom_set_scoped` take a `&mut Atom` instead of the handle and run inside one atomic unit: everything written through it lands together, or nothing does. Open one with `store::atom(&handle)` and finish with `Atom::commit`; dropping an uncommitted unit rolls it back.

Which storage answers a collection, and how it is asked, stays inside the store half:

- Add a collection by creating a module under `src/state/store/` that declares its `Key`, its `Scope` and its `Record`, and answers the six statements the verbs are written in terms of — read, write, read-all, replace-all, read-scoped and replace-scoped, each on a `&mut SqliteConnection`. Its statements live in that module and nowhere else; a collection with no narrower group sets `Scope = Key`. A collection implements those statements rather than the verbs: the verbs, and the checks that a replace carries unique keys and no key outside its scope, are the trait defaults and must not be re-implemented per collection.
- Prefer the scoped verbs and atomic units to reading a whole collection and filtering it in the caller: a full read there is the performance defect this interface exists to avoid.
- `sqlx` may only appear inside the persistence layer: `src/state/store/` and `src/state/instances/adapters/sqlite/`. `Handle::pool` is visible to the store half alone; a caller reaches the instance half by passing `State::pool` or a transaction it opened into a step.
- The instance half answers step by step. A step that writes takes the `&mut Transaction` its caller opened and never commits it; a step that only reads takes a generic `Executor`, so it can also join an open unit. A guard belongs to the step that acts on it, so no caller can skip the check by writing its own statement.
- Signatures return the crate's own `Result`; a storage's error type must not appear in them, so a replacement storage can report the same failures.
- A record that cannot be interpreted (for example a stored port outside its type) is logged inside the store and skipped, while a collection that cannot be read at all returns `Err` for the caller to decide whether to degrade.
- Call sites that have not moved yet keep working while the collections are migrated one at a time. When you touch one, move that call onto the half that owns the table instead of adding a query beside it; new code never adds a query outside the persistence layer.

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
