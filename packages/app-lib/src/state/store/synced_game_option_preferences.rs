//! Whether a shared game setting is followed, one per option.

use sqlx::SqliteConnection;

use super::Collection;

/// The preferences, one per option.
pub(crate) struct SyncedGameOptionPreferences;

/// Where a preference came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GameOptionPreferenceSource {
    CatalogDefault,
    DiscoveryDefault,
    User,
}

impl GameOptionPreferenceSource {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::CatalogDefault => "catalog_default",
            Self::DiscoveryDefault => "discovery_default",
            Self::User => "user",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "catalog_default" => Some(Self::CatalogDefault),
            "discovery_default" => Some(Self::DiscoveryDefault),
            "user" => Some(Self::User),
            _ => None,
        }
    }
}

/// What a preference carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GameOptionPreferenceRecord {
    pub enabled: bool,
    pub source: GameOptionPreferenceSource,
    pub revision: i64,
}

impl Collection for SyncedGameOptionPreferences {
    type Key = String;
    type Scope = String;
    type Record = GameOptionPreferenceRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(bool, String, i64)> = sqlx::query_as(
            "
            SELECT enabled, source, revision
            FROM synced_game_option_preferences
            WHERE option_id = ?
            ",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.and_then(to_record))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query(
                "DELETE FROM synced_game_option_preferences WHERE option_id = ?",
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
        let rows: Vec<(String, bool, String, i64)> = sqlx::query_as(
            "
            SELECT option_id, enabled, source, revision
            FROM synced_game_option_preferences
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(option_id, enabled, source, revision)| {
                Some((option_id, to_record((enabled, source, revision))?))
            })
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM synced_game_option_preferences")
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
        Self::write(conn, scope, records.first().map(|(_, record)| record))
            .await
    }
}

fn to_record(row: (bool, String, i64)) -> Option<GameOptionPreferenceRecord> {
    let (enabled, source, revision) = row;
    Some(GameOptionPreferenceRecord {
        enabled,
        source: to_source(&source)?,
        revision,
    })
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &str,
    record: &GameOptionPreferenceRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO synced_game_option_preferences
            (option_id, enabled, source, revision)
        VALUES (?, ?, ?, ?)
        ON CONFLICT (option_id) DO UPDATE SET
            enabled = excluded.enabled,
            source = excluded.source,
            revision = excluded.revision
        ",
    )
    .bind(key)
    .bind(record.enabled)
    .bind(record.source.as_str())
    .bind(record.revision)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// A preference whose source this build does not know says nothing about the
/// option, so it is read as no record at all.
fn to_source(source: &str) -> Option<GameOptionPreferenceSource> {
    GameOptionPreferenceSource::parse(source).or_else(|| {
        tracing::warn!(
            %source,
            "Skipping a game setting preference whose source this build does not know"
        );
        None
    })
}
