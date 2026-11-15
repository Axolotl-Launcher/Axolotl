//! Unsupported persistent data is opaque, not absent. Reads may omit it, but
//! mutation and collection cleanup must not consume its recovery information.

use sqlx::{Connection, Executor, Row, SqliteConnection, SqlitePool};
use std::collections::HashSet;

const STAGES: &str = "'installed','minecraft_installing','installing','pack_installed','pack_installing','not_installed'";
const FEATURES: &str =
    "'none','migrated_server_last_play_time','migrated_launch_hooks'";
const LOADERS: &str = "'vanilla','forge','neoforge','neo_forge','neo','fabric','quilt','cleanroom','legacy_fabric','legacyfabric','babric','optifine','opti_fine','lite_loader','liteloader'";
const COMPONENTS: &str = "'vanilla','forge','neoforge','fabric','quilt','cleanroom','legacy_fabric','babric','optifine','lite_loader','optifabric'";
const SOURCES: &str = "'local','curseforge','mcarchive','modrinth_modpack','server_project','imported_modpack','shared_instance'";

fn unsupported(id: &str) -> String {
    let jobs = unsupported_active_job("j.");
    let file_journal = unsupported_protocol("j.payload", None);
    let legacy_journal = unsupported_protocol("j.payload", None);
    let cleanup = unsupported_protocol("j.payload", None);
    let plan = unsupported_protocol("j.payload", Some("$.plan.schemaVersion"));
    let execution = unsupported_protocol("j.payload", None);
    let artifact = unsupported_protocol("j.artifact", None);
    format!(
        "EXISTS(SELECT 1 FROM instances i WHERE i.id={id} AND (i.install_stage NOT IN ({STAGES}) OR i.launcher_feature_version NOT IN ({FEATURES}) OR i.update_channel NOT IN ('release','beta'))) OR EXISTS(SELECT 1 FROM instance_loader_components c WHERE c.instance_id={id} AND (c.kind NOT IN ({COMPONENTS}) OR c.role NOT IN ('primary','adjunct'))) OR EXISTS(SELECT 1 FROM instance_content_sets c WHERE c.instance_id={id} AND (c.loader NOT IN ({LOADERS}) OR c.source_kind NOT IN ({SOURCES}) OR c.status NOT IN ('available','installing','stale','missing_files'))) OR EXISTS(SELECT 1 FROM instance_content_provider_refs r JOIN instance_content_entries e ON e.id=r.content_entry_id WHERE e.instance_id={id} AND r.provider NOT IN ('modrinth','curseforge','mcarchive','local')) OR EXISTS(SELECT 1 FROM instance_content_entries e WHERE e.instance_id={id} AND (e.project_type NOT IN ('mod','datapack','resourcepack','shader','shaderpack','schematic','world_save') OR e.source_kind NOT IN ({SOURCES}) OR e.ownership_kind NOT IN ('pack_managed','user_added','local_discovered') OR e.server_requirement NOT IN ('required','optional','unsupported','unknown') OR e.client_requirement NOT IN ('required','optional','unsupported','unknown'))) OR EXISTS(SELECT 1 FROM install_jobs j WHERE j.instance_id={id} AND ({jobs})) OR EXISTS(SELECT 1 FROM content_file_operations j WHERE j.instance_id={id} AND ({file_journal})) OR EXISTS(SELECT 1 FROM resource_legacy_rollback_journals j WHERE j.instance_id={id} AND ({legacy_journal})) OR EXISTS(SELECT 1 FROM resource_history_cleanup_plans j WHERE j.instance_id={id} AND j.consumed=0 AND ({cleanup})) OR EXISTS(SELECT 1 FROM resource_operation_plans j WHERE j.instance_id={id} AND j.completed=0 AND ({plan})) OR EXISTS(SELECT 1 FROM resource_execution_journals j JOIN resource_operation_plans p ON p.id=j.plan_id WHERE p.instance_id={id} AND p.completed=0 AND ({execution})) OR EXISTS(SELECT 1 FROM resource_installed_artifacts j JOIN instance_content_entries e ON e.id=j.entry_id WHERE e.instance_id={id} AND ({artifact}))"
    )
}

pub(crate) async fn unsupported_instances(
    pool: &SqlitePool,
) -> crate::Result<HashSet<String>> {
    let mut connection = pool.acquire().await?;
    let file: String = sqlx::query_scalar(
        "SELECT file FROM pragma_database_list WHERE name='main'",
    )
    .fetch_one(&mut *connection)
    .await?;
    if std::path::Path::new(&file)
        .file_name()
        .and_then(|name| name.to_str())
        != Some("system.db")
    {
        return Ok(HashSet::new());
    }
    let mut transaction = connection.begin_with("BEGIN IMMEDIATE").await?;
    let unsupported = unsupported_on(&mut transaction).await?;
    transaction.commit().await?;
    Ok(unsupported)
}

pub(crate) async fn ensure_instance_supported(
    pool: &SqlitePool,
    instance_id: &str,
) -> crate::Result<()> {
    if pool
        .connect_options()
        .get_filename()
        .file_name()
        .is_none_or(|name| name != "system.db")
    {
        return Ok(());
    }
    let statement = format!(
        "SELECT EXISTS(SELECT 1 FROM instances known WHERE known.id=? AND ({}))",
        unsupported("known.id")
    );
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let unsupported =
        sqlx::query_scalar::<_, bool>(sqlx::AssertSqlSafe(statement))
            .bind(instance_id)
            .fetch_one(&mut *transaction)
            .await?;
    transaction.commit().await?;
    if unsupported {
        return Err(crate::ErrorKind::InputError("This instance has newer persistent data or unfinished recovery; it was preserved".into()).into());
    }
    Ok(())
}

pub(super) async fn unsupported_on(
    connection: &mut SqliteConnection,
) -> Result<HashSet<String>, sqlx::Error> {
    let statement = format!(
        "SELECT id FROM instances known WHERE {}",
        unsupported("known.id")
    );
    Ok(
        sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(statement))
            .fetch_all(connection)
            .await?
            .into_iter()
            .collect(),
    )
}

pub(crate) fn legacy_protocol() -> u32 {
    1
}

pub(crate) fn decode_payload<T: serde::de::DeserializeOwned>(
    payload: &str,
) -> crate::Result<T> {
    let value: serde_json::Value = serde_json::from_str(payload)?;
    for key in ["schema_version", "schemaVersion"] {
        if value
            .get(key)
            .is_some_and(|version| version.as_u64() != Some(1))
        {
            return Err(crate::ErrorKind::InputError("Unsupported persistent payload protocol; its data was preserved".into()).into());
        }
    }
    Ok(serde_json::from_value(value)?)
}

fn unsupported_protocol(payload: &str, nested: Option<&str>) -> String {
    let check = |path: &str| {
        format!(
            "(json_type({payload},'{path}') IS NOT NULL AND (json_type({payload},'{path}') <> 'integer' OR json_extract({payload},'{path}') <> 1))"
        )
    };
    let mut fields = vec![check("$.schema_version"), check("$.schemaVersion")];
    if let Some(path) = nested {
        fields.push(check(path));
    }
    format!(
        "CASE WHEN json_valid({payload},6) THEN {} ELSE 1 END",
        fields.join(" OR ")
    )
}

pub(super) fn unsupported_job(prefix: &str) -> String {
    format!(
        "{prefix}kind NOT IN ('create_instance','create_modpack_instance','import_instance','duplicate_instance','install_existing_instance','upgrade_unmanaged_instance','install_pack_to_existing_instance','install_content','change_content','download_java','export_resource') OR {prefix}status NOT IN ('queued','running','canceling','waiting_for_user','succeeded','failed','interrupted','canceled') OR CASE WHEN json_valid({prefix}state,6) THEN (json_extract({prefix}state,'$.schema_version') IS NOT NULL AND json_extract({prefix}state,'$.schema_version') <> 1) OR (json_extract({prefix}state,'$._format') IS NOT NULL AND json_extract({prefix}state,'$._format') <> 'install-v1') ELSE 1 END"
    )
}

/// The stable completion head is independent of the payload protocol. A
/// producer may publish succeeded + finished only after recovery obligations
/// are settled. Unknown completed jobs remain opaque but do not occupy scope.
pub(super) fn unsupported_active_job(prefix: &str) -> String {
    let unknown = unsupported_job(prefix);
    format!(
        "({unknown}) AND NOT ({prefix}status='succeeded' AND {prefix}finished IS NOT NULL)"
    )
}

pub(crate) async fn unknown_recovery_payloads(
    pool: &SqlitePool,
) -> crate::Result<bool> {
    for (table, nested) in [
        ("resource_history", None),
        ("content_file_operations", None),
        ("resource_legacy_rollback_journals", None),
        ("resource_history_cleanup_plans", None),
        ("resource_operation_plans", Some("$.plan.schemaVersion")),
        ("resource_execution_journals", None),
    ] {
        let unknown = unsupported_protocol("payload", nested);
        let active = match table {
            "resource_history_cleanup_plans" => "consumed=0 AND ",
            "resource_operation_plans" => "completed=0 AND ",
            "resource_execution_journals" => {
                "plan_id IN (SELECT id FROM resource_operation_plans WHERE completed=0) AND "
            }
            _ => "",
        };
        let statement = format!(
            "SELECT EXISTS(SELECT 1 FROM {table} WHERE {active}({unknown}))"
        );
        if sqlx::query_scalar::<_, bool>(sqlx::AssertSqlSafe(statement))
            .fetch_one(pool)
            .await?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) async fn ensure_history_cleanup_supported(
    pool: &SqlitePool,
    instance_id: &str,
) -> crate::Result<()> {
    let unknown = unsupported_protocol("payload", None);
    let statement = format!(
        "SELECT EXISTS(SELECT 1 FROM resource_history WHERE instance_id=? AND ({unknown}))"
    );
    if sqlx::query_scalar::<_, bool>(sqlx::AssertSqlSafe(statement))
        .bind(instance_id)
        .fetch_one(pool)
        .await?
    {
        return Err(crate::ErrorKind::InputError("History cleanup is paused because newer history data may retain backup files".into()).into());
    }
    Ok(())
}

pub(super) async fn install(
    connection: &mut SqliteConnection,
    tables: &[(&str, &str, bool)],
) -> Result<(), sqlx::Error> {
    for (schema, table, owns_instance) in tables {
        if !owns_instance {
            continue;
        }
        let id = if *table == "instances" {
            "OLD.id"
        } else {
            "OLD.instance_id"
        };
        for event in ["UPDATE", "DELETE"] {
            let statement = format!(
                "CREATE TEMP TRIGGER IF NOT EXISTS opaque_{table}_{event} BEFORE {event} ON \"{schema}\".\"{table}\" WHEN ({}) AND EXISTS(SELECT 1 FROM instances WHERE id={id}) BEGIN SELECT RAISE(ABORT,'unsupported instance data is preserved'); END",
                unsupported(id)
            );
            connection
                .execute(sqlx::AssertSqlSafe(statement.as_str()))
                .await?;
        }
    }
    // Provider references have no instance_id column of their own. A broad
    // replacement must not delete a reference merely because it was skipped.
    for event in ["UPDATE", "DELETE"] {
        let statement = format!(
            "CREATE TEMP TRIGGER IF NOT EXISTS opaque_provider_{event} BEFORE {event} ON instances.instance_content_provider_refs WHEN OLD.provider NOT IN ('modrinth','curseforge','mcarchive','local') BEGIN SELECT RAISE(ABORT,'unsupported provider data is preserved'); END"
        );
        connection
            .execute(sqlx::AssertSqlSafe(statement.as_str()))
            .await?;
    }
    let columns = sqlx::query("PRAGMA operations.table_info(install_jobs)")
        .fetch_all(&mut *connection)
        .await?;
    let unchanged = columns
        .iter()
        .map(|row| row.try_get::<String, _>("name"))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|name| name != "instance_id")
        .map(|name| {
            let name = name.replace('"', "\"\"");
            format!("NEW.\"{name}\" IS OLD.\"{name}\"")
        })
        .collect::<Vec<_>>()
        .join(" AND ");
    for event in ["UPDATE", "DELETE"] {
        let unknown = unsupported_job("OLD.");
        let detach = if event == "UPDATE" {
            format!(
                " AND NOT (OLD.status='succeeded' AND OLD.finished IS NOT NULL AND OLD.instance_id IS NOT NULL AND NEW.instance_id IS NULL AND {unchanged})"
            )
        } else {
            String::new()
        };
        let statement = format!(
            "CREATE TEMP TRIGGER IF NOT EXISTS opaque_job_{event} BEFORE {event} ON operations.install_jobs WHEN ({unknown}){detach} BEGIN SELECT RAISE(ABORT,'unsupported operation data is preserved'); END"
        );
        connection
            .execute(sqlx::AssertSqlSafe(statement.as_str()))
            .await?;
    }
    for (table, payload, nested) in [
        ("content_file_operations", "payload", None),
        ("resource_legacy_rollback_journals", "payload", None),
        ("resource_history_cleanup_plans", "payload", None),
        (
            "resource_operation_plans",
            "payload",
            Some("$.plan.schemaVersion"),
        ),
        ("resource_execution_journals", "payload", None),
    ] {
        let unknown = unsupported_protocol(&format!("OLD.{payload}"), nested);
        for event in ["UPDATE", "DELETE"] {
            let pending = if table == "resource_history_cleanup_plans"
                && event == "DELETE"
            {
                " AND OLD.consumed=0"
            } else {
                ""
            };
            let owner = if table == "resource_execution_journals" {
                String::new()
            } else {
                " AND EXISTS(SELECT 1 FROM instances WHERE id=OLD.instance_id)"
                    .into()
            };
            let statement = format!(
                "CREATE TEMP TRIGGER IF NOT EXISTS opaque_protocol_{table}_{event} BEFORE {event} ON operations.{table} WHEN ({unknown}){owner}{pending} BEGIN SELECT RAISE(ABORT,'unsupported recovery protocol is preserved'); END"
            );
            connection
                .execute(sqlx::AssertSqlSafe(statement.as_str()))
                .await?;
        }
    }
    Ok(())
}
