//! Proof that the official Minecraft login succeeded on this installation.

use super::{Collection, Handle};

/// The proof that official login was verified once.
pub(crate) struct OfficialLoginProof;

/// There is one proof per installation.
pub(crate) struct Installation;

/// What the proof carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OfficialLoginProofRecord {
    pub mac: String,
}

impl Collection for OfficialLoginProof {
    type Key = Installation;
    type Record = OfficialLoginProofRecord;

    async fn set(
        handle: &Handle,
        _key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM official_login_proof WHERE id = 0")
                .execute(handle.pool())
                .await?;

            return Ok(());
        };

        sqlx::query(
            "
            INSERT INTO official_login_proof (id, verified, version, mac)
            VALUES (0, 1, 1, ?)
            ON CONFLICT (id) DO UPDATE SET
                verified = 1,
                version = 1,
                mac = excluded.mac
            ",
        )
        .bind(&record.mac)
        .execute(handle.pool())
        .await?;

        Ok(())
    }

    async fn get(
        handle: &Handle,
        _key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let mac: Option<String> = sqlx::query_scalar(
            "
            SELECT mac
            FROM official_login_proof
            WHERE id = 0 AND verified = 1 AND version = 1
            ",
        )
        .fetch_optional(handle.pool())
        .await?;

        Ok(mac.map(|mac| Self::Record { mac }))
    }

    async fn set_all(
        handle: &Handle,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        let mut transaction = handle.pool().begin().await?;
        // One installation holds at most one proof, so a whole write is the
        // proof it is given, or none.
        sqlx::query("DELETE FROM official_login_proof")
            .execute(&mut *transaction)
            .await?;
        for (_, record) in records {
            sqlx::query(
                "
                INSERT INTO official_login_proof (id, verified, version, mac)
                VALUES (0, 1, 1, ?)
                ",
            )
            .bind(&record.mac)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;

        Ok(())
    }

    async fn get_all(
        handle: &Handle,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let mac: Option<String> = sqlx::query_scalar(
            "
            SELECT mac
            FROM official_login_proof
            WHERE id = 0 AND verified = 1 AND version = 1
            ",
        )
        .fetch_optional(handle.pool())
        .await?;

        Ok(mac
            .map(|mac| vec![(Installation, Self::Record { mac })])
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn handle() -> Handle {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        Handle::new(pool)
    }

    fn proof(mac: &str) -> OfficialLoginProofRecord {
        OfficialLoginProofRecord {
            mac: mac.to_string(),
        }
    }

    #[tokio::test]
    async fn a_proof_round_trips_and_clears() {
        let handle = handle().await;

        OfficialLoginProof::set(&handle, &Installation, Some(&proof("first")))
            .await
            .unwrap();
        assert_eq!(
            OfficialLoginProof::get(&handle, &Installation)
                .await
                .unwrap(),
            Some(proof("first"))
        );

        OfficialLoginProof::set(&handle, &Installation, Some(&proof("second")))
            .await
            .unwrap();
        assert_eq!(
            OfficialLoginProof::get(&handle, &Installation)
                .await
                .unwrap(),
            Some(proof("second"))
        );

        OfficialLoginProof::set(&handle, &Installation, None)
            .await
            .unwrap();
        assert_eq!(
            OfficialLoginProof::get(&handle, &Installation)
                .await
                .unwrap(),
            None
        );
        assert!(
            OfficialLoginProof::get_all(&handle)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_proof_the_table_does_not_trust_is_no_proof() {
        let handle = handle().await;
        sqlx::raw_sql(
            "PRAGMA ignore_check_constraints = ON;
             INSERT INTO official_login_proof (id, verified, version, mac)
             VALUES (0, 0, 1, 'unverified');
             PRAGMA ignore_check_constraints = OFF;",
        )
        .execute(handle.pool())
        .await
        .unwrap();

        assert_eq!(
            OfficialLoginProof::get(&handle, &Installation)
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn the_proof_table_migrates_fresh_and_existing_databases() {
        let migrations = sqlx::migrate!();
        for existing in [false, true] {
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await
                .unwrap();
            if existing {
                let previous = sqlx::migrate::Migrator {
                    migrations: std::borrow::Cow::Owned(
                        migrations
                            .iter()
                            .filter(|migration| {
                                migration.version < 20261002120000
                            })
                            .cloned()
                            .collect(),
                    ),
                    ..sqlx::migrate::Migrator::DEFAULT
                };
                previous.run(&pool).await.unwrap();
            }
            migrations.run(&pool).await.unwrap();
            sqlx::query(
                "INSERT INTO official_login_proof (id, verified, version, mac)
                 VALUES (0, 1, 1, 'test')",
            )
            .execute(&pool)
            .await
            .unwrap();
            let foreign_key_errors: Vec<(String, i64, String, i64)> =
                sqlx::query_as("PRAGMA foreign_key_check")
                    .fetch_all(&pool)
                    .await
                    .unwrap();
            assert!(foreign_key_errors.is_empty());
        }
    }
}
