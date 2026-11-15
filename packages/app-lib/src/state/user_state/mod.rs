//! Independent user choices. Native files remain the authority for file facts.
pub(crate) mod favorites;
pub(crate) mod joins;
pub(crate) mod shared_logs;
#[cfg(test)]
mod tests;
pub(crate) mod worlds;

use super::persistence::{
    Persistence, document::Document, status::Availability,
};
use serde_json::{Map, Value};

#[derive(Clone)]
pub(crate) struct Domain {
    pub(crate) document: Document,
}
impl Domain {
    pub(crate) fn new(
        persistence: &Persistence,
        name: &str,
        protocol: &'static str,
    ) -> Self {
        Self {
            document: persistence
                .document(format!("state/{name}.json"), protocol, 1)
                .expect("constant user-state path"),
        }
    }
    pub(crate) async fn read(&self) -> crate::Result<Map<String, Value>> {
        let read = self.document.read().await?;
        if !matches!(
            read.availability,
            Availability::Ready | Availability::Recovered
        ) {
            return Err(crate::ErrorKind::FSError(
                "User state is unavailable or uses a newer protocol".into(),
            )
            .into());
        }
        let data = read
            .snapshot
            .expect("readable document")
            .data()
            .expect("payload")
            .clone();
        validate(&data)?;
        Ok(data)
    }
    pub(crate) async fn update<F>(&self, change: F) -> crate::Result<()>
    where
        F: FnOnce(&mut Map<String, Value>) -> crate::Result<()>
            + Send
            + 'static,
    {
        self.document
            .update(move |data| {
                validate(data)?;
                change(entries(data))
            })
            .await?;
        Ok(())
    }
}
pub(crate) fn validate(data: &Map<String, Value>) -> crate::Result<()> {
    if !data.get("entries").is_some_and(Value::is_object) {
        return Err(crate::ErrorKind::InputError(
            "User-state entries are unreadable".into(),
        )
        .into());
    }
    Ok(())
}
pub(crate) fn entries(
    data: &mut Map<String, Value>,
) -> &mut Map<String, Value> {
    data.get_mut("entries")
        .and_then(Value::as_object_mut)
        .expect("validated entries")
}
pub(crate) fn supported(raw: &Value) -> bool {
    raw.is_object()
        && raw.get("protocol").is_none()
        && ["schema_version", "schemaVersion"].into_iter().all(|key| {
            raw.get(key).is_none_or(|value| value.as_u64() == Some(1))
        })
}
pub(crate) fn ensure_supported(raw: &Value) -> crate::Result<()> {
    if supported(raw) {
        Ok(())
    } else {
        Err(crate::ErrorKind::InputError(
            "This record uses a newer data format; it was preserved".into(),
        )
        .into())
    }
}
pub(crate) fn key(parts: &[&str]) -> String {
    serde_json::to_string(parts).expect("string tuple")
}

pub(crate) async fn forget_instance(persistence: &Persistence, instance: &str) {
    for (kind, domain) in [
        ("worlds", worlds::domain(persistence)),
        ("joins", joins::domain(persistence)),
        ("pins", favorites::pins(persistence)),
    ] {
        let id = instance.to_owned();
        if let Err(error) = domain.update(move |entries| {
            entries.retain(|_, raw| {
                let understood=match kind {"worlds"=>worlds::decode(raw).is_some(),"joins"=>joins::decode(raw).is_some(),_=>supported(raw)&&raw.get("pinned_at").and_then(Value::as_i64).and_then(|time|chrono::DateTime::<chrono::Utc>::from_timestamp(time,0)).is_some()};
                !understood || raw.get("instance_id").and_then(Value::as_str) != Some(&id)
            });
            Ok(())
        }).await { tracing::warn!(%error, "Instance user-state cleanup deferred; records retained"); }
    }
}
