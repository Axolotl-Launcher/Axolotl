//! Which instance has which shared pack, and where it was put.

use sqlx::SqliteConnection;

use super::Collection;

/// The pack-to-instance links.
pub(crate) struct SyncedPackInstances;

/// A link is identified by the pack and the instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SyncedPackInstanceKey {
    pub pack_id: String,
    pub instance_id: String,
}

/// Every link one pack has.
pub(crate) type SyncedPackInstanceScope = String;

/// What a link carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SyncedPackInstanceRecord {
    pub excluded: bool,
    pub materialized_path: Option<String>,
    pub modified_at: i64,
}

impl Collection for SyncedPackInstances {
    type Key = SyncedPackInstanceKey;
    type Scope = SyncedPackInstanceScope;
    type Record = SyncedPackInstanceRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        key.pack_id == *scope
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(bool, Option<String>, i64)> = sqlx::query_as(
            "
            SELECT excluded, materialized_path, modified_at
            FROM synced_pack_instances
            WHERE pack_id = ? AND instance_id = ?
            ",
        )
        .bind(&key.pack_id)
        .bind(&key.instance_id)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(
            row.map(|(excluded, materialized_path, modified_at)| {
                Self::Record {
                    excluded,
                    materialized_path,
                    modified_at,
                }
            }),
        )
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query(
                "DELETE FROM synced_pack_instances
                 WHERE pack_id = ? AND instance_id = ?",
            )
            .bind(&key.pack_id)
            .bind(&key.instance_id)
            .execute(&mut *conn)
            .await?;

            return Ok(());
        };

        write_row(conn, key, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, String, bool, Option<String>, i64)> =
            sqlx::query_as(
                "
                SELECT pack_id, instance_id, excluded, materialized_path,
                    modified_at
                FROM synced_pack_instances
                ",
            )
            .fetch_all(&mut *conn)
            .await?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    pack_id,
                    instance_id,
                    excluded,
                    materialized_path,
                    modified_at,
                )| {
                    (
                        Self::Key {
                            pack_id,
                            instance_id,
                        },
                        Self::Record {
                            excluded,
                            materialized_path,
                            modified_at,
                        },
                    )
                },
            )
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM synced_pack_instances")
            .execute(&mut *conn)
            .await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }

    async fn read_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, bool, Option<String>, i64)> = sqlx::query_as(
            "
            SELECT instance_id, excluded, materialized_path, modified_at
            FROM synced_pack_instances
            WHERE pack_id = ?
            ",
        )
        .bind(scope)
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(instance_id, excluded, materialized_path, modified_at)| {
                (
                    Self::Key {
                        pack_id: scope.clone(),
                        instance_id,
                    },
                    Self::Record {
                        excluded,
                        materialized_path,
                        modified_at,
                    },
                )
            })
            .collect())
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        for (key, _) in Self::read_scoped(conn, scope).await? {
            let kept = records.iter().any(|(given, _)| given == &key);
            if kept {
                continue;
            }
            sqlx::query(
                "DELETE FROM synced_pack_instances
                 WHERE pack_id = ? AND instance_id = ?",
            )
            .bind(&key.pack_id)
            .bind(&key.instance_id)
            .execute(&mut *conn)
            .await?;
        }
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &SyncedPackInstanceKey,
    record: &SyncedPackInstanceRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO synced_pack_instances
            (pack_id, instance_id, excluded, materialized_path, modified_at)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(pack_id, instance_id) DO UPDATE SET
            excluded = excluded.excluded,
            materialized_path = excluded.materialized_path,
            modified_at = excluded.modified_at
        ",
    )
    .bind(&key.pack_id)
    .bind(&key.instance_id)
    .bind(record.excluded)
    .bind(&record.materialized_path)
    .bind(record.modified_at)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
