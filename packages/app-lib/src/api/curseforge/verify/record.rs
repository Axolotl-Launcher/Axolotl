//! Verification and persistence of installed CurseForge files.

use super::fingerprint::fingerprint_and_sha1_file;
use super::worker::{CurseForgeDatabaseTask, CurseForgeVerificationTask};
use crate::api::curseforge::dto::CurseForgeFile;
use crate::state::{
    ContentProviderRef, ContentSourceKind, CurseForgeFileId,
    CurseForgeProjectId, ProjectType,
};
use crate::util::fetch::{sha1_file_async, sha1_file_cancellable};
use crate::{ErrorKind, State};
use std::path::Path;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub(crate) struct DownloadedCurseForgeFile {
    pub(crate) relative_path: String,
}

async fn remove_curseforge_staged_file(path: &Path) {
    if tokio::fs::try_exists(path).await.unwrap_or(false)
        && let Err(error) = crate::util::io::remove_file(path).await
    {
        tracing::warn!(
            path = %path.display(),
            %error,
            "Failed to remove staged CurseForge download"
        );
    }
}

pub(crate) async fn restore_curseforge_materialization(
    instance_id: &str,
    full_path: &Path,
    previous_path: Option<&Path>,
) {
    let Ok(state) = State::get().await else {
        return;
    };
    let _instance_lock = state.lock_instance_content(instance_id).await;
    if let Err(error) =
        crate::state::restore_project_materialization(full_path, previous_path)
            .await
    {
        tracing::error!(
            instance_id,
            path = %full_path.display(),
            %error,
            "Failed to restore CurseForge file after database failure"
        );
    }
}

pub(crate) async fn restore_curseforge_materializations(
    instance_id: &str,
    tasks: &[CurseForgeDatabaseTask],
) {
    for task in tasks.iter().rev() {
        restore_curseforge_materialization(
            instance_id,
            &task.full_path,
            task.previous_path.as_deref(),
        )
        .await;
    }
}

pub(crate) async fn finalize_curseforge_materializations(
    tasks: &[CurseForgeDatabaseTask],
) {
    for task in tasks {
        if let Err(error) = crate::state::finalize_project_materialization(
            task.previous_path.as_deref(),
        )
        .await
        {
            tracing::warn!(
                path = %task.full_path.display(),
                %error,
                "Failed to remove previous CurseForge file backup"
            );
        }
    }
}

pub(crate) async fn verify_and_record_curseforge_modpack_file(
    task: CurseForgeVerificationTask,
    database_tx: mpsc::Sender<CurseForgeDatabaseTask>,
) -> crate::Result<()> {
    if task.cancellation.is_cancelled() {
        remove_curseforge_staged_file(&task.download_path).await;
        return Err(ErrorKind::OtherError(
            "CurseForge modpack verification canceled".to_string(),
        )
        .into());
    }

    let state = State::get().await?;
    let verified = match verify_installed_curseforge_file(
        &task.download_path,
        &task.file,
        Some(&task.cancellation),
        task.download_result.as_ref(),
    )
    .await
    {
        Ok(verified) => verified,
        Err(error) => {
            remove_curseforge_staged_file(&task.download_path).await;
            return Err(error);
        }
    };
    if task.cancellation.is_cancelled() {
        remove_curseforge_staged_file(&task.download_path).await;
        return Err(ErrorKind::OtherError(
            "CurseForge modpack verification canceled".to_string(),
        )
        .into());
    }
    // Only materialization needs the per-instance lock. Hashing and database
    // persistence run in separate workers so verification does not serialize
    // unrelated content operations for the whole hash duration.
    let instance_lock = tokio::select! {
        biased;
        _ = task.cancellation.cancelled() => {
            remove_curseforge_staged_file(&task.download_path).await;
            return Err(ErrorKind::OtherError(
                "CurseForge materialization canceled".to_string(),
            ).into());
        }
        lock = state.lock_instance_content(&task.instance_id) => lock,
    };
    let previous_path = match crate::state::materialize_project_download(
        &task.download_path,
        &task.full_path,
    )
    .await
    {
        Ok(previous_path) => previous_path,
        Err(error) => {
            drop(instance_lock);
            remove_curseforge_staged_file(&task.download_path).await;
            return Err(error);
        }
    };
    drop(instance_lock);
    remove_curseforge_staged_file(&task.download_path).await;

    let project_id = CurseForgeProjectId::new(task.file.mod_id)?;
    let file_id = CurseForgeFileId::new(task.file.id)?;
    let record = crate::state::instances::commands::ProjectFileRecord {
        relative_path: task.relative_path.clone(),
        sha1: verified.sha1,
        size: verified.size,
        project_type: task.project_type,
        source_kind: ContentSourceKind::CurseForge,
        ownership_kind: task.ownership_kind,
        provider_ref: Some(ContentProviderRef::CurseForge {
            project_id,
            file_id: Some(file_id),
        }),
        origin: true,
        known_modrinth_project_id: None,
        known_modrinth_version_id: None,
    };
    let verified_pending = match verified.pending_completion {
        CurseForgePendingCompletionProof::None => None,
        CurseForgePendingCompletionProof::AuthoritativeSha1
        | CurseForgePendingCompletionProof::AuthoritativeFingerprint => {
            Some((project_id, file_id))
        }
    };
    let database_task = CurseForgeDatabaseTask {
        instance_id: task.instance_id.clone(),
        record,
        verified_pending,
        expected_bytes: task.expected_bytes,
        full_path: task.full_path.clone(),
        previous_path,
    };
    let restore_full_path = database_task.full_path.clone();
    let restore_previous_path = database_task.previous_path.clone();
    let send_result = tokio::select! {
        biased;
        _ = task.cancellation.cancelled() => Err(ErrorKind::OtherError(
            "CurseForge modpack database registration canceled".to_string(),
        ).into()),
        result = database_tx.send(database_task) => result.map_err(|_| ErrorKind::OtherError(
            "CurseForge modpack database worker stopped".to_string(),
        ).into()),
    };
    if let Err(error) = send_result {
        restore_curseforge_materialization(
            &task.instance_id,
            &restore_full_path,
            restore_previous_path.as_deref(),
        )
        .await;
        return Err(error);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CurseForgePendingCompletionProof {
    None,
    AuthoritativeSha1,
    AuthoritativeFingerprint,
}

pub(crate) struct VerifiedInstalledCurseForgeFile {
    pub(crate) size: u64,
    pub(crate) sha1: String,
    pub(crate) pending_completion: CurseForgePendingCompletionProof,
}

pub(crate) async fn verify_installed_curseforge_file(
    path: &Path,
    file: &CurseForgeFile,
    cancellation: Option<&CancellationToken>,
    download: Option<&crate::util::fetch::DownloadResult>,
) -> crate::Result<VerifiedInstalledCurseForgeFile> {
    if cancellation.is_some_and(CancellationToken::is_cancelled) {
        return Err(ErrorKind::OtherError(
            "CurseForge verification canceled".into(),
        )
        .into());
    }
    if let Some(expected_sha1) = file
        .hashes
        .iter()
        .find(|hash| hash.algo == 1 && !hash.value.trim().is_empty())
        .map(|hash| hash.value.as_str())
    {
        if let Some(download) = download
            && download.path == path
            && download
                .verified_sha1
                .as_deref()
                .is_some_and(|hash| hash.eq_ignore_ascii_case(expected_sha1))
            && let Some(proof) = &download.verified_file
            && proof.matches(path, download.size).await
        {
            return Ok(VerifiedInstalledCurseForgeFile {
                size: download.size,
                sha1: expected_sha1.to_string(),
                pending_completion:
                    CurseForgePendingCompletionProof::AuthoritativeSha1,
            });
        }
        let (size, sha1) = match cancellation {
            Some(cancellation) => {
                sha1_file_cancellable(path, cancellation).await?
            }
            None => sha1_file_async(path).await?,
        };
        if !sha1.eq_ignore_ascii_case(expected_sha1) {
            return Err(
                ErrorKind::HashError(expected_sha1.to_string(), sha1).into()
            );
        }
        return Ok(VerifiedInstalledCurseForgeFile {
            size,
            sha1,
            pending_completion:
                CurseForgePendingCompletionProof::AuthoritativeSha1,
        });
    }

    if file.file_fingerprint != 0 {
        let (size, sha1, fingerprint) =
            fingerprint_and_sha1_file(path, cancellation).await?;
        if fingerprint as u64 != file.file_fingerprint {
            return Err(ErrorKind::InputError(
                "The downloaded file does not match the required CurseForge fingerprint"
                    .to_string(),
            )
            .into());
        }
        return Ok(VerifiedInstalledCurseForgeFile {
            size,
            sha1,
            pending_completion:
                CurseForgePendingCompletionProof::AuthoritativeFingerprint,
        });
    }

    let (size, sha1) = match cancellation {
        Some(cancellation) => sha1_file_cancellable(path, cancellation).await?,
        None => sha1_file_async(path).await?,
    };
    Ok(VerifiedInstalledCurseForgeFile {
        size,
        sha1,
        pending_completion: CurseForgePendingCompletionProof::None,
    })
}

pub(crate) async fn record_installed_curseforge_file_with_permit(
    instance_id: &str,
    relative_path: &str,
    full_path: &Path,
    file: &CurseForgeFile,
    project_type: ProjectType,
    ownership_kind: crate::state::instances::ContentOwnershipKind,
    database_permit: Option<tokio::sync::SemaphorePermit<'_>>,
    state: &State,
) -> crate::Result<()> {
    let verified =
        verify_installed_curseforge_file(full_path, file, None, None).await?;
    record_verified_curseforge_file_with_permit(
        instance_id,
        relative_path,
        file,
        project_type,
        ownership_kind,
        database_permit,
        state,
        verified,
    )
    .await
}

pub(crate) async fn record_verified_curseforge_file_with_permit(
    instance_id: &str,
    relative_path: &str,
    file: &CurseForgeFile,
    project_type: ProjectType,
    ownership_kind: crate::state::instances::ContentOwnershipKind,
    database_permit: Option<tokio::sync::SemaphorePermit<'_>>,
    state: &State,
    verified: VerifiedInstalledCurseForgeFile,
) -> crate::Result<()> {
    match verified.pending_completion {
        CurseForgePendingCompletionProof::None => {
            let provider_ref = ContentProviderRef::CurseForge {
                project_id: CurseForgeProjectId::new(file.mod_id)?,
                file_id: Some(CurseForgeFileId::new(file.id)?),
            };
            crate::state::instances::commands::record_project_files_atomic_with_permit(
                instance_id,
                &[crate::state::instances::commands::ProjectFileRecord {
                    relative_path: relative_path.to_string(),
                    sha1: verified.sha1,
                    size: verified.size,
                    project_type,
                    source_kind: ContentSourceKind::CurseForge,
                    ownership_kind,
                    provider_ref: Some(provider_ref),
                    origin: true,
                    known_modrinth_project_id: None,
                    known_modrinth_version_id: None,
                }],
                &[],
                database_permit,
                state,
            )
            .await
        }
        CurseForgePendingCompletionProof::AuthoritativeSha1
        | CurseForgePendingCompletionProof::AuthoritativeFingerprint => {
            crate::state::instances::commands::record_project_files_atomic_with_permit(
                instance_id,
                &[crate::state::instances::commands::ProjectFileRecord {
                    relative_path: relative_path.to_string(),
                    sha1: verified.sha1,
                    size: verified.size,
                    project_type,
                    source_kind: ContentSourceKind::CurseForge,
                    ownership_kind,
                    provider_ref: Some(ContentProviderRef::CurseForge {
                        project_id: CurseForgeProjectId::new(file.mod_id)?,
                        file_id: Some(CurseForgeFileId::new(file.id)?),
                    }),
                    origin: true,
                    known_modrinth_project_id: None,
                    known_modrinth_version_id: None,
                }],
                &[(CurseForgeProjectId::new(file.mod_id)?, CurseForgeFileId::new(file.id)?)],
                database_permit,
                state,
            )
            .await
        }
    }
}
