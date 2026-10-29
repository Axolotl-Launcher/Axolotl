//! The skins an account saved, and where each one sits in its list.
//!
//! A saved skin names its texture, so the texture it points at has to be
//! written before it.

use sqlx::SqliteConnection;

use super::Collection;

/// The saved skins, one per account and texture.
pub(crate) struct CustomMinecraftSkins;

/// Which player model a saved skin is drawn on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SkinVariant {
    Classic,
    Slim,
    Unknown,
}

impl SkinVariant {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Classic => "CLASSIC",
            Self::Slim => "SLIM",
            Self::Unknown => "UNKNOWN",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "CLASSIC" => Some(Self::Classic),
            "SLIM" => Some(Self::Slim),
            "UNKNOWN" => Some(Self::Unknown),
            _ => None,
        }
    }
}

/// A saved skin is identified by the account and the texture it is drawn from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CustomMinecraftSkinKey {
    pub minecraft_user_uuid: String,
    pub texture_key: String,
}

/// What a saved skin carries beyond its identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CustomMinecraftSkinRecord {
    pub variant: SkinVariant,
    pub cape_id: Option<String>,
    pub display_order: i64,
}

impl Collection for CustomMinecraftSkins {
    type Key = CustomMinecraftSkinKey;
    type Scope = String;
    type Record = CustomMinecraftSkinRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        key.minecraft_user_uuid == *scope
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(String, Option<String>, i64)> = sqlx::query_as(
            "
            SELECT variant, cape_id, display_order
            FROM custom_minecraft_skins
            WHERE minecraft_user_uuid = ? AND texture_key = ?
            ",
        )
        .bind(&key.minecraft_user_uuid)
        .bind(&key.texture_key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.and_then(|(variant, cape_id, display_order)| {
            Some(Self::Record {
                variant: to_variant(&variant)?,
                cape_id,
                display_order,
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
                "DELETE FROM custom_minecraft_skins
                 WHERE minecraft_user_uuid = ? AND texture_key = ?",
            )
            .bind(&key.minecraft_user_uuid)
            .bind(&key.texture_key)
            .execute(&mut *conn)
            .await?;

            return Ok(());
        };

        write_row(conn, key, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows = sqlx::query_as(
            "
            SELECT minecraft_user_uuid, texture_key, variant, cape_id,
                display_order
            FROM custom_minecraft_skins
            ORDER BY display_order ASC, rowid ASC
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(to_records(rows))
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
        let rows = sqlx::query_as(
            "
            SELECT minecraft_user_uuid, texture_key, variant, cape_id,
                display_order
            FROM custom_minecraft_skins
            WHERE minecraft_user_uuid = ?
            ORDER BY display_order ASC, rowid ASC
            ",
        )
        .bind(scope)
        .fetch_all(&mut *conn)
        .await?;

        Ok(to_records(rows))
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

fn to_records(
    rows: Vec<(String, String, String, Option<String>, i64)>,
) -> Vec<(CustomMinecraftSkinKey, CustomMinecraftSkinRecord)> {
    rows.into_iter()
        .filter_map(
            |(
                minecraft_user_uuid,
                texture_key,
                variant,
                cape_id,
                display_order,
            )| {
                Some((
                    CustomMinecraftSkinKey {
                        minecraft_user_uuid,
                        texture_key,
                    },
                    CustomMinecraftSkinRecord {
                        variant: to_variant(&variant)?,
                        cape_id,
                        display_order,
                    },
                ))
            },
        )
        .collect()
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &CustomMinecraftSkinKey,
    record: &CustomMinecraftSkinRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO custom_minecraft_skins (
            minecraft_user_uuid, texture_key, variant, cape_id, display_order
        )
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT (minecraft_user_uuid, texture_key) DO UPDATE SET
            variant = excluded.variant,
            cape_id = excluded.cape_id,
            display_order = excluded.display_order
        ",
    )
    .bind(&key.minecraft_user_uuid)
    .bind(&key.texture_key)
    .bind(record.variant.as_str())
    .bind(&record.cape_id)
    .bind(record.display_order)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// Removes the saved skins the replacement leaves out, and only those.
async fn delete_omitted(
    conn: &mut SqliteConnection,
    scope: Option<&String>,
    records: &[(CustomMinecraftSkinKey, CustomMinecraftSkinRecord)],
) -> crate::Result<()> {
    let existing: Vec<(String, String, String)> = match scope {
        Some(minecraft_user_uuid) => {
            sqlx::query_as(
                "SELECT minecraft_user_uuid, texture_key, variant
             FROM custom_minecraft_skins
             WHERE minecraft_user_uuid = ?",
            )
            .bind(minecraft_user_uuid)
            .fetch_all(&mut *conn)
            .await?
        }
        None => {
            sqlx::query_as(
                "SELECT minecraft_user_uuid, texture_key, variant
             FROM custom_minecraft_skins",
            )
            .fetch_all(&mut *conn)
            .await?
        }
    };

    for (minecraft_user_uuid, texture_key, variant) in existing {
        // A skin whose variant this build cannot read was never part of the
        // replacement, so it is not one the replacement leaves out.
        if to_variant(&variant).is_none() {
            continue;
        }
        let kept = records.iter().any(|(key, _)| {
            key.minecraft_user_uuid == minecraft_user_uuid
                && key.texture_key == texture_key
        });
        if kept {
            continue;
        }
        sqlx::query(
            "DELETE FROM custom_minecraft_skins
             WHERE minecraft_user_uuid = ? AND texture_key = ?",
        )
        .bind(&minecraft_user_uuid)
        .bind(&texture_key)
        .execute(&mut *conn)
        .await?;
    }

    Ok(())
}

/// The player model a stored skin names, or nothing when this build does not
/// know it: a saved skin can only be drawn when its model can be read.
fn to_variant(variant: &str) -> Option<SkinVariant> {
    SkinVariant::parse(variant).or_else(|| {
        tracing::warn!(
            %variant,
            "Skipping a saved skin whose variant this build does not know"
        );
        None
    })
}
