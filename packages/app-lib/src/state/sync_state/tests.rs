use super::{journal::Transaction, manifest, *};
use serde_json::{Map, json};

fn seeded() -> manifest::Collections {
    [(
        "example".into(),
        Map::from_iter([(
            "a".into(),
            json!({"key":"a","record":{"value":1,"future":"retain"}}),
        )]),
    )]
    .into()
}

#[tokio::test]
async fn prepared_generation_is_invisible_until_decided_and_replay_is_idempotent()
 {
    let root = tempfile::tempdir().unwrap();
    let initial = manifest::initial(root.path(), seeded(), false)
        .await
        .unwrap();
    let mut transaction = Transaction::begin(root.path().to_owned(), None)
        .await
        .unwrap();
    transaction.collections.get_mut("example").unwrap()["a"]["record"]["value"] =
        json!(2);
    transaction.changed = true;
    let token = transaction.prepare().await.unwrap().unwrap();
    assert_eq!(
        manifest::load(root.path()).await.unwrap().0.transaction,
        initial.transaction
    );
    drop(transaction);
    // A prepared intent without a committed coordinator token remains inactive.
    drop(
        Transaction::begin(root.path().to_owned(), None)
            .await
            .unwrap(),
    );
    assert_eq!(manifest::load(root.path()).await.unwrap().0.revision, 0);
    drop(
        Transaction::begin(root.path().to_owned(), Some(token.clone()))
            .await
            .unwrap(),
    );
    drop(
        Transaction::begin(root.path().to_owned(), Some(token))
            .await
            .unwrap(),
    );
    let (head, data) = manifest::load(root.path()).await.unwrap();
    assert_eq!(head.revision, 1);
    assert_eq!(data["example"]["a"]["record"]["value"], 2);
    assert_eq!(data["example"]["a"]["record"]["future"], "retain");
}

#[tokio::test]
async fn dropped_unit_never_changes_head_and_reused_identity_cannot_change_body()
 {
    let root = tempfile::tempdir().unwrap();
    let initial = manifest::initial(root.path(), seeded(), false)
        .await
        .unwrap();
    let mut transaction = Transaction::begin(root.path().to_owned(), None)
        .await
        .unwrap();
    transaction.collections.clear();
    transaction.changed = true;
    drop(transaction);
    assert_eq!(manifest::load(root.path()).await.unwrap().1, seeded());
    let mut altered = initial;
    altered.legacy_extensions_present = true;
    assert!(manifest::publish(root.path(), altered, 0).await.is_err());
}

#[tokio::test]
async fn damaged_prepared_objects_never_publish_and_unknown_journal_survives() {
    let root = tempfile::tempdir().unwrap();
    manifest::initial(root.path(), seeded(), false)
        .await
        .unwrap();
    let mut transaction = Transaction::begin(root.path().to_owned(), None)
        .await
        .unwrap();
    transaction.changed = true;
    let token = transaction.prepare().await.unwrap().unwrap();
    let journal_path = root.path().join(format!("state/journals/{token}.json"));
    let body = tokio::fs::read(&journal_path).await.unwrap();
    let mut intent: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let shard = intent["desired"]["shards"][0]["file"].as_str().unwrap();
    tokio::fs::write(root.path().join(shard), b"broken")
        .await
        .unwrap();
    assert!(transaction.finish(&token).await.is_err());
    assert_eq!(manifest::load(root.path()).await.unwrap().0.revision, 0);
    intent["schema_version"] = json!(99);
    let newer = serde_json::to_vec(&intent).unwrap();
    tokio::fs::write(&journal_path, &newer).await.unwrap();
    assert!(transaction.finish(&token).await.is_err());
    assert_eq!(tokio::fs::read(journal_path).await.unwrap(), newer);
}

#[tokio::test]
async fn backup_contains_decision_journal_and_large_binary_object() {
    let root = tempfile::tempdir().unwrap();
    let backup = tempfile::tempdir().unwrap();
    let mut data = seeded();
    data.get_mut("example").unwrap()["a"]["record"]["document"] =
        json!(vec![13u8; 300_000]);
    manifest::initial(root.path(), data.clone(), false)
        .await
        .unwrap();
    let mut transaction = Transaction::begin(root.path().to_owned(), None)
        .await
        .unwrap();
    transaction.changed = true;
    let token = transaction.prepare().await.unwrap().unwrap();
    transaction.finish(&token).await.unwrap();
    manifest::backup(root.path(), backup.path(), Some(&token))
        .await
        .unwrap();
    drop(transaction);
    drop(
        Transaction::begin(backup.path().to_owned(), Some(token))
            .await
            .unwrap(),
    );
    assert_eq!(manifest::load(backup.path()).await.unwrap().1, data);
}

#[tokio::test]
async fn unknown_nested_records_and_opaque_legacy_refs_pause_cleanup() {
    let root = tempfile::tempdir().unwrap();
    let mut data = seeded();
    data.get_mut("example").unwrap()["a"]["record"]["schemaVersion"] = json!(3);
    manifest::initial(root.path(), data, false).await.unwrap();
    assert!(ensure_supported(root.path()).await.is_err());
    let other = tempfile::tempdir().unwrap();
    manifest::initial(other.path(), seeded(), true)
        .await
        .unwrap();
    assert!(ensure_supported(other.path()).await.is_err());
}

#[tokio::test]
async fn file_writer_is_serialized_and_cancellation_releases_reservation() {
    let root = tempfile::tempdir().unwrap();
    manifest::initial(root.path(), seeded(), false)
        .await
        .unwrap();
    let first = Transaction::begin(root.path().to_owned(), None)
        .await
        .unwrap();
    let next = Transaction::begin(root.path().to_owned(), None);
    tokio::pin!(next);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(25), &mut next)
            .await
            .is_err()
    );
    drop(first);
    let second = tokio::time::timeout(std::time::Duration::from_secs(2), next)
        .await
        .unwrap()
        .unwrap();
    drop(second);
}
