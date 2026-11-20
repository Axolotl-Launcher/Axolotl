use super::Persistence;
use super::document::{Document, sidecar};
use super::import::Import;
use super::status::Availability;
use serde_json::{Map, Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn setup() -> (tempfile::TempDir, Persistence, Document) {
    let temporary = tempfile::tempdir().unwrap();
    let persistence = Persistence::new(temporary.path());
    let document = persistence
        .document("state/example.json", "example", 1)
        .unwrap();
    (temporary, persistence, document)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn separate_handles_serialize_updates_without_losing_unknown_fields() {
    let (_temporary, persistence, document) = setup();
    document
        .update(|data| {
            data.insert(
                "future".into(),
                json!({"nested": [1, {"opaque": true}]}),
            );
            Ok(())
        })
        .await
        .unwrap();
    let mut tasks = Vec::new();
    for index in 0..32 {
        let document = persistence
            .document("state/example.json", "example", 1)
            .unwrap();
        tasks.push(tokio::spawn(async move {
            document
                .update(move |data| {
                    data.insert(format!("key-{index}"), json!(index));
                    Ok(())
                })
                .await
                .unwrap();
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    let snapshot = document.read().await.unwrap().snapshot.unwrap();
    assert_eq!(snapshot.revision(), Some(33));
    assert_eq!(snapshot.data().unwrap().len(), 33);
    assert_eq!(
        snapshot.data().unwrap()["future"],
        json!({"nested": [1, {"opaque": true}]})
    );
}

#[tokio::test]
async fn updates_preserve_envelope_and_nested_unknown_fields() {
    let (_temporary, _persistence, document) = setup();
    std::fs::create_dir_all(document.path().parent().unwrap()).unwrap();
    std::fs::write(document.path(), serde_json::to_vec(&json!({
        "protocol":"example", "schema_version":1, "revision":4,
        "future_envelope":{"keep":true}, "data":{"nested":{"known":1,"future":[1,2]}}
    })).unwrap()).unwrap();
    document
        .update(|data| {
            data["nested"]["known"] = json!(2);
            Ok(())
        })
        .await
        .unwrap();
    let raw: Value =
        serde_json::from_slice(&std::fs::read(document.path()).unwrap())
            .unwrap();
    assert_eq!(raw["future_envelope"], json!({"keep":true}));
    assert_eq!(raw["data"]["nested"], json!({"known":2,"future":[1,2]}));
}

#[tokio::test]
async fn future_schema_and_protocol_remain_byte_for_byte_read_only() {
    for (protocol, version) in [("example", 2), ("new-format", 1)] {
        let (_temporary, _persistence, document) = setup();
        std::fs::create_dir_all(document.path().parent().unwrap()).unwrap();
        let bytes = serde_json::to_vec(&json!({"protocol":protocol,"schema_version":version,"revision":1,"data":{},"opaque":true})).unwrap();
        std::fs::write(document.path(), &bytes).unwrap();
        assert!(matches!(
            document.read().await.unwrap().availability,
            Availability::ReadOnly(_)
        ));
        assert!(
            document
                .update(|data| {
                    data.insert("change".into(), json!(1));
                    Ok(())
                })
                .await
                .is_err()
        );
        assert_eq!(std::fs::read(document.path()).unwrap(), bytes);
        assert!(!sidecar(document.path(), ".previous").exists());
    }
}

#[tokio::test]
async fn corrupt_document_recovers_previous_good_copy_and_preserves_original() {
    let (_temporary, _persistence, document) = setup();
    for value in [1, 2] {
        document
            .update(move |data| {
                data.insert("value".into(), json!(value));
                Ok(())
            })
            .await
            .unwrap();
    }
    std::fs::write(document.path(), b"{broken").unwrap();
    let recovered = document.read().await.unwrap();
    assert_eq!(recovered.availability, Availability::Recovered);
    assert_eq!(
        recovered.snapshot.unwrap().data().unwrap()["value"],
        json!(1)
    );
    let archives: Vec<_> = std::fs::read_dir(document.path().parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.to_string_lossy().contains(".corrupt-"))
        .collect();
    assert_eq!(archives.len(), 1);
    assert_eq!(std::fs::read(&archives[0]).unwrap(), b"{broken");
}

#[tokio::test]
async fn damaged_document_is_isolated_and_cannot_be_silently_reset() {
    let (_temporary, persistence, document) = setup();
    std::fs::create_dir_all(document.path().parent().unwrap()).unwrap();
    std::fs::write(document.path(), b"broken").unwrap();
    assert!(matches!(
        document.read().await.unwrap().availability,
        Availability::Unavailable(_)
    ));
    assert!(document.update(|_| Ok(())).await.is_err());
    let other = persistence
        .document("state/other.json", "other", 1)
        .unwrap();
    other
        .update(|data| {
            data.insert("works".into(), json!(true));
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(
        other.read().await.unwrap().availability,
        Availability::Ready
    );
    assert_eq!(std::fs::read(document.path()).unwrap(), b"broken");
}

#[tokio::test]
async fn missing_primary_with_broken_backup_is_unavailable_instead_of_empty() {
    let (_temporary, _persistence, document) = setup();
    std::fs::create_dir_all(document.path().parent().unwrap()).unwrap();
    std::fs::write(sidecar(document.path(), ".previous"), b"broken").unwrap();
    assert!(matches!(
        document.read().await.unwrap().availability,
        Availability::Unavailable(_)
    ));
    assert!(document.update(|_| Ok(())).await.is_err());
    assert!(!document.path().exists());
}

#[tokio::test]
async fn failed_update_keeps_published_bytes_and_revision() {
    let (_temporary, _persistence, document) = setup();
    document
        .update(|data| {
            data.insert("value".into(), json!(1));
            Ok(())
        })
        .await
        .unwrap();
    let bytes = std::fs::read(document.path()).unwrap();
    assert!(
        document
            .update(|data| {
                data.insert("value".into(), json!(2));
                Err(crate::ErrorKind::InputError("validation failed".into())
                    .into())
            })
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(document.path()).unwrap(), bytes);
}

#[tokio::test]
async fn oversized_document_is_not_loaded_or_overwritten() {
    let (_temporary, _persistence, document) = setup();
    std::fs::create_dir_all(document.path().parent().unwrap()).unwrap();
    let file = std::fs::File::create(document.path()).unwrap();
    file.set_len((16 << 20) + 1).unwrap();
    assert!(matches!(
        document.read().await.unwrap().availability,
        Availability::Unavailable(_)
    ));
    assert!(document.update(|_| Ok(())).await.is_err());
    assert_eq!(
        std::fs::metadata(document.path()).unwrap().len(),
        (16 << 20) + 1
    );
}

fn validate(snapshot: &super::document::Snapshot) -> crate::Result<()> {
    let data: Map<String, Value> = snapshot.decode()?;
    if !data.contains_key("value") {
        return Err(
            crate::ErrorKind::InputError("value is required".into()).into()
        );
    }
    Ok(())
}

#[tokio::test]
async fn imports_retry_source_failure_and_prefer_existing_new_authority() {
    let (_temporary, persistence, document) = setup();
    let import =
        Import::new(&persistence, "example", document.clone()).unwrap();
    assert!(
        import
            .run(
                || async {
                    Err(crate::ErrorKind::InputError(
                        "source unavailable".into(),
                    )
                    .into())
                },
                validate
            )
            .await
            .is_err()
    );
    let imported = import
        .run(
            || async { Ok(Map::from_iter([("value".into(), json!(1))])) },
            validate,
        )
        .await
        .unwrap();
    assert_eq!(
        imported.snapshot.unwrap().data().unwrap()["value"],
        json!(1)
    );
    document
        .update(|data| {
            data.insert("value".into(), json!(9));
            Ok(())
        })
        .await
        .unwrap();
    let result = import
        .run(
            || async { panic!("completed imports must not read old source") },
            validate,
        )
        .await
        .unwrap();
    assert_eq!(result.snapshot.unwrap().data().unwrap()["value"], json!(9));
}

#[tokio::test]
async fn publication_without_receipt_resumes_without_reading_legacy_snapshot() {
    let (_temporary, persistence, document) = setup();
    // Simulates termination after target publication, before receipt publication.
    document
        .update(|data| {
            data.insert("value".into(), json!(7));
            Ok(())
        })
        .await
        .unwrap();
    let import = Import::new(&persistence, "example", document).unwrap();
    let result = import
        .run(|| async { panic!("published target wins") }, validate)
        .await
        .unwrap();
    assert_eq!(result.snapshot.unwrap().data().unwrap()["value"], json!(7));
    assert!(
        persistence
            .root()
            .join("data/persistence/imports/example.json")
            .is_file()
    );
}

#[tokio::test]
async fn completed_handover_never_resurrects_deleted_or_corrupt_authority() {
    for corrupt in [false, true] {
        let (_temporary, persistence, document) = setup();
        let import =
            Import::new(&persistence, "example", document.clone()).unwrap();
        import
            .run(
                || async { Ok(Map::from_iter([("value".into(), json!(1))])) },
                validate,
            )
            .await
            .unwrap();
        if corrupt {
            std::fs::write(document.path(), b"broken").unwrap();
        } else {
            std::fs::remove_file(document.path()).unwrap();
        }
        let result = import
            .run(
                || async { panic!("old data must not be resurrected") },
                validate,
            )
            .await
            .unwrap();
        assert!(result.snapshot.is_none());
        assert_eq!(document.path().exists(), corrupt);
    }
}

#[tokio::test]
async fn corrupt_or_future_receipt_never_reopens_legacy_source() {
    for bytes in [
        b"broken".as_slice(),
        br#"{"protocol":"future","schema_version":99}"#.as_slice(),
    ] {
        let (_temporary, persistence, document) = setup();
        let marker = persistence
            .root()
            .join("data/persistence/imports/example.json");
        std::fs::create_dir_all(marker.parent().unwrap()).unwrap();
        std::fs::write(&marker, bytes).unwrap();
        let import =
            Import::new(&persistence, "example", document.clone()).unwrap();
        let result = import
            .run(
                || async { panic!("opaque receipt forbids old import") },
                validate,
            )
            .await
            .unwrap();
        assert!(matches!(result.availability, Availability::Unavailable(_)));
        assert!(!document.path().exists());
        assert_eq!(std::fs::read(&marker).unwrap(), bytes);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_imports_publish_once() {
    let (_temporary, persistence, document) = setup();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let import =
            Import::new(&persistence, "example", document.clone()).unwrap();
        let calls = calls.clone();
        tasks.push(tokio::spawn(async move {
            import
                .run(
                    move || async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        Ok(Map::from_iter([("value".into(), json!(1))]))
                    },
                    validate,
                )
                .await
                .unwrap();
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn state_paths_and_import_names_cannot_escape_or_alias_receipts() {
    let (_temporary, persistence, document) = setup();
    for path in [
        "../outside.json",
        "C:/outside.json",
        "state/../../outside.json",
        "state/file.json:stream",
        "state/file. ",
    ] {
        assert!(persistence.document(path, "test", 1).is_err(), "{path}");
    }
    assert!(Import::new(&persistence, "../accounts", document).is_err());
    let marker = persistence
        .document("data/persistence/imports/example.json", "test", 1)
        .unwrap();
    assert!(Import::new(&persistence, "example", marker).is_err());
}

#[tokio::test]
async fn invalid_new_authority_wins_over_legacy_even_without_receipt() {
    let (_temporary, persistence, document) = setup();
    std::fs::create_dir_all(document.path().parent().unwrap()).unwrap();
    std::fs::write(document.path(), b"broken").unwrap();
    let import =
        Import::new(&persistence, "example", document.clone()).unwrap();
    let result = import
        .run(
            || async {
                panic!("new authority must not be replaced from legacy")
            },
            validate,
        )
        .await
        .unwrap();
    assert!(matches!(result.availability, Availability::Unavailable(_)));
    assert_eq!(std::fs::read(document.path()).unwrap(), b"broken");
    assert!(
        !persistence
            .root()
            .join("data/persistence/imports/example.json")
            .exists()
    );
}

#[tokio::test]
async fn snapshots_include_receipt_and_current_authority_and_refuse_missing_data()
 {
    let (_temporary, persistence, document) = setup();
    let import =
        Import::new(&persistence, "example", document.clone()).unwrap();
    import
        .run(
            || async { Ok(Map::from_iter([("value".into(), json!(1))])) },
            validate,
        )
        .await
        .unwrap();
    document
        .update(|data| {
            data.insert("value".into(), json!(2));
            Ok(())
        })
        .await
        .unwrap();
    let files = super::import::snapshot(&persistence, "example")
        .await
        .unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(
        files[0].0,
        std::path::Path::new("data/persistence/imports/example.json")
    );
    assert_eq!(files[1].0, std::path::Path::new("state/example.json"));
    assert_eq!(files[1].1, std::fs::read(document.path()).unwrap());
    std::fs::remove_file(document.path()).unwrap();
    assert!(
        super::import::snapshot(&persistence, "example")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn canceled_caller_cannot_release_a_worker_mid_update() {
    let (_temporary, _persistence, document) = setup();
    let (started, receiver) = tokio::sync::oneshot::channel();
    let (release, wait) = std::sync::mpsc::channel();
    let first = document.clone();
    let task = tokio::spawn(async move {
        first
            .update(move |data| {
                let _ = started.send(());
                wait.recv().unwrap();
                data.insert("first".into(), json!(true));
                Ok(())
            })
            .await
    });
    receiver.await.unwrap();
    task.abort();
    let second = document.clone();
    let task = tokio::spawn(async move {
        second
            .update(|data| {
                data.insert("second".into(), json!(true));
                Ok(())
            })
            .await
    });
    release.send(()).unwrap();
    task.await.unwrap().unwrap();
    let data = document.read().await.unwrap().snapshot.unwrap();
    assert_eq!(data.data().unwrap()["first"], json!(true));
    assert_eq!(data.data().unwrap()["second"], json!(true));
}

#[tokio::test]
async fn deleted_fields_are_not_reimported_from_the_old_snapshot() {
    let (_temporary, persistence, document) = setup();
    let import =
        Import::new(&persistence, "example", document.clone()).unwrap();
    import
        .run(
            || async { Ok(Map::from_iter([("value".into(), json!(1))])) },
            |_| Ok(()),
        )
        .await
        .unwrap();
    document
        .update(|data| {
            data.remove("value");
            Ok(())
        })
        .await
        .unwrap();
    let result = import
        .run(
            || async { panic!("deleted values must remain deleted") },
            |_| Ok(()),
        )
        .await
        .unwrap();
    assert!(
        result
            .snapshot
            .unwrap()
            .data()
            .unwrap()
            .get("value")
            .is_none()
    );
}
