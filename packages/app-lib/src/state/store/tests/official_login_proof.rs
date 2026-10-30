//! What the official login proof collection promises.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::official_login_proof::{
    Installation, OfficialLoginProof, OfficialLoginProofRecord,
};

fn proof(mac: &str) -> OfficialLoginProofRecord {
    OfficialLoginProofRecord {
        mac: mac.to_string(),
    }
}

#[tokio::test]
async fn a_proof_round_trips_and_clears() {
    let handle = support::handle().await;

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
    let handle = support::handle().await;
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
                        .filter(|migration| migration.version < 20261002120000)
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
