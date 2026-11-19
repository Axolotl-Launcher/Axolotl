use super::orchestration::{SyncedOptionJoinResolution, synced_options_path};
use crate::State;
use crate::state::SyncedOption;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub(super) enum PendingAction {
    SelectSource,
    Join {
        resolution: Option<SyncedOptionJoinResolution>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct PendingChange {
    #[serde(skip)]
    pub observation_id: Option<String>,
    pub instance_id: String,
    pub option: SyncedOption,
    pub action: PendingAction,
}

const PROTOCOL: &str = "axolotl-sync-pending";
const MAX_PENDING_BYTES: u64 = 4 * 1024 * 1024;

fn document(
    persistence: &crate::state::persistence::Persistence,
) -> crate::state::persistence::document::Document {
    persistence
        .document(crate::state::layout::SYNC_PENDING, PROTOCOL, 1)
        .expect("constant pending path")
}

fn decode(raw: &serde_json::Value) -> Option<PendingChange> {
    let supported = |value: &serde_json::Value| {
        value.is_object()
            && value.get("protocol").is_none()
            && ["schema_version", "schemaVersion"].into_iter().all(|key| {
                value
                    .get(key)
                    .is_none_or(|version| version.as_u64() == Some(1))
            })
    };
    if !supported(raw) || !supported(raw.get("action")?) {
        return None;
    }
    serde_json::from_value(raw.clone()).ok()
}

async fn initialize(
    state: &State,
) -> crate::Result<crate::state::persistence::document::Document> {
    use crate::state::persistence::{import::Import, status::Availability};
    use serde_json::{Map, Value, json};
    use tokio::io::AsyncReadExt;
    let persistence = state.persistence.clone();
    let target = document(&persistence);
    let legacy = synced_options_path(state).join("pending-changes.json");
    let result = Import::new(&persistence, "sync-pending", target.clone())?
        .run(
            move || async move {
                let rows: Vec<Value> = match tokio::fs::File::open(&legacy)
                    .await
                {
                    Ok(file) => {
                        if file.metadata().await?.len() > MAX_PENDING_BYTES {
                            return Err(crate::ErrorKind::InputError(
                                "Pending sync requests exceed size limit"
                                    .into(),
                            )
                            .into());
                        }
                        let mut bytes = Vec::new();
                        file.take(MAX_PENDING_BYTES + 1)
                            .read_to_end(&mut bytes)
                            .await?;
                        if bytes.len() as u64 > MAX_PENDING_BYTES {
                            return Err(crate::ErrorKind::InputError(
                                "Pending sync requests grew beyond size limit"
                                    .into(),
                            )
                            .into());
                        }
                        serde_json::from_slice(&bytes)?
                    }
                    Err(error)
                        if error.kind() == std::io::ErrorKind::NotFound =>
                    {
                        Vec::new()
                    }
                    Err(error) => return Err(error.into()),
                };
                let entries = rows
                    .into_iter()
                    .enumerate()
                    .map(|(index, row)| (index.to_string(), row))
                    .collect::<Map<_, _>>();
                Ok(Map::from_iter([("entries".into(), json!(entries))]))
            },
            |snapshot| {
                if !snapshot.data().is_some_and(|data| {
                    data.get("entries").is_some_and(Value::is_object)
                }) {
                    return Err(crate::ErrorKind::InputError(
                        "Pending sync document is unreadable".into(),
                    )
                    .into());
                }
                Ok(())
            },
        )
        .await?;
    if !matches!(
        result.availability,
        Availability::Ready | Availability::Recovered
    ) {
        return Err(crate::ErrorKind::FSError(
            "Pending sync requests unavailable or newer".into(),
        )
        .into());
    }
    Ok(target)
}

pub(super) async fn read(state: &State) -> crate::Result<Vec<PendingChange>> {
    let document = initialize(state).await?;
    let read = document.read().await?;
    if !matches!(
        read.availability,
        crate::state::persistence::status::Availability::Ready
            | crate::state::persistence::status::Availability::Recovered
    ) {
        return Err(crate::ErrorKind::FSError(
            "Pending sync requests unavailable or newer".into(),
        )
        .into());
    }
    let data = read
        .snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.data())
        .ok_or_else(|| {
            crate::ErrorKind::FSError(
                "Pending sync requests unavailable".into(),
            )
        })?;
    let entries = data
        .get("entries")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            crate::ErrorKind::FSError("Pending sync entries unavailable".into())
        })?;
    Ok(entries
        .iter()
        .filter_map(|(id, raw)| {
            let mut change = decode(raw)?;
            change.observation_id = Some(id.clone());
            Some(change)
        })
        .collect())
}

/// Mutations affect only understood actions. A newer action remains opaque and
/// durable while this version can still replay independent supported requests.
pub(super) async fn save(
    change: PendingChange,
    state: &State,
) -> crate::Result<()> {
    initialize(state)
        .await?
        .update(move |data| {
            let entries = data
                .get_mut("entries")
                .and_then(serde_json::Value::as_object_mut)
                .ok_or_else(|| {
                    crate::ErrorKind::FSError(
                        "Pending sync entries unavailable".into(),
                    )
                })?;
            save_entry(entries, change)
        })
        .await?;
    Ok(())
}

pub(super) async fn remove(
    change: &PendingChange,
    state: &State,
) -> crate::Result<()> {
    let change = change.clone();
    initialize(state)
        .await?
        .update(move |data| {
            let entries = data
                .get_mut("entries")
                .and_then(serde_json::Value::as_object_mut)
                .ok_or_else(|| {
                    crate::ErrorKind::FSError(
                        "Pending sync entries unavailable".into(),
                    )
                })?;
            remove_entry(entries, &change);
            Ok(())
        })
        .await?;
    Ok(())
}

fn save_entry(
    entries: &mut serde_json::Map<String, serde_json::Value>,
    change: PendingChange,
) -> crate::Result<()> {
    // Preserve extension fields on an understood request when its known
    // fields change. Each publication still receives a fresh replay receipt.
    let mut replacement = entries
        .values()
        .find(|raw| {
            decode(raw).is_some_and(|previous| {
                previous.instance_id == change.instance_id
                    && previous.option == change.option
                    && std::mem::discriminant(&previous.action)
                        == std::mem::discriminant(&change.action)
            })
        })
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let known = serde_json::to_value(&change)?;
    let replacement_object =
        replacement.as_object_mut().expect("object request");
    for (key, value) in known.as_object().expect("serialized request") {
        if key == "action" {
            let action = replacement_object
                .entry(key.clone())
                .or_insert_with(|| serde_json::json!({}));
            let action = action.as_object_mut().expect("decoded action object");
            for (field, value) in value.as_object().expect("serialized action")
            {
                action.insert(field.clone(), value.clone());
            }
        } else {
            replacement_object.insert(key.clone(), value.clone());
        }
    }
    entries.retain(|_, raw| {
        decode(raw).is_none_or(|previous| {
            previous.option != change.option
                || (change.action != PendingAction::SelectSource
                    && previous.instance_id != change.instance_id)
        })
    });
    entries.insert(uuid::Uuid::new_v4().to_string(), replacement);
    Ok(())
}

fn remove_entry(
    entries: &mut serde_json::Map<String, serde_json::Value>,
    change: &PendingChange,
) {
    entries.retain(|id, raw| {
        if change
            .observation_id
            .as_ref()
            .is_some_and(|expected| expected != id)
        {
            return true;
        }
        decode(raw).is_none_or(|current| {
            current.instance_id != change.instance_id
                || current.option != change.option
                || current.action != change.action
        })
    });
}

pub(super) async fn cancel(
    option: SyncedOption,
    instance_id: Option<&str>,
    state: &State,
) -> crate::Result<()> {
    let instance_id = instance_id.map(str::to_owned);
    initialize(state)
        .await?
        .update(move |data| {
            let entries = data
                .get_mut("entries")
                .and_then(serde_json::Value::as_object_mut)
                .ok_or_else(|| {
                    crate::ErrorKind::FSError(
                        "Pending sync entries unavailable".into(),
                    )
                })?;
            entries.retain(|_, raw| {
                decode(raw).is_none_or(|change| {
                    change.option != option
                        || instance_id
                            .as_ref()
                            .is_some_and(|id| change.instance_id != *id)
                })
            });
            Ok(())
        })
        .await?;
    Ok(())
}

pub(in crate::api::instance) async fn contains(
    instance_id: &str,
    option: SyncedOption,
    state: &State,
) -> crate::Result<bool> {
    Ok(read(state).await?.iter().any(|change| {
        change.instance_id == instance_id && change.option == option
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(instance: &str) -> PendingChange {
        PendingChange {
            observation_id: None,
            instance_id: instance.into(),
            option: SyncedOption::CommandHistory,
            action: PendingAction::Join { resolution: None },
        }
    }

    #[test]
    fn future_actions_and_versions_stay_opaque() {
        let mut raw = serde_json::to_value(request("instance")).unwrap();
        raw["future_hint"] = json!(true);
        assert!(decode(&raw).is_some());
        raw["action"]["action"] = json!("future");
        assert!(decode(&raw).is_none());
        raw = serde_json::to_value(request("instance")).unwrap();
        raw["schemaVersion"] = json!(2);
        assert!(decode(&raw).is_none());
        raw.as_object_mut().unwrap().remove("schemaVersion");
        raw["action"]["schema_version"] = json!(2);
        assert!(decode(&raw).is_none());
    }

    #[test]
    fn replacing_and_acknowledging_supported_request_retains_opaque_data() {
        let mut raw = serde_json::to_value(request("instance")).unwrap();
        raw["future_hint"] = json!({"keep":true});
        raw["action"]["future_action_hint"] = json!([1, 2]);
        let future = json!({"action":{"action":"future"},"opaque":true});
        let mut entries = serde_json::Map::from_iter([
            ("observed".into(), raw),
            ("future".into(), future.clone()),
        ]);
        let mut observed = request("instance");
        observed.observation_id = Some("observed".into());
        save_entry(&mut entries, request("instance")).unwrap();
        remove_entry(&mut entries, &observed);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries["future"], future);
        let (id, replacement) =
            entries.iter().find(|(id, _)| *id != "future").unwrap();
        assert_eq!(replacement["future_hint"], json!({"keep":true}));
        assert_eq!(replacement["action"]["future_action_hint"], json!([1, 2]));
        let mut current = decode(replacement).unwrap();
        current.observation_id = Some(id.clone());
        remove_entry(&mut entries, &current);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries["future"], future);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_requests_keep_independent_instances_and_opaque_entries()
    {
        let temporary = tempfile::tempdir().unwrap();
        let persistence =
            crate::state::persistence::Persistence::new(temporary.path());
        let target = document(&persistence);
        target
            .update(|data| {
                data.insert(
                    "entries".into(),
                    json!({"future":{"action":{"action":"future"}}}),
                );
                Ok(())
            })
            .await
            .unwrap();
        let mut tasks = Vec::new();
        for index in 0..16 {
            let target = document(&persistence);
            tasks.push(tokio::spawn(async move {
                target
                    .update(move |data| {
                        let entries = data
                            .get_mut("entries")
                            .unwrap()
                            .as_object_mut()
                            .unwrap();
                        save_entry(entries, request(&index.to_string()))
                    })
                    .await
                    .unwrap();
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        let snapshot = target.read().await.unwrap().snapshot.unwrap();
        let entries = snapshot.data().unwrap()["entries"].as_object().unwrap();
        assert_eq!(entries.len(), 17);
        assert_eq!(
            entries.values().filter(|raw| decode(raw).is_some()).count(),
            16
        );
        assert_eq!(entries["future"], json!({"action":{"action":"future"}}));
    }

    #[tokio::test]
    async fn pending_import_receipt_prevents_deleted_requests_from_returning() {
        use crate::state::persistence::import::Import;
        let temporary = tempfile::tempdir().unwrap();
        let persistence =
            crate::state::persistence::Persistence::new(temporary.path());
        let target = document(&persistence);
        let import =
            Import::new(&persistence, "sync-pending", target.clone()).unwrap();
        import.run(|| async {
            Ok(serde_json::Map::from_iter([("entries".into(), json!({
                "legacy": serde_json::to_value(request("legacy")).unwrap()
            }))]))
        }, |_| Ok(())).await.unwrap();
        target
            .update(|data| {
                data.insert("entries".into(), json!({}));
                Ok(())
            })
            .await
            .unwrap();
        let read = import
            .run(
                || async {
                    panic!("retired legacy source must not load again")
                },
                |_| Ok(()),
            )
            .await
            .unwrap();
        assert_eq!(
            read.snapshot.unwrap().data().unwrap()["entries"],
            json!({})
        );
    }
}
