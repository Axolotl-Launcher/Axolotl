use super::{Domain, ensure_supported, key, supported};
use crate::state::persistence::Persistence;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;

pub(crate) fn domain(persistence: &Persistence) -> Domain {
    Domain::new(
        persistence,
        "resource_favorites",
        "axolotl-resource-favorites",
    )
}
pub(crate) fn pins(persistence: &Persistence) -> Domain {
    Domain::new(persistence, "instance_pins", "axolotl-instance-pins")
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Favorite {
    pub source: String,
    pub project_id: String,
    pub content_type: String,
    pub saved_at: i64,
}
pub(crate) async fn all(
    persistence: &Persistence,
) -> crate::Result<Vec<Favorite>> {
    let data = domain(persistence).read().await?;
    let mut entries = data["entries"]
        .as_object()
        .expect("entries")
        .values()
        .filter(|raw| supported(raw))
        .filter_map(|raw| serde_json::from_value::<Favorite>(raw.clone()).ok())
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| {
        b.saved_at.cmp(&a.saved_at).then_with(|| {
            (&a.source, &a.project_id).cmp(&(&b.source, &b.project_id))
        })
    });
    Ok(entries)
}
pub(crate) async fn save(
    persistence: &Persistence,
    record: Favorite,
) -> crate::Result<()> {
    domain(persistence)
        .update(move |entries| {
            let id = key(&[&record.source, &record.project_id]);
            if let Some(raw) = entries.get(&id) {
                ensure_supported(raw)?;
                let _: Favorite = serde_json::from_value(raw.clone())?;
            }
            let raw = entries
                .entry(id)
                .or_insert_with(|| json!({"saved_at":record.saved_at}));
            ensure_supported(raw)?;
            if raw.get("content_type").is_some_and(|kind| {
                !matches!(
                    kind.as_str(),
                    Some("mod" | "resourcepack" | "datapack" | "shader")
                )
            }) {
                return Err(crate::ErrorKind::InputError(
                    "Favorite type is unsupported".into(),
                )
                .into());
            }
            raw["source"] = json!(record.source);
            raw["project_id"] = json!(record.project_id);
            raw["content_type"] = json!(record.content_type);
            Ok(())
        })
        .await
}
pub(crate) async fn remove(
    persistence: &Persistence,
    keys: Vec<(String, String)>,
) -> crate::Result<()> {
    domain(persistence)
        .update(move |entries| {
            for (source, project) in keys {
                let id = key(&[&source, &project]);
                if let Some(raw) = entries.get(&id) {
                    if !supported(raw) {
                        continue;
                    }
                    let Ok(favorite) =
                        serde_json::from_value::<Favorite>(raw.clone())
                    else {
                        continue;
                    };
                    if !matches!(
                        favorite.content_type.as_str(),
                        "mod" | "resourcepack" | "datapack" | "shader"
                    ) {
                        continue;
                    }
                }
                entries.remove(&id);
            }
            Ok(())
        })
        .await
}
pub(crate) async fn pin_values(
    persistence: &Persistence,
) -> HashMap<String, DateTime<Utc>> {
    let data = match pins(persistence).read().await {
        Ok(data) => data,
        Err(error) => {
            tracing::warn!(%error,"Instance pins unavailable; instance loading continues");
            return HashMap::new();
        }
    };
    data["entries"]
        .as_object()
        .expect("entries")
        .values()
        .filter(|raw| supported(raw))
        .filter_map(|raw| {
            Some((
                raw.get("instance_id")?.as_str()?.to_owned(),
                DateTime::from_timestamp(raw.get("pinned_at")?.as_i64()?, 0)?,
            ))
        })
        .collect()
}
pub(crate) async fn set_pin(
    persistence: &Persistence,
    instance: &str,
    time: Option<i64>,
) -> crate::Result<()> {
    let instance = instance.to_owned();
    pins(persistence)
        .update(move |entries| {
            if let Some(raw) = entries.get(&instance) {
                ensure_supported(raw)?;
                if raw
                    .get("pinned_at")
                    .and_then(Value::as_i64)
                    .and_then(|time| DateTime::<Utc>::from_timestamp(time, 0))
                    .is_none()
                {
                    return Err(crate::ErrorKind::InputError(
                        "Instance pin is unreadable".into(),
                    )
                    .into());
                }
            }
            if let Some(time) = time {
                let raw = entries
                    .entry(instance.clone())
                    .or_insert_with(|| json!({}));
                raw["instance_id"] = json!(instance);
                raw["pinned_at"] = json!(time);
            } else {
                entries.remove(&instance);
            }
            Ok(())
        })
        .await
}
