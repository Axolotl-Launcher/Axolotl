use super::*;

pub(super) async fn install_pack(
    job_id: Uuid,
    job_state: &mut InstallJobState,
    location: CreatePackLocation,
    instance_id: String,
    reason: DownloadReason,
) -> crate::Result<InstallExecutionOutcome<()>> {
    let reporter = InstallProgressReporter::new(job_id, job_state.clone());
    reporter
        .update(
            InstallPhaseId::DownloadingPackFile,
            None,
            modpack_details(&location),
        )
        .await?;

    let create_pack = match location {
        CreatePackLocation::FromVersionId {
            project_id,
            version_id,
            title,
            icon_url,
        } => {
            reporter
                .set_context(
                    InstallErrorContext::new("download modpack file")
                        .project_id(project_id.clone())
                        .version_id(version_id.clone())
                        .build(),
                )
                .await?;
            generate_pack_from_version_id_with_reporter(
                project_id,
                version_id,
                title,
                icon_url,
                instance_id.clone(),
                reason,
                reporter.clone(),
            )
            .await?
        }
        CreatePackLocation::FromFile { path } => {
            reporter
                .set_context(
                    InstallErrorContext::new("read local modpack file")
                        .source_path(path.display().to_string())
                        .build(),
                )
                .await?;
            match crate::api::pack::detect::detect_local_pack(&path).await {
                Ok(detected) => {
                    if detected.format
                        != crate::api::pack::detect::LocalPackFormat::Mrpack
                    {
                        // Non-mrpack format — dispatch to format-specific
                        // installer via install_local_pack_file.
                        return install_local_pack_file(
                            detected,
                            path,
                            instance_id,
                            reporter,
                        )
                        .await;
                    }
                    // Mrpack — fall through to standard mrpack install.
                    generate_pack_from_file(path, instance_id.clone()).await?
                }
                Err(detect_error) => {
                    // No format recognised — try recursive extraction
                    // (3-level deep search for sub-archives, bundled
                    // packs, etc.) before giving up.
                    tracing::debug!(
                        "Local pack format detection failed, trying recursive extraction: {detect_error}"
                    );
                    return install_local_pack_file_recursive(
                        path,
                        instance_id,
                        reporter,
                        0,
                        3,
                    )
                    .await;
                }
            }
        }
    };

    let outcome = install_zipped_mrpack_files_with_reporter(
        create_pack,
        false,
        reason,
        reporter,
    )
    .await?;
    Ok(match outcome {
        MrpackInstallOutcome::Completed(_) => {
            InstallExecutionOutcome::Completed(())
        }
        MrpackInstallOutcome::WaitingForUser(reason) => {
            InstallExecutionOutcome::WaitingForUser(reason)
        }
    })
}

/// Recursively tries to detect and install a modpack, up to max_depth levels.
#[async_recursion::async_recursion]
async fn install_local_pack_file_recursive(
    path: PathBuf,
    instance_id: String,
    reporter: InstallProgressReporter,
    current_depth: usize,
    max_depth: usize,
) -> crate::Result<InstallExecutionOutcome<()>> {
    // First try standard detection - this will already include our InstanceFolder fallback
    if let Ok(detected) =
        crate::api::pack::detect::detect_local_pack(&path).await
    {
        // If it's a standard format (including InstanceFolder), just install it
        return install_local_pack_file(detected, path, instance_id, reporter)
            .await;
    }

    // If standard detection failed and we're not at max depth, try to look for
    // sub-compressed files to extract and check
    if current_depth < max_depth {
        // Report progress: scanning/extracting phase (high-latency operation)
        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "archive".to_string());
        reporter
            .update(
                InstallPhaseId::ResolvingPack,
                None,
                InstallPhaseDetails::Modpack {
                    project_id: None,
                    version_id: None,
                    title: Some(format!("Scanning {filename} (level {current_depth}/{max_depth})")),
                },
            )
            .await?;

        let state = State::get().await?;
        let scratch =
            crate::api::pack::archive_util::create_import_scratch_dir(&state)
                .await?;

        // Extract the entire archive to check for sub-packs
        // First, let's list all entries to find potential sub-compressed files
        let file = std::fs::File::open(&path)?;
        let mut archive = match zip::ZipArchive::new(file) {
            Ok(a) => a,
            Err(_) => {
                // Not a valid zip, can't proceed further
                let _ = tokio::fs::remove_dir_all(&scratch).await;
                return Err(crate::ErrorKind::InputError(
                    "Unrecognized modpack format: no known pack manifest was found in the archive".to_string()
                ).into());
            }
        };

        let mut sub_archive_paths = Vec::new();

        // Collect all potential sub-archive files
        for i in 0..archive.len() {
            let lower_name = {
                let entry = archive
                    .by_index_raw(i)
                    .map_err(|e| ErrorKind::OtherError(e.to_string()))?;
                let name = crate::api::pack::detect::decode_zip_entry_name(
                    entry.name_raw(),
                );
                name.to_lowercase()
            }; // entry dropped here, releasing the mutable borrow on archive

            // Check if it looks like a compressed file
            if lower_name.ends_with(".zip") || lower_name.ends_with(".mrpack") {
                // Extract this sub-archive
                reporter
                    .update(
                        InstallPhaseId::ExtractingOverrides,
                        Some(InstallProgress {
                            current: sub_archive_paths.len() as u64 + 1,
                            total: archive.len() as u64,
                            secondary: None,
                        }),
                        InstallPhaseDetails::Modpack {
                            project_id: None,
                            version_id: None,
                            title: Some(format!("Extracting {filename}")),
                        },
                    )
                    .await?;

                let mut entry = archive
                    .by_index(i)
                    .map_err(|e| ErrorKind::OtherError(e.to_string()))?;
                let sub_path = scratch.join(
                    entry
                        .mangled_name()
                        .map_err(|e| ErrorKind::OtherError(e.to_string()))?,
                );

                if let Some(parent) = sub_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                let mut out = std::fs::File::create(&sub_path)?;
                std::io::copy(&mut entry, &mut out)?;

                sub_archive_paths.push(sub_path);
            }
        }

        // Try to install each sub-archive recursively
        for sub_path in sub_archive_paths {
            let result = install_local_pack_file_recursive(
                sub_path,
                instance_id.clone(),
                reporter.clone(),
                current_depth + 1,
                max_depth,
            )
            .await;

            if result.is_ok() {
                // Success! Clean up and return
                let _ = tokio::fs::remove_dir_all(&scratch).await;
                return result;
            }
        }

        // Clean up scratch directory
        let _ = tokio::fs::remove_dir_all(&scratch).await;
    }

    // If all else fails, return error
    Err(ErrorKind::InputError(
        "Unrecognized modpack format: no known pack manifest was found in the archive"
            .to_string(),
    ).into())
}

/// Dispatches a local non-mrpack modpack file to its format-specific
/// installer, based on the detected pack format.
#[async_recursion::async_recursion]
async fn install_local_pack_file(
    detected: crate::api::pack::detect::DetectedLocalPack,
    path: PathBuf,
    instance_id: String,
    reporter: InstallProgressReporter,
) -> crate::Result<InstallExecutionOutcome<()>> {
    use crate::api::pack::detect::LocalPackFormat;

    let source_filename = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string());
    match detected.format {
        LocalPackFormat::Mrpack => {
            let create_pack =
                generate_pack_from_file(path, instance_id.clone()).await?;
            return Ok(
                match install_zipped_mrpack_files_with_reporter(
                    create_pack,
                    false,
                    DownloadReason::Modpack,
                    reporter,
                )
                .await?
                {
                    MrpackInstallOutcome::Completed(_) => {
                        InstallExecutionOutcome::Completed(())
                    }
                    MrpackInstallOutcome::WaitingForUser(reason) => {
                        InstallExecutionOutcome::WaitingForUser(reason)
                    }
                },
            );
        }
        LocalPackFormat::CurseForge => {
            let skipped_missing_content_paths = reporter
                .current_state()
                .await?
                .skipped_missing_content_paths;
            let result = crate::api::curseforge::install_modpack_from_local_archive_with_reporter(
                instance_id,
                path,
                detected.base_folder,
                source_filename,
                false,
                reporter,
                crate::launcher::InstanceCompletionPolicy::DeferToInstallJob,
            )
            .await?;
            if let Some(reason) = curseforge_manual_download_pause(
                &result,
                &skipped_missing_content_paths,
            ) {
                return Ok(InstallExecutionOutcome::WaitingForUser(reason));
            }
        }
        LocalPackFormat::Mcbbs => {
            crate::api::pack::install_mcbbs::install_mcbbs_pack_with_reporter(
                instance_id,
                path,
                detected.base_folder,
                source_filename,
                reporter,
            )
            .await?;
        }
        LocalPackFormat::Hmcl => {
            crate::api::pack::install_hmcl::install_hmcl_pack_with_reporter(
                instance_id,
                path,
                detected.base_folder,
                source_filename,
                reporter,
            )
            .await?;
        }
        LocalPackFormat::MmcExport => {
            crate::api::pack::install_mmc_zip::install_mmc_zip_with_reporter(
                instance_id,
                path,
                detected.base_folder,
                source_filename,
                reporter,
            )
            .await?;
        }
        LocalPackFormat::LauncherBundled => {
            let inner_entry = detected.inner_pack_entry.ok_or_else(|| {
                ErrorKind::InputError(
                    "Launcher bundle is missing its inner modpack file"
                        .to_string(),
                )
            })?;
            let state = State::get().await?;
            let scratch =
                crate::api::pack::archive_util::create_import_scratch_dir(
                    &state,
                )
                .await?;
            let inner_name = inner_entry
                .rsplit('/')
                .next()
                .unwrap_or("modpack.zip")
                .to_string();
            let inner_path = scratch.join(&inner_name);
            crate::api::pack::archive_util::extract_archive_entry_to_file(
                path,
                inner_entry,
                inner_path.clone(),
            )
            .await?;

            // Use our recursive function to install the inner pack
            let result = install_local_pack_file_recursive(
                inner_path,
                instance_id,
                reporter,
                1, // already one level deep
                3,
            )
            .await;

            // Clean up temporary directory
            if let Err(error) = tokio::fs::remove_dir_all(&scratch).await {
                tracing::warn!(
                    "Failed to clean up modpack import scratch directory {}: {error}",
                    scratch.display()
                );
            }

            return result;
        }
        LocalPackFormat::PlainArchive => {
            let version_id = detected.plain_version_id.ok_or_else(|| {
                ErrorKind::InputError(
                    "Could not locate the instance version in the archive"
                        .to_string(),
                )
            })?;
            crate::api::pack::install_plain_archive::install_plain_archive_with_reporter(
                instance_id,
                path,
                detected.base_folder,
                version_id,
                source_filename,
                reporter,
            )
            .await?;
        }
        LocalPackFormat::InstanceFolder => {
            // Extract the base folder contents to a temporary directory
            let state = State::get().await?;
            let scratch =
                crate::api::pack::archive_util::create_import_scratch_dir(
                    &state,
                )
                .await?;

            // Extract the instance folder contents
            crate::api::pack::archive_util::extract_archive_subdir(
                path,
                detected.base_folder,
                scratch.clone(),
            )
            .await?;

            // Now import it as a generic instance
            let details = InstallPhaseDetails::Modpack {
                project_id: None,
                version_id: None,
                title: source_filename.clone(),
            };

            crate::api::pack::import::generic::import_generic(
                scratch,
                &instance_id,
                reporter,
                details,
                false,
                &crate::api::pack::import::ImportOverrides::default(),
                None, // Not compatible mode
            )
            .await?;
        }
    }
    Ok(InstallExecutionOutcome::Completed(()))
}

pub(super) fn curseforge_manual_download_pause(
    result: &crate::api::curseforge::CurseForgeModpackInstallResult,
    skipped_missing_content_paths: &[String],
) -> Option<InstallPauseReason> {
    let missing_downloads = result
        .content
        .manual_downloads
        .iter()
        .filter(|download| {
            !skipped_missing_content_paths.contains(&download.file_name)
        })
        .collect::<Vec<_>>();
    if missing_downloads.is_empty() {
        return None;
    }
    Some(InstallPauseReason::MissingRequiredContent {
        failed_files: missing_downloads.len() as u64,
        paths: missing_downloads
            .iter()
            .map(|download| download.file_name.clone())
            .collect(),
    })
}

pub(super) fn curseforge_world_was_imported_manually(
    job_state: &InstallJobState,
    request: &crate::api::curseforge::CurseForgeWorldInstallRequest,
) -> bool {
    let project_id = request.project_id.to_string();
    let file_id = request.file_id.to_string();
    job_state.download_items().iter().any(|item| {
        item.status == crate::install::model::DownloadItemStatus::Completed
            && item.project_id.as_deref() == Some(project_id.as_str())
            && item.version_id.as_deref() == Some(file_id.as_str())
    })
}
