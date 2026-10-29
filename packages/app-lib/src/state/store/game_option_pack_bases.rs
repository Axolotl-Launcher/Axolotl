//! The `options.txt` a modpack ships, kept per instance as its base.

use sqlx::SqliteConnection;

use super::Collection;

/// The pack bases, one per instance.
pub(crate) struct GameOptionPackBases;

/// A pack base is identified by the instance it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackBaseKey {
    pub instance_id: String,
}

/// Where the base came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PackBaseSource {
    ClientOverrides,
    Overrides,
    None,
}

impl PackBaseSource {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ClientOverrides => "client_overrides",
            Self::Overrides => "overrides",
            Self::None => "none",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "client_overrides" => Some(Self::ClientOverrides),
            "overrides" => Some(Self::Overrides),
            "none" => Some(Self::None),
            _ => None,
        }
    }
}

/// The base a modpack update left behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackBaseRecord {
    pub pack_version_id: Option<String>,
    pub source: PackBaseSource,
    pub sha1: Option<String>,
    pub encoding: Option<String>,
    pub document: Option<Vec<u8>>,
}

impl Collection for GameOptionPackBases {
    type Key = PackBaseKey;
    type Scope = PackBaseKey;
    type Record = PackBaseRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(
            Option<String>,
            String,
            Option<String>,
            Option<String>,
            Option<Vec<u8>>,
        )> = sqlx::query_as(
            "
            SELECT pack_version_id, source, sha1, encoding, document
            FROM instance_game_option_pack_bases
            WHERE instance_id = ?
            ",
        )
        .bind(&key.instance_id)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.and_then(
            |(pack_version_id, source, sha1, encoding, document)| {
                Some(Self::Record {
                    pack_version_id,
                    source: to_source(&source)?,
                    sha1,
                    encoding,
                    document,
                })
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
                "DELETE FROM instance_game_option_pack_bases
                 WHERE instance_id = ?",
            )
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
        let rows: Vec<(
            String,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
            Option<Vec<u8>>,
        )> = sqlx::query_as(
            "
            SELECT instance_id, pack_version_id, source, sha1, encoding,
                document
            FROM instance_game_option_pack_bases
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(
                |(
                    instance_id,
                    pack_version_id,
                    source,
                    sha1,
                    encoding,
                    document,
                )| {
                    Some((
                        PackBaseKey { instance_id },
                        PackBaseRecord {
                            pack_version_id,
                            source: to_source(&source)?,
                            sha1,
                            encoding,
                            document,
                        },
                    ))
                },
            )
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM instance_game_option_pack_bases")
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

async fn write_row(
    conn: &mut SqliteConnection,
    key: &PackBaseKey,
    record: &PackBaseRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO instance_game_option_pack_bases
            (instance_id, pack_version_id, source, sha1, encoding, document)
        VALUES (?, ?, ?, ?, ?, ?)
        ON CONFLICT (instance_id) DO UPDATE SET
            pack_version_id = excluded.pack_version_id,
            source = excluded.source,
            sha1 = excluded.sha1,
            encoding = excluded.encoding,
            document = excluded.document
        ",
    )
    .bind(&key.instance_id)
    .bind(&record.pack_version_id)
    .bind(record.source.as_str())
    .bind(&record.sha1)
    .bind(&record.encoding)
    .bind(&record.document)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// The source a stored base names, or nothing when this build does not know
/// it: a base is only useful while its provenance can be read.
fn to_source(source: &str) -> Option<PackBaseSource> {
    PackBaseSource::parse(source).or_else(|| {
        tracing::warn!(
            %source,
            "Skipping a pack base whose source this build does not know"
        );
        None
    })
}
