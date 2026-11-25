use super::{journal::Transaction, manifest, *};
use serde_json::{Map, Value, json};

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
    transaction.collections.get_mut("example").unwrap()["a"]["record"]["value"] =
        json!(2);
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

#[tokio::test]
async fn unchanged_collection_operations_preserve_generation_and_unknown_fields()
 {
    use crate::state::store::synced_game_option_state::{
        GameOptionStateRecord, Singleton, SyncedGameOptionState,
    };
    use crate::state::store::synced_servers::SyncedServers;
    use crate::state::store::{Handle, atom, databases};
    let temp = tempfile::tempdir().unwrap();
    let pool = databases::open(temp.path()).await.unwrap();
    let handle = Handle::new(pool);
    let root = super::root(&handle.persistence()).await.unwrap();
    let record = GameOptionStateRecord {
        revision: 1,
        catalog_revision: 1,
    };
    let raw_key = serde_json::to_value(Singleton).unwrap();
    let id = key(&raw_key).unwrap();
    let mut unit = atom(&handle).await.unwrap();
    let sync = unit.sync().await.unwrap();
    sync.collections.entry("synced_game_option_state".into()).or_default().insert(id.clone(),
        json!({"key":raw_key,"extra":42,"record":{"revision":1,"catalog_revision":1,"future":"retain"}}));
    sync.changed = true;
    unit.commit().await.unwrap();
    let snapshot = manifest::load(&root).await.unwrap();

    SyncedGameOptionState::set(&handle, &Singleton, Some(&record))
        .await
        .unwrap();
    SyncedGameOptionState::set_all(&handle, &[(Singleton, record.clone())])
        .await
        .unwrap();
    SyncedServers::set(&handle, &"missing".into(), None)
        .await
        .unwrap();
    let current = manifest::load(&root).await.unwrap();
    assert_eq!(current.0.transaction, snapshot.0.transaction);
    assert_eq!(current.0.revision, snapshot.0.revision);
    assert_eq!(current.1, snapshot.1);
    let mut journals = tokio::fs::read_dir(root.join("state/journals"))
        .await
        .unwrap();
    let mut count = 0;
    while journals.next_entry().await.unwrap().is_some() {
        count += 1;
    }
    assert_eq!(count, 2);

    let mut unit = atom(&handle).await.unwrap();
    let changed = GameOptionStateRecord {
        revision: 2,
        ..record
    };
    SyncedGameOptionState::atom_set(&mut unit, &Singleton, Some(&changed))
        .await
        .unwrap();
    SyncedGameOptionState::atom_set(&mut unit, &Singleton, Some(&changed))
        .await
        .unwrap();
    SyncedServers::atom_set(&mut unit, &"missing".into(), None)
        .await
        .unwrap();
    unit.commit().await.unwrap();
    let (head, collections) = manifest::load(&root).await.unwrap();
    assert_eq!(head.revision, snapshot.0.revision + 1);
    let raw = &collections["synced_game_option_state"][&id];
    assert_eq!(raw["record"]["revision"], 2);
    assert_eq!(raw["record"]["future"], "retain");
    assert_eq!(raw["extra"], 42);
}

async fn prepare_cleanup_fixture(root: &Path) -> (Vec<String>, String, String) {
    let mut data = seeded();
    data.get_mut("example").unwrap()["a"]["record"]["document"] =
        json!(vec![13u8; 5400]);
    manifest::initial(root, data, false).await.unwrap();
    let mut tokens = Vec::new();
    for revision in 1..=5 {
        let mut tx = Transaction::begin(root.to_owned(), None).await.unwrap();
        tx.collections.get_mut("example").unwrap()["a"]["record"]["value"] =
            json!(revision);
        tx.changed = true;
        let token = tx.prepare().await.unwrap().unwrap();
        if revision <= 2 {
            // Use the legacy per-generation layout for the cleanup candidates.
            let mut intent =
                journal::read_intent(root, &token, true).await.unwrap();
            let shard = &mut intent.desired.shards[0];
            let body = tokio::fs::read(root.join(&shard.file)).await.unwrap();
            shard.file = format!("state/generations/{token}/0.json");
            crate::state::persistence::document::write_json(
                &root.join(&shard.file),
                body,
            )
            .await
            .unwrap();
            tokio::fs::write(
                root.join(format!("state/journals/{token}.json")),
                serde_json::to_vec(&intent).unwrap(),
            )
            .await
            .unwrap();
        }
        if revision != 5 {
            tx.finish(&token).await.unwrap();
        }
        tokens.push(token);
    }
    let mut tx = Transaction::begin(root.to_owned(), None).await.unwrap();
    tx.changed = true;
    let abandoned = tx.prepare().await.unwrap().unwrap();
    let decision = tokens.pop().unwrap();
    (tokens, decision, abandoned)
}

#[tokio::test]
async fn unchanged_shards_are_shared_across_distinct_generations() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let initial = manifest::initial(root, seeded(), false).await.unwrap();
    let mut tx = Transaction::begin(root.to_owned(), None).await.unwrap();
    tx.collections.insert(
        "other".into(),
        Map::from_iter([("b".into(), json!({"key":"b","record":{"value":2}}))]),
    );
    tx.changed = true;
    let next = tx.prepare().await.unwrap().unwrap();
    tx.finish(&next).await.unwrap();
    let (current, _) = manifest::load(root).await.unwrap();
    assert_ne!(current.transaction, initial.transaction);
    assert_eq!(current.shards[0].file, initial.shards[0].file);
    assert_ne!(current.shards[1].file, initial.shards[0].file);
    assert!(!root.join("state/generations").join(next).exists());
}

#[tokio::test]
async fn cleanup_dry_run_and_resume_keep_previous_and_replayable_decision() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let (tokens, decision, abandoned) = prepare_cleanup_fixture(root).await;
    let journal_path =
        |token: &str| root.join(format!("state/journals/{token}.json"));
    let old_file = root.join(format!("state/generations/{}/0.json", tokens[0]));
    let preview = gc::scan(
        root,
        Some(&decision),
        false,
        None,
        |_| {},
        |_, err| panic!("{err}"),
    )
    .await
    .unwrap();
    assert_eq!(preview.candidates, 2);
    assert_eq!(preview.retained_for_recovery, 3);
    assert_eq!(preview.retained_recent_or_uncommitted, 1);
    assert!(old_file.exists());
    // Simulate a process interrupted after removing a file but before clearing
    // the candidate's journal. The next invocation can finish that generation.
    tokio::fs::remove_file(&old_file).await.unwrap();
    let first = gc::scan(
        root,
        Some(&decision),
        true,
        Some(1),
        |_| {},
        |_, err| panic!("{err}"),
    )
    .await
    .unwrap();
    assert_eq!(first.removed, 1);
    assert!(first.limit_reached);
    let second = gc::scan(
        root,
        Some(&decision),
        true,
        None,
        |_| {},
        |_, err| panic!("{err}"),
    )
    .await
    .unwrap();
    assert_eq!(second.removed, 1);
    for token in &tokens[..2] {
        assert!(!journal_path(token).exists());
    }
    for token in tokens[2..].iter().chain([&decision, &abandoned]) {
        assert!(journal_path(token).exists());
    }

    // Recover from the previous head, then restore the current head to test
    // replays against the same state seen by the coordinator during cleanup.
    let head_path = root.join("state/head.json");
    let current = tokio::fs::read(&head_path).await.unwrap();
    tokio::fs::write(&head_path, b"damaged").await.unwrap();
    assert_eq!(manifest::load(root).await.unwrap().0.revision, 3);
    tokio::fs::write(&head_path, current).await.unwrap();
    journal::replay(root, &decision).await.unwrap();
    let (recovered, data) = manifest::load(root).await.unwrap();
    assert_eq!(recovered.transaction, decision);
    assert_eq!(
        data["example"]["a"]["record"]["document"],
        json!(vec![13u8; 5400])
    );
}

#[tokio::test]
async fn cleanup_retains_unknown_journal_and_stops_on_corrupt_recovery_root() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let (tokens, decision, _) = prepare_cleanup_fixture(root).await;
    let unknown = root.join(format!("state/journals/{}.json", tokens[0]));
    let mut intent: Value =
        serde_json::from_slice(&tokio::fs::read(&unknown).await.unwrap())
            .unwrap();
    intent["schema_version"] = json!(2);
    tokio::fs::write(&unknown, serde_json::to_vec(&intent).unwrap())
        .await
        .unwrap();
    let report =
        gc::scan(root, Some(&decision), false, None, |_| {}, |_, _| {})
            .await
            .unwrap();
    assert_eq!(report.failed, 1);
    assert_eq!(report.candidates, 1);

    let previous = root.join("state/head.json.previous");
    tokio::fs::write(&previous, b"damaged").await.unwrap();
    assert!(
        gc::scan(root, Some(&decision), true, None, |_| {}, |_, _| {})
            .await
            .is_err()
    );
    assert_eq!(tokio::fs::read(previous).await.unwrap(), b"damaged");
    assert!(unknown.exists());
    assert!(
        root.join(format!("state/journals/{}.json", tokens[1]))
            .exists()
    );
}

#[tokio::test]
async fn cleanup_reports_progress_and_resumes_after_a_batch_limit() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let (tokens, decision, _) = prepare_cleanup_fixture(root).await;
    let mut intent =
        journal::read_intent(root, &tokens[0], true).await.unwrap();
    let body = tokio::fs::read(root.join(&intent.desired.shards[0].file))
        .await
        .unwrap();
    for _ in 0..260 {
        let token = uuid::Uuid::new_v4().to_string();
        intent.desired.transaction = token.clone();
        intent.desired.shards[0].file =
            format!("state/generations/{token}/0.json");
        let path = root.join(&intent.desired.shards[0].file);
        tokio::fs::create_dir(path.parent().unwrap()).await.unwrap();
        tokio::fs::write(path, &body).await.unwrap();
        tokio::fs::write(
            root.join(format!("state/journals/{token}.json")),
            serde_json::to_vec(&intent).unwrap(),
        )
        .await
        .unwrap();
    }
    let mut batches = Vec::new();
    let preview = gc::scan(
        root,
        Some(&decision),
        false,
        None,
        |report| batches.push(report.journals_scanned),
        |_, error| panic!("{error}"),
    )
    .await
    .unwrap();
    assert_eq!(preview.candidates, 262);
    assert_eq!(batches, [128, 256]);
    let first = gc::scan(
        root,
        Some(&decision),
        true,
        Some(128),
        |_| {},
        |_, error| panic!("{error}"),
    )
    .await
    .unwrap();
    assert_eq!(first.removed, 128);
    let second = gc::scan(
        root,
        Some(&decision),
        true,
        None,
        |_| {},
        |_, error| panic!("{error}"),
    )
    .await
    .unwrap();
    assert_eq!(second.removed, 134);
    assert_eq!(
        first.removed_files + second.removed_files,
        preview.candidate_files
    );
    assert_eq!(
        first.removed_bytes + second.removed_bytes,
        preview.candidate_bytes
    );
    journal::replay(root, &decision).await.unwrap();
}
