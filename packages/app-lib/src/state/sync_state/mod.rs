//! Independent shared-source publication and per-instance projection state.
pub(crate) mod collection;
pub(crate) mod journal;
pub(crate) mod manifest;
pub(crate) mod servers;
#[cfg(test)]
mod tests;
use crate::state::persistence::Persistence;
use std::path::{Path, PathBuf};
pub(crate) async fn root(persistence: &Persistence) -> crate::Result<PathBuf> {
    if let Some(state) = crate::State::get_if_initialized() {
        if state.persistence.root() == persistence.root() {
            return Ok(state.directories.synced_options_dir());
        }
    }
    Ok(
        crate::state::store::sync_documents::configured_root(
            persistence.root(),
        )
        .await?
        .join("synced-options"),
    )
}
pub(crate) fn supported(raw: &serde_json::Value) -> bool {
    raw.is_object()
        && raw.get("protocol").is_none()
        && ["schema_version", "schemaVersion"].into_iter().all(|key| {
            raw.get(key).is_none_or(|value| value.as_u64() == Some(1))
        })
}
pub(crate) fn key(raw: &serde_json::Value) -> crate::Result<String> {
    use sha2::{Digest, Sha256};
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(raw)?)))
}
pub(crate) async fn ensure_supported(root: &Path) -> crate::Result<()> {
    let (head, collections) = manifest::load(root).await?;
    if head.legacy_extensions_present
        || !head.extra.is_empty()
        || head.native.values().any(|native| !native.extra.is_empty())
        || head.shards.iter().any(|shard| !shard.extra.is_empty())
        || collections
            .iter()
            .filter(|(name, _)| !name.starts_with("$legacy/"))
            .any(|(name, entries)| {
                entries.values().any(|raw| {
                    !supported(raw)
                        || raw["record"].get("protocol").is_some()
                        || raw["record"]
                            .get("schemaVersion")
                            .is_some_and(|version| version.as_u64() != Some(1))
                        || raw["record"].get("schema_version").is_some_and(
                            |version| {
                                if name == "synced_hotbar_state" {
                                    !matches!(version.as_u64(), Some(1 | 2))
                                } else {
                                    version.as_u64() != Some(1)
                                }
                            },
                        )
                })
            })
    {
        return Err(crate::ErrorKind::InputError(
            "Sync cleanup paused for unknown references".into(),
        )
        .into());
    }
    Ok(())
}

/// Native text is published with the same durable decision as all other shared
/// sources. The traditional filename is only a detached projection.
pub(crate) async fn native_text(
    state: &crate::State,
    name: &str,
    bytes: Vec<u8>,
) -> crate::Result<()> {
    if name != "command_history" || bytes.len() > 16 << 20 {
        return Err(crate::ErrorKind::InputError(
            "Invalid native text source".into(),
        )
        .into());
    }
    let mut unit = crate::state::store::atom(&state.data).await?;
    let transaction = unit.sync().await?;
    if transaction.head.legacy_extensions_present {
        return Err(crate::ErrorKind::InputError(
            "Opaque legacy sync state retained".into(),
        )
        .into());
    }
    let raw = transaction
        .collections
        .entry("native_text".into())
        .or_default()
        .entry(name)
        .or_insert_with(|| serde_json::json!({"record":{}}));
    if !supported(raw) || !supported(&raw["record"]) {
        return Err(crate::ErrorKind::InputError(
            "Unknown native text source retained".into(),
        )
        .into());
    }
    raw["record"]["document"] = serde_json::to_value(bytes)?;
    transaction.changed = true;
    unit.commit().await
}

pub(crate) async fn native_text_read(
    state: &crate::State,
    name: &str,
) -> crate::Result<Option<Vec<u8>>> {
    let mut unit = crate::state::store::atom(&state.data).await?;
    let transaction = unit.sync().await?;
    let result = match transaction
        .collections
        .get("native_text")
        .and_then(|entries| entries.get(name))
    {
        Some(raw) if supported(raw) => {
            Some(serde_json::from_value(raw["record"]["document"].clone())?)
        }
        Some(_) => {
            return Err(crate::ErrorKind::InputError(
                "Unknown native text source retained".into(),
            )
            .into());
        }
        None => None,
    };
    unit.commit().await?;
    Ok(result)
}

/// Optional instance metadata is retired independently of the instance row.
/// Unknown records stay intact; they can never prevent removing an instance.
pub(crate) async fn forget_instance(
    state: &crate::State,
    instance: &str,
) -> crate::Result<()> {
    let mut unit = crate::state::store::atom(&state.data).await?;
    let transaction = unit.sync().await?;
    if transaction.head.legacy_extensions_present {
        return Err(crate::ErrorKind::InputError(
            "Opaque sync instance references retained".into(),
        )
        .into());
    }
    for name in [
        "instance_servers",
        "instance_server_projection_entries",
        "instance_server_pack_state",
        "instance_sync_checkpoints",
        "synced_pack_instances",
        "game_option_pack_bases",
        "game_option_update_state",
        "game_option_locale_origins",
    ] {
        if let Some(rows) = transaction.collections.get_mut(name) {
            let count = rows.len();
            rows.retain(|_, raw| {
                if !supported(raw) || !supported(&raw["record"]) {
                    return true;
                }
                let key = &raw["key"];
                !(key.as_str() == Some(instance)
                    || key
                        .get("instance_id")
                        .and_then(serde_json::Value::as_str)
                        == Some(instance)
                    || (name == "game_option_locale_origins"
                        && key
                            .get("scope")
                            .and_then(serde_json::Value::as_str)
                            == Some(instance)))
            });
            transaction.changed |= count != rows.len();
        }
    }
    unit.commit().await
}
