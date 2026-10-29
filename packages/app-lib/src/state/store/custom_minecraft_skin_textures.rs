//! The PNG bytes the saved skins of an account are drawn from.

use sqlx::SqliteConnection;

use super::Collection;

/// The textures, one per texture key.
pub(crate) struct CustomMinecraftSkinTextures;

/// What a texture carries: the bytes of the PNG itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CustomMinecraftSkinTextureRecord {
    pub texture: Vec<u8>,
}

impl Collection for CustomMinecraftSkinTextures {
    type Key = String;
    type Scope = String;
    type Record = CustomMinecraftSkinTextureRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let texture: Option<Vec<u8>> = sqlx::query_scalar(
            "SELECT texture FROM custom_minecraft_skin_textures
             WHERE texture_key = ?",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(texture.map(|texture| Self::Record { texture }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query(
                "DELETE FROM custom_minecraft_skin_textures
                 WHERE texture_key = ?",
            )
            .bind(key)
            .execute(&mut *conn)
            .await?;

            return Ok(());
        };

        write_row(conn, key, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, Vec<u8>)> = sqlx::query_as(
            "SELECT texture_key, texture FROM custom_minecraft_skin_textures",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(texture_key, texture)| {
                (texture_key, CustomMinecraftSkinTextureRecord { texture })
            })
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        delete_omitted(conn, None, records).await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }

    async fn read_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        Ok(Self::read(conn, scope)
            .await?
            .map(|record| vec![(scope.clone(), record)])
            .unwrap_or_default())
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        delete_omitted(conn, Some(scope), records).await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }
}

async fn write_row(
    conn: &mut SqliteConnection,
    texture_key: &str,
    record: &CustomMinecraftSkinTextureRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO custom_minecraft_skin_textures (texture_key, texture)
        VALUES (?, ?)
        ON CONFLICT (texture_key) DO UPDATE SET texture = excluded.texture
        ",
    )
    .bind(texture_key)
    .bind(&record.texture)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// Removes the textures the replacement leaves out, and only those.
async fn delete_omitted(
    conn: &mut SqliteConnection,
    scope: Option<&String>,
    records: &[(String, CustomMinecraftSkinTextureRecord)],
) -> crate::Result<()> {
    let existing: Vec<String> = match scope {
        Some(texture_key) => {
            sqlx::query_scalar(
                "SELECT texture_key FROM custom_minecraft_skin_textures
             WHERE texture_key = ?",
            )
            .bind(texture_key)
            .fetch_all(&mut *conn)
            .await?
        }
        None => {
            sqlx::query_scalar(
                "SELECT texture_key FROM custom_minecraft_skin_textures",
            )
            .fetch_all(&mut *conn)
            .await?
        }
    };

    for texture_key in existing {
        if records.iter().any(|(key, _)| key == &texture_key) {
            continue;
        }
        sqlx::query(
            "DELETE FROM custom_minecraft_skin_textures WHERE texture_key = ?",
        )
        .bind(&texture_key)
        .execute(&mut *conn)
        .await?;
    }

    Ok(())
}
