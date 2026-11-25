use super::COMMAND_HISTORY_FILE;
use super::files::{
    CheckpointStatus, LinkMode, MAX_PROJECTION_BYTES, begin_checkpoint,
    checkpoint, detached_projection_matches, finish_checkpoint,
    instance_game_dir, instance_is_running, publish_detached_copy,
    read_detached_projection, sha1_bytes, sync_files_are_protected,
};
use super::orchestration::{
    create_synced_directories, option_effective, synced_options_path,
};
use crate::State;
use crate::state::{InstanceMetadata, SyncedOption};
use crate::util::io;
use std::path::PathBuf;

const COMMAND_HISTORY_LIMIT: usize = 50;

pub async fn get_command_history() -> crate::Result<String> {
    let state = State::get().await?;
    let _guard = state.lock_synced_options().await;
    let bytes =
        crate::state::sync_state::native_text_read(&state, "command_history")
            .await?
            .unwrap_or_default();
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub async fn set_command_history(contents: &str) -> crate::Result<String> {
    let state = State::get().await?;
    let _guard = state.lock_synced_options().await;
    create_synced_directories(&state).await?;
    let normalized = normalize_command_history(contents);
    write_shared_command_history(normalized.as_bytes().to_vec(), &state)
        .await?;
    refresh_command_history_links(&state).await?;
    Ok(normalized)
}

pub(super) async fn ensure_command_history(
    metadata: &InstanceMetadata,
    state: &State,
) -> crate::Result<()> {
    if sync_files_are_protected(metadata)
        || instance_is_running(metadata, state).await?
    {
        return Ok(());
    }
    create_synced_directories(state).await?;
    let canonical = command_history_path(state);
    let canonical_bytes = if let Some(bytes) =
        crate::state::sync_state::native_text_read(state, "command_history")
            .await?
    {
        bytes
    } else {
        let local =
            instance_game_dir(metadata, state)?.join(COMMAND_HISTORY_FILE);
        let contents = if local.exists() {
            String::from_utf8_lossy(&io::read(&local).await?).into_owned()
        } else {
            String::new()
        };
        let bytes = normalize_command_history(&contents).into_bytes();
        write_shared_command_history(bytes.clone(), state).await?;
        bytes
    };
    if !detached_projection_matches(&canonical, &canonical_bytes).await? {
        publish_detached_copy(&canonical, canonical_bytes.clone()).await?;
    }
    let target = instance_game_dir(metadata, state)?.join(COMMAND_HISTORY_FILE);
    let expected = sha1_bytes(&canonical_bytes);
    if checkpoint(
        &metadata.instance.id,
        SyncedOption::CommandHistory,
        "default",
        state,
    )
    .await?
    .is_some_and(|value| value.matches_projection(&expected, 0))
        && detached_projection_matches(&target, &canonical_bytes).await?
    {
        return Ok(());
    }
    let proof = begin_checkpoint(
        &metadata.instance.id,
        SyncedOption::CommandHistory,
        "default",
        &expected,
        None,
        0,
        state,
    )
    .await?;
    if sync_files_are_protected(metadata)
        || instance_is_running(metadata, state).await?
    {
        return Ok(());
    }
    publish_detached_copy(&target, canonical_bytes).await?;
    if sync_files_are_protected(metadata)
        || instance_is_running(metadata, state).await?
    {
        return Ok(());
    }
    finish_checkpoint(
        &metadata.instance.id,
        SyncedOption::CommandHistory,
        "default",
        LinkMode::Copy,
        &proof,
        state,
    )
    .await
}

pub(super) async fn reconcile_command_history(
    metadata: &InstanceMetadata,
    state: &State,
) -> crate::Result<()> {
    if sync_files_are_protected(metadata)
        || instance_is_running(metadata, state).await?
    {
        return Ok(());
    }
    if !option_effective(metadata, SyncedOption::CommandHistory, state).await? {
        return Ok(());
    }
    reconcile_projection(metadata, state).await
}

async fn reconcile_projection(
    metadata: &InstanceMetadata,
    state: &State,
) -> crate::Result<()> {
    let local = instance_game_dir(metadata, state)?.join(COMMAND_HISTORY_FILE);
    let current_checkpoint = checkpoint(
        &metadata.instance.id,
        SyncedOption::CommandHistory,
        "default",
        state,
    )
    .await?;
    if current_checkpoint
        .as_ref()
        .is_some_and(|value| value.status == CheckpointStatus::Pending)
    {
        return ensure_command_history(metadata, state).await;
    }
    let Some(bytes) =
        read_detached_projection(&local, MAX_PROJECTION_BYTES).await?
    else {
        return ensure_command_history(metadata, state).await;
    };
    let actual = sha1_bytes(&bytes);
    let expected = current_checkpoint.map(|value| value.expected_sha1);
    if expected.as_deref() != Some(actual.as_str()) {
        write_shared_command_history(
            normalize_command_history(&String::from_utf8_lossy(&bytes))
                .into_bytes(),
            state,
        )
        .await?;
        refresh_command_history_links(state).await?;
    } else {
        ensure_command_history(metadata, state).await?;
    }
    Ok(())
}

async fn refresh_command_history_links(state: &State) -> crate::Result<()> {
    let instances = crate::state::list_instances(&state.pool).await?;
    for metadata in instances {
        if option_effective(&metadata, SyncedOption::CommandHistory, state)
            .await?
        {
            ensure_command_history(&metadata, state).await?;
        }
    }
    Ok(())
}

pub(super) fn normalize_command_history(contents: &str) -> String {
    let lines = contents.lines().collect::<Vec<_>>();
    let start = lines.len().saturating_sub(COMMAND_HISTORY_LIMIT);
    let mut normalized = lines[start..].join("\n");
    if !normalized.is_empty() {
        normalized.push('\n');
    }
    normalized
}

pub(super) async fn merge_command_history_from_instance(
    metadata: &InstanceMetadata,
    state: &State,
) -> crate::Result<()> {
    let bytes =
        crate::state::sync_state::native_text_read(state, "command_history")
            .await?
            .unwrap_or_default();
    let canonical = String::from_utf8_lossy(&bytes).into_owned();
    let local_path =
        instance_game_dir(metadata, state)?.join(COMMAND_HISTORY_FILE);
    let local = if local_path.exists() {
        String::from_utf8_lossy(&io::read(&local_path).await?).into_owned()
    } else {
        String::new()
    };
    let canonical_lines = canonical.lines().collect::<Vec<_>>();
    let available = COMMAND_HISTORY_LIMIT.saturating_sub(canonical_lines.len());
    let mut seen = canonical_lines
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let mut imported = Vec::new();
    for line in local.lines().rev() {
        if imported.len() == available {
            break;
        }
        if seen.insert(line) {
            imported.push(line);
        }
    }
    imported.reverse();
    imported.extend(canonical_lines);
    let merged = normalize_command_history(&imported.join("\n"));
    if merged != normalize_command_history(&canonical) {
        write_shared_command_history(merged.into_bytes(), state).await?;
        refresh_command_history_links(state).await?;
    }
    Ok(())
}

pub(super) async fn write_shared_command_history(
    bytes: Vec<u8>,
    state: &State,
) -> crate::Result<()> {
    crate::state::sync_state::native_text(
        state,
        "command_history",
        bytes.clone(),
    )
    .await?;
    let path = command_history_path(state);
    if !detached_projection_matches(&path, &bytes).await? {
        publish_detached_copy(&path, bytes).await?;
    }
    Ok(())
}

pub(super) fn command_history_path(state: &State) -> PathBuf {
    synced_options_path(state).join(COMMAND_HISTORY_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::store::instance_sync_preferences::{
        InstancePreferenceKey, InstancePreferenceRecord,
        InstanceSyncPreferences,
    };
    use crate::state::sync_state::{manifest, native_text};
    use std::path::Path;

    async fn snapshot(root: &Path) -> (u64, usize, usize) {
        async fn count(path: PathBuf) -> usize {
            let mut entries = tokio::fs::read_dir(path).await.unwrap();
            let mut count = 0;
            while entries.next_entry().await.unwrap().is_some() {
                count += 1;
            }
            count
        }
        (
            manifest::load(root).await.unwrap().0.revision,
            count(root.join("state/generations")).await,
            count(root.join("state/journals")).await,
        )
    }

    #[tokio::test]
    async fn synced_command_history_converges_and_repairs_pending_stale_and_linked_files()
     {
        let temp = tempfile::tempdir().unwrap();
        let dirs = crate::state::DirectoryInfo {
            settings_dir: temp.path().join("settings"),
            config_dir: temp.path().join("content"),
            app_identifier: "command-history-sync-test".into(),
        };
        tokio::fs::create_dir_all(dirs.instances_dir().join("a"))
            .await
            .unwrap();
        let pool = crate::state::store::databases::open(&dirs.settings_dir)
            .await
            .unwrap();
        sqlx::raw_sql(
            "INSERT INTO instances(id, path, install_stage, launcher_feature_version, name, created, modified)
             VALUES ('a', 'a', 'installed', 'none', 'A', 1, 1);
             INSERT INTO instance_content_sets(id, instance_id, name, source_kind, status, game_version, loader, created, modified)
             VALUES ('set-a', 'a', 'A', 'local', 'available', '1.21', 'vanilla', 1, 1);
             UPDATE instances SET applied_content_set_id = 'set-a' WHERE id = 'a';",
        ).execute(&pool).await.unwrap();
        let state = crate::state::test_state(dirs, pool).await.unwrap();
        InstanceSyncPreferences::set(
            &state.data,
            &InstancePreferenceKey {
                instance_id: "a".into(),
                feature: "command_history".into(),
            },
            Some(&InstancePreferenceRecord { enabled: true }),
        )
        .await
        .unwrap();
        let metadata = crate::state::get_instance("a", &state.pool)
            .await
            .unwrap()
            .unwrap();
        let root = crate::state::sync_state::root(&state.persistence)
            .await
            .unwrap();
        let local = instance_game_dir(&metadata, &state)
            .unwrap()
            .join(COMMAND_HISTORY_FILE);
        let contents = b"/help\n";
        write_shared_command_history(contents.to_vec(), &state)
            .await
            .unwrap();
        ensure_command_history(&metadata, &state).await.unwrap();
        assert_eq!(tokio::fs::read(&local).await.unwrap(), contents);
        let before = snapshot(&root).await;
        let modified = tokio::fs::metadata(&local)
            .await
            .unwrap()
            .modified()
            .unwrap();
        for _ in 0..3 {
            reconcile_projection(&metadata, &state).await.unwrap();
            native_text(&state, "command_history", contents.to_vec())
                .await
                .unwrap();
        }
        assert_eq!(snapshot(&root).await, before);
        assert_eq!(
            tokio::fs::metadata(&local)
                .await
                .unwrap()
                .modified()
                .unwrap(),
            modified
        );

        #[cfg(windows)]
        assert_watcher_converges(&metadata, &state, &local, &root).await;

        let sha1 = sha1_bytes(contents);
        begin_checkpoint(
            "a",
            SyncedOption::CommandHistory,
            "default",
            &sha1,
            None,
            0,
            &state,
        )
        .await
        .unwrap();
        let pending = snapshot(&root).await;
        reconcile_projection(&metadata, &state).await.unwrap();
        assert_eq!(snapshot(&root).await.0, pending.0 + 1);
        assert!(
            checkpoint("a", SyncedOption::CommandHistory, "default", &state)
                .await
                .unwrap()
                .unwrap()
                .matches_projection(&sha1, 0)
        );

        let current_contents = b"/time set day\n";
        // The old local hash still matches its checkpoint when the shared
        // source changes independently.
        native_text(&state, "command_history", current_contents.to_vec())
            .await
            .unwrap();
        reconcile_projection(&metadata, &state).await.unwrap();
        assert_eq!(tokio::fs::read(&local).await.unwrap(), current_contents);
        assert_eq!(
            tokio::fs::read(command_history_path(&state)).await.unwrap(),
            current_contents
        );

        let alias = temp.path().join("alias.txt");
        tokio::fs::hard_link(&local, &alias).await.unwrap();
        reconcile_projection(&metadata, &state).await.unwrap();
        tokio::fs::write(&alias, b"alias edit").await.unwrap();
        assert_eq!(tokio::fs::read(&local).await.unwrap(), current_contents);
        let converged = snapshot(&root).await;
        reconcile_projection(&metadata, &state).await.unwrap();
        assert_eq!(snapshot(&root).await, converged);

        tokio::fs::write(&local, b"/weather clear\r\n")
            .await
            .unwrap();
        reconcile_projection(&metadata, &state).await.unwrap();
        assert_eq!(
            crate::state::sync_state::native_text_read(
                &state,
                "command_history"
            )
            .await
            .unwrap()
            .as_deref(),
            Some(b"/weather clear\n".as_slice())
        );
    }

    #[cfg(windows)]
    async fn assert_watcher_converges(
        metadata: &InstanceMetadata,
        state: &State,
        local: &Path,
        root: &Path,
    ) {
        use notify::RecursiveMode;
        use notify_debouncer_mini::{DebounceEventResult, new_debouncer};
        use std::time::Duration;

        let (send, mut receive) = tokio::sync::mpsc::unbounded_channel();
        let mut watcher = new_debouncer(
            Duration::from_secs(1),
            move |events: DebounceEventResult| {
                let _ = send.send(events);
            },
        )
        .unwrap();
        watcher
            .watcher()
            .watch(local.parent().unwrap(), RecursiveMode::NonRecursive)
            .unwrap();
        let before = snapshot(root).await;
        // Use the same replace operation that produces a launcher-authored
        // event, then run the command-history reconciliation for the event.
        publish_detached_copy(local, tokio::fs::read(local).await.unwrap())
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                let events = receive.recv().await.unwrap().unwrap();
                if events.iter().any(|event| event.path == local) {
                    reconcile_projection(metadata, state).await.unwrap();
                    break;
                }
            }
        })
        .await
        .expect("Windows watcher did not report projection publication");
        let followup = tokio::time::timeout(Duration::from_secs(4), async {
            loop {
                let events = receive.recv().await.unwrap().unwrap();
                if events.iter().any(|event| event.path == local) {
                    return events;
                }
            }
        })
        .await;
        assert!(
            followup.is_err(),
            "unexpected projection rewrite: {followup:?}"
        );
        assert_eq!(snapshot(root).await, before);
    }
}
