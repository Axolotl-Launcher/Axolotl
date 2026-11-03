use super::*;
use crate::state::instances::adapters::sqlite::test_support::test_pool;

#[tokio::test]
async fn instance_existence_check_passes_for_existing_rows() {
    let pool = test_pool().await;
    sqlx::query("INSERT INTO instances (id) VALUES ('instance')")
        .execute(&pool)
        .await
        .expect("insert instance");
    let mut tx = pool.begin().await.expect("begin transaction");

    ensure_instance_exists("instance", &mut tx)
        .await
        .expect("existing instance passes");
}

#[tokio::test]
async fn instance_existence_check_reports_deleted_instances() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("begin transaction");

    let error = ensure_instance_exists("missing", &mut tx)
        .await
        .expect_err("missing instance must fail");
    assert!(
        error.to_string().contains("This instance has been deleted"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn content_write_parents_check_requires_instance_and_set() {
    let pool = test_pool().await;
    sqlx::query("INSERT INTO instances (id) VALUES ('instance')")
        .execute(&pool)
        .await
        .expect("insert instance");
    sqlx::query("INSERT INTO instance_content_sets (id) VALUES ('set')")
        .execute(&pool)
        .await
        .expect("insert set");
    let mut tx = pool.begin().await.expect("begin transaction");

    ensure_content_write_parents("instance", "set", &mut tx)
        .await
        .expect("existing parents pass");

    let missing_set = ensure_content_write_parents("instance", "gone", &mut tx)
        .await
        .expect_err("missing content set must fail");
    assert!(
        missing_set
            .to_string()
            .contains("content set for this instance has been deleted"),
        "unexpected error: {missing_set}"
    );

    let missing_instance = ensure_content_write_parents("gone", "set", &mut tx)
        .await
        .expect_err("missing instance must fail");
    assert!(
        missing_instance
            .to_string()
            .contains("This instance has been deleted"),
        "unexpected error: {missing_instance}"
    );
}

#[tokio::test]
async fn checks_run_inside_transactions() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("begin transaction");

    ensure_instance_exists("instance", &mut tx)
        .await
        .expect_err("missing instance must fail inside a transaction");
    tx.rollback().await.expect("rollback transaction");

    sqlx::query("INSERT INTO instances (id) VALUES ('instance')")
        .execute(&pool)
        .await
        .expect("insert instance in transaction");
    let mut tx = pool.begin().await.expect("begin transaction");
    ensure_instance_exists("instance", &mut tx)
        .await
        .expect("existing instance passes inside a transaction");
}
