//! Where a mod setting's label was found, per scope and option.

use sqlx::SqliteConnection;

use super::Collection;

/// The origins, one per scope and option.
pub(crate) struct GameOptionLocaleOrigins;

/// An origin is identified by the scope it is known in and the option it
/// describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocaleOriginKey {
    pub scope: String,
    pub option_id: String,
}

/// What an origin carries beyond its key.
///
/// The observation and the resolved origin are opaque text: what one says is
/// the caller's business.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocaleOriginRecord {
    pub source_instance_id: Option<String>,
    pub source_game_version: Option<String>,
    pub backfilled: bool,
    pub observation: Option<String>,
    pub origin: Option<String>,
}

impl Collection for GameOptionLocaleOrigins {
    type Key = LocaleOriginKey;
    type Scope = String;
    type Record = LocaleOriginRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        key.scope == *scope
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(
            Option<String>,
            Option<String>,
            bool,
            Option<String>,
            Option<String>,
        )> = sqlx::query_as(
            "
            SELECT source_instance_id, source_game_version, backfilled,
                observation_json, origin_json
            FROM game_option_locale_origins
            WHERE scope = ? AND option_id = ?
            ",
        )
        .bind(&key.scope)
        .bind(&key.option_id)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.map(
            |(
                source_instance_id,
                source_game_version,
                backfilled,
                observation,
                origin,
            )| Self::Record {
                source_instance_id,
                source_game_version,
                backfilled,
                observation,
                origin,
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
                "DELETE FROM game_option_locale_origins
                 WHERE scope = ? AND option_id = ?",
            )
            .bind(&key.scope)
            .bind(&key.option_id)
            .execute(&mut *conn)
            .await?;

            return Ok(());
        };

        write_row(conn, key, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<StoredOriginRow> = sqlx::query_as(
            "
            SELECT scope, option_id, source_instance_id, source_game_version,
                backfilled, observation_json, origin_json
            FROM game_option_locale_origins
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows.into_iter().map(to_entry).collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM game_option_locale_origins")
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
        let rows: Vec<StoredOriginRow> = sqlx::query_as(
            "
            SELECT scope, option_id, source_instance_id, source_game_version,
                backfilled, observation_json, origin_json
            FROM game_option_locale_origins
            WHERE scope = ?
            ",
        )
        .bind(scope)
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows.into_iter().map(to_entry).collect())
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM game_option_locale_origins WHERE scope = ?")
            .bind(scope)
            .execute(&mut *conn)
            .await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }
}

/// A stored origin as it is read back: the key first, then the record.
type StoredOriginRow = (
    String,
    String,
    Option<String>,
    Option<String>,
    bool,
    Option<String>,
    Option<String>,
);

fn to_entry(row: StoredOriginRow) -> (LocaleOriginKey, LocaleOriginRecord) {
    let (
        scope,
        option_id,
        source_instance_id,
        source_game_version,
        backfilled,
        observation,
        origin,
    ) = row;
    (
        LocaleOriginKey { scope, option_id },
        LocaleOriginRecord {
            source_instance_id,
            source_game_version,
            backfilled,
            observation,
            origin,
        },
    )
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &LocaleOriginKey,
    record: &LocaleOriginRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO game_option_locale_origins
            (scope, option_id, source_instance_id, source_game_version,
             backfilled, observation_json, origin_json)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT (scope, option_id) DO UPDATE SET
            source_instance_id = excluded.source_instance_id,
            source_game_version = excluded.source_game_version,
            backfilled = excluded.backfilled,
            observation_json = excluded.observation_json,
            origin_json = excluded.origin_json
        ",
    )
    .bind(&key.scope)
    .bind(&key.option_id)
    .bind(&record.source_instance_id)
    .bind(&record.source_game_version)
    .bind(record.backfilled)
    .bind(&record.observation)
    .bind(&record.origin)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
