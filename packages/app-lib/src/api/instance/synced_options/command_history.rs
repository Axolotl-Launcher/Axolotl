use super::COMMAND_HISTORY_FILE;
use super::files::{
    CheckpointStatus, LinkMode, begin_checkpoint, checkpoint,
    finish_checkpoint, instance_game_dir, instance_is_running,
    publish_detached_copy, sha1_bytes, sha1_file, sync_files_are_protected,
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
    publish_detached_copy(&canonical, canonical_bytes.clone()).await?;
    let target = instance_game_dir(metadata, state)?.join(COMMAND_HISTORY_FILE);
    let expected = sha1_bytes(&canonical_bytes);
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
    let local = instance_game_dir(metadata, state)?.join(COMMAND_HISTORY_FILE);
    if !local.exists() {
        return ensure_command_history(metadata, state).await;
    }
    let symlink = tokio::fs::symlink_metadata(&local)
        .await
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false);
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
    let actual = sha1_file(&local).await?;
    let expected = current_checkpoint.map(|value| value.expected_sha1);
    if !symlink && expected.as_deref() != Some(actual.as_str()) {
        let contents =
            String::from_utf8_lossy(&io::read(&local).await?).into_owned();
        write_shared_command_history(
            normalize_command_history(&contents).into_bytes(),
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
    publish_detached_copy(&command_history_path(state), bytes).await
}

pub(super) fn command_history_path(state: &State) -> PathBuf {
    synced_options_path(state).join(COMMAND_HISTORY_FILE)
}
