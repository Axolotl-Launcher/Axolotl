//! Share metadata is a document; deletion credentials never enter that file.
use super::{Domain, ensure_supported, entries, supported, validate};
use crate::state::persistence::Persistence;
use crate::state::store::{Handle, secrets};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

pub(crate) struct SharedLogs;
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SharedLogRecord {
    pub url: String,
    pub raw: String,
    pub token: String,
    pub provider: String,
    pub instance_id: Option<String>,
    pub instance_name: Option<String>,
    pub truncated: bool,
    pub created_at: i64,
}
#[derive(Serialize, Deserialize)]
struct Metadata {
    url: String,
    raw: String,
    provider: String,
    instance_id: Option<String>,
    instance_name: Option<String>,
    truncated: bool,
    created_at: i64,
    #[serde(default)]
    token_ref: Option<String>,
}
pub(crate) fn domain(persistence: &Persistence) -> Domain {
    Domain::new(persistence, "shared_logs", "axolotl-shared-logs")
}
fn sessions() -> &'static Mutex<HashMap<String, String>> {
    static VALUES: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(HashMap::new()))
}
fn token_prefix(persistence: &Persistence, id: &str) -> String {
    let root = format!(
        "{:x}",
        Sha256::digest(persistence.root().to_string_lossy().as_bytes())
    );
    format!("{}:", secrets::key(&["shared-log", &root, id]))
}
fn valid_token_ref(
    persistence: &Persistence,
    id: &str,
    reference: &str,
) -> bool {
    reference
        .strip_prefix(&token_prefix(persistence, id))
        .and_then(|generation| {
            uuid::Uuid::parse_str(generation)
                .ok()
                .map(|uuid| uuid.to_string() == generation)
        })
        .unwrap_or(false)
}
pub(crate) fn new_token_ref(persistence: &Persistence, id: &str) -> String {
    format!("{}{}", token_prefix(persistence, id), uuid::Uuid::new_v4())
}
pub(crate) fn keep_token(access: secrets::Access, key: &str, token: &str) {
    if access.write(key, Some(token)).is_ok() {
        sessions().lock().expect("session tokens").remove(key);
    } else {
        sessions()
            .lock()
            .expect("session tokens")
            .insert(key.to_owned(), token.to_owned());
    }
}
fn read_token(access: secrets::Access, key: &str) -> String {
    if let Some(value) =
        sessions().lock().expect("session tokens").get(key).cloned()
    {
        return value;
    }
    access.read(key).ok().flatten().unwrap_or_default()
}
impl SharedLogs {
    pub(crate) async fn get_all(
        handle: &Handle,
    ) -> crate::Result<Vec<(String, SharedLogRecord)>> {
        let access = secrets::access();
        let persistence = handle.persistence();
        domain(&persistence).document.inspect(move|read|{
            if !matches!(read.availability,crate::state::persistence::status::Availability::Ready|crate::state::persistence::status::Availability::Recovered){return Err(crate::ErrorKind::FSError("Share history unavailable".into()).into());}
            let snapshot=read.snapshot.expect("readable history");let data=snapshot.data().expect("payload");validate(data)?;
            let mut logs=Vec::new();
            for(id,raw)in data["entries"].as_object().expect("entries") {
                if !supported(raw){continue;}
                let Ok(metadata)=serde_json::from_value::<Metadata>(raw.clone())else{continue;};
                let token=metadata.token_ref.as_deref().filter(|key|valid_token_ref(&persistence,id,key)).map(|key|read_token(access,key)).unwrap_or_default();
                logs.push((id.clone(),SharedLogRecord{url:metadata.url,raw:metadata.raw,token,provider:metadata.provider,instance_id:metadata.instance_id,instance_name:metadata.instance_name,truncated:metadata.truncated,created_at:metadata.created_at}));
            }
            logs.sort_by(|a,b|b.1.created_at.cmp(&a.1.created_at).then_with(||a.0.cmp(&b.0)));Ok(logs)
        }).await
    }
    pub(crate) async fn set(
        handle: &Handle,
        id: &String,
        record: Option<&SharedLogRecord>,
    ) -> crate::Result<()> {
        let persistence = handle.persistence();
        let service = domain(&persistence);
        let access = secrets::access();
        let id = id.clone();
        let record = record.cloned();
        let new_ref = record
            .as_ref()
            .filter(|record| !record.token.is_empty())
            .map(|_| new_token_ref(&persistence, &id));
        let credential = new_ref
            .clone()
            .zip(record.as_ref().map(|record| record.token.clone()));
        // The completion callback runs under the same document lock, so a
        // concurrent deletion cannot be followed by an orphan credential write.
        let removed = std::sync::Arc::new(Mutex::new(None::<String>));
        let cleanup = removed.clone();
        let trusted_root = persistence.clone();
        service
            .document
            .update_and_then(
                move |data| {
                    validate(data)?;
                    let entries = entries(data);
                    if let Some(raw) = entries.get(&id) {
                        ensure_supported(raw)?;
                        let metadata: Metadata = serde_json::from_value(raw.clone())?;
                        if metadata.token_ref.as_deref().is_some_and(|reference|!valid_token_ref(&trusted_root,&id,reference)) {
                            return Err(crate::ErrorKind::InputError("Shared-log credential reference is invalid; record preserved".into()).into());
                        }
                    }
                    if let Some(record) = record {
                        let raw = entries.entry(id).or_insert_with(
                            || json!({"created_at":record.created_at}),
                        );
                        raw["url"] = json!(record.url);
                        raw["raw"] = json!(record.raw);
                        raw["provider"] = json!(record.provider);
                        raw["instance_id"] = json!(record.instance_id);
                        raw["instance_name"] = json!(record.instance_name);
                        raw["truncated"] = json!(record.truncated);
                        if let Some(key) = new_ref {
                            *removed.lock().expect("replaced credential") = raw
                                .get("token_ref")
                                .and_then(Value::as_str)
                                .map(str::to_owned);
                            raw["token_ref"] = json!(key);
                        }
                    } else if let Some(raw) = entries.remove(&id) {
                        *removed.lock().expect("removed credential") = raw
                            .get("token_ref")
                            .and_then(Value::as_str)
                            .map(str::to_owned);
                    }
                    Ok(())
                },
                move |_| {
                    if let Some((key, token)) = credential {
                        keep_token(access, &key, &token);
                    }
                    if let Some(key) =
                        cleanup.lock().expect("removed credential").take()
                    {
                        sessions().lock().expect("session tokens").remove(&key);
                        let _ = access.write(&key, None);
                    }
                    Ok(())
                },
            )
            .await?;
        Ok(())
    }
}
