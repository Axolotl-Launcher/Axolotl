//! Instance grouping backed by a standalone JSON file in the app's settings
//! directory, next to `settings.json`.
//!
//! Groups used to live in SQLite (`instance_groups` /
//! `instance_group_memberships`). That layout could not express membership for
//! JSON-backed instances (`local:` ids, i.e. every `.minecraft` and
//! multi-library instance) because they have no row in the `instances` table,
//! and it duplicated membership into the per-instance sidecars. This module is
//! the single source of truth instead; the SQL tables are left untouched but
//! are no longer read or written (they are only read once, at import time).

use crate::state::State;
use crate::state::instances::adapters::sqlite::instance_groups as legacy_groups;
use crate::state::instances::adapters::sqlite::instance_groups::LegacyGroups;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
#[cfg(test)]
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use tracing::warn;

pub const INSTANCE_GROUPS_FILE_NAME: &str = "instance_groups.json";

/// Current `instance_groups.json` schema version.
pub const INSTANCE_GROUPS_SCHEMA_VERSION: u32 = 1;

pub const FAVORITES_GROUP_ID: &str = "group:favorites";
const FAVORITES_GROUP_NAME: &str = "Pinned";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupDefinition {
    pub id: String,
    pub name: String,
}

/// On-disk shape. The order of `groups` *is* the display order, so no
/// `display_order` field is needed: the public API only ever communicates
/// ordering through the order of `list()` and `set_order()`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstanceGroupsFile {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub groups: Vec<GroupDefinition>,
    /// instance id -> group ids.
    #[serde(default)]
    pub memberships: HashMap<String, Vec<String>>,
}

impl Default for InstanceGroupsFile {
    fn default() -> Self {
        Self {
            schema_version: INSTANCE_GROUPS_SCHEMA_VERSION,
            groups: vec![GroupDefinition {
                id: FAVORITES_GROUP_ID.to_string(),
                name: FAVORITES_GROUP_NAME.to_string(),
            }],
            memberships: HashMap::new(),
        }
    }
}

impl InstanceGroupsFile {
    fn update_membership(
        &mut self,
        instance_id: &str,
        add_group_ids: &[String],
        remove_group_ids: &[String],
    ) {
        let groups =
            self.memberships.entry(instance_id.to_string()).or_default();
        groups.retain(|id| !remove_group_ids.contains(id));
        for id in add_group_ids {
            if !groups.contains(id) {
                groups.push(id.clone());
            }
        }
    }

    fn has_group(&self, id: &str) -> bool {
        self.groups.iter().any(|group| group.id == id)
    }

    /// Guarantees the favorites sentinel exists and sits first, drops
    /// memberships pointing at groups that no longer exist, and drops instances
    /// left with no groups at all.
    fn normalize(&mut self) {
        self.schema_version = INSTANCE_GROUPS_SCHEMA_VERSION;

        match self.groups.iter().position(|g| g.id == FAVORITES_GROUP_ID) {
            Some(0) => {}
            Some(index) => {
                let favorites = self.groups.remove(index);
                self.groups.insert(0, favorites);
            }
            None => self.groups.insert(
                0,
                GroupDefinition {
                    id: FAVORITES_GROUP_ID.to_string(),
                    name: FAVORITES_GROUP_NAME.to_string(),
                },
            ),
        }

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
}

static FILE_ACCESS: std::sync::Mutex<()> = std::sync::Mutex::new(());

static STORE: OnceLock<RwLock<Option<InstanceGroupsFile>>> = OnceLock::new();

fn file_path() -> Option<PathBuf> {
    let state = State::get_if_initialized()?;
    Some(
        state
            .directories
            .settings_dir
            .join(crate::state::layout::INSTANCE_GROUPS),
    )
}

fn read_from_disk(path: &Path) -> crate::Result<Option<InstanceGroupsFile>> {
    let file = crate::state::persistence::read_group_file::<InstanceGroupsFile>(
        path,
        INSTANCE_GROUPS_SCHEMA_VERSION,
    )?;
    Ok(file.map(|mut file| {
        file.normalize();
        file
    }))
}

fn load_for_display(path: &Path) -> Option<InstanceGroupsFile> {
    match read_from_disk(path) {
        Ok(file) => Some(file.unwrap_or_default()),
        Err(error) => {
            warn!(%error, file = INSTANCE_GROUPS_FILE_NAME, "Could not load groups");
            None
        }
    }
}

/// Freeze the on-disk authority under the same lock used by every writer.
/// Preserve the original bytes, including fields from newer readers.
pub(crate) fn snapshot(
    root: &std::path::Path,
) -> crate::Result<Option<Vec<u8>>> {
    let _guard = FILE_ACCESS
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let path = root.join(crate::state::layout::INSTANCE_GROUPS);
    match crate::state::persistence::document::snapshot_json(&path)? {
        Some(bytes) => Ok(Some(bytes)),
        None => crate::state::persistence::document::snapshot_json(&root.join(
            format!("{}.previous", crate::state::layout::INSTANCE_GROUPS),
        )),
    }
}

fn store(path: &Path) -> &'static RwLock<Option<InstanceGroupsFile>> {
    STORE.get_or_init(|| RwLock::new(load_for_display(path)))
}

fn with_read<T>(f: impl FnOnce(&InstanceGroupsFile) -> T) -> T {
    if let Some(path) = file_path() {
        let guard = store(&path).read().unwrap_or_else(|e| e.into_inner());
        if let Some(file) = guard.as_ref() {
            return f(file);
        }
    }
    f(&InstanceGroupsFile::default())
}

fn with_write<T>(
    f: impl FnOnce(&mut InstanceGroupsFile) -> T,
) -> crate::Result<T> {
    try_with_write(|file| Ok(f(file)))
}

fn try_with_write<T>(
    f: impl FnOnce(&mut InstanceGroupsFile) -> crate::Result<T>,
) -> crate::Result<T> {
    crate::state::store::recovery_mode::ensure_state_writable()?;
    let path = file_path();
    let uninitialized = RwLock::new(None);
    let cache = match &path {
        Some(path) => store(path),
        None => &uninitialized,
    };
    write_group_file(cache, path.as_deref(), f)
}

fn write_group_file<T>(
    cache: &RwLock<Option<InstanceGroupsFile>>,
    path: Option<&Path>,
    f: impl FnOnce(&mut InstanceGroupsFile) -> crate::Result<T>,
) -> crate::Result<T> {
    crate::state::persistence::edit_group_file(
        cache,
        &FILE_ACCESS,
        path,
        read_from_disk,
        InstanceGroupsFile::normalize,
        f,
    )
}

// ── Reads ───────────────────────────────────────────────────────────────────

pub fn list() -> Vec<GroupDefinition> {
    with_read(|file| file.groups.clone())
}

pub fn group_exists(id: &str) -> bool {
    with_read(|file| file.has_group(id))
}

/// Group ids for one instance, in the order they were assigned. Synchronous so
/// that `instance_metadata_from_instance` can call it.
pub fn group_ids_for(instance_id: &str) -> Vec<String> {
    with_read(|file| {
        file.memberships
            .get(instance_id)
            .cloned()
            .unwrap_or_default()
    })
}

fn instances_in_group(
    file: &InstanceGroupsFile,
    group_id: &str,
) -> Vec<String> {
    file.memberships
        .iter()
        .filter(|(_, group_ids)| group_ids.iter().any(|id| id == group_id))
        .map(|(instance_id, _)| instance_id.clone())
        .collect()
}

// ── Writes ──────────────────────────────────────────────────────────────────

/// Adds a group at the top of the list, matching the previous SQL behavior
/// where new groups got the lowest `display_order`. Favorites stays first.
pub fn create(id: &str, name: &str) -> crate::Result<()> {
    with_write(|file| {
        let definition = GroupDefinition {
            id: id.to_string(),
            name: name.to_string(),
        };
        // `normalize` moves favorites back to index 0 afterwards.
        file.groups.insert(0, definition);
    })
}

/// Renames a group, returning the affected instance ids, or `None` when the
/// group does not exist.
pub fn rename(id: &str, name: &str) -> crate::Result<Option<Vec<String>>> {
    with_write(|file| {
        let group = file.groups.iter_mut().find(|group| group.id == id)?;
        group.name = name.to_string();
        Some(instances_in_group(file, id))
    })
}

/// Deletes a group and strips it from every instance, returning the affected
/// instance ids, or `None` when the group does not exist.
pub fn delete(id: &str) -> crate::Result<Option<Vec<String>>> {
    with_write(|file| {
        let index = file.groups.iter().position(|group| group.id == id)?;
        let affected = instances_in_group(file, id);
        file.groups.remove(index);
        // `normalize` prunes the now-dangling ids out of `memberships`.
        Some(affected)
    })
}

/// Reorders groups to match `group_ids`. Groups missing from the argument keep
/// their relative order at the end, so a stale frontend list can never drop a
/// group. Favorites is pinned first regardless.
pub fn set_order(group_ids: &[String]) -> crate::Result<()> {
    with_write(|file| reorder_groups(&mut file.groups, group_ids))
}

/// Reorders `groups` to match `group_ids`. Groups the argument omits keep their
/// relative order after the listed ones, so a stale frontend list can never drop
/// a group. Pure so the ordering rules can be tested without the process-wide
/// store, which tests would otherwise race over.
fn reorder_groups(groups: &mut Vec<GroupDefinition>, group_ids: &[String]) {
    let mut remaining = std::mem::take(groups);
    let mut ordered = Vec::with_capacity(remaining.len());
    for id in group_ids {
        if id == FAVORITES_GROUP_ID {
            continue;
        }
        if let Some(index) = remaining.iter().position(|group| &group.id == id)
        {
            ordered.push(remaining.remove(index));
        }
    }
    ordered.extend(remaining);
    *groups = ordered;
}

/// Replaces the group list of each given instance wholesale.
pub fn set_memberships(updates: &[(String, Vec<String>)]) -> crate::Result<()> {
    replace_memberships(updates, false)
}

pub(crate) fn set_memberships_checked(
    updates: &[(String, Vec<String>)],
) -> crate::Result<()> {
    replace_memberships(updates, true)
}

fn replace_memberships(
    updates: &[(String, Vec<String>)],
    validate: bool,
) -> crate::Result<()> {
    try_with_write(|file| {
        if validate {
            for id in updates.iter().flat_map(|(_, ids)| ids) {
                require_group(file, id)?;
            }
        }
        for (instance_id, group_ids) in updates {
            if group_ids.is_empty() {
                file.memberships.remove(instance_id);
            } else {
                file.memberships
                    .insert(instance_id.clone(), group_ids.clone());
            }
        }
        Ok(())
    })
}

fn require_group(file: &InstanceGroupsFile, id: &str) -> crate::Result<()> {
    if !file.has_group(id) {
        return Err(crate::ErrorKind::InputError(format!(
            "Unknown instance group {id}"
        ))
        .into());
    }
    Ok(())
}

/// Replaces the group list of a single instance.
pub fn set_instance_groups(
    instance_id: &str,
    group_ids: &[String],
) -> crate::Result<()> {
    set_memberships(&[(instance_id.to_string(), group_ids.to_vec())])
}

/// Applies relationship changes under the store's write lock, preserving all
/// memberships outside the requested groups.
pub fn update_memberships(
    updates: &[(String, Vec<String>, Vec<String>)],
) -> crate::Result<()> {
    try_with_write(|file| {
        for (_, added, removed) in updates {
            for id in added.iter().chain(removed) {
                require_group(file, id)?;
            }
        }
        for (instance_id, added, removed) in updates {
            file.update_membership(instance_id, added, removed);
        }
        Ok(())
    })
}

/// Looks a group up by name, creating it if absent, and returns its id.
/// Used by the legacy launcher import, which only knows group names.
pub fn find_or_create_by_name(name: &str) -> crate::Result<String> {
    with_write(|file| {
        if let Some(group) = file.groups.iter().find(|group| group.name == name)
        {
            return group.id.clone();
        }
        let id = uuid::Uuid::new_v4().to_string();
        file.groups.push(GroupDefinition {
            id: id.clone(),
            name: name.to_string(),
        });
        id
    })
}

/// Adds one group to an instance, keeping its existing groups.
pub fn add_instance_to_group(
    instance_id: &str,
    group_id: &str,
) -> crate::Result<()> {
    with_write(|file| {
        file.update_membership(instance_id, &[group_id.to_string()], &[]);
    })
}

/// Drops an instance's membership entry, e.g. after the instance is deleted.
pub fn forget_instance(instance_id: &str) -> crate::Result<()> {
    with_write(|file| {
        file.memberships.remove(instance_id);
    })
}

/// Re-keys an instance's membership, e.g. when a `local:` id changes because the
/// instance directory moved.
pub fn rename_instance(old_id: &str, new_id: &str) -> crate::Result<()> {
    if old_id == new_id {
        return Ok(());
    }
    with_write(|file| {
        if let Some(group_ids) = file.memberships.remove(old_id) {
            file.memberships.insert(new_id.to_string(), group_ids);
        }
    })
}

// ── One-time import ─────────────────────────────────────────────────────────

async fn import_sql_groups(
    pool: &sqlx::SqlitePool,
) -> crate::Result<InstanceGroupsFile> {
    let mut file = InstanceGroupsFile {
        schema_version: INSTANCE_GROUPS_SCHEMA_VERSION,
        groups: Vec::new(),
        memberships: HashMap::new(),
    };

    match legacy_groups::read_legacy(pool).await? {
        LegacyGroups::Named {
            groups,
            memberships,
        } => {
            for (id, name) in groups {
                file.groups.push(GroupDefinition { id, name });
            }
            for (instance_id, group_id) in memberships {
                file.memberships
                    .entry(instance_id)
                    .or_default()
                    .push(group_id);
            }
        }
        LegacyGroups::Memberships(rows) => {
            let mut group_ids = HashMap::<String, String>::new();
            for (instance_id, group_name) in rows {
                let group_id = group_ids
                    .entry(group_name.clone())
                    .or_insert_with(|| uuid::Uuid::new_v4().to_string())
                    .clone();
                file.memberships
                    .entry(instance_id)
                    .or_default()
                    .push(group_id.clone());
                if !file.groups.iter().any(|group| group.id == group_id) {
                    file.groups.push(GroupDefinition {
                        id: group_id,
                        name: group_name,
                    });
                }
            }
        }
        LegacyGroups::Unrecognised(columns) => {
            warn!(
                columns = ?columns,
                "Skipping instance group import from an unrecognized SQLite schema"
            );
        }
        LegacyGroups::Absent => {}
    }

    file.normalize();
    Ok(file)
}

/// Import a preserved legacy image before the compact core stops exposing its
/// grouping tables. Existing documents always take precedence.
pub(crate) async fn import_from_pool(
    pool: &sqlx::SqlitePool,
    root: &std::path::Path,
) -> crate::Result<()> {
    let path = root.join(crate::state::layout::INSTANCE_GROUPS);
    for candidate in [&path, &path.with_extension("json.previous")] {
        match tokio::fs::symlink_metadata(candidate).await {
            Ok(_) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    let file = import_sql_groups(pool).await?;
    // Startup publishes atomically before runtime writers are available, so a
    // snapshot sees either absence or a complete file without holding STORE.
    crate::state::persistence::document::write_json(
        &path,
        serde_json::to_vec_pretty(&file)?,
    )
    .await
}

/// Creates `instance_groups.json` from the pre-existing SQLite tables and
/// instance sidecars, the first time the app runs after this change.
///
/// The SQL access here is read-only and happens exactly once; afterwards the
/// tables are never touched again (they are kept as-is so no DB migration is
/// needed).
pub async fn ensure_imported(state: &State) -> crate::Result<()> {
    let Some(path) = file_path() else {
        return Ok(());
    };
    import_from_pool(&state.pool, &state.directories.settings_dir).await?;
    let mut cached = store(&path).write().unwrap_or_else(|e| e.into_inner());
    let _file_guard = FILE_ACCESS.lock().unwrap_or_else(|e| e.into_inner());
    *cached = load_for_display(&path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[test]
    fn unchanged_membership_needs_no_publication_but_real_changes_need_a_store()
    {
        assert!(State::get_if_initialized().is_none());
        let before = with_read(Clone::clone);
        forget_instance("ungrouped-instance").unwrap();
        update_memberships(&[("ungrouped-instance".into(), vec![], vec![])])
            .unwrap();
        assert_eq!(with_read(Clone::clone), before);
        assert!(create("new-group", "Needs persistence").is_err());
        assert_eq!(with_read(Clone::clone), before);
    }

    fn file_with_groups(ids: &[&str]) -> InstanceGroupsFile {
        InstanceGroupsFile {
            schema_version: INSTANCE_GROUPS_SCHEMA_VERSION,
            groups: ids
                .iter()
                .map(|id| GroupDefinition {
                    id: id.to_string(),
                    name: id.to_string(),
                })
                .collect(),
            memberships: HashMap::new(),
        }
    }

    #[test]
    fn truncated_group_document_stays_read_only_until_repaired() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(crate::state::layout::INSTANCE_GROUPS);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let truncated = br#"{"schema_version":1,"groups":[{"id":"keep""#;
        fs::write(&path, truncated).unwrap();
        let cache = RwLock::new(load_for_display(&path));
        let result = write_group_file(&cache, Some(&path), |file| {
            file.groups.push(GroupDefinition {
                id: "new".into(),
                name: "New group".into(),
            });
            Ok(())
        });
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("editing is disabled")
        );
        assert!(cache.read().unwrap().is_none());
        assert_eq!(fs::read(&path).unwrap(), truncated);

        fs::write(
            &path,
            serde_json::to_vec(&file_with_groups(&["keep"])).unwrap(),
        )
        .unwrap();
        write_group_file(&cache, Some(&path), |file| {
            file.update_membership("instance", &["keep".into()], &[]);
            Ok(())
        })
        .unwrap();
        let file = read_from_disk(&path).unwrap().unwrap();
        assert_eq!(file.memberships["instance"], ["keep"]);
    }

    #[test]
    fn snapshot_waits_for_the_complete_standalone_write() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(crate::state::layout::INSTANCE_GROUPS);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let guard = FILE_ACCESS
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        fs::write(&path, b"{").unwrap();
        let (sent, received) = std::sync::mpsc::channel();
        let (started, start) = std::sync::mpsc::channel();
        let directory = root.path().to_owned();
        let reader = std::thread::spawn(move || {
            started.send(()).unwrap();
            sent.send(snapshot(&directory).unwrap()).unwrap();
        });
        start.recv().unwrap();
        assert!(
            received
                .recv_timeout(std::time::Duration::from_millis(50))
                .is_err()
        );
        fs::write(&path, br#"{"schema_version":99,"future":"retained"}"#)
            .unwrap();
        drop(guard);
        let bytes = received
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["future"],
            "retained"
        );
        reader.join().unwrap();
    }

    #[test]
    fn normalize_inserts_favorites_first() {
        let mut file = file_with_groups(&["a", "b"]);
        file.normalize();
        assert_eq!(file.groups[0].id, FAVORITES_GROUP_ID);
        assert_eq!(file.groups.len(), 3);
    }

    #[test]
    fn normalize_moves_favorites_to_front() {
        let mut file = file_with_groups(&["a", FAVORITES_GROUP_ID, "b"]);
        file.normalize();
        assert_eq!(file.groups[0].id, FAVORITES_GROUP_ID);
        assert_eq!(file.groups[1].id, "a");
        assert_eq!(file.groups[2].id, "b");
    }

    #[test]
    fn normalize_drops_orphan_memberships() {
        let mut file = file_with_groups(&["a"]);
        file.memberships.insert(
            "instance".to_string(),
            vec!["a".to_string(), "nonexistent".to_string()],
        );

        file.normalize();
        assert_eq!(file.memberships["instance"], &["a"]);
    }

    #[test]
    fn normalize_drops_memberships_for_nonexistent_instances() {
        let mut file = file_with_groups(&["a"]);
        file.memberships.insert(
            "instance".to_string(),
            vec!["nonexistent1".to_string(), "nonexistent2".to_string()],
        );
        file.normalize();
        assert!(!file.memberships.contains_key("instance"));
    }

    fn group_ids(groups: &[GroupDefinition]) -> Vec<String> {
        groups.iter().map(|group| group.id.clone()).collect()
    }

    #[test]
    fn membership_changes_preserve_other_groups_and_favorites() {
        let mut file = file_with_groups(&["a", "b"]);
        file.normalize();
        file.update_membership(
            "instance",
            &["a".into(), FAVORITES_GROUP_ID.into()],
            &[],
        );
        file.update_membership("instance", &["b".into(), "b".into()], &[]);
        assert_eq!(
            file.memberships["instance"],
            &["a", FAVORITES_GROUP_ID, "b"]
        );
        file.update_membership("instance", &[], &["a".into()]);
        assert_eq!(file.memberships["instance"], &[FAVORITES_GROUP_ID, "b"]);
        file.update_membership("instance", &[], &[FAVORITES_GROUP_ID.into()]);
        assert_eq!(file.memberships["instance"], &["b"]);
        file.update_membership("instance", &[], &["b".into()]);
        file.normalize();
        assert!(!file.memberships.contains_key("instance"));
    }

    #[test]
    fn concurrent_relationship_changes_do_not_overwrite_each_other() {
        let file = RwLock::new(file_with_groups(&["a", "b"]));
        std::thread::scope(|scope| {
            for id in ["a", "b", FAVORITES_GROUP_ID] {
                let file = &file;
                scope.spawn(move || {
                    let mut file = file.write().unwrap();
                    file.update_membership("instance", &[id.into()], &[]);
                    file.normalize();
                });
            }
        });
        let file = file.into_inner().unwrap();
        let groups = &file.memberships["instance"];
        assert_eq!(groups.len(), 3);
        for id in ["a", "b", FAVORITES_GROUP_ID] {
            assert!(groups.iter().any(|group| group == id));
        }
    }

    #[test]
    fn set_order_ignores_unknown_ids() {
        let mut groups = file_with_groups(&["a", "b", "c"]).groups;

        reorder_groups(&mut groups, &["unknown".to_string(), "c".to_string()]);

        // The unknown id matches nothing and simply contributes no group.
        assert_eq!(group_ids(&groups), vec!["c", "a", "b"]);
    }

    #[test]
    fn set_order_retains_missing_groups_at_end() {
        let mut groups = file_with_groups(&["a", "b", "c"]).groups;
        groups.push(GroupDefinition {
            id: "extra".to_string(),
            name: "extra".to_string(),
        });

        reorder_groups(&mut groups, &["b".to_string()]);

        // `b` moves to the front and every group the argument omitted survives
        // after it, so a stale list can never drop a group. Favorites is pinned
        // by `normalize`, which the store applies around this call.
        assert_eq!(group_ids(&groups), vec!["b", "a", "c", "extra"]);
    }

    #[test]
    fn set_order_keeps_favorites_out_of_the_reorder() {
        let mut groups = file_with_groups(&["a", FAVORITES_GROUP_ID]).groups;

        reorder_groups(&mut groups, &[FAVORITES_GROUP_ID.to_string()]);

        // Favorites is skipped explicitly and survives at its previous position;
        // `normalize` moves it to the front afterwards.
        assert_eq!(group_ids(&groups), vec!["a", FAVORITES_GROUP_ID]);
    }

    #[test]
    fn normalize_does_not_lose_favorites_membership() {
        let mut file = file_with_groups(&[]);
        file.memberships.insert(
            "my_instance".to_string(),
            vec![FAVORITES_GROUP_ID.to_string()],
        );
        file.normalize();
        assert_eq!(file.memberships["my_instance"], &[FAVORITES_GROUP_ID]);
    }

    #[test]
    fn normalize_deduplicates_group_ids() {
        let mut file = file_with_groups(&["a", "b"]);
        file.memberships.insert(
            "instance".to_string(),
            vec!["a".to_string(), "a".to_string(), "b".to_string()],
        );
        file.normalize();
        assert_eq!(file.memberships["instance"].len(), 2);
    }

    #[tokio::test]
    async fn imports_legacy_instance_group_schema() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE instance_groups (
                instance_id TEXT NOT NULL,
                group_name TEXT NOT NULL,
                PRIMARY KEY (instance_id, group_name)
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO instance_groups (instance_id, group_name)
             VALUES ('instance-1', 'Survival'), ('instance-2', 'Survival'),
                    ('instance-1', 'Creative')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let file = import_sql_groups(&pool).await.unwrap();

        assert_eq!(file.groups.len(), 3);
        assert_eq!(file.groups[0].id, FAVORITES_GROUP_ID);
        assert_eq!(file.memberships["instance-1"].len(), 2);
        assert_eq!(file.memberships["instance-2"].len(), 1);
        assert!(file.groups.iter().any(|group| group.name == "Survival"));
    }
}
