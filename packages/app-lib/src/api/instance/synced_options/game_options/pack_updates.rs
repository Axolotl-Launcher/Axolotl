//! Keeps game settings intact when a modpack is installed or updated.

use super::MAX_OPTIONS_BYTES;
use super::options_file::{input_error, options_path, sha1_bytes};
use super::write_shared_settings::{
    apply_shared_settings_to_instance, capture_instance_options,
    sync_is_active_for_instance,
};
use crate::state::store::Collection as _;
use crate::state::store::game_option_pack_bases::{
    GameOptionPackBases, PackBaseKey, PackBaseRecord, PackBaseSource,
};
use crate::state::store::game_option_update_state::{
    GameOptionUpdateState, UpdateStateKey, UpdateStateRecord,
};
use crate::state::store::instance_sync_checkpoints::{
    CheckpointScope, InstanceSyncCheckpoints,
};
use crate::state::{InstanceLink, InstanceMetadata, State, SyncedOption};
use crate::util::io;
use encoding_rs::{UTF_8, UTF_16BE, UTF_16LE};
use std::io::ErrorKind;
use std::path::Path;

const YOSBR_OPTIONS_PATH: &str = "config/yosbr/options.txt";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameOptionsPackSource {
    ClientOverrides,
    Overrides,
    ClientOverridesYosbr,
    OverridesYosbr,
}

impl GameOptionsPackSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::ClientOverrides | Self::ClientOverridesYosbr => {
                "client_overrides"
            }
            Self::Overrides | Self::OverridesYosbr => "overrides",
        }
    }

    fn is_yosbr(self) -> bool {
        matches!(self, Self::ClientOverridesYosbr | Self::OverridesYosbr)
    }
}

fn detected_encoding_name(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return UTF_16LE.name().to_string();
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return UTF_16BE.name().to_string();
    }
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return UTF_8.name().to_string();
    }
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(bytes, true);
    detector.guess(None, true).name().to_string()
}

fn pack_version_id(metadata: &InstanceMetadata) -> Option<&str> {
    match &metadata.link {
        InstanceLink::ModrinthModpack { version_id, .. } => Some(version_id),
        InstanceLink::ServerProjectModpack {
            content_version_id, ..
        } => Some(content_version_id),
        InstanceLink::ImportedModpack { version_id, .. } => {
            version_id.as_deref()
        }
        InstanceLink::SharedInstance { .. } => None,
        _ => None,
    }
}

async fn read_pack_options(
    path: &Path,
    name: &str,
) -> crate::Result<Option<(Vec<u8>, String, String)>> {
    let metadata = match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io::IOError::with_path(error, path).into()),
    };
    if metadata.file_type().is_symlink() {
        return Err(input_error(format!(
            "Refusing to capture a symlinked {name}"
        )));
    }
    if metadata.len() > MAX_OPTIONS_BYTES as u64 {
        return Err(input_error(format!(
            "The modpack {name} is too large to capture for syncing"
        )));
    }
    let bytes = io::read(path).await?;
    let sha1 = sha1_bytes(&bytes);
    let encoding = detected_encoding_name(&bytes);
    Ok(Some((bytes, sha1, encoding)))
}

async fn options_target_exists(path: &Path) -> crate::Result<bool> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(input_error("Refusing to replace a symlinked options.txt"))
        }
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io::IOError::with_path(error, path).into()),
    }
}

pub(super) async fn materialize_yosbr_options_if_missing(
    metadata: &InstanceMetadata,
    state: &State,
) -> crate::Result<()> {
    let path = options_path(metadata, state)?;
    if options_target_exists(&path).await? {
        return Ok(());
    }
    let template_path =
        crate::state::instances::commands::instance_content_root(
            &state.directories,
            &metadata.instance,
        )?
        .join(YOSBR_OPTIONS_PATH);
    let Some((bytes, _, _)) =
        read_pack_options(&template_path, "YOSBR options.txt").await?
    else {
        return Ok(());
    };
    io::write(&path, bytes).await?;
    Ok(())
}

pub(in crate::api::instance) async fn detach_instance(
    metadata: &InstanceMetadata,
    state: &State,
) -> crate::Result<()> {
    let feature = SyncedOption::GameOptions.as_str();
    InstanceSyncCheckpoints::set_scoped(
        &state.data,
        &CheckpointScope {
            instance_id: metadata.instance.id.clone(),
            feature: feature.to_string(),
        },
        &[],
    )
    .await?;
    Ok(())
}

/// Before replacing modpack files, saves `options.txt` and shares any settings the
/// player changed in Minecraft.
pub(in crate::api::instance) async fn prepare_instance_update_with_state(
    metadata: &InstanceMetadata,
    state: &State,
) -> crate::Result<()> {
    let update_state = UpdateStateKey {
        instance_id: metadata.instance.id.clone(),
    };
    GameOptionUpdateState::set(&state.data, &update_state, None).await?;
    if sync_is_active_for_instance(metadata, state).await? {
        let _ = capture_instance_options(metadata, state, true).await?;
    }
    let path = options_path(metadata, state)?;
    let record = if path.exists() {
        let file_metadata = tokio::fs::symlink_metadata(&path)
            .await
            .map_err(|error| io::IOError::with_path(error, &path))?;
        if file_metadata.file_type().is_symlink() {
            return Err(input_error(
                "Refusing to prepare a symlinked options.txt for a modpack update",
            ));
        }
        if file_metadata.len() > MAX_OPTIONS_BYTES as u64 {
            return Err(input_error(
                "options.txt is too large to preserve for a modpack update",
            ));
        }
        let bytes = io::read(&path).await?;
        UpdateStateRecord::Replaced {
            sha1: sha1_bytes(&bytes),
            document: bytes,
        }
    } else {
        UpdateStateRecord::Absent
    };
    GameOptionUpdateState::set(&state.data, &update_state, Some(&record))
        .await?;
    Ok(())
}

/// Keeps the new modpack's `options.txt` as its default, then reapplies the user's
/// shared settings.
pub async fn capture_pack_base(
    instance_id: &str,
    source: Option<GameOptionsPackSource>,
) -> crate::Result<()> {
    let state = State::get().await?;
    let _guard = state.lock_synced_options().await;
    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or_else(|| input_error("Unknown instance"))?;
    let path = options_path(&metadata, &state)?;
    let feature = SyncedOption::GameOptions.as_str();
    InstanceSyncCheckpoints::set_scoped(
        &state.data,
        &CheckpointScope {
            instance_id: instance_id.to_string(),
            feature: feature.to_string(),
        },
        &[],
    )
    .await?;

    if source.is_none() {
        let pending = GameOptionUpdateState::get(
            &state.data,
            &UpdateStateKey {
                instance_id: instance_id.to_string(),
            },
        )
        .await?;
        if let Some(pending) = pending {
            match pending {
                UpdateStateRecord::Replaced { document, .. } => {
                    io::write(&path, document).await?;
                }
                UpdateStateRecord::Absent => {
                    if path.exists() {
                        io::remove_file(&path).await?;
                    }
                }
            }
        }
    }

    let captured = if source.is_some_and(GameOptionsPackSource::is_yosbr) {
        let template_path =
            crate::state::instances::commands::instance_content_root(
                &state.directories,
                &metadata.instance,
            )?
            .join(YOSBR_OPTIONS_PATH);
        let template = read_pack_options(&template_path, "YOSBR options.txt")
            .await?
            .ok_or_else(|| {
                input_error("The modpack YOSBR options.txt is missing")
            })?;
        if !options_target_exists(&path).await? {
            io::write(&path, template.0.clone()).await?;
        }
        Some(template)
    } else {
        read_pack_options(&path, "options.txt").await?
    };
    let (document_bytes, sha1, encoding) = captured
        .map(|(document, sha1, encoding)| {
            (Some(document), Some(sha1), Some(encoding))
        })
        .unwrap_or((None, None, None));
    let source = match source {
        Some(
            GameOptionsPackSource::ClientOverrides
            | GameOptionsPackSource::ClientOverridesYosbr,
        ) => PackBaseSource::ClientOverrides,
        Some(
            GameOptionsPackSource::Overrides
            | GameOptionsPackSource::OverridesYosbr,
        ) => PackBaseSource::Overrides,
        None => PackBaseSource::None,
    };
    let version_id = pack_version_id(&metadata);
    GameOptionPackBases::set(
        &state.data,
        &PackBaseKey {
            instance_id: instance_id.to_string(),
        },
        Some(&PackBaseRecord {
            pack_version_id: version_id.map(str::to_string),
            source,
            sha1,
            encoding,
            document: document_bytes,
        }),
    )
    .await?;
    GameOptionUpdateState::set(
        &state.data,
        &UpdateStateKey {
            instance_id: instance_id.to_string(),
        },
        None,
    )
    .await?;

    if sync_is_active_for_instance(&metadata, &state).await?
        && let Err(error) =
            apply_shared_settings_to_instance(&metadata, &state, true).await
    {
        tracing::warn!(
            "Captured the modpack options.txt for {}, but shared settings could not be overlaid yet: {error}",
            metadata.instance.id
        );
    }
    Ok(())
}
