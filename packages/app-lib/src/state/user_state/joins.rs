use super::{Domain, ensure_supported, key, supported};
use crate::state::persistence::Persistence;
use crate::state::store::Handle;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};

pub(crate) struct JoinLog;
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct JoinLogKey {
    pub instance_id: String,
    pub host: String,
    pub port: u16,
}
pub(crate) struct InstanceJoins {
    pub instance_id: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct JoinLogEntry {
    pub join_time: DateTime<Utc>,
}
pub(crate) fn domain(persistence: &Persistence) -> Domain {
    Domain::new(persistence, "joins", "axolotl-server-joins")
}
pub(super) fn decode(raw: &Value) -> Option<(JoinLogKey, JoinLogEntry)> {
    if !supported(raw) {
        return None;
    }
    Some((
        JoinLogKey {
            instance_id: raw.get("instance_id")?.as_str()?.into(),
            host: raw.get("host")?.as_str()?.into(),
            port: raw.get("port")?.as_u64()?.try_into().ok()?,
        },
        JoinLogEntry {
            join_time: DateTime::from_timestamp(
                raw.get("join_time")?.as_i64()?,
                0,
            )?,
        },
    ))
}
impl JoinLog {
    pub(crate) async fn get_scoped(
        handle: &Handle,
        scope: &InstanceJoins,
    ) -> crate::Result<Vec<(JoinLogKey, JoinLogEntry)>> {
        let data = domain(&handle.persistence()).read().await?;
        Ok(data["entries"]
            .as_object()
            .expect("entries")
            .values()
            .filter_map(decode)
            .filter(|(key, _)| key.instance_id == scope.instance_id)
            .collect())
    }
    pub(crate) async fn set(
        handle: &Handle,
        record_key: &JoinLogKey,
        record: Option<&JoinLogEntry>,
    ) -> crate::Result<()> {
        let record_key = record_key.clone();
        let record = record.copied();
        domain(&handle.persistence())
            .update(move |entries| {
                let id = key(&[
                    &record_key.instance_id,
                    &record_key.host,
                    &record_key.port.to_string(),
                ]);
                if let Some(raw) = entries.get(&id) {
                    ensure_supported(raw)?;
                    if decode(raw).is_none() {
                        return Err(crate::ErrorKind::InputError(
                            "Server join record is unreadable".into(),
                        )
                        .into());
                    }
                }
                if let Some(record) = record {
                    let raw = entries.entry(id).or_insert_with(|| json!({}));
                    raw["instance_id"] = json!(record_key.instance_id);
                    raw["host"] = json!(record_key.host);
                    raw["port"] = json!(record_key.port);
                    raw["join_time"] = json!(record.join_time.timestamp());
                } else {
                    entries.remove(&id);
                }
                Ok(())
            })
            .await
    }
}
