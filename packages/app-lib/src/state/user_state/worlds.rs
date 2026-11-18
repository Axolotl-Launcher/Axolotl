use super::{Domain, ensure_supported, key, supported};
use crate::state::attached_world_data::AttachedWorldData;
use crate::state::persistence::Persistence;
use crate::worlds::{DisplayStatus, WorldType};
use serde_json::{Value, json};
use std::collections::HashMap;

pub(crate) fn domain(persistence: &Persistence) -> Domain {
    Domain::new(persistence, "worlds", "axolotl-world-choices")
}
pub(super) fn decode(
    raw: &Value,
) -> Option<(WorldType, String, AttachedWorldData)> {
    if !supported(raw) {
        return None;
    }
    let kind = match raw.get("world_type")?.as_str()? {
        "singleplayer" => WorldType::Singleplayer,
        "server" => WorldType::Server,
        _ => return None,
    };
    let display_status = match raw.get("display_status")?.as_str()? {
        "normal" => DisplayStatus::Normal,
        "hidden" => DisplayStatus::Hidden,
        "favorite" => DisplayStatus::Favorite,
        _ => return None,
    };
    let optional = |key| -> Option<Option<String>> {
        match raw.get(key) {
            None | Some(Value::Null) => Some(None),
            Some(Value::String(value)) => Some(Some(value.clone())),
            _ => None,
        }
    };
    Some((
        kind,
        raw.get("world_id")?.as_str()?.into(),
        AttachedWorldData {
            display_status,
            project_id: optional("project_id")?,
            content_kind: optional("content_kind")?,
        },
    ))
}
pub(crate) async fn get_all(
    persistence: &Persistence,
    instance: &str,
) -> crate::Result<HashMap<(WorldType, String), AttachedWorldData>> {
    let data = domain(persistence).read().await?;
    Ok(data["entries"]
        .as_object()
        .expect("entries")
        .values()
        .filter(|raw| {
            raw.get("instance_id").and_then(Value::as_str) == Some(instance)
        })
        .filter_map(|raw| {
            decode(raw).map(|(kind, id, data)| ((kind, id), data))
        })
        .collect())
}
pub(crate) async fn set(
    persistence: &Persistence,
    instance: &str,
    kind: WorldType,
    world: &str,
    field: &'static str,
    value: Value,
) -> crate::Result<()> {
    let instance = instance.to_owned();
    let world = world.to_owned();
    domain(persistence).update(move |entries| {
        let id = key(&[&instance, kind.as_str(), &world]);
        let raw = entries.entry(id).or_insert_with(|| json!({"instance_id":instance,"world_type":kind.as_str(),"world_id":world,"display_status":"normal","project_id":null,"content_kind":null}));
        ensure_supported(raw)?;
        if decode(raw).is_none() { return Err(crate::ErrorKind::InputError("World annotations are unreadable".into()).into()); }
        raw[field] = value; Ok(())
    }).await
}
pub(crate) async fn remove(
    persistence: &Persistence,
    instance: &str,
    kind: WorldType,
    world: &str,
) -> crate::Result<()> {
    let id = key(&[instance, kind.as_str(), world]);
    domain(persistence)
        .update(move |entries| {
            if let Some(raw) = entries.get(&id) {
                ensure_supported(raw)?;
                if decode(raw).is_none() {
                    return Err(crate::ErrorKind::InputError(
                        "World annotations are unreadable".into(),
                    )
                    .into());
                }
            }
            entries.remove(&id);
            Ok(())
        })
        .await
}
