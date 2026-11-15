//! Read-only handover of user choices, independently per authority.
use super::legacy_documents::{columns, read_source_for};
use crate::state::persistence::{Persistence, import::Import};
use crate::state::user_state::{self, Domain, key};
use serde_json::{Map, Value, json};
use sqlx::{Connection, Row, SqliteConnection, sqlite::SqliteConnectOptions};
use std::path::{Path, PathBuf};

pub(crate) async fn initialize(root: &Path, snapshot: Option<PathBuf>) {
    for (name, domain, protocol) in [
        ("worlds", "records", "axolotl-world-choices"),
        ("joins", "records", "axolotl-server-joins"),
        ("shared_logs", "records", "axolotl-shared-logs"),
        (
            "resource_favorites",
            "storage",
            "axolotl-resource-favorites",
        ),
        ("instance_pins", "instances", "axolotl-instance-pins"),
    ] {
        if let Err(error) =
            initialize_domain(root, name, domain, protocol, snapshot.clone())
                .await
        {
            tracing::warn!(name,%error,"Optional user-state import unavailable; core startup continues");
        }
    }
}
async fn initialize_domain(
    root: &Path,
    name: &'static str,
    domain: &'static str,
    protocol: &'static str,
    snapshot: Option<PathBuf>,
) -> crate::Result<()> {
    let persistence = Persistence::new(root);
    let target = Domain::new(&persistence, name, protocol);
    let source_root = root.to_owned();
    let access = super::secrets::access();
    Import::new(&persistence, name, target.document)?
        .run(
            move || async move {
                let connection = read_source_for(
                    &source_root,
                    domain,
                    match name {
                        "worlds" => "attached_world_data",
                        "joins" => "join_log",
                        "instance_pins" => "instances",
                        other => other,
                    },
                    snapshot,
                )
                .await?;
                let mut data = Map::from_iter([("entries".into(), json!({}))]);
                if let Some(mut connection) = connection {
                    load(
                        &mut connection,
                        &persistence,
                        name,
                        access,
                        &mut data,
                    )
                    .await?;
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
                }
                Ok(data)
            },
            |snapshot| {
                user_state::validate(snapshot.data().ok_or_else(|| {
                    crate::ErrorKind::InputError(
                        "Missing user-state payload".into(),
                    )
                })?)
            },
        )
        .await?;
    Ok(())
}
async fn load(
    connection: &mut SqliteConnection,
    persistence: &Persistence,
    name: &str,
    access: super::secrets::Access,
    data: &mut Map<String, Value>,
) -> crate::Result<()> {
    let entries = data
        .get_mut("entries")
        .and_then(Value::as_object_mut)
        .expect("entries");
    let table = match name {
        "worlds" => "attached_world_data",
        "instance_pins" => "instances",
        "joins" => "join_log",
        other => other,
    };
    let schema = columns(connection, table).await?;
    if schema.is_empty() {
        return Ok(());
    }
    match name {
        "worlds"=>{
            let project=if schema.contains("project_id"){"project_id"}else{"NULL"};let content=if schema.contains("content_kind"){"content_kind"}else{"NULL"};
            let query=format!("SELECT instance_id,world_type,world_id,display_status,{project} AS project_id,{content} AS content_kind FROM attached_world_data");
            for row in sqlx::query(sqlx::AssertSqlSafe(query)).fetch_all(&mut *connection).await? {
                let instance:String=row.try_get("instance_id")?;let kind:String=row.try_get("world_type")?;let world:String=row.try_get("world_id")?;
                entries.insert(key(&[&instance,&kind,&world]),json!({"instance_id":instance,"world_type":kind,"world_id":world,"display_status":row.try_get::<String,_>("display_status")?,"project_id":row.try_get::<Option<String>,_>("project_id")?,"content_kind":row.try_get::<Option<String>,_>("content_kind")?}));
            }
        }
        "joins"=>{
            for(instance,host,port,time)in sqlx::query_as::<_,(String,String,i64,i64)>("SELECT instance_id,host,port,join_time FROM join_log").fetch_all(&mut *connection).await? {
                entries.insert(key(&[&instance,&host,&port.to_string()]),json!({"instance_id":instance,"host":host,"port":port,"join_time":time}));
            }
        }
        "resource_favorites"=>{
            for(source,project,kind,time)in sqlx::query_as::<_,(String,String,String,i64)>("SELECT source,project_id,content_type,saved_at FROM resource_favorites").fetch_all(&mut *connection).await? {
                entries.insert(key(&[&source,&project]),json!({"source":source,"project_id":project,"content_type":kind,"saved_at":time}));
            }
        }
        "instance_pins"=>{
            if schema.contains("pinned_at") {
                for(instance,time)in sqlx::query_as::<_,(String,i64)>("SELECT id,pinned_at FROM instances WHERE pinned_at IS NOT NULL").fetch_all(&mut *connection).await? {entries.insert(instance.clone(),json!({"instance_id":instance,"pinned_at":time}));}
            }
        }
        "shared_logs"=>{
            let mut tokens=Vec::new();
            for row in sqlx::query("SELECT id,url,raw,token,provider,instance_id,instance_name,truncated,created_at FROM shared_logs").fetch_all(&mut *connection).await? {
                let id:String=row.try_get("id")?;let token:String=row.try_get("token")?;
                let token_ref=if token.is_empty(){None}else {let reference=user_state::shared_logs::new_token_ref(persistence,&id);tokens.push((reference.clone(),token));Some(reference)};
                entries.insert(id,json!({"url":row.try_get::<String,_>("url")?,"raw":row.try_get::<String,_>("raw")?,"provider":row.try_get::<String,_>("provider")?,"instance_id":row.try_get::<Option<String>,_>("instance_id")?,"instance_name":row.try_get::<Option<String>,_>("instance_name")?,"truncated":row.try_get::<bool,_>("truncated")?,"created_at":row.try_get::<i64,_>("created_at")?,"token_ref":token_ref}));
            }
            for(reference,token)in tokens {user_state::shared_logs::keep_token(access,&reference,&token);}
        }
        _=>unreachable!("constant domain"),
    }
    Ok(())
}

pub(super) async fn ensure_cleanup_supported(root: &Path) -> crate::Result<()> {
    let persistence = Persistence::new(root);
    for (name, protocol) in [
        ("worlds", "axolotl-world-choices"),
        ("joins", "axolotl-server-joins"),
        ("shared_logs", "axolotl-shared-logs"),
        ("resource_favorites", "axolotl-resource-favorites"),
        ("instance_pins", "axolotl-instance-pins"),
    ] {
        let data = Domain::new(&persistence, name, protocol).read().await?;
        if data
            .get("legacy_extensions_present")
            .and_then(Value::as_bool)
            == Some(true)
            || data["entries"]
                .as_object()
                .expect("entries")
                .values()
                .any(|raw| !user_state::supported(raw))
        {
            return Err(crate::ErrorKind::InputError("Cleanup paused because user-state references use an unsupported protocol".into()).into());
        }
    }
    Ok(())
}

/// Legacy SQL fixtures opt into a one-time handover under their stable pool root.
#[cfg(test)]
pub(crate) async fn initialize_test_pool(pool: &sqlx::SqlitePool) {
    let persistence = super::persistence_for_pool(pool);
    let snapshot = persistence.root().join("legacy-test-fixture.db");
    let mut connection = pool.acquire().await.expect("fixture connection");
    crate::state::db_backup::create_verified_sqlite_snapshot(
        &mut connection,
        &snapshot,
    )
    .await
    .expect("fixture snapshot");
    drop(connection);
    initialize(persistence.root(), Some(snapshot.clone())).await;
    let _ = tokio::fs::remove_file(snapshot).await;
}
