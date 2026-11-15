//! Read-only handover coordination for independent authorities. Account token
//! columns are never selected; shared-log credentials have their own adapter.

use crate::state::accounts::document::Metadata;
use crate::state::persistence::{Persistence, import::Import};
use serde_json::{Map, Value, json};
use sqlx::{Connection, Row, SqliteConnection, sqlite::SqliteConnectOptions};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// These retired domains have no file-retention relationships in the known
/// protocol. Opaque extensions remain in their preserved source database and
/// pause cleanup rather than being interpreted as an empty reference set.
pub(crate) async fn ensure_cleanup_supported(root: &Path) -> crate::Result<()> {
    let persistence = Persistence::new(root);
    for (path, protocol) in [
        ("accounts.json", "axolotl-accounts"),
        ("state/telemetry.json", "axolotl-telemetry"),
        ("state/media.json", "axolotl-media"),
    ] {
        let read = persistence.document(path, protocol, 1)?.read().await?;
        if !matches!(
            read.availability,
            crate::state::persistence::status::Availability::Ready
                | crate::state::persistence::status::Availability::Recovered
        ) {
            return Err(crate::ErrorKind::InputError("Cleanup is paused because independent state cannot be interpreted".into()).into());
        }
        let snapshot = read.snapshot.expect("readable document");
        if protocol == "axolotl-media" {
            let data = snapshot.data().expect("readable payload");
            crate::state::media::validate(data)?;
            for name in ["screenshots", "groups", "textures", "offline_skins"] {
                if data[name]
                    .as_object()
                    .expect("validated map")
                    .values()
                    .any(|value| !crate::state::media::supported(value))
                {
                    return Err(crate::ErrorKind::InputError("Cleanup is paused because media references use a newer protocol".into()).into());
                }
            }
            for saved in data["saved_skins"]
                .as_object()
                .expect("validated map")
                .values()
            {
                if saved.as_object().is_none_or(|entries| {
                    entries
                        .values()
                        .any(|value| !crate::state::media::supported(value))
                }) {
                    return Err(crate::ErrorKind::InputError("Cleanup is paused because skin references cannot be interpreted".into()).into());
                }
            }
        }
        if snapshot
            .data()
            .and_then(|data| data.get("legacy_extensions_present"))
            .and_then(Value::as_bool)
            == Some(true)
        {
            return Err(crate::ErrorKind::InputError("Cleanup is paused because opaque legacy extensions may retain files".into()).into());
        }
    }
    super::legacy_user_state::ensure_cleanup_supported(root).await?;
    Ok(())
}

pub(crate) async fn initialize(root: &Path) {
    super::legacy_user_state::initialize(root, None).await;
    for domain in ["accounts", "telemetry"] {
        if let Err(error) = initialize_domain(root, domain, None).await {
            tracing::warn!(domain, %error, "Independent state import unavailable; preserving legacy files and continuing startup");
        }
    }
    if let Err(error) = super::legacy_media::initialize(root, None).await {
        tracing::warn!(%error, "Media import unavailable; native screenshots remain accessible");
    }
}

pub(crate) async fn initialize_from_snapshot(root: &Path, snapshot: &Path) {
    // Optional imports must be retryable after the temporary migrated image is
    // dropped, even when the core has already switched to its new layout.
    let retry = root.join("databases/legacy-document-import.db");
    if !retry.is_file() {
        let result = async {
            tokio::fs::create_dir_all(root.join("databases")).await?;
            let mut connection =
                crate::state::db_backup::open_read_only_db(snapshot).await?;
            crate::state::db_backup::create_verified_sqlite_snapshot(
                &mut connection,
                &retry,
            )
            .await?;
            connection.close().await?;
            Ok::<_, crate::Error>(())
        }
        .await;
        if let Err(error) = result {
            tracing::warn!(%error,"Optional legacy retry snapshot could not be retained; original source remains preserved");
        }
    }

    super::legacy_user_state::initialize(root, Some(snapshot.to_path_buf()))
        .await;
    if let Err(error) =
        super::records_db::initialize(root, Some(snapshot.to_path_buf())).await
    {
        tracing::warn!(%error, "Independent playtime import unavailable; core startup continues");
    }
    for domain in ["accounts", "telemetry"] {
        if let Err(error) =
            initialize_domain(root, domain, Some(snapshot.to_path_buf())).await
        {
            tracing::warn!(domain, %error, "Optional state snapshot import unavailable; launcher startup continues");
        }
    }
    if let Err(error) =
        super::legacy_media::initialize(root, Some(snapshot.to_path_buf()))
            .await
    {
        tracing::warn!(%error, "Media snapshot import unavailable; preserving the source");
    }
}

async fn initialize_domain(
    root: &Path,
    domain: &'static str,
    snapshot: Option<PathBuf>,
) -> crate::Result<()> {
    let persistence = Persistence::new(root);
    let target = if domain == "accounts" {
        "accounts.json"
    } else {
        "state/telemetry.json"
    };
    let protocol = if domain == "accounts" {
        "axolotl-accounts"
    } else {
        "axolotl-telemetry"
    };
    let document = persistence.document(target, protocol, 1)?;
    let root = root.to_path_buf();
    let result = Import::new(&persistence, domain, document)?
        .run(
            move || async move {
                let connection = if let Some(snapshot) = snapshot {
                    Some(
                        SqliteConnection::connect_with(
                            &SqliteConnectOptions::new()
                                .filename(snapshot)
                                .read_only(true),
                        )
                        .await?,
                    )
                } else {
                    source(&root, domain).await?
                };
                let Some(mut connection) = connection else {
                    return Ok(if domain == "accounts" {
                        Map::from_iter([
                            ("accounts".into(), json!({})),
                            ("default_account_id".into(), Value::Null),
                        ])
                    } else {
                        Map::from_iter([
                            ("installation_id".into(), Value::Null),
                            ("last_heartbeat_day".into(), Value::Null),
                            ("outbox".into(), json!({})),
                        ])
                    });
                };
                let mut data = if domain == "accounts" {
                    accounts(&mut connection).await?
                } else {
                    telemetry(&mut connection).await?
                };
                if !columns(&mut connection, "extension_records")
                    .await?
                    .is_empty()
                {
                    let opaque: bool = sqlx::query_scalar(
                        "SELECT EXISTS(SELECT 1 FROM extension_records)",
                    )
                    .fetch_one(&mut connection)
                    .await?;
                    data.insert(
                        "legacy_extensions_present".into(),
                        json!(opaque),
                    );
                }
                connection.close().await?;
                Ok(data)
            },
            move |snapshot| {
                let data = snapshot.data().ok_or_else(|| {
                    crate::ErrorKind::InputError("Missing state payload".into())
                })?;
                if domain == "accounts" {
                    crate::state::accounts::account_map(data)?;
                } else if !data.get("outbox").is_some_and(Value::is_object) {
                    return Err(crate::ErrorKind::InputError(
                        "Telemetry queue is not an object".into(),
                    )
                    .into());
                }
                Ok(())
            },
        )
        .await?;
    if !result.availability.writable() {
        tracing::warn!(domain, status = ?result.availability, "Independent state is unavailable; old domain remains retired");
    }
    Ok(())
}

pub(super) async fn source(
    root: &Path,
    domain: &str,
) -> crate::Result<Option<SqliteConnection>> {
    let experimental = root.join("databases").join(format!("{domain}.db"));
    let candidates: Vec<PathBuf> = if tokio::fs::try_exists(&experimental)
        .await?
    {
        vec![experimental]
    } else if root
        .join("databases/legacy-document-import.db")
        .try_exists()?
    {
        vec![root.join("databases/legacy-document-import.db")]
    } else if root.join("databases/system.db").try_exists()? {
        // An initialized experimental set never falls back to an older app.db.
        Vec::new()
    } else {
        let channel = crate::state::db::read_update_channel_state(root).await?;
        for name in ["release", "beta"] {
            for marker in ["app.db.replacement.json", ".channel-reconcile"] {
                if tokio::fs::try_exists(root.join(name).join(marker)).await? {
                    return Err(crate::ErrorKind::FSError(
                        "Legacy channel reconciliation has not completed"
                            .into(),
                    )
                    .into());
                }
            }
        }
        let release =
            tokio::fs::try_exists(root.join("release/app.db")).await?;
        let beta = tokio::fs::try_exists(root.join("beta/app.db")).await?;
        let initial = match (release, beta) {
            (true, false) => "release",
            (false, true) => "beta",
            _ => crate::state::db::default_update_channel(),
        };
        let selected = channel.active_channel.as_deref().unwrap_or(initial);
        if !matches!(selected, "release" | "beta") {
            return Err(crate::ErrorKind::InputError(
                "Invalid legacy database channel".into(),
            )
            .into());
        }
        vec![root.join(selected).join("app.db"), root.join("app.db")]
    };
    for path in candidates {
        if !tokio::fs::try_exists(&path).await? {
            continue;
        }
        let mut connection = SqliteConnection::connect_with(
            &SqliteConnectOptions::new().filename(path).read_only(true),
        )
        .await?;
        sqlx::query("BEGIN").execute(&mut connection).await?;
        return Ok(Some(connection));
    }
    Ok(None)
}

pub(super) async fn columns(
    connection: &mut SqliteConnection,
    table: &str,
) -> crate::Result<HashSet<String>> {
    Ok(
        sqlx::query_scalar::<_, String>(
            "SELECT name FROM pragma_table_info(?)",
        )
        .bind(table)
        .fetch_all(connection)
        .await?
        .into_iter()
        .collect(),
    )
}

pub(super) async fn read_source_for(
    root: &Path,
    domain: &str,
    table: &str,
    snapshot: Option<PathBuf>,
) -> crate::Result<Option<SqliteConnection>> {
    let retained = root.join("databases/legacy-document-import.db");
    // Only the old-app handover creates this migrated retry snapshot. Blank
    // baseline domains are not authoritative before the core import commits;
    // afterward it is needed only for a legacy table absent from its domain.
    // Import::run checks the document and receipt before reaching this loader.
    if let Some(path) = snapshot {
        return open_snapshot(path).await.map(Some);
    }
    let active = root.join("databases").join(format!("{domain}.db"));
    if active.is_file() {
        let mut current = source(root, domain).await?.expect("active source");
        if retained.is_file()
            && (!core_import_complete(root).await?
                || columns(&mut current, table).await?.is_empty())
        {
            current.close().await?;
            return open_snapshot(retained).await.map(Some);
        }
        return Ok(Some(current));
    }
    if retained.is_file() {
        return open_snapshot(retained).await.map(Some);
    }
    let connection = source(root, domain).await?;
    if let Some(connection) = connection {
        connection.close().await?;
        return Err(crate::ErrorKind::FSError(
            "Legacy choices await the migrated application snapshot".into(),
        )
        .into());
    }
    Ok(None)
}
async fn open_snapshot(path: PathBuf) -> crate::Result<SqliteConnection> {
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(path).read_only(true),
    )
    .await?;
    sqlx::query("BEGIN").execute(&mut connection).await?;
    Ok(connection)
}
async fn core_import_complete(root: &Path) -> crate::Result<bool> {
    let path = root.join("databases/system.db");
    if !path.is_file() {
        return Ok(false);
    }
    let mut connection = open_snapshot(path).await?;
    let table = !columns(&mut connection, "database_imports")
        .await?
        .is_empty();
    let complete = if table {
        sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM database_imports WHERE source='app.db' AND completed=1)").fetch_one(&mut connection).await?
    } else {
        false
    };
    connection.close().await?;
    Ok(complete)
}

async fn accounts(
    connection: &mut SqliteConnection,
) -> crate::Result<Map<String, Value>> {
    let columns = columns(connection, "minecraft_users").await?;
    if columns.is_empty() {
        return Ok(Map::from_iter([
            ("accounts".into(), json!({})),
            ("default_account_id".into(), Value::Null),
        ]));
    }
    let fields = [
        "account_id",
        "uuid",
        "username",
        "account_type",
        "active",
        "expires",
        "yggdrasil_api_root",
        "yggdrasil_server_name",
        "yggdrasil_login",
        "secrets_externalized",
    ];
    let select = fields
        .iter()
        .map(|field| {
            if columns.contains(*field) {
                format!("\"{field}\"")
            } else {
                let default = match *field {
                    "active" | "expires" | "secrets_externalized" => "0",
                    "account_type" => "'microsoft'",
                    _ => "''",
                };
                format!("{default} AS \"{field}\"")
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {select} FROM minecraft_users"
    )))
    .fetch_all(&mut *connection)
    .await?;
    let mut identities = Map::new();
    let mut selected = Value::Null;
    for row in rows {
        let uuid: String = row.try_get("uuid")?;
        let kind: String = row.try_get("account_type")?;
        let mut id: String = row.try_get("account_id")?;
        if id.is_empty() {
            id = format!("{kind}:{uuid}");
        }
        let mark: i64 = row.try_get("secrets_externalized")?;
        let mut credential_fields: Vec<String> =
            ["access_token", "refresh_token", "yggdrasil_client_token"]
                .into_iter()
                .enumerate()
                .filter_map(|(index, field)| {
                    ((mark & (1 << index)) != 0).then(|| field.to_owned())
                })
                .collect();
        if mark & !7 != 0 {
            credential_fields.push(format!("unsupported-legacy-flags:{mark}"));
        }
        let metadata = Metadata {
            uuid,
            username: row.try_get("username")?,
            account_type: kind,
            expires: row.try_get("expires")?,
            yggdrasil_api_root: row.try_get("yggdrasil_api_root")?,
            yggdrasil_server_name: row.try_get("yggdrasil_server_name")?,
            yggdrasil_login: row.try_get("yggdrasil_login")?,
            credential_fields,
        };
        if row.try_get::<bool, _>("active")? {
            selected = json!(id);
        }
        identities.insert(id, serde_json::to_value(metadata)?);
    }
    Ok(Map::from_iter([
        ("accounts".into(), Value::Object(identities)),
        ("default_account_id".into(), selected),
    ]))
}

async fn telemetry(
    connection: &mut SqliteConnection,
) -> crate::Result<Map<String, Value>> {
    let mut data = Map::from_iter([
        ("installation_id".into(), Value::Null),
        ("last_heartbeat_day".into(), Value::Null),
        ("outbox".into(), json!({})),
    ]);
    if !columns(connection, "telemetry_identity").await?.is_empty() {
        if let Some(row) = sqlx::query("SELECT installation_id,last_heartbeat_day FROM telemetry_identity WHERE id=0").fetch_optional(&mut *connection).await? {
            data.insert("installation_id".into(), json!(row.try_get::<String, _>("installation_id")?));
            data.insert("last_heartbeat_day".into(), json!(row.try_get::<Option<String>, _>("last_heartbeat_day")?));
        }
    }
    if !columns(connection, "telemetry_outbox").await?.is_empty() {
        let rows = sqlx::query("SELECT event_id,event_type,json(payload) AS payload,created_at,next_attempt_at,attempts,size_bytes,dedupe_key,occurrence_count FROM telemetry_outbox ORDER BY created_at DESC,event_id DESC LIMIT 100")
            .fetch_all(&mut *connection).await?;
        let outbox = data
            .get_mut("outbox")
            .and_then(Value::as_object_mut)
            .expect("queue object");
        let mut total = 0;
        for row in rows {
            let payload: String = row.try_get("payload")?;
            total += payload.len();
            if total > 2 * 1024 * 1024 {
                break;
            }
            outbox.insert(row.try_get("event_id")?, json!({
                "event_type":row.try_get::<String,_>("event_type")?, "payload":payload,
                "created_at":row.try_get::<i64,_>("created_at")?, "next_attempt_at":row.try_get::<i64,_>("next_attempt_at")?,
                "attempts":row.try_get::<i64,_>("attempts")?, "size_bytes":row.try_get::<i64,_>("size_bytes")?,
                "dedupe_key":row.try_get::<String,_>("dedupe_key")?, "occurrence_count":row.try_get::<i64,_>("occurrence_count")?
            }));
        }
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn legacy(root: &Path) -> (PathBuf, sqlx::SqlitePool) {
        let file = root.join("app.db");
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&file)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::query("CREATE TABLE minecraft_users(account_id TEXT,uuid TEXT,username TEXT,account_type TEXT,active INTEGER,expires INTEGER,secrets_externalized INTEGER,access_token TEXT,refresh_token TEXT)")
            .execute(&pool).await.unwrap();
        (file, pool)
    }

    #[tokio::test]
    async fn account_import_copies_metadata_and_key_names_but_never_plaintext_tokens()
     {
        let root = tempfile::tempdir().unwrap();
        let (file, pool) = legacy(root.path()).await;
        sqlx::query("INSERT INTO minecraft_users VALUES('microsoft:test','00000000-0000-4000-8000-000000000001','Player','microsoft',1,0,0,'legacy-secret-access','legacy-secret-refresh')")
            .execute(&pool).await.unwrap();
        pool.close().await;
        initialize_from_snapshot(root.path(), &file).await;
        let bytes =
            tokio::fs::read_to_string(root.path().join("accounts.json"))
                .await
                .unwrap();
        assert!(!bytes.contains("legacy-secret"));
        let raw: Value = serde_json::from_str(&bytes).unwrap();
        assert_eq!(raw["data"]["default_account_id"], "microsoft:test");
        assert_eq!(
            raw["data"]["accounts"]["microsoft:test"]["username"],
            "Player"
        );
        assert_eq!(
            raw["data"]["accounts"]["microsoft:test"]["credential_fields"],
            json!([])
        );
        assert!(
            root.path()
                .join("state/persistence/imports/accounts.json")
                .exists()
        );
        let reopened = crate::state::accounts::Accounts::new(
            &Persistence::new(root.path()),
        );
        assert_eq!(
            reopened.get_all().await.unwrap()[0].2,
            crate::state::minecraft_auth::CredentialPersistence::RequiresLogin
        );
    }

    #[tokio::test]
    async fn externalized_tokens_keep_the_same_keyring_keys_without_reading_plaintext()
     {
        let host = crate::state::store::secrets::testing::with_store();
        let root = tempfile::tempdir().unwrap();
        let (file, pool) = legacy(root.path()).await;
        sqlx::query("INSERT INTO minecraft_users VALUES('microsoft:test','00000000-0000-4000-8000-000000000001','Player','microsoft',1,0,3,'ignored-secret','ignored-secret')")
            .execute(&pool).await.unwrap();
        pool.close().await;
        let key = super::super::secrets::key(&[
            "minecraft-user",
            "microsoft:test",
            "access_token",
        ]);
        host.store().unwrap().put(&key, "keyring-access");
        let key = super::super::secrets::key(&[
            "minecraft-user",
            "microsoft:test",
            "refresh_token",
        ]);
        host.store().unwrap().put(&key, "keyring-refresh");
        initialize_from_snapshot(root.path(), &file).await;
        let accounts = crate::state::accounts::Accounts::new(
            &Persistence::new(root.path()),
        );
        let list = accounts.get_all().await.unwrap();
        assert_eq!(list[0].1.access_token, "keyring-access");
        assert_eq!(list[0].1.refresh_token, "keyring-refresh");
    }

    #[tokio::test]
    async fn old_schema_without_token_columns_still_imports_identity() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("app.db");
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&file)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::query("CREATE TABLE minecraft_users(uuid TEXT,username TEXT,active INTEGER,expires INTEGER)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO minecraft_users VALUES('00000000-0000-4000-8000-000000000001','Player',1,0)").execute(&pool).await.unwrap();
        pool.close().await;
        initialize_from_snapshot(root.path(), &file).await;
        let raw: Value = serde_json::from_slice(
            &tokio::fs::read(root.path().join("accounts.json"))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            raw["data"]["accounts"]["microsoft:00000000-0000-4000-8000-000000000001"]
                ["username"],
            "Player"
        );
    }

    #[tokio::test]
    async fn corrupt_retired_domain_files_do_not_abort_core_database_open() {
        let root = tempfile::tempdir().unwrap();
        let pool = crate::state::store::databases::open(root.path())
            .await
            .unwrap();
        pool.close().await;
        drop(pool);
        for (domain, document) in [
            ("accounts", "accounts.json"),
            ("telemetry", "state/telemetry.json"),
        ] {
            tokio::fs::remove_file(root.path().join(document))
                .await
                .unwrap();
            tokio::fs::remove_file(
                root.path()
                    .join(format!("state/persistence/imports/{domain}.json")),
            )
            .await
            .unwrap();
            tokio::fs::write(
                root.path().join(format!("databases/{domain}.db")),
                b"corrupt legacy domain",
            )
            .await
            .unwrap();
        }
        let pool = crate::state::store::databases::open(root.path())
            .await
            .unwrap();
        let domains: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_database_list")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(domains.iter().any(|name| name == "instances"));
        assert!(
            !domains
                .iter()
                .any(|name| name == "accounts" || name == "telemetry")
        );
        for domain in ["accounts", "telemetry"] {
            assert_eq!(
                tokio::fs::read(
                    root.path().join(format!("databases/{domain}.db"))
                )
                .await
                .unwrap(),
                b"corrupt legacy domain"
            );
        }
        pool.close().await;
    }
}
