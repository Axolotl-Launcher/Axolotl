//! Listing, creation, and settings management for servers.

use chrono::Utc;
use uuid::Uuid;

use crate::state::remove_log_buffer;
use crate::util::io::{self, IOError};
use crate::{ErrorKind, Result, State};

use super::files::safe_relative_join;
use super::lifecycle::is_running;
use super::manifest::{
    LinkedWorld, ServerInfo, ServerManifest, build_server_info, read_manifest,
    sanitize_folder_name, server_path, type_default_jar_name, write_manifest,
};

pub async fn list() -> Result<Vec<ServerInfo>> {
    let state = State::get().await?;
    let servers_dir = state.directories.servers_dir();
    if !servers_dir.exists() {
        return Ok(Vec::new());
    }

    let mut servers = Vec::new();
    let mut entries = tokio::fs::read_dir(&servers_dir)
        .await
        .map_err(|e| IOError::with_path(e, &servers_dir))?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| IOError::with_path(e, &servers_dir))?
    {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Ok(manifest) = read_manifest(&path).await else {
            continue;
        };
        servers.push(build_server_info(&manifest, &path).await);
    }
    servers.sort_by(|a, b| {
        a.manifest
            .name
            .to_lowercase()
            .cmp(&b.manifest.name.to_lowercase())
    });
    Ok(servers)
}

pub async fn get(server_id: &str) -> Result<ServerInfo> {
    let path = server_path(server_id).await?;
    let manifest = read_manifest(&path).await?;
    Ok(build_server_info(&manifest, &path).await)
}

pub async fn create(
    name: &str,
    server_type: &str,
    game_version: &str,
    loader_version: Option<String>,
    java_path: Option<String>,
    memory_mb: Option<u32>,
) -> Result<ServerManifest> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ErrorKind::InputError(
            "Server name cannot be empty".to_string(),
        )
        .as_error());
    }

    let state = State::get().await?;
    let id = Uuid::new_v4().to_string();
    let dir_name = format!("{}-{}", sanitize_folder_name(name), &id[..8]);
    let dir = safe_relative_join(&state.directories.servers_dir(), &dir_name)?;
    io::create_dir_all(&dir).await?;

    let manifest = ServerManifest {
        id: dir_name,
        name: name.to_string(),
        server_type: server_type.to_string(),
        game_version: game_version.to_string(),
        loader_version,
        jar_name: type_default_jar_name(server_type),
        java_path,
        memory_mb,
        icon_path: None,
        modpack: None,
        install_state: None,
        install_error: None,
        jvm_args: Vec::new(),
        pre_launch_hook: None,
        home_pinned_at: None,
        linked_world: None,
        created_at: Utc::now(),
        last_started_at: None,
        last_exit_crashed: false,
    };
    write_manifest(&dir, &manifest).await?;
    Ok(manifest)
}

/// Sets or clears the server icon. `None` resets to the default icon.
pub async fn set_icon(
    server_id: &str,
    icon_path: Option<String>,
) -> Result<ServerManifest> {
    let path = server_path(server_id).await?;
    let mut manifest = read_manifest(&path).await?;
    manifest.icon_path = icon_path;
    write_manifest(&path, &manifest).await?;
    Ok(manifest)
}

/// Sets or clears the multiplayer entry a server is linked to. `None` removes
/// the link.
///
/// A saved multiplayer entry can only launch one managed server, so setting a
/// link also drops the identical link from the other servers.
pub async fn set_linked_world(
    server_id: &str,
    linked: Option<LinkedWorld>,
) -> Result<ServerManifest> {
    let path = server_path(server_id).await?;
    let mut manifest = read_manifest(&path).await?;
    let linked = normalize_linked_world(linked);

    if let Some(linked) = linked.as_ref() {
        clear_linked_world_elsewhere(server_id, linked).await?;
    }

    manifest.linked_world = linked;
    write_manifest(&path, &manifest).await?;
    Ok(manifest)
}

/// Trims the identifier fields of a link and treats an empty one as "no link".
fn normalize_linked_world(linked: Option<LinkedWorld>) -> Option<LinkedWorld> {
    linked.and_then(|linked| {
        let address = linked.address.trim().to_string();
        let instance_id = linked.instance_id.trim().to_string();
        (!address.is_empty() && !instance_id.is_empty()).then_some(
            LinkedWorld {
                instance_id,
                address,
            },
        )
    })
}

/// Whether two links point at the same saved multiplayer entry. Addresses are
/// compared case-insensitively because they are typed by hand.
fn same_linked_world(a: &LinkedWorld, b: &LinkedWorld) -> bool {
    a.instance_id == b.instance_id && a.address.eq_ignore_ascii_case(&b.address)
}

/// Drops `linked` from every other server manifest, so one multiplayer entry
/// never stays linked to two managed servers (the home widget and unlinking
/// both assume a single match).
async fn clear_linked_world_elsewhere(
    server_id: &str,
    linked: &LinkedWorld,
) -> Result<()> {
    for info in list().await? {
        let other = &info.manifest;
        if other.id == server_id {
            continue;
        }
        let Some(existing) = other.linked_world.as_ref() else {
            continue;
        };
        if !same_linked_world(existing, linked) {
            continue;
        }

        let other_path = server_path(&other.id).await?;
        let mut other_manifest = read_manifest(&other_path).await?;
        other_manifest.linked_world = None;
        write_manifest(&other_path, &other_manifest).await?;
    }
    Ok(())
}

pub async fn update_settings(
    server_id: &str,
    name: Option<String>,
    java_path: Option<String>,
    memory_mb: Option<u32>,
    jvm_args: Option<Vec<String>>,
    pre_launch_hook: Option<String>,
    home_pinned: Option<bool>,
) -> Result<ServerManifest> {
    let path = server_path(server_id).await?;
    let mut manifest = read_manifest(&path).await?;
    if let Some(name) = name {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(ErrorKind::InputError(
                "Server name cannot be empty".to_string(),
            )
            .as_error());
        }
        manifest.name = name;
    }
    if let Some(java_path) = java_path {
        manifest.java_path = if java_path.is_empty() {
            None
        } else {
            Some(java_path)
        };
    }
    if let Some(memory_mb) = memory_mb {
        manifest.memory_mb = Some(memory_mb);
    }
    if let Some(jvm_args) = jvm_args {
        manifest.jvm_args = jvm_args;
    }
    if let Some(pre_launch_hook) = pre_launch_hook {
        let pre_launch_hook = pre_launch_hook.trim();
        manifest.pre_launch_hook = if pre_launch_hook.is_empty() {
            None
        } else {
            Some(pre_launch_hook.to_string())
        };
    }
    if let Some(home_pinned) = home_pinned {
        manifest.home_pinned_at = home_pinned.then(Utc::now);
    }
    write_manifest(&path, &manifest).await?;
    Ok(manifest)
}

pub async fn delete(server_id: &str) -> Result<()> {
    if is_running(server_id) {
        return Err(ErrorKind::InputError(
            "Stop the server before deleting it".to_string(),
        )
        .as_error());
    }
    let path = server_path(server_id).await?;
    remove_log_buffer(server_id);
    tokio::fs::remove_dir_all(&path)
        .await
        .map_err(|e| IOError::with_path(e, &path))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linked(instance_id: &str, address: &str) -> LinkedWorld {
        LinkedWorld {
            instance_id: instance_id.to_string(),
            address: address.to_string(),
        }
    }

    #[test]
    fn normalization_trims_fields_and_drops_empty_links() {
        assert_eq!(normalize_linked_world(None), None);
        assert_eq!(
            normalize_linked_world(Some(linked("", "localhost:25565"))),
            None
        );
        assert_eq!(
            normalize_linked_world(Some(linked("instance-1", "   "))),
            None
        );
        assert_eq!(
            normalize_linked_world(Some(linked(
                " instance-1 ",
                " localhost:25565 "
            ))),
            Some(linked("instance-1", "localhost:25565"))
        );
    }

    #[test]
    fn duplicate_links_compare_ignoring_address_case() {
        assert!(same_linked_world(
            &linked("instance-1", "Localhost:25565"),
            &linked("instance-1", "localhost:25565")
        ));
        assert!(!same_linked_world(
            &linked("instance-1", "localhost:25565"),
            &linked("instance-2", "localhost:25565")
        ));
        assert!(!same_linked_world(
            &linked("instance-1", "localhost:25565"),
            &linked("instance-1", "localhost:25566")
        ));
    }
}
