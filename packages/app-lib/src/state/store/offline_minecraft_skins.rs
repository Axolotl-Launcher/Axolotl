//! The skin an offline account shows locally.
//!
//! The variant column is constrained the same way a saved skin's is, so the two
//! collections carry the same variant.

use sqlx::SqliteConnection;

use super::Collection;
use super::custom_minecraft_skins::SkinVariant;

/// The offline skin selections, one per account.
pub(crate) struct OfflineMinecraftSkins;

/// What a selection carries: the texture it shows, drawn on a player model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OfflineMinecraftSkinRecord {
    pub texture_key: String,
    pub variant: SkinVariant,
}

impl Collection for OfflineMinecraftSkins {
    type Key = String;
    type Scope = String;
    type Record = OfflineMinecraftSkinRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT texture_key, variant FROM offline_minecraft_skins
             WHERE minecraft_user_uuid = ?",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.and_then(|(texture_key, variant)| {
            Some(Self::Record {
                texture_key,
                variant: to_variant(&variant)?,
            })
        }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query(
                "DELETE FROM offline_minecraft_skins
                 WHERE minecraft_user_uuid = ?",
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
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT minecraft_user_uuid, texture_key, variant
             FROM offline_minecraft_skins",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(minecraft_user_uuid, texture_key, variant)| {
                Some((
                    minecraft_user_uuid,
                    OfflineMinecraftSkinRecord {
                        texture_key,
                        variant: to_variant(&variant)?,
                    },
                ))
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
    minecraft_user_uuid: &str,
    record: &OfflineMinecraftSkinRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO offline_minecraft_skins (
            minecraft_user_uuid, texture_key, variant
        )
        VALUES (?, ?, ?)
        ON CONFLICT (minecraft_user_uuid) DO UPDATE SET
            texture_key = excluded.texture_key,
            variant = excluded.variant
        ",
    )
    .bind(minecraft_user_uuid)
    .bind(&record.texture_key)
    .bind(record.variant.as_str())
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// Removes the selections the replacement leaves out, and only those.
async fn delete_omitted(
    conn: &mut SqliteConnection,
    scope: Option<&String>,
    records: &[(String, OfflineMinecraftSkinRecord)],
) -> crate::Result<()> {
    let existing: Vec<String> = match scope {
        Some(minecraft_user_uuid) => {
            sqlx::query_scalar(
                "SELECT minecraft_user_uuid FROM offline_minecraft_skins
             WHERE minecraft_user_uuid = ?",
            )
            .bind(minecraft_user_uuid)
            .fetch_all(&mut *conn)
            .await?
        }
        None => {
            sqlx::query_scalar(
                "SELECT minecraft_user_uuid FROM offline_minecraft_skins",
            )
            .fetch_all(&mut *conn)
            .await?
        }
    };

    for minecraft_user_uuid in existing {
        if records.iter().any(|(key, _)| key == &minecraft_user_uuid) {
            continue;
        }
        sqlx::query(
            "DELETE FROM offline_minecraft_skins WHERE minecraft_user_uuid = ?",
        )
        .bind(&minecraft_user_uuid)
        .execute(&mut *conn)
        .await?;
    }

    Ok(())
}

/// The player model a stored selection names, or nothing when this build does
/// not know it: a selection can only be drawn when its model can be read.
fn to_variant(variant: &str) -> Option<SkinVariant> {
    SkinVariant::parse(variant).or_else(|| {
        tracing::warn!(
            %variant,
            "Skipping an offline skin whose variant this build does not know"
        );
        None
    })
}
