# derived.db migrations

Migrations for the rebuildable database opened by
`src/state/store/derived_db.rs`. Applied by `sqlx::migrate!("./derived_migrations")`
at that database's own connection, in one transaction per file.

## Naming

`<VERSION>_<DESCRIPTION>.sql`, as SQLx reads them: a bare integer version, then
the description, for example `0001_derived-cache.sql`. Unlike `migrations/`,
which uses 14-digit timestamps because app.db is shared by launchers of every
channel, this database has a single writer, so versions count up from `0001`.
Keep them dense and in ascending order.

A file is immutable once it has been applied anywhere, including a development
database. A change is a new file with a higher version; a statement that must
not run in a transaction starts with `-- no-transaction`.

## Outside the downgrade tool

`scripts/axolotl/downgrade-app-db.mjs` indexes the `_sqlx_migrations` table of
**app.db only**. Nothing here is registered in `REVERTIBLE_SCHEMA`, and
deriving a downgrade for this database is not a thing to add: its records are
rebuildable, so the answer to a version this build does not know is to disable
the database and keep the file (see the module documentation in
`derived_db.rs`), not to remove what a newer launcher wrote.

For the same reason there is no `derived_meta` table and no schema version
constant in code: SQLx's migration table is the only version record.
