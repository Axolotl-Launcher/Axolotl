//! Stable publication revisions and a complete raw relationship snapshot.
use futures::TryStreamExt;
use serde_json::{Map, Value, json};
use sqlx::{
    Column, Executor, Row, SqliteConnection, SqlitePool, TypeInfo, ValueRef,
};
const MAX_SNAPSHOT_BYTES: usize = 64 * 1024 * 1024;

fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

pub(crate) const STATE: &str = "CREATE TABLE IF NOT EXISTS instances.instance_recovery_publications(instance_id TEXT PRIMARY KEY,revision INTEGER NOT NULL,published_revision INTEGER NOT NULL DEFAULT 0,deleted INTEGER NOT NULL DEFAULT 0)";

pub(crate) async fn install(
    connection: &mut SqliteConnection,
    durable: bool,
) -> Result<(), sqlx::Error> {
    if durable {
        connection.execute(STATE).await?;
        connection.execute("CREATE TABLE IF NOT EXISTS main.instance_recovery_generation(singleton INTEGER PRIMARY KEY CHECK(singleton=1),identity TEXT NOT NULL)").await?;
        sqlx::query("INSERT OR IGNORE INTO main.instance_recovery_generation VALUES(1,?)").bind(uuid::Uuid::new_v4().to_string()).execute(&mut *connection).await?;
        connection.execute("INSERT OR IGNORE INTO instances.instance_recovery_publications(instance_id,revision) SELECT id,1 FROM instances.instances").await?;
    }
    // Ownership changes dirty both sides. The revision is part of the same
    // transaction as the authoritative row, including foreign-key cascades.
    for table in known_instance_tables() {
        let columns: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM pragma_table_info(?, 'instances')",
        )
        .bind(table)
        .fetch_all(&mut *connection)
        .await?;
        if columns.is_empty() {
            continue;
        }
        for event in ["INSERT", "UPDATE", "DELETE"] {
            let mut changes = String::new();
            let prefixes: &[&str] = match event {
                "INSERT" => &["NEW"],
                "DELETE" => &["OLD"],
                _ => &["OLD", "NEW"],
            };
            for prefix in prefixes {
                let id = if table == "instances" {
                    format!("{prefix}.id")
                } else if columns.iter().any(|c| c == "instance_id") {
                    format!("{prefix}.instance_id")
                } else if columns.iter().any(|c| c == "content_set_id") {
                    format!(
                        "(SELECT instance_id FROM instance_content_sets WHERE id={prefix}.content_set_id)"
                    )
                } else if columns.iter().any(|c| c == "content_entry_id") {
                    format!(
                        "(SELECT instance_id FROM instance_content_entries WHERE id={prefix}.content_entry_id)"
                    )
                } else {
                    continue;
                };
                let deleted =
                    i32::from(table == "instances" && event == "DELETE");
                let revive =
                    i32::from(table == "instances" && event == "INSERT");
                changes.push_str(&format!("INSERT INTO instance_recovery_publications(instance_id,revision,deleted) SELECT {id},1,{deleted} WHERE {id} IS NOT NULL ON CONFLICT(instance_id) DO UPDATE SET revision=revision+1,deleted=CASE WHEN {deleted}=1 THEN 1 WHEN {revive}=1 THEN 0 ELSE deleted END;"));
            }
            if !changes.is_empty() {
                connection.execute(sqlx::AssertSqlSafe(format!("CREATE TEMP TRIGGER IF NOT EXISTS manifest_{table}_{event} AFTER {event} ON instances.{} BEGIN {changes} END", quote(table)))).await?;
            }
        }
    }
    for (table, owner) in resource_tables() {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM instances.sqlite_schema WHERE type='table' AND name=?)").bind(table).fetch_one(&mut *connection).await?;
        if !exists {
            continue;
        }
        for event in ["INSERT", "UPDATE", "DELETE"] {
            let prefixes: &[&str] = match event {
                "INSERT" => &["NEW"],
                "DELETE" => &["OLD"],
                _ => &["OLD", "NEW"],
            };
            let mut changes = String::new();
            for row in prefixes {
                let predicate = owner.replace("$row", row);
                changes.push_str(&format!("UPDATE instance_recovery_publications SET revision=revision+1 WHERE deleted=0 AND instance_id IN ({predicate});"));
            }
            connection.execute(sqlx::AssertSqlSafe(format!("CREATE TEMP TRIGGER IF NOT EXISTS manifest_resource_{table}_{event} AFTER {event} ON instances.{} BEGIN {changes} END", quote(table)))).await?;
        }
    }
    Ok(())
}

pub(crate) async fn pending(
    pool: &SqlitePool,
) -> crate::Result<Vec<(String, i64, bool)>> {
    Ok(sqlx::query_as("SELECT instance_id,revision,deleted FROM instance_recovery_publications WHERE revision>published_revision ORDER BY instance_id").fetch_all(pool).await?)
}
pub(crate) async fn revision(
    pool: &SqlitePool,
    id: &str,
) -> crate::Result<i64> {
    Ok(sqlx::query_scalar("SELECT revision FROM instance_recovery_publications WHERE instance_id=? AND deleted=0").bind(id).fetch_one(pool).await?)
}

pub(crate) async fn rows(
    connection: &mut SqliteConnection,
    id: &str,
) -> crate::Result<Map<String, Value>> {
    let mut result = Map::new();
    let mut total = 0usize;
    // Every authoritative instance relationship is retained. Columns unknown to this reader are
    // copied as values rather than disappearing in a typed model roundtrip.
    let tables = known_instance_tables();
    for table in tables {
        let columns: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM pragma_table_info(?, 'instances')",
        )
        .bind(&table)
        .fetch_all(&mut *connection)
        .await?;
        let predicate = if table == "instances" {
            "id=?".to_owned()
        } else if columns.iter().any(|c| c == "instance_id") {
            "instance_id=?".into()
        } else if columns.iter().any(|c| c == "content_set_id") {
            "content_set_id IN (SELECT id FROM instance_content_sets WHERE instance_id=?)".into()
        } else if columns.iter().any(|c| c == "content_entry_id") {
            "content_entry_id IN (SELECT id FROM instance_content_entries WHERE instance_id=?)".into()
        } else {
            continue;
        };
        let query = format!(
            "SELECT * FROM instances.{} WHERE {predicate}",
            quote(&table)
        );
        let mut rows = sqlx::query(sqlx::AssertSqlSafe(query))
            .bind(id)
            .fetch(&mut *connection);
        let mut values = Vec::new();
        while let Some(row) = rows.try_next().await? {
            values.push(capture_row(&row, &mut total)?);
        }
        result.insert(table.to_owned(), Value::Array(values));
    }
    for (table, predicate) in [
        (
            "resource_installed_artifacts",
            "entry_id IN (SELECT id FROM instance_content_entries WHERE instance_id=?)",
        ),
        ("resource_pack_unlinks", "instance_id=?"),
    ] {
        let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM instances.sqlite_schema WHERE type='table' AND name=?)").bind(table).fetch_one(&mut *connection).await?;
        if !exists {
            continue;
        }
        let mut rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT * FROM instances.{} WHERE {predicate}",
            quote(table)
        )))
        .bind(id)
        .fetch(&mut *connection);
        let mut values = Vec::new();
        while let Some(row) = rows.try_next().await? {
            values.push(capture_row(&row, &mut total)?);
        }
        result.insert(table.into(), Value::Array(values));
    }
    Ok(result)
}

pub(crate) async fn publish<F, Fut>(
    pool: &SqlitePool,
    id: &str,
    expected: i64,
    write: F,
) -> crate::Result<bool>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = crate::Result<()>>,
{
    let (mut tx, tx_diagnostics) =
        crate::state::store::lock_diagnostics::begin(pool, "BEGIN IMMEDIATE")
            .await?;
    let revision:Option<i64>=sqlx::query_scalar("SELECT revision FROM instance_recovery_publications WHERE instance_id=?").bind(id).fetch_optional(&mut *tx).await?;
    if revision != Some(expected) {
        return Ok(false);
    }
    write().await?;
    sqlx::query("UPDATE instance_recovery_publications SET published_revision=? WHERE instance_id=? AND revision=?").bind(expected).bind(id).bind(expected).execute(&mut *tx).await?;
    tx_diagnostics.observe("commit_wait", tx.commit()).await?;
    tx_diagnostics.complete("committed");
    Ok(true)
}

pub(crate) async fn snapshot(
    pool: &SqlitePool,
    id: &str,
    expected: i64,
) -> crate::Result<
    Option<(
        crate::state::instances::InstanceMetadata,
        Map<String, Value>,
    )>,
> {
    let (mut tx, tx_diagnostics) =
        crate::state::store::lock_diagnostics::begin(pool, "BEGIN IMMEDIATE")
            .await?;
    let current:Option<i64>=sqlx::query_scalar("SELECT revision FROM instance_recovery_publications WHERE instance_id=? AND deleted=0").bind(id).fetch_optional(&mut *tx).await?;
    if current != Some(expected) {
        return Ok(None);
    }
    if crate::state::store::compatibility::unsupported_on(&mut tx)
        .await?
        .contains(id)
    {
        return Err(crate::ErrorKind::InputError("Unsupported instance data cannot be published as a supported recovery copy".into()).into());
    }
    let Some(record) =
        super::instance_rows::recovery_metadata_on(id, &mut tx).await?
    else {
        return Ok(None);
    };
    let loader_components =
        super::loader_component_rows::recovery_components_on(id, &mut tx)
            .await?;
    let metadata = crate::state::instances::InstanceMetadata {
        instance: record.instance,
        applied_content_set: record.applied_content_set,
        link: record.link,
        groups: crate::state::instance_groups::group_ids_for(id),
        launch_overrides: record.launch_overrides,
        loader_components,
        synced_options: Default::default(),
    };
    let result = rows(&mut tx, id).await?;
    tx_diagnostics.observe("commit_wait", tx.commit()).await?;
    tx_diagnostics.complete("committed");
    Ok(Some((metadata, result)))
}
pub(crate) async fn force_dirty(
    pool: &SqlitePool,
    id: &str,
) -> crate::Result<()> {
    let diagnostics = crate::state::store::lock_diagnostics::Scope::new(
        pool,
        "autocommit_write",
    );
    diagnostics.label("instance_recovery_force_dirty");
    diagnostics.observe("statement_wait", sqlx::query("UPDATE instance_recovery_publications SET published_revision=0 WHERE instance_id=?").bind(id).execute(pool)).await?;
    diagnostics.complete("committed");
    Ok(())
}

fn known_instance_tables() -> Vec<&'static str> {
    vec![
        "instances",
        "instance_links",
        "instance_launch_overrides",
        "instance_content_sets",
        "instance_content_set_remote_refs",
        "instance_content_set_sync_state",
        "instance_files",
        "instance_content_entries",
        "instance_loader_components",
        "instance_pack_members",
        "instance_pending_manual_downloads",
        "instance_content_dependencies",
        "instance_content_provider_refs",
        "instance_post_upgrade_notices",
    ]
}

pub(crate) async fn generation(pool: &SqlitePool) -> crate::Result<String> {
    Ok(sqlx::query_scalar("SELECT identity FROM main.instance_recovery_generation WHERE singleton=1").fetch_one(pool).await?)
}

/// Provider roots also exist before a pack member has a materialized entry.
/// Keep the union independent of runtime views so connection setup may install
/// the tracking triggers before creating those views.
fn resource_tables() -> [(&'static str, &'static str); 2] {
    [
        (
            "resource_installed_artifacts",
            "SELECT instance_id FROM instance_content_entries WHERE id=$row.entry_id",
        ),
        ("resource_pack_unlinks", "SELECT $row.instance_id"),
    ]
}

fn capture_row(
    row: &sqlx::sqlite::SqliteRow,
    total: &mut usize,
) -> crate::Result<Value> {
    let mut object = Map::new();
    for column in row.columns() {
        let name = column.name();
        let raw = row.try_get_raw(name)?;
        let value = if raw.is_null() {
            Value::Null
        } else {
            match raw.type_info().name() {
                "INTEGER" => json!(row.try_get::<i64, _>(name)?),
                "REAL" => json!(row.try_get::<f64, _>(name)?),
                "BLOB" => {
                    json!({"sqlite_blob":row.try_get::<Vec<u8>,_>(name)?})
                }
                _ => json!(row.try_get::<String, _>(name)?),
            }
        };
        object.insert(name.to_owned(), value);
    }
    let value = Value::Object(object);
    let size = serde_json::to_vec(&value)?.len();
    *total = total.checked_add(size).ok_or_else(|| {
        crate::ErrorKind::InputError("Recovery snapshot size overflow".into())
    })?;
    if size > 512 * 1024 || *total > MAX_SNAPSHOT_BYTES {
        return Err(crate::ErrorKind::InputError(
            "Recovery relationships exceed the supported bounded snapshot size"
                .into(),
        )
        .into());
    }
    Ok(value)
}

pub(crate) async fn metadata(
    pool: &SqlitePool,
    id: &str,
) -> crate::Result<Option<crate::state::instances::InstanceMetadata>> {
    let (mut transaction, transaction_diagnostics) =
        crate::state::store::lock_diagnostics::begin(pool, "BEGIN").await?;
    if crate::state::store::compatibility::unsupported_on(&mut transaction)
        .await?
        .contains(id)
    {
        return Err(crate::ErrorKind::InputError(
            "Unsupported instance recovery descriptor".into(),
        )
        .into());
    }
    let Some(record) =
        super::instance_rows::recovery_metadata_on(id, &mut transaction)
            .await?
    else {
        return Ok(None);
    };
    let loader_components =
        super::loader_component_rows::recovery_components_on(
            id,
            &mut transaction,
        )
        .await?;
    let metadata = crate::state::instances::InstanceMetadata {
        instance: record.instance,
        applied_content_set: record.applied_content_set,
        link: record.link,
        groups: crate::state::instance_groups::group_ids_for(id),
        launch_overrides: record.launch_overrides,
        loader_components,
        synced_options: Default::default(),
    };
    transaction_diagnostics
        .observe("commit_wait", transaction.commit())
        .await?;
    transaction_diagnostics.complete("committed");
    Ok(Some(metadata))
}

pub(crate) async fn live_ids(pool: &SqlitePool) -> crate::Result<Vec<String>> {
    Ok(sqlx::query_scalar("SELECT id FROM instances ORDER BY id")
        .fetch_all(pool)
        .await?)
}
