use crate::state::{
    layout,
    persistence::{Persistence, status::Availability},
};
use futures::TryStreamExt;
use serde_json::json;
use sqlx::{
    Connection, SqliteConnection, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, Weak},
    time::Duration,
};
use tokio::sync::OnceCell;

pub(super) const FILE: &str = "resources.db";
pub(super) const RECEIPT: &str = "data/persistence/resources.json";
pub(super) const PREVIOUS_RECEIPT: &str =
    "data/persistence/resources.json.previous";
const PROTOCOL: &str = "axolotl-resource-identities";
const APPLICATION_ID: i64 = 0x4158_5249;
const CONTRACT: i64 = 1;

const SCHEMA: &str = "
CREATE TABLE resources (
    id TEXT PRIMARY KEY NOT NULL,
    redirect_to TEXT REFERENCES resources(id),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    CHECK (redirect_to IS NULL OR redirect_to <> id)
);
CREATE TABLE resource_source_projects (
    source TEXT NOT NULL,
    project_id TEXT NOT NULL,
    resource_id TEXT NOT NULL DEFAULT (lower(hex(randomblob(16)))),
    PRIMARY KEY (source, project_id),
    FOREIGN KEY (resource_id) REFERENCES resources(id) DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX resource_source_projects_resource ON resource_source_projects(resource_id);
CREATE TABLE resource_source_types (
    source TEXT NOT NULL,
    project_id TEXT NOT NULL,
    content_type TEXT NOT NULL,
    PRIMARY KEY (source, project_id, content_type),
    FOREIGN KEY (source, project_id) REFERENCES resource_source_projects(source, project_id)
);
CREATE TABLE resource_aliases (
    source TEXT NOT NULL,
    alias TEXT NOT NULL,
    project_id TEXT NOT NULL,
    PRIMARY KEY (source, alias, project_id),
    FOREIGN KEY (source, project_id) REFERENCES resource_source_projects(source, project_id)
);";

fn unavailable(message: &str) -> crate::Error {
    crate::ErrorKind::FSError(message.into()).into()
}

#[derive(PartialEq, Eq)]
enum Handover {
    Pending,
    Available,
    Excluded,
}

async fn handover(root: &Path) -> crate::Result<Handover> {
    let present = |path: PathBuf| async move {
        match tokio::fs::symlink_metadata(path).await {
            Ok(metadata) if metadata.is_file() => Ok(true),
            Ok(_) => Err(unavailable(
                "Resource identity handover is not a regular file",
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(false)
            }
            Err(error) => Err(error.into()),
        }
    };
    if !present(root.join(RECEIPT)).await?
        && !present(root.join(PREVIOUS_RECEIPT)).await?
    {
        return Ok(Handover::Pending);
    }
    let receipt = Persistence::new(root).document(RECEIPT, PROTOCOL, 1)?;
    let read = receipt.read().await?;
    if matches!(
        read.availability,
        Availability::Ready | Availability::Recovered
    ) && let Some(payload) =
        read.snapshot.as_ref().and_then(|snapshot| snapshot.data())
        && payload.get("file").and_then(serde_json::Value::as_str) == Some(FILE)
    {
        match payload
            .get("available")
            .and_then(serde_json::Value::as_bool)
        {
            Some(true) => return Ok(Handover::Available),
            Some(false) => return Ok(Handover::Excluded),
            None => {}
        }
    }
    Err(unavailable(
        "Resource identity handover is unreadable or unsupported",
    ))
}

async fn ensure_available(root: &Path) -> crate::Result<()> {
    match handover(root).await? {
        Handover::Available => Ok(()),
        Handover::Pending => {
            Err(unavailable("Resource identity handover is incomplete"))
        }
        Handover::Excluded => Err(unavailable(
            "Resource identities were excluded from the restored snapshot",
        )),
    }
}

async fn write_receipt(root: &Path, available: bool) -> crate::Result<()> {
    Persistence::new(root)
        .document(RECEIPT, PROTOCOL, 1)?
        .update(move |payload| {
            payload.insert("file".into(), json!(FILE));
            payload.insert("available".into(), json!(available));
            Ok(())
        })
        .await?;
    Ok(())
}

pub(super) async fn stage_excluded_receipt(staged: &Path) -> crate::Result<()> {
    write_receipt(staged, false).await
}

async fn options(path: &Path) -> crate::Result<SqliteConnectOptions> {
    let path = crate::state::db_backup::sqlite_native_path(path).await?;
    Ok(SqliteConnectOptions::new()
        .filename(path.as_ref())
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5)))
}

async fn validate_connection(
    connection: &mut SqliteConnection,
) -> crate::Result<()> {
    let id: i64 = sqlx::query_scalar("PRAGMA application_id")
        .fetch_one(&mut *connection)
        .await?;
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *connection)
        .await?;
    if (id, version) != (APPLICATION_ID, CONTRACT) {
        return Err(unavailable(
            "Resource identities use an unsupported storage protocol",
        ));
    }
    for query in [
        "SELECT id,redirect_to,created_at FROM resources LIMIT 0",
        "SELECT source,project_id,resource_id FROM resource_source_projects LIMIT 0",
        "SELECT source,project_id,content_type FROM resource_source_types LIMIT 0",
        "SELECT source,alias,project_id FROM resource_aliases LIMIT 0",
    ] {
        sqlx::query(sqlx::AssertSqlSafe(query))
            .execute(&mut *connection)
            .await?;
    }
    Ok(())
}

async fn validate_rows(connection: &mut SqliteConnection) -> crate::Result<()> {
    let integrity: Vec<String> = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_all(&mut *connection)
        .await?;
    if integrity != ["ok"]
        || sqlx::query("PRAGMA foreign_key_check")
            .fetch_optional(connection)
            .await?
            .is_some()
    {
        return Err(unavailable(
            "Resource identity database failed its integrity checks",
        ));
    }
    Ok(())
}

pub(crate) async fn initialize(
    root: &Path,
    snapshot: Option<&Path>,
) -> crate::Result<()> {
    let data = layout::data(root);
    let path = data.join(FILE);
    super::recovery_mode::ensure_path_writable(&path)?;
    if handover(root).await? != Handover::Pending {
        return Ok(());
    }
    if snapshot.is_none()
        && ["app.db", "release/app.db", "beta/app.db"]
            .iter()
            .any(|file| root.join(file).is_file())
    {
        return Err(unavailable(
            "Resource identity import is awaiting its migrated source snapshot",
        ));
    }
    tokio::fs::create_dir_all(&data).await?;
    let mut connection = SqliteConnection::connect_with(
        &options(&path).await?.create_if_missing(true),
    )
    .await?;
    let mut transaction = connection.begin_with("BEGIN IMMEDIATE").await?;
    let id: i64 = sqlx::query_scalar("PRAGMA application_id")
        .fetch_one(&mut *transaction)
        .await?;
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *transaction)
        .await?;
    let objects: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sqlite_schema)")
            .fetch_one(&mut *transaction)
            .await?;
    if (id, version, objects) == (0, 0, false) {
        sqlx::raw_sql(SCHEMA).execute(&mut *transaction).await?;
        if let Some(snapshot) = snapshot {
            import(&mut transaction, snapshot).await?;
        }
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "PRAGMA application_id = {APPLICATION_ID}"
        )))
        .execute(&mut *transaction)
        .await?;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "PRAGMA user_version = {CONTRACT}"
        )))
        .execute(&mut *transaction)
        .await?;
    }
    validate_connection(&mut transaction).await?;
    validate_rows(&mut transaction).await?;
    transaction.commit().await?;
    connection.close().await?;
    write_receipt(root, true).await
}

async fn import(
    destination: &mut SqliteConnection,
    snapshot: &Path,
) -> crate::Result<()> {
    let mut source =
        crate::state::db_backup::open_read_only_db(snapshot).await?;
    let mut rows = sqlx::query_as::<_, (String, Option<String>, i64)>(
        "SELECT id,redirect_to,created_at FROM resources",
    )
    .fetch(&mut source);
    sqlx::query("PRAGMA defer_foreign_keys = ON")
        .execute(&mut *destination)
        .await?;
    while let Some((id, redirect, timestamp)) = rows.try_next().await? {
        sqlx::query(
            "INSERT INTO resources(id,redirect_to,created_at) VALUES(?,?,?)",
        )
        .bind(id)
        .bind(redirect)
        .bind(timestamp)
        .execute(&mut *destination)
        .await?;
    }
    drop(rows);
    for (select, insert) in [
        (
            "SELECT source,project_id,resource_id FROM resource_source_projects",
            "INSERT INTO resource_source_projects(source,project_id,resource_id) VALUES(?,?,?)",
        ),
        (
            "SELECT source,project_id,content_type FROM resource_source_types",
            "INSERT INTO resource_source_types(source,project_id,content_type) VALUES(?,?,?)",
        ),
        (
            "SELECT source,alias,project_id FROM resource_aliases ORDER BY rowid",
            "INSERT INTO resource_aliases(source,alias,project_id) VALUES(?,?,?)",
        ),
    ] {
        let mut rows = sqlx::query_as::<_, (String, String, String)>(
            sqlx::AssertSqlSafe(select),
        )
        .fetch(&mut source);
        while let Some((first, second, third)) = rows.try_next().await? {
            sqlx::query(sqlx::AssertSqlSafe(insert))
                .bind(first)
                .bind(second)
                .bind(third)
                .execute(&mut *destination)
                .await?;
        }
    }
    source.close().await?;
    Ok(())
}

async fn open(root: &Path, read_only: bool) -> crate::Result<SqlitePool> {
    ensure_available(root).await?;
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(
            options(&layout::data(root).join(FILE))
                .await?
                .read_only(read_only),
        )
        .await?;
    let mut connection = pool.acquire().await?;
    validate_connection(&mut connection).await?;
    validate_rows(&mut connection).await?;
    drop(connection);
    Ok(pool)
}

pub(crate) async fn pool_for(core: &SqlitePool) -> crate::Result<SqlitePool> {
    if let Some(root) = super::recovery_mode::root(core) {
        return open(&root, true).await;
    }
    let core_options = core.connect_options();
    if core_options
        .get_filename()
        .file_name()
        .is_none_or(|file| file != "system.db")
    {
        #[cfg(test)]
        return Ok(core.clone());
        #[cfg(not(test))]
        return Err(unavailable(
            "Resource identities need the launcher core handle",
        ));
    }
    type Entry = (Weak<SqliteConnectOptions>, Arc<OnceCell<SqlitePool>>);
    static POOLS: OnceLock<Mutex<HashMap<usize, Entry>>> = OnceLock::new();
    let cell = {
        let mut pools = POOLS
            .get_or_init(Mutex::default)
            .lock()
            .expect("resource identity pools");
        pools.retain(|_, (owner, _)| owner.strong_count() != 0);
        pools
            .entry(Arc::as_ptr(&core_options) as usize)
            .or_insert_with(|| {
                (Arc::downgrade(&core_options), Arc::new(OnceCell::new()))
            })
            .1
            .clone()
    };
    let root = super::persistence_for_pool(core);
    let pool = cell.get_or_try_init(|| open(root.root(), false)).await?;
    Ok(pool.clone())
}

pub(super) async fn backup(root: &Path, staging: &Path) -> crate::Result<()> {
    ensure_available(root).await?;
    let bytes = tokio::fs::read(root.join(RECEIPT)).await?;
    let mut source = SqliteConnection::connect_with(
        &options(&layout::data(root).join(FILE))
            .await?
            .read_only(true),
    )
    .await?;
    validate_connection(&mut source).await?;
    crate::state::db_backup::create_verified_sqlite_snapshot(
        &mut source,
        &staging.join(FILE),
    )
    .await?;
    source.close().await?;
    crate::state::persistence::document::write_blob(
        &staging.join(RECEIPT),
        bytes,
    )
    .await
}

pub(super) async fn validate_backup(staging: &Path) -> crate::Result<()> {
    ensure_available(staging).await?;
    let mut connection = SqliteConnection::connect_with(
        &options(&staging.join(FILE)).await?.read_only(true),
    )
    .await?;
    validate_connection(&mut connection).await?;
    validate_rows(&mut connection).await?;
    connection.close().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{
        ResourceType, SourceId, SourceProjectId, SourceProjectRef,
    };
    use crate::state::resources::ResourceStore;

    #[tokio::test]
    async fn imported_identities_are_independent_and_do_not_restore_retired_data()
     {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        let snapshot = root.join("legacy-fixture.db");
        let mut connection = SqliteConnection::connect_with(
            &options(&snapshot).await.unwrap().create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::raw_sql(SCHEMA)
            .execute(&mut connection)
            .await
            .unwrap();
        sqlx::raw_sql("
            INSERT INTO resources VALUES ('kept',NULL,0),('old','kept',1);
            INSERT INTO resource_source_projects VALUES ('modrinth','abcdefgh','kept'),('curseforge','17','kept');
            INSERT INTO resource_source_types VALUES ('modrinth','abcdefgh','mod');
            INSERT INTO resource_aliases VALUES ('modrinth','a-mod','abcdefgh');
            CREATE TABLE resource_identity_evidence (payload TEXT);
            INSERT INTO resource_identity_evidence VALUES ('not an authority');
        ").execute(&mut connection).await.unwrap();
        connection.close().await.unwrap();
        let source = tokio::fs::read(&snapshot).await.unwrap();

        let data = layout::data(root);
        tokio::fs::create_dir_all(&data).await.unwrap();
        tokio::fs::write(data.join(FILE), b"damaged").await.unwrap();
        assert!(initialize(root, Some(&snapshot)).await.is_err());
        assert_eq!(tokio::fs::read(data.join(FILE)).await.unwrap(), b"damaged");
        assert!(!root.join(RECEIPT).exists());
        tokio::fs::remove_file(data.join(FILE)).await.unwrap();
        initialize(root, Some(&snapshot)).await.unwrap();
        let pool = open(root, false).await.unwrap();
        let projects: Vec<(String, String)> = sqlx::query_as(
            "SELECT source,resource_id FROM resource_source_projects ORDER BY source",
        ).fetch_all(&pool).await.unwrap();
        assert_eq!(
            projects,
            vec![
                ("curseforge".to_owned(), "kept".to_owned()),
                ("modrinth".to_owned(), "kept".to_owned())
            ]
        );
        let redirected: String = sqlx::query_scalar(
            "SELECT redirect_to FROM resources WHERE id='old'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(redirected, "kept");
        let evidence: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_schema WHERE name='resource_identity_evidence'").fetch_one(&pool).await.unwrap();
        assert_eq!(evidence, 0);
        pool.close().await;

        let staging = tempfile::tempdir().unwrap();
        backup(root, staging.path()).await.unwrap();
        validate_backup(staging.path()).await.unwrap();
        tokio::fs::remove_file(data.join(FILE)).await.unwrap();
        initialize(root, Some(&snapshot)).await.unwrap();
        assert!(open(root, false).await.is_err());
        assert!(!data.join(FILE).exists());
        assert_eq!(tokio::fs::read(&snapshot).await.unwrap(), source);
    }

    #[tokio::test]
    async fn creating_a_project_does_not_wait_for_a_core_write_reservation() {
        let temporary = tempfile::tempdir().unwrap();
        let core = super::super::databases::open(temporary.path())
            .await
            .unwrap();
        let reserved = core.begin_with("BEGIN IMMEDIATE").await.unwrap();
        let project = SourceProjectRef {
            source: SourceId::modrinth(),
            project_id: SourceProjectId::new("abcdefgh").unwrap(),
        };
        let store = ResourceStore::new(core.clone());
        let identity = tokio::time::timeout(
            Duration::from_secs(2),
            store.ensure_project(&project, &[ResourceType::Mod]),
        )
        .await
        .unwrap()
        .unwrap();
        let found = store.lookup(&project).await.unwrap();
        assert_eq!(found, Some(identity));
        reserved.rollback().await.unwrap();
        let tables: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM instances.sqlite_schema WHERE name IN ('resources','resource_identity_evidence','resource_source_projects')",
        ).fetch_one(&core).await.unwrap();
        assert_eq!(tables, 0);
        pool_for(&core).await.unwrap().close().await;
        core.close().await;
    }

    #[tokio::test]
    async fn a_future_resource_protocol_leaves_installed_references_available()
    {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        let core = super::super::databases::open(root).await.unwrap();
        sqlx::raw_sql("
            INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified)
                VALUES('instance','game','installed','none','Game',0,0);
            INSERT INTO instance_content_sets(id,instance_id,name,source_kind,status,game_version,loader,created,modified)
                VALUES('set','instance','Local','local','available','1.21','vanilla',0,0);
            UPDATE instances SET applied_content_set_id='set' WHERE id='instance';
            INSERT INTO instance_files(id,instance_id,relative_path,file_name,enabled,sha1,size,missing,added_at,modified_at)
                VALUES('file','instance','mods/mod.jar','mod.jar',1,'hash',10,0,0,0);
            INSERT INTO instance_content_entries(id,instance_id,content_set_id,file_id,project_type,source_kind,server_requirement,client_requirement,enabled,added_at,modified_at,ownership_kind)
                VALUES('entry','instance','set','file','mod','modrinth','unknown','unknown',1,0,0,'user_added');
            INSERT INTO instance_content_provider_refs(content_entry_id,provider,provider_project_id,provider_release_id,provider_file_id,is_origin)
                VALUES('entry','modrinth','abcdefgh','version1','file1',1);
        ").execute(&core).await.unwrap();
        let path = layout::data(root).join(FILE);
        let mut connection =
            SqliteConnection::connect_with(&options(&path).await.unwrap())
                .await
                .unwrap();
        sqlx::query("PRAGMA user_version = 100")
            .execute(&mut connection)
            .await
            .unwrap();
        connection.close().await.unwrap();
        let original = tokio::fs::read(&path).await.unwrap();
        let store = ResourceStore::new(core.clone());
        let project = SourceProjectRef {
            source: SourceId::modrinth(),
            project_id: SourceProjectId::new("abcdefgh").unwrap(),
        };
        assert!(
            store
                .ensure_project(&project, &[ResourceType::Mod])
                .await
                .is_err()
        );
        assert!(store.entry_resources("instance").await.unwrap().is_empty());
        let links = super::super::resource_entry_links::for_instance(
            &super::super::Handle::new(core.clone()),
            "instance",
        )
        .await
        .unwrap();
        assert_eq!(links.len(), 1);
        let installed = &links[0];
        assert!(installed.resource_id.is_none());
        assert_eq!(installed.project_id, "abcdefgh");
        assert_eq!(installed.release_id.as_deref(), Some("version1"));
        assert_eq!(installed.artifact_id.as_deref(), Some("file1"));
        assert_eq!(tokio::fs::read(path).await.unwrap(), original);
        core.close().await;
    }
}
