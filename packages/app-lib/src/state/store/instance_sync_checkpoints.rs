//! The checkpoint a synced option leaves while its write is in flight.

use sqlx::SqliteConnection;

use super::{Collection, Handle};

/// The checkpoints, one per instance, feature and variant.
pub(crate) struct InstanceSyncCheckpoints;

/// One checkpoint is identified by the instance, the feature and the variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckpointKey {
    pub instance_id: String,
    pub feature: String,
    pub variant: String,
}

/// Every checkpoint one instance keeps for one feature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckpointScope {
    pub instance_id: String,
    pub feature: String,
}

/// Whether the write behind a checkpoint has landed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckpointStatus {
    Pending,
    Ready,
}

impl CheckpointStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "ready" => Some(Self::Ready),
            _ => None,
        }
    }
}

/// How the synced file was kept in step with the one it was written from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckpointLinkMode {
    Copy,
    Hard,
    Symbolic,
}

impl CheckpointLinkMode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Copy => "copy",
            Self::Hard => "hard",
            Self::Symbolic => "symbolic",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "copy" => Some(Self::Copy),
            "hard" => Some(Self::Hard),
            "symbolic" => Some(Self::Symbolic),
            _ => None,
        }
    }
}

/// What a checkpoint carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckpointRecord {
    pub expected_sha1: String,
    pub merge_base: Option<Vec<u8>>,
    pub source_revision: i64,
    pub status: CheckpointStatus,
    pub link_mode: Option<CheckpointLinkMode>,
}

impl Collection for InstanceSyncCheckpoints {
    type Key = CheckpointKey;
    type Scope = CheckpointScope;
    type Record = CheckpointRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope.instance_id == key.instance_id && scope.feature == key.feature
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(
            String,
            Option<Vec<u8>>,
            i64,
            String,
            Option<String>,
        )> = sqlx::query_as(
            "
                SELECT expected_sha1, merge_base, source_revision, status,
                    link_mode
                FROM instance_sync_checkpoints
                WHERE instance_id = ? AND feature = ? AND variant = ?
                ",
        )
        .bind(&key.instance_id)
        .bind(&key.feature)
        .bind(&key.variant)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.and_then(
            |(
                expected_sha1,
                merge_base,
                source_revision,
                status,
                link_mode,
            )| {
                to_record(
                    expected_sha1,
                    merge_base,
                    source_revision,
                    &status,
                    link_mode.as_deref(),
                )
            },
        ))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query(
                "DELETE FROM instance_sync_checkpoints
                 WHERE instance_id = ? AND feature = ? AND variant = ?",
            )
            .bind(&key.instance_id)
            .bind(&key.feature)
            .bind(&key.variant)
            .execute(&mut *conn)
            .await?;

            return Ok(());
        };

        write_row(conn, key, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(
            String,
            String,
            String,
            String,
            Option<Vec<u8>>,
            i64,
            String,
            Option<String>,
        )> = sqlx::query_as(
            "
            SELECT instance_id, feature, variant, expected_sha1, merge_base,
                source_revision, status, link_mode
            FROM instance_sync_checkpoints
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(
                |(
                    instance_id,
                    feature,
                    variant,
                    expected_sha1,
                    merge_base,
                    source_revision,
                    status,
                    link_mode,
                )| {
                    Some((
                        CheckpointKey {
                            instance_id,
                            feature,
                            variant,
                        },
                        to_record(
                            expected_sha1,
                            merge_base,
                            source_revision,
                            &status,
                            link_mode.as_deref(),
                        )?,
                    ))
                },
            )
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM instance_sync_checkpoints")
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
        let rows: Vec<(
            String,
            String,
            Option<Vec<u8>>,
            i64,
            String,
            Option<String>,
        )> = sqlx::query_as(
            "
            SELECT variant, expected_sha1, merge_base, source_revision,
                status, link_mode
            FROM instance_sync_checkpoints
            WHERE instance_id = ? AND feature = ?
            ",
        )
        .bind(&scope.instance_id)
        .bind(&scope.feature)
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(
                |(
                    variant,
                    expected_sha1,
                    merge_base,
                    source_revision,
                    status,
                    link_mode,
                )| {
                    Some((
                        CheckpointKey {
                            instance_id: scope.instance_id.clone(),
                            feature: scope.feature.clone(),
                            variant,
                        },
                        to_record(
                            expected_sha1,
                            merge_base,
                            source_revision,
                            &status,
                            link_mode.as_deref(),
                        )?,
                    ))
                },
            )
            .collect())
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query(
            "DELETE FROM instance_sync_checkpoints
             WHERE instance_id = ? AND feature = ?",
        )
        .bind(&scope.instance_id)
        .bind(&scope.feature)
        .execute(&mut *conn)
        .await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }
}

impl InstanceSyncCheckpoints {
    /// The instances whose checkpoint of `feature` recorded the same expected
    /// bytes, leaving out `except`.
    pub(crate) async fn instances_with_expected_sha1(
        handle: &Handle,
        feature: &str,
        expected_sha1: &str,
        except: &str,
    ) -> crate::Result<Vec<String>> {
        let mut connection = handle.connection().await?;
        let rows: Vec<String> = sqlx::query_scalar(
            "
            SELECT instance_id FROM instance_sync_checkpoints
            WHERE feature = ? AND expected_sha1 = ?
              AND instance_id != ?
            ",
        )
        .bind(feature)
        .bind(expected_sha1)
        .bind(except)
        .fetch_all(&mut *connection)
        .await?;

        Ok(rows)
    }
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &CheckpointKey,
    record: &CheckpointRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO instance_sync_checkpoints
            (instance_id, feature, variant, expected_sha1, merge_base,
             source_revision, status, link_mode)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT (instance_id, feature, variant) DO UPDATE SET
            expected_sha1 = excluded.expected_sha1,
            merge_base = excluded.merge_base,
            source_revision = excluded.source_revision,
            status = excluded.status,
            link_mode = excluded.link_mode
        ",
    )
    .bind(&key.instance_id)
    .bind(&key.feature)
    .bind(&key.variant)
    .bind(&record.expected_sha1)
    .bind(&record.merge_base)
    .bind(record.source_revision)
    .bind(record.status.as_str())
    .bind(record.link_mode.map(CheckpointLinkMode::as_str))
    .execute(&mut *conn)
    .await?;

    Ok(())
}

fn to_record(
    expected_sha1: String,
    merge_base: Option<Vec<u8>>,
    source_revision: i64,
    status: &str,
    link_mode: Option<&str>,
) -> Option<CheckpointRecord> {
    let link_mode = match link_mode {
        Some(value) => Some(to_link_mode(value)?),
        None => None,
    };
    Some(CheckpointRecord {
        expected_sha1,
        merge_base,
        source_revision,
        status: to_status(status)?,
        link_mode,
    })
}

fn to_status(status: &str) -> Option<CheckpointStatus> {
    CheckpointStatus::parse(status).or_else(|| {
        tracing::warn!(
            %status,
            "Skipping a sync checkpoint whose status this build does not know"
        );
        None
    })
}

fn to_link_mode(link_mode: &str) -> Option<CheckpointLinkMode> {
    CheckpointLinkMode::parse(link_mode).or_else(|| {
        tracing::warn!(
            %link_mode,
            "Skipping a sync checkpoint whose link mode this build does not know"
        );
        None
    })
}
