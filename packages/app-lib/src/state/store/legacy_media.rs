//! One-time read-only media snapshot. Original databases remain untouched.

use super::legacy_documents::columns;
use crate::state::media::{Media, empty, validate};
use crate::state::persistence::{
    Persistence, document::write_blob, import::Import,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Connection, Row, SqliteConnection};
use std::path::{Path, PathBuf};

pub(crate) async fn initialize(
    root: &Path,
    snapshot: Option<PathBuf>,
) -> crate::Result<()> {
    let persistence = Persistence::new(root);
    let media = Media::new(&persistence);
    let root = root.to_owned();
    Import::new(&persistence, "media", media.document.clone())?
        .run(
            move || async move {
                let connection = super::legacy_documents::read_source_for(
                    &root,
                    "media",
                    "screenshots",
                    snapshot,
                )
                .await?;
                let mut data = empty();
                if let Some(mut connection) = connection {
                    load(&mut connection, &media, &mut data).await?;
                    connection.close().await?;
                }
                Ok(data)
            },
            |snapshot| {
                validate(snapshot.data().ok_or_else(|| {
                    crate::ErrorKind::InputError("Missing media payload".into())
                })?)
            },
        )
        .await?;
    Ok(())
}

async fn load(
    connection: &mut SqliteConnection,
    media: &Media,
    data: &mut Map<String, Value>,
) -> crate::Result<()> {
    let screenshot_columns = columns(connection, "screenshots").await?;
    if !screenshot_columns.is_empty() {
        let membership = !columns(connection, "screenshot_group_memberships")
            .await?
            .is_empty();
        let group = if membership {
            "(SELECT group_id FROM screenshot_group_memberships WHERE screenshot_id=s.id)"
        } else {
            "NULL"
        };
        let editor = if screenshot_columns.contains("editor_state") {
            "s.editor_state"
        } else {
            "NULL"
        };
        let query = format!(
            "SELECT s.id,s.instance_id,s.file_name,s.content_hash,s.created_at,{group} AS group_id,{editor} AS editor_state FROM screenshots s"
        );
        for row in sqlx::query(sqlx::AssertSqlSafe(query))
            .fetch_all(&mut *connection)
            .await?
        {
            let id: String = row.try_get("id")?;
            let mut identity = json!({
                "id": id, "instance_id": row.try_get::<String,_>("instance_id")?,
                "file_name": row.try_get::<String,_>("file_name")?,
                "fingerprint": row.try_get::<String,_>("content_hash")?,
                "authored_at": row.try_get::<i64,_>("created_at")?,
                "group_id": row.try_get::<Option<String>,_>("group_id")?,
            });
            // No runtime editor model. Retain the exact opaque legacy value.
            if let Some(editor) =
                row.try_get::<Option<String>, _>("editor_state")?
            {
                if !editor.is_empty() {
                    identity["legacy_editor_state"] = json!(editor);
                }
            }
            data["screenshots"]
                .as_object_mut()
                .expect("empty map")
                .insert(id, identity);
        }
    }
    if !columns(connection, "screenshot_groups").await?.is_empty() {
        for (id, name, order) in sqlx::query_as::<_, (String, String, i64)>(
            "SELECT id,name,display_order FROM screenshot_groups",
        )
        .fetch_all(&mut *connection)
        .await?
        {
            data["groups"]
                .as_object_mut()
                .expect("empty map")
                .insert(id, json!({"name":name,"display_order":order}));
        }
    }
    if !columns(connection, "custom_minecraft_skin_textures")
        .await?
        .is_empty()
    {
        let rows = sqlx::query_as::<_, (String, Vec<u8>)>(
            "SELECT texture_key,texture FROM custom_minecraft_skin_textures",
        )
        .fetch_all(&mut *connection)
        .await?;
        for (key, bytes) in rows {
            let digest = format!("{:x}", Sha256::digest(&bytes));
            let path = media.texture_path(&digest)?;
            write_blob(&path, bytes.clone()).await?;
            if crate::util::io::read(&path).await? != bytes {
                return Err(crate::ErrorKind::FSError(
                    "Imported skin did not pass byte verification".into(),
                )
                .into());
            }
            data["textures"]
                .as_object_mut()
                .expect("empty map")
                .insert(key, json!({"sha256":digest}));
        }
    }
    if !columns(connection, "custom_minecraft_skins")
        .await?
        .is_empty()
    {
        let rows = sqlx::query_as::<_, (String,String,String,Option<String>,i64)>("SELECT minecraft_user_uuid,texture_key,variant,cape_id,display_order FROM custom_minecraft_skins").fetch_all(&mut *connection).await?;
        for (player, key, variant, cape, order) in rows {
            let player = uuid::Uuid::parse_str(&player)
                .map(|id| id.to_string())
                .unwrap_or(player);
            let saved = data["saved_skins"]
                .as_object_mut()
                .expect("empty map")
                .entry(player)
                .or_insert_with(|| json!({}));
            saved.as_object_mut().expect("skin map").insert(
                key,
                json!({"variant":variant,"cape_id":cape,"display_order":order}),
            );
        }
    }
    if !columns(connection, "offline_minecraft_skins")
        .await?
        .is_empty()
    {
        for (player, key, variant) in sqlx::query_as::<_, (String,String,String)>("SELECT minecraft_user_uuid,texture_key,variant FROM offline_minecraft_skins").fetch_all(&mut *connection).await? {
            let player = uuid::Uuid::parse_str(&player).map(|id| id.to_string()).unwrap_or(player);
            data["offline_skins"].as_object_mut().expect("empty map").insert(player, json!({"texture_key":key,"variant":variant}));
        }
    }
    if !columns(connection, "extension_records").await?.is_empty() {
        let opaque: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM extension_records)",
        )
        .fetch_one(&mut *connection)
        .await?;
        data.insert("legacy_extensions_present".into(), json!(opaque));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqliteConnectOptions;
    #[tokio::test]
    async fn imports_exact_skin_bytes_editor_attachment_groups_and_backup_blobs()
     {
        let temp = tempfile::tempdir().unwrap();
        let core = super::super::databases::open(temp.path()).await.unwrap();
        // Discard only the intentionally empty first-install media authority.
        for relative in [
            "data/media/media.json",
            "data/persistence/imports/media.json",
        ] {
            tokio::fs::remove_file(temp.path().join(relative))
                .await
                .unwrap();
        }
        let source = temp.path().join("media-source.db");
        let mut connection = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&source)
                .create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::raw_sql("CREATE TABLE screenshots(id TEXT,instance_id TEXT,file_name TEXT,content_hash TEXT,created_at INTEGER,editor_state TEXT);CREATE TABLE screenshot_groups(id TEXT,name TEXT,display_order INTEGER);CREATE TABLE screenshot_group_memberships(screenshot_id TEXT,group_id TEXT);CREATE TABLE custom_minecraft_skin_textures(texture_key TEXT,texture BLOB);CREATE TABLE custom_minecraft_skins(minecraft_user_uuid TEXT,texture_key TEXT,variant TEXT,cape_id TEXT,display_order INTEGER);CREATE TABLE offline_minecraft_skins(minecraft_user_uuid TEXT,texture_key TEXT,variant TEXT);INSERT INTO screenshots VALUES('stable','instance','image.png','sha1',123,'opaque-editor');INSERT INTO screenshot_groups VALUES('group','Saved',2);INSERT INTO screenshot_group_memberships VALUES('stable','group');").execute(&mut connection).await.unwrap();
        let bytes = b"legacy-image-content";
        let player = uuid::Uuid::new_v4();
        sqlx::query(
            "INSERT INTO custom_minecraft_skin_textures VALUES('skin',?)",
        )
        .bind(bytes.as_slice())
        .execute(&mut connection)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO custom_minecraft_skins VALUES(?,'skin','SLIM',NULL,4)",
        )
        .bind(player.to_string())
        .execute(&mut connection)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO offline_minecraft_skins VALUES(?,'skin','SLIM')",
        )
        .bind(player.to_string())
        .execute(&mut connection)
        .await
        .unwrap();
        connection.close().await.unwrap();
        initialize(temp.path(), Some(source.clone())).await.unwrap();
        let persistence = Persistence::new(temp.path());
        let media = Media::new(&persistence);
        assert_eq!(media.skin_texture("skin").await.unwrap(), bytes);
        let rows = media.screenshots().await.unwrap();
        assert_eq!(rows[0].id, "stable");
        assert_eq!(rows[0].group_id.as_deref(), Some("group"));
        assert_eq!(
            media.read().await.unwrap()["screenshots"]["stable"]["legacy_editor_state"],
            "opaque-editor"
        );
        assert_eq!(
            media
                .offline_skin(player)
                .await
                .unwrap()
                .unwrap()
                .texture_key,
            "skin"
        );
        let digest = format!("{:x}", Sha256::digest(bytes));
        let backup = super::super::databases::backup(&core, temp.path())
            .await
            .unwrap();
        assert_eq!(
            tokio::fs::read(backup.join(format!("media/skins/{digest}.png")))
                .await
                .unwrap(),
            bytes
        );
        media.forget("instance", "image.png").await.unwrap();
        initialize(temp.path(), Some(source)).await.unwrap();
        assert!(media.screenshots().await.unwrap().is_empty());
        core.close().await;
    }
}
