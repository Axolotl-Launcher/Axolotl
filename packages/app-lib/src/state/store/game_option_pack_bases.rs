//! The `options.txt` a modpack ships, kept per instance as its base.

use super::{Collection, Handle};

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
    type Record = PackBaseRecord;

    async fn set(
        handle: &Handle,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query(
                "DELETE FROM instance_game_option_pack_bases
                 WHERE instance_id = ?",
            )
            .bind(&key.instance_id)
            .execute(handle.pool())
            .await?;

            return Ok(());
        };

        sqlx::query(
            "
            INSERT INTO instance_game_option_pack_bases
                (instance_id, pack_version_id, source, sha1, encoding,
                 document)
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
        .execute(handle.pool())
        .await?;

        Ok(())
    }

    async fn get(
        handle: &Handle,
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
        .fetch_optional(handle.pool())
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

    async fn set_all(
        handle: &Handle,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        let mut transaction = handle.pool().begin().await?;
        // The call replaces the collection, so a key the caller leaves out is
        // gone rather than left behind for a later read to return.
        sqlx::query("DELETE FROM instance_game_option_pack_bases")
            .execute(&mut *transaction)
            .await?;
        for (key, record) in records {
            sqlx::query(
                "
                INSERT INTO instance_game_option_pack_bases
                    (instance_id, pack_version_id, source, sha1, encoding,
                     document)
                VALUES (?, ?, ?, ?, ?, ?)
                ",
            )
            .bind(&key.instance_id)
            .bind(&record.pack_version_id)
            .bind(record.source.as_str())
            .bind(&record.sha1)
            .bind(&record.encoding)
            .bind(&record.document)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;

        Ok(())
    }

    async fn get_all(
        handle: &Handle,
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
        .fetch_all(handle.pool())
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

#[cfg(test)]
mod tests {
    use super::*;

    async fn handle(instance: &str) -> Handle {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO instances
                (id, path, install_stage, launcher_feature_version,
                 name, created, modified)
             VALUES (?, ?, 'installed', '1', ?, 1, 1)",
        )
        .bind(instance)
        .bind(instance)
        .bind(instance)
        .execute(&pool)
        .await
        .unwrap();
        Handle::new(pool)
    }

    fn key(instance_id: &str) -> PackBaseKey {
        PackBaseKey {
            instance_id: instance_id.to_string(),
        }
    }

    #[tokio::test]
    async fn a_pack_base_round_trips_and_clears() {
        let handle = handle("instance").await;
        let record = PackBaseRecord {
            pack_version_id: Some("version".to_string()),
            source: PackBaseSource::Overrides,
            sha1: Some("abc".to_string()),
            encoding: Some("utf-8".to_string()),
            document: Some(b"options".to_vec()),
        };

        GameOptionPackBases::set(&handle, &key("instance"), Some(&record))
            .await
            .unwrap();
        assert_eq!(
            GameOptionPackBases::get(&handle, &key("instance"))
                .await
                .unwrap(),
            Some(record)
        );
        assert_eq!(
            GameOptionPackBases::get(&handle, &key("instance"))
                .await
                .unwrap()
                .map(|record| record.source),
            Some(PackBaseSource::Overrides)
        );
        assert_eq!(
            GameOptionPackBases::get_all(&handle).await.unwrap().len(),
            1
        );

        GameOptionPackBases::set(&handle, &key("instance"), None)
            .await
            .unwrap();
        assert_eq!(
            GameOptionPackBases::get(&handle, &key("instance"))
                .await
                .unwrap(),
            None
        );
    }
}
