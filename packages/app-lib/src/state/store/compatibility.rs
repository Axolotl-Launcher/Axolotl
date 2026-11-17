//! Unsupported persistent data is opaque, not absent. Reads may omit it, but
//! mutation and collection cleanup must not consume its recovery information.

use sqlx::{Executor, SqliteConnection, SqlitePool};
use std::collections::HashSet;

const STAGES: &str = "'installed','minecraft_installing','installing','pack_installed','pack_installing','not_installed'";
const FEATURES: &str =
    "'none','migrated_server_last_play_time','migrated_launch_hooks'";
const LOADERS: &str = "'vanilla','forge','neoforge','neo_forge','neo','fabric','quilt','cleanroom','legacy_fabric','legacyfabric','babric','optifine','opti_fine','lite_loader','liteloader'";
const COMPONENTS: &str = "'vanilla','forge','neoforge','fabric','quilt','cleanroom','legacy_fabric','babric','optifine','lite_loader','optifabric'";
const SOURCES: &str = "'local','curseforge','mcarchive','modrinth_modpack','server_project','imported_modpack','shared_instance'";

fn unsupported(id: &str) -> String {
    let artifact = unsupported_protocol("j.artifact", None);
    format!(
        "EXISTS(SELECT 1 FROM instances i WHERE i.id={id} AND (i.install_stage NOT IN ({STAGES}) OR i.launcher_feature_version NOT IN ({FEATURES}) OR i.update_channel NOT IN ('release','beta'))) OR EXISTS(SELECT 1 FROM instance_loader_components c WHERE c.instance_id={id} AND (c.kind NOT IN ({COMPONENTS}) OR c.role NOT IN ('primary','adjunct'))) OR EXISTS(SELECT 1 FROM instance_content_sets c WHERE c.instance_id={id} AND (c.loader NOT IN ({LOADERS}) OR c.source_kind NOT IN ({SOURCES}) OR c.status NOT IN ('available','installing','stale','missing_files'))) OR EXISTS(SELECT 1 FROM instance_content_provider_refs r JOIN instance_content_entries e ON e.id=r.content_entry_id WHERE e.instance_id={id} AND r.provider NOT IN ('modrinth','curseforge','mcarchive','local')) OR EXISTS(SELECT 1 FROM instance_content_entries e WHERE e.instance_id={id} AND (e.project_type NOT IN ('mod','datapack','resourcepack','shader','shaderpack','schematic','world_save') OR e.source_kind NOT IN ({SOURCES}) OR e.ownership_kind NOT IN ('pack_managed','user_added','local_discovered') OR e.server_requirement NOT IN ('required','optional','unsupported','unknown') OR e.client_requirement NOT IN ('required','optional','unsupported','unknown'))) OR EXISTS(SELECT 1 FROM resource_installed_artifacts j JOIN instance_content_entries e ON e.id=j.entry_id WHERE e.instance_id={id} AND ({artifact}))"
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
    // One SELECT supplies a consistent read snapshot without reserving the
    // writer needed by installation, synchronization or recovery publication.
    Ok(unsupported_on(&mut connection).await?)
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
    let (mut transaction, transaction_diagnostics) =
        crate::state::store::lock_diagnostics::begin(pool, "BEGIN IMMEDIATE")
            .await?;
    let unsupported =
        sqlx::query_scalar::<_, bool>(sqlx::AssertSqlSafe(statement))
            .bind(instance_id)
            .fetch_one(&mut *transaction)
            .await?;
    transaction_diagnostics
        .observe("commit_wait", transaction.commit())
        .await?;
    transaction_diagnostics.complete("committed");
    if unsupported {
        return Err(crate::ErrorKind::InputError("This instance has newer persistent data or unfinished recovery; it was preserved".into()).into());
    }
    Ok(())
}

pub(crate) async fn unsupported_on(
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
    let mut connection = pool.acquire().await?;
    let file_records =
        super::recovery_documents::active(&mut connection).await?;
    drop(connection);
    if file_records {
        if super::recovery_documents::ensure_cleanup_supported(pool)
            .await
            .is_err()
        {
            return Ok(true);
        }
    }
    for (table, nested) in [("resource_history", None)] {
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
    let mut connection = pool.acquire().await?;
    let file_records =
        super::recovery_documents::active(&mut connection).await?;
    drop(connection);
    if file_records {
        super::recovery_documents::ensure_instance_supported(pool, instance_id)
            .await?;
    }
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::instances::adapters::sqlite::{
        content_entries, instance_rows, loader_component_rows,
    };
    use crate::state::store::{configuration, databases};
    use sqlx::Connection;

    async fn instance(pool: &SqlitePool, id: &str, stage: &str) {
        sqlx::query("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified) VALUES(?,?,?,'none','test',0,0)")
            .bind(id).bind(id).bind(stage).execute(pool).await.unwrap();
    }

    #[tokio::test]
    async fn instance_listing_filters_future_data_while_another_writer_is_reserved()
     {
        let temporary = tempfile::tempdir().unwrap();
        let pool = databases::open(temporary.path()).await.unwrap();
        instance(&pool, "known", "installed").await;
        instance(&pool, "future", "future-stage").await;
        // Initialize both connections before the writer reserves core so this
        // test specifically guards compatibility reads, not connection setup.
        let mut writer = pool.acquire().await.unwrap();
        let reader = pool.acquire().await.unwrap();
        drop(reader);
        let transaction = writer.begin_with("BEGIN IMMEDIATE").await.unwrap();
        let listed = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            async {
                let unsupported = unsupported_instances(&pool).await.unwrap();
                assert_eq!(unsupported, HashSet::from(["future".to_owned()]));
                instance_rows::list_instances(&pool).await.unwrap()
            },
        )
        .await;
        transaction.rollback().await.unwrap();
        let listed = listed.expect(
            "listing must not reserve a writer for its compatibility SELECT",
        );
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "known");
    }

    #[tokio::test]
    async fn current_model_variants_are_not_mistaken_for_future_formats() {
        use crate::state::{
            ContentOwnershipKind, ContentRequirement, ContentSetStatus,
            ContentSourceKind, InstanceInstallStage, LauncherFeatureVersion,
            LoaderComponentKind, LoaderComponentRole, ModLoader, ProjectType,
        };
        let temporary = tempfile::tempdir().unwrap();
        let pool = databases::open(temporary.path()).await.unwrap();
        instance(&pool, "instance", "installed").await;
        for stage in [
            InstanceInstallStage::Installed,
            InstanceInstallStage::MinecraftInstalling,
            InstanceInstallStage::PackInstalled,
            InstanceInstallStage::PackInstalling,
            InstanceInstallStage::NotInstalled,
        ] {
            sqlx::query("UPDATE instances SET install_stage=?")
                .bind(stage.as_str())
                .execute(&pool)
                .await
                .unwrap();
            assert!(unsupported_instances(&pool).await.unwrap().is_empty());
        }
        for feature in [
            LauncherFeatureVersion::None,
            LauncherFeatureVersion::MigratedServerLastPlayTime,
            LauncherFeatureVersion::MigratedLaunchHooks,
        ] {
            sqlx::query("UPDATE instances SET launcher_feature_version=?")
                .bind(feature.as_str())
                .execute(&pool)
                .await
                .unwrap();
            assert!(unsupported_instances(&pool).await.unwrap().is_empty());
        }
        sqlx::query("INSERT INTO instance_content_sets(id,instance_id,name,source_kind,status,game_version,loader,created,modified) VALUES('set','instance','test','local','available','1.21','vanilla',0,0)").execute(&pool).await.unwrap();
        for loader in [
            ModLoader::Vanilla,
            ModLoader::Forge,
            ModLoader::NeoForge,
            ModLoader::Fabric,
            ModLoader::Quilt,
            ModLoader::Cleanroom,
            ModLoader::LegacyFabric,
            ModLoader::Babric,
            ModLoader::OptiFine,
            ModLoader::LiteLoader,
        ] {
            sqlx::query("UPDATE instance_content_sets SET loader=?")
                .bind(loader.as_str())
                .execute(&pool)
                .await
                .unwrap();
            assert!(unsupported_instances(&pool).await.unwrap().is_empty());
        }
        for source in [
            ContentSourceKind::Local,
            ContentSourceKind::CurseForge,
            ContentSourceKind::McArchive,
            ContentSourceKind::ModrinthModpack,
            ContentSourceKind::ServerProject,
            ContentSourceKind::ImportedModpack,
            ContentSourceKind::SharedInstance,
        ] {
            sqlx::query("UPDATE instance_content_sets SET source_kind=?")
                .bind(source.as_str())
                .execute(&pool)
                .await
                .unwrap();
            assert!(unsupported_instances(&pool).await.unwrap().is_empty());
        }
        for status in [
            ContentSetStatus::Available,
            ContentSetStatus::Installing,
            ContentSetStatus::Stale,
            ContentSetStatus::MissingFiles,
        ] {
            sqlx::query("UPDATE instance_content_sets SET status=?")
                .bind(status.as_str())
                .execute(&pool)
                .await
                .unwrap();
            assert!(unsupported_instances(&pool).await.unwrap().is_empty());
        }
        sqlx::query("INSERT INTO instance_loader_components(instance_id,kind,role) VALUES('instance','vanilla','primary')").execute(&pool).await.unwrap();
        for kind in [
            LoaderComponentKind::Vanilla,
            LoaderComponentKind::Forge,
            LoaderComponentKind::NeoForge,
            LoaderComponentKind::Fabric,
            LoaderComponentKind::Quilt,
            LoaderComponentKind::Cleanroom,
            LoaderComponentKind::LegacyFabric,
            LoaderComponentKind::Babric,
            LoaderComponentKind::OptiFine,
            LoaderComponentKind::LiteLoader,
            LoaderComponentKind::OptiFabric,
        ] {
            sqlx::query("UPDATE instance_loader_components SET kind=?")
                .bind(kind.as_str())
                .execute(&pool)
                .await
                .unwrap();
            assert!(unsupported_instances(&pool).await.unwrap().is_empty());
        }
        for role in [LoaderComponentRole::Primary, LoaderComponentRole::Adjunct]
        {
            sqlx::query("UPDATE instance_loader_components SET role=?")
                .bind(role.as_str())
                .execute(&pool)
                .await
                .unwrap();
            assert!(unsupported_instances(&pool).await.unwrap().is_empty());
        }
        sqlx::query("INSERT INTO instance_content_entries(id,instance_id,content_set_id,project_type,source_kind,server_requirement,client_requirement,added_at,modified_at) VALUES('entry','instance','set','mod','local','unknown','unknown',0,0)").execute(&pool).await.unwrap();
        for ownership in [
            ContentOwnershipKind::PackManaged,
            ContentOwnershipKind::UserAdded,
            ContentOwnershipKind::LocalDiscovered,
        ] {
            sqlx::query("UPDATE instance_content_entries SET ownership_kind=?")
                .bind(ownership.as_str())
                .execute(&pool)
                .await
                .unwrap();
            assert!(unsupported_instances(&pool).await.unwrap().is_empty());
        }
        for requirement in [
            ContentRequirement::Required,
            ContentRequirement::Optional,
            ContentRequirement::Unsupported,
            ContentRequirement::Unknown,
        ] {
            sqlx::query("UPDATE instance_content_entries SET server_requirement=?,client_requirement=?").bind(requirement.as_str()).bind(requirement.as_str()).execute(&pool).await.unwrap();
            assert!(unsupported_instances(&pool).await.unwrap().is_empty());
        }
        for project in ProjectType::iterator() {
            sqlx::query("UPDATE instance_content_entries SET project_type=?")
                .bind(project.get_name())
                .execute(&pool)
                .await
                .unwrap();
            assert!(unsupported_instances(&pool).await.unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn unknown_stage_and_loader_are_preserved_without_hiding_known_instances()
     {
        let temporary = tempfile::tempdir().unwrap();
        let pool = databases::open(temporary.path()).await.unwrap();
        instance(&pool, "known", "installed").await;
        instance(&pool, "stage", "future-stage").await;
        instance(&pool, "loader", "installed").await;
        sqlx::query("INSERT INTO instance_loader_components(instance_id,kind,role) VALUES('loader','future-loader','primary')").execute(&pool).await.unwrap();
        let instances = instance_rows::list_instances(&pool).await.unwrap();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].id, "known");
        assert!(
            instance_rows::get_instance_by_id("stage", &pool)
                .await
                .is_err()
        );
        assert!(
            sqlx::query("UPDATE instances SET name='changed' WHERE id='stage'")
                .execute(&pool)
                .await
                .is_err()
        );
        assert!(
            sqlx::query("DELETE FROM instances WHERE id='loader'")
                .execute(&pool)
                .await
                .is_err()
        );
        let mut transaction = pool.begin().await.unwrap();
        assert!(
            loader_component_rows::replace_loader_components(
                "loader",
                &[],
                &mut transaction
            )
            .await
            .is_err()
        );
        transaction.rollback().await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM instance_loader_components WHERE kind='future-loader'").fetch_one(&pool).await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn unknown_provider_is_skipped_and_cannot_be_removed_by_older_cleanup()
     {
        let temporary = tempfile::tempdir().unwrap();
        let pool = databases::open(temporary.path()).await.unwrap();
        instance(&pool, "instance", "installed").await;
        sqlx::query("INSERT INTO instance_content_sets(id,instance_id,name,source_kind,status,game_version,loader,created,modified) VALUES('set','instance','test','local','available','1.21','vanilla',0,0)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO instance_content_entries(id,instance_id,content_set_id,project_type,source_kind,server_requirement,client_requirement,added_at,modified_at,ownership_kind) VALUES('entry','instance','set','mod','local','unknown','unknown',0,0,'local_discovered')").execute(&pool).await.unwrap();
        assert!(unsupported_instances(&pool).await.unwrap().is_empty());
        sqlx::query("INSERT INTO instance_content_provider_refs(content_entry_id,provider,provider_project_id,is_origin) VALUES('entry','modrinth','abcdefgh',0)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO instance_content_provider_refs(content_entry_id,provider,provider_project_id,is_origin) VALUES('entry','future-provider','opaque',1)").execute(&pool).await.unwrap();
        let references =
            content_entries::get_content_provider_refs("entry", &pool)
                .await
                .unwrap();
        assert_eq!(references.len(), 1);
        assert!(
            unsupported_instances(&pool)
                .await
                .unwrap()
                .contains("instance")
        );
        assert!(
            sqlx::query(
                "DELETE FROM instance_content_entries WHERE id='entry'"
            )
            .execute(&pool)
            .await
            .is_err()
        );
        assert!(sqlx::query("DELETE FROM instance_content_provider_refs WHERE content_entry_id='entry'").execute(&pool).await.is_err());
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM instance_content_provider_refs WHERE provider='future-provider'").fetch_one(&pool).await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn unknown_instance_with_json_preferences_does_not_block_startup() {
        let temporary = tempfile::tempdir().unwrap();
        let pool = databases::open(temporary.path()).await.unwrap();
        instance(&pool, "instance", "installed").await;
        instance_rows::set_instance_sync_preference(
            "instance",
            crate::state::SyncedOption::Screenshots,
            true,
            &pool,
        )
        .await
        .unwrap();
        sqlx::query("INSERT INTO instance_loader_components(instance_id,kind,role) VALUES('instance','future-loader','primary')").execute(&pool).await.unwrap();
        pool.close().await;
        drop(pool);
        let pool = databases::open(temporary.path()).await.unwrap();
        assert!(
            unsupported_instances(&pool)
                .await
                .unwrap()
                .contains("instance")
        );
        use crate::state::store::sync_feature_settings::{
            FeatureSettingsKey, FeatureSettingsRecord, SyncFeatureSettings,
        };
        SyncFeatureSettings::set(
            &crate::state::store::Handle::new(pool.clone()),
            &FeatureSettingsKey {
                feature: "screenshots".into(),
            },
            Some(&FeatureSettingsRecord {
                globally_enabled: true,
                new_instance_default: false,
            }),
        )
        .await
        .unwrap();
        let bytes =
            tokio::fs::read(temporary.path().join("settings/sync.json"))
                .await
                .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["data"]["instances"]["instance"]["screenshots"], true);
    }

    #[tokio::test]
    async fn future_file_recovery_is_preserved_without_hiding_launchable_instances()
     {
        use crate::state::store::{
            Collection, Handle, content_file_operations::ContentFileOperations,
        };
        let temporary = tempfile::tempdir().unwrap();
        let pool = databases::open(temporary.path()).await.unwrap();
        instance(&pool, "future", "installed").await;
        instance(&pool, "known", "installed").await;
        let handle = Handle::new(pool.clone());
        let path = temporary.path().join("state/future-record.json");
        let value = serde_json::json!({"payload":"{\"schema_version\":2}","committed":true});
        let hash = crate::state::recovery::record::write(
            &path,
            "content_file_operations",
            "future",
            Some("future"),
            1,
            value,
        )
        .await
        .unwrap();
        sqlx::query("INSERT INTO main.operation_publications(kind,id,instance_id,revision,object_path,sha256,deleted) VALUES('content_file_operations','future','future',1,?,?,0)").bind(path.to_string_lossy().into_owned()).bind(hash).execute(&pool).await.unwrap();
        assert!(
            crate::state::store::recovery_documents::ensure_instance_supported(
                &pool, "future"
            )
            .await
            .is_err()
        );
        assert!(
            crate::state::store::recovery_documents::ensure_instance_supported(
                &pool, "known"
            )
            .await
            .is_ok()
        );
        assert!(
            crate::state::store::recovery_documents::ensure_cleanup_supported(
                &pool
            )
            .await
            .is_err()
        );
        assert!(
            ContentFileOperations::set(&handle, &"future".into(), None)
                .await
                .is_err()
        );
        assert!(
            instance_rows::get_instance_by_id("future", &pool)
                .await
                .unwrap()
                .is_some()
        );
    }
}
