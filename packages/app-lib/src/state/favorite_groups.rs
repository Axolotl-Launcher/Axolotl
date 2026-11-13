//! Favorite grouping backed by a standalone JSON file in the app's settings
//! directory, next to `settings.json`.
//!
//! The favorites themselves live in the resources store; this module only
//! records which user-created groups each favorite (`source:project_id`)
//! belongs to.

use crate::state::State;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};
use tracing::warn;

pub const FAVORITE_GROUPS_FILE_NAME: &str = "favorite_groups.json";

/// Current `favorite_groups.json` schema version.
pub const FAVORITE_GROUPS_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FavoriteGroupDefinition {
    pub id: String,
    pub name: String,
}

/// On-disk shape. The order of `groups` *is* the display order, so no
/// `display_order` field is needed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FavoriteGroupsFile {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub groups: Vec<FavoriteGroupDefinition>,
    /// "source:project_id" -> group ids.
    #[serde(default)]
    pub memberships: HashMap<String, Vec<String>>,
}

impl Default for FavoriteGroupsFile {
    fn default() -> Self {
        Self {
            schema_version: FAVORITE_GROUPS_SCHEMA_VERSION,
            groups: Vec::new(),
            memberships: HashMap::new(),
        }
    }
}

impl FavoriteGroupsFile {
    fn update_membership(
        &mut self,
        favorite_key: &str,
        add_group_ids: &[String],
        remove_group_ids: &[String],
    ) {
        let group_ids = self
            .memberships
            .entry(favorite_key.to_string())
            .or_default();
        group_ids.retain(|id| !remove_group_ids.contains(id));
        for id in add_group_ids {
            if !group_ids.contains(id) {
                group_ids.push(id.clone());
            }
        }
    }

    fn has_group(&self, id: &str) -> bool {
        self.groups.iter().any(|group| group.id == id)
    }

    fn forget(&mut self, favorite_key: &str) {
        self.memberships.remove(favorite_key);
    }

    /// Drops duplicate group ids, drops memberships pointing at groups that no
    /// longer exist, and drops favorites left with no groups at all.
    fn normalize(&mut self) {
        self.schema_version = FAVORITE_GROUPS_SCHEMA_VERSION;

        let mut seen = HashSet::new();
        self.groups.retain(|group| seen.insert(group.id.clone()));

        let known = self
            .groups
            .iter()
            .map(|group| group.id.clone())
            .collect::<HashSet<_>>();

        for group_ids in self.memberships.values_mut() {
            let mut seen = HashSet::new();
            group_ids
                .retain(|id| known.contains(id) && seen.insert(id.clone()));
        }
        self.memberships
            .retain(|_, group_ids| !group_ids.is_empty());
    }

    /// Validates every referenced group before changing anything, so an
    /// unknown group id cannot leave a partially applied update behind.
    fn apply_membership_updates(
        &mut self,
        updates: &[(String, Vec<String>, Vec<String>)],
    ) -> crate::Result<()> {
        for (_, added, removed) in updates {
            for id in added.iter().chain(removed) {
                if !self.has_group(id) {
                    return Err(crate::ErrorKind::InputError(format!(
                        "Unknown favorite group {id}"
                    ))
                    .into());
                }
            }
        }
        for (favorite_key, added, removed) in updates {
            self.update_membership(favorite_key, added, removed);
        }
        Ok(())
    }
}

static STORE: OnceLock<RwLock<FavoriteGroupsFile>> = OnceLock::new();

fn file_path() -> Option<PathBuf> {
    let state = State::get_if_initialized()?;
    Some(
        state
            .directories
            .settings_dir
            .join(FAVORITE_GROUPS_FILE_NAME),
    )
}

/// Reads the store from disk, or `None` when it has never been written.
fn read_from_disk() -> Option<FavoriteGroupsFile> {
    let path = file_path()?;
    if !path.exists() {
        return None;
    }
    match fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|content| {
            serde_json::from_str::<FavoriteGroupsFile>(&content)
                .map_err(|e| e.to_string())
        }) {
        Ok(mut file) => {
            file.normalize();
            Some(file)
        }
        Err(error) => {
            // A corrupt file must not wipe the user's groups silently on the
            // next write, so bail out to an empty store without touching disk.
            warn!("Failed to read {FAVORITE_GROUPS_FILE_NAME}: {error}");
            None
        }
    }
}

fn write_to_disk(file: &FavoriteGroupsFile) -> crate::Result<()> {
    let state = State::get_if_initialized().ok_or_else(|| {
        crate::ErrorKind::InputError(
            "Favorite group store is not initialized".to_string(),
        )
    })?;
    let path = state
        .directories
        .settings_dir
        .join(FAVORITE_GROUPS_FILE_NAME);
    fs::write(path, serde_json::to_string_pretty(file)?)?;
    Ok(())
}

fn store() -> &'static RwLock<FavoriteGroupsFile> {
    STORE.get_or_init(|| RwLock::new(read_from_disk().unwrap_or_default()))
}

fn with_read<T>(f: impl FnOnce(&FavoriteGroupsFile) -> T) -> T {
    let guard = store().read().unwrap_or_else(|e| e.into_inner());
    f(&guard)
}

/// Mutates the store and persists the whole file. `f` returns the favorite
/// keys whose membership changed, so callers can emit the change event.
fn with_write<T>(f: impl FnOnce(&mut FavoriteGroupsFile) -> T) -> T {
    let mut guard = store().write().unwrap_or_else(|e| e.into_inner());
    let result = f(&mut guard);
    guard.normalize();
    if let Err(error) = write_to_disk(&guard) {
        warn!("Failed to save {FAVORITE_GROUPS_FILE_NAME}: {error}");
    }
    result
}

// ── Reads ───────────────────────────────────────────────────────────────────

pub fn list() -> Vec<FavoriteGroupDefinition> {
    with_read(|file| file.groups.clone())
}

pub fn group_exists(id: &str) -> bool {
    with_read(|file| file.has_group(id))
}

/// Group ids for one favorite, in the order they were assigned.
pub fn group_ids_for(favorite_key: &str) -> Vec<String> {
    with_read(|file| {
        file.memberships
            .get(favorite_key)
            .cloned()
            .unwrap_or_default()
    })
}

/// Every favorite that belongs to at least one group, sorted by favorite key.
pub fn memberships() -> Vec<(String, Vec<String>)> {
    with_read(|file| {
        let mut memberships = file
            .memberships
            .iter()
            .map(|(favorite_key, group_ids)| {
                (favorite_key.clone(), group_ids.clone())
            })
            .collect::<Vec<_>>();
        memberships.sort_by(|left, right| left.0.cmp(&right.0));
        memberships
    })
}

fn favorites_in_group(
    file: &FavoriteGroupsFile,
    group_id: &str,
) -> Vec<String> {
    file.memberships
        .iter()
        .filter(|(_, group_ids)| group_ids.iter().any(|id| id == group_id))
        .map(|(favorite_key, _)| favorite_key.clone())
        .collect()
}

// ── Writes ──────────────────────────────────────────────────────────────────

/// Adds a group at the top of the list, matching instance groups.
pub fn create(id: &str, name: &str) {
    with_write(|file| {
        file.groups.insert(
            0,
            FavoriteGroupDefinition {
                id: id.to_string(),
                name: name.to_string(),
            },
        );
    });
}

/// Renames a group, returning the affected favorite keys, or `None` when the
/// group does not exist.
pub fn rename(id: &str, name: &str) -> Option<Vec<String>> {
    with_write(|file| {
        let group = file.groups.iter_mut().find(|group| group.id == id)?;
        group.name = name.to_string();
        Some(favorites_in_group(file, id))
    })
}

/// Deletes a group and strips it from every favorite, returning the affected
/// favorite keys, or `None` when the group does not exist.
pub fn delete(id: &str) -> Option<Vec<String>> {
    with_write(|file| {
        let index = file.groups.iter().position(|group| group.id == id)?;
        let affected = favorites_in_group(file, id);
        file.groups.remove(index);
        // `normalize` prunes the now-dangling ids out of `memberships`.
        Some(affected)
    })
}

/// Replaces the group list of each given favorite wholesale.
pub fn set_memberships(updates: &[(String, Vec<String>)]) {
    with_write(|file| {
        for (favorite_key, group_ids) in updates {
            if group_ids.is_empty() {
                file.memberships.remove(favorite_key);
            } else {
                file.memberships
                    .insert(favorite_key.clone(), group_ids.clone());
            }
        }
    });
}

/// Applies relationship changes under the store's write lock, preserving all
/// memberships outside the requested favorites.
pub fn update_memberships(
    updates: &[(String, Vec<String>, Vec<String>)],
) -> crate::Result<()> {
    let mut guard = store().write().unwrap_or_else(|e| e.into_inner());
    let mut file = guard.clone();
    file.apply_membership_updates(updates)?;
    file.normalize();
    write_to_disk(&file)?;
    *guard = file;
    Ok(())
}

/// Drops a favorite's membership entry, e.g. after the favorite is removed.
pub fn forget(favorite_key: &str) {
    with_write(|file| file.forget(favorite_key));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_with_groups(ids: &[&str]) -> FavoriteGroupsFile {
        FavoriteGroupsFile {
            schema_version: FAVORITE_GROUPS_SCHEMA_VERSION,
            groups: ids
                .iter()
                .map(|id| FavoriteGroupDefinition {
                    id: id.to_string(),
                    name: id.to_string(),
                })
                .collect(),
            memberships: HashMap::new(),
        }
    }

    fn group_ids(groups: &[FavoriteGroupDefinition]) -> Vec<String> {
        groups.iter().map(|group| group.id.clone()).collect()
    }

    #[test]
    fn normalize_deduplicates_group_ids() {
        let mut file = file_with_groups(&["a", "b", "a"]);
        file.normalize();
        assert_eq!(group_ids(&file.groups), vec!["a", "b"]);
    }

    #[test]
    fn normalize_drops_orphan_memberships() {
        let mut file = file_with_groups(&["a"]);
        file.memberships.insert(
            "modrinth:sodium".to_string(),
            vec!["a".to_string(), "nonexistent".to_string()],
        );

        file.normalize();
        assert_eq!(file.memberships["modrinth:sodium"], &["a"]);
    }

    #[test]
    fn normalize_deduplicates_memberships_preserving_order() {
        let mut file = file_with_groups(&["a", "b"]);
        file.memberships.insert(
            "modrinth:sodium".to_string(),
            vec![
                "b".to_string(),
                "a".to_string(),
                "b".to_string(),
                "a".to_string(),
            ],
        );

        file.normalize();
        assert_eq!(file.memberships["modrinth:sodium"], &["b", "a"]);
    }

    #[test]
    fn normalize_drops_memberships_without_groups() {
        let mut file = file_with_groups(&["a"]);
        file.memberships.insert(
            "modrinth:sodium".to_string(),
            vec!["nonexistent".to_string()],
        );

        file.normalize();
        assert!(!file.memberships.contains_key("modrinth:sodium"));
    }

    #[test]
    fn membership_changes_preserve_other_groups() {
        let mut file = file_with_groups(&["a", "b"]);
        file.normalize();
        file.update_membership("modrinth:sodium", &["a".into()], &[]);
        file.update_membership("modrinth:lithium", &["b".into()], &[]);
        file.update_membership(
            "modrinth:sodium",
            &["b".into(), "b".into()],
            &[],
        );
        assert_eq!(file.memberships["modrinth:sodium"], &["a", "b"]);

        file.update_membership("modrinth:sodium", &[], &["a".into()]);
        assert_eq!(file.memberships["modrinth:sodium"], &["b"]);
        assert_eq!(file.memberships["modrinth:lithium"], &["b"]);
        file.update_membership("modrinth:sodium", &[], &["b".into()]);
        file.normalize();
        assert!(!file.memberships.contains_key("modrinth:sodium"));
        assert_eq!(file.memberships["modrinth:lithium"], &["b"]);
    }

    #[test]
    fn update_memberships_rejects_unknown_group_without_partial_writes() {
        let mut file = file_with_groups(&["a"]);
        let mut candidate = file.clone();
        let updates = vec![
            ("modrinth:sodium".to_string(), vec!["a".to_string()], vec![]),
            (
                "modrinth:lithium".to_string(),
                vec!["nonexistent".to_string()],
                vec![],
            ),
        ];

        assert!(candidate.apply_membership_updates(&updates).is_err());
        // Validation ran before mutation, so the clone is untouched.
        assert!(candidate.memberships.is_empty());

        // The unchanged original proves nothing leaked into it either.
        file.normalize();
        assert!(file.memberships.is_empty());
    }

    #[test]
    fn forget_drops_only_the_named_favorite() {
        let mut file = file_with_groups(&["a"]);
        file.memberships
            .insert("modrinth:sodium".to_string(), vec!["a".to_string()]);
        file.memberships
            .insert("modrinth:lithium".to_string(), vec!["a".to_string()]);

        file.forget("modrinth:sodium");
        assert!(!file.memberships.contains_key("modrinth:sodium"));
        assert_eq!(file.memberships["modrinth:lithium"], &["a"]);
    }

    #[test]
    fn concurrent_relationship_changes_do_not_overwrite_each_other() {
        let file = RwLock::new(file_with_groups(&["a", "b"]));
        std::thread::scope(|scope| {
            for id in ["a", "b"] {
                let file = &file;
                scope.spawn(move || {
                    let mut file = file.write().unwrap();
                    file.update_membership(
                        "modrinth:sodium",
                        &[id.into()],
                        &[],
                    );
                    file.normalize();
                });
            }
        });
        let file = file.into_inner().unwrap();
        let groups = &file.memberships["modrinth:sodium"];
        assert_eq!(groups.len(), 2);
        for id in ["a", "b"] {
            assert!(groups.iter().any(|group| group == id));
        }
    }

    #[test]
    fn default_file_uses_the_current_schema_version() {
        let file = FavoriteGroupsFile::default();
        assert_eq!(file.schema_version, FAVORITE_GROUPS_SCHEMA_VERSION);
        assert!(file.groups.is_empty());
        assert!(file.memberships.is_empty());
    }
}
