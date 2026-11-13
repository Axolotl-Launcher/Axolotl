use super::*;
use crate::api::curseforge::{
    CurseForgePackExpectedContent, CurseForgePackExpectedMember,
};
use crate::install::model::{InstallCleanup, InstallRollbackState};
use crate::install::{
    InstallJobState, InstallJobStatus, InstallRequest, recovery, store,
};
use crate::state::instances::adapters::sqlite::{
    content_entries, files, pack_members,
};
use crate::state::instances::{ContentOwnershipKind, PackMemberOverrideKind};
use crate::state::{InstanceInstallStage, ModLoader, ProjectType};

async fn record(
    state: &State,
    instance: &str,
    path: &str,
    project: u32,
    release: u32,
    ownership: ContentOwnershipKind,
) -> String {
    let metadata = crate::state::get_instance(instance, &state.pool)
        .await
        .unwrap()
        .unwrap();
    let root = state.directories.instance_game_dir(&metadata.instance);
    let target = root.join(path);
    crate::util::io::create_dir_all(target.parent().unwrap())
        .await
        .unwrap();
    let bytes = format!("project {project} release {release}");
    crate::util::io::write(&target, bytes.as_bytes())
        .await
        .unwrap();
    let (size, sha1) = sha1_file_async(&target).await.unwrap();
    let reference = ContentProviderRef::CurseForge {
        project_id: CurseForgeProjectId::new(project).unwrap(),
        file_id: Some(CurseForgeFileId::new(release).unwrap()),
    };
    crate::state::record_project_file_atomic(
        instance,
        path,
        &sha1,
        size,
        if path.starts_with("shaderpacks/") {
            ProjectType::ShaderPack
        } else {
            ProjectType::Mod
        },
        ContentSourceKind::CurseForge,
        ownership,
        Some(&reference),
        true,
        None,
        state,
    )
    .await
    .unwrap();
    sha1
}

fn expected(
    project: u32,
    release: u32,
    path: &str,
) -> CurseForgePackExpectedMember {
    CurseForgePackExpectedMember {
        project_id: project,
        file_id: release,
        project_type: if path.starts_with("shaderpacks/") {
            ProjectType::ShaderPack
        } else {
            ProjectType::Mod
        },
        expected_relative_path: path.into(),
        required: true,
        expected_sha1: None,
        expected_size: None,
        expected_fingerprint: None,
        manual_download: None,
    }
}

#[tokio::test]
async fn managed_update_replaces_old_files_and_recovers_persisted_baseline() {
    crate::event::EventState::init().await.unwrap();
    let temp = tempfile::tempdir().unwrap().keep();
    let state = State::init_for_test(temp.to_string_lossy().into_owned())
        .await
        .unwrap();
    let created = crate::api::instance::create(
        format!("CF update {}", uuid::Uuid::new_v4()),
        "1.21.1".into(),
        ModLoader::Vanilla,
        None,
        None,
        InstanceLink::CurseForgeModpack {
            project_id: "925200".into(),
            version_id: "1".into(),
        },
        None,
        None,
    )
    .await
    .unwrap();
    let id = &created.instance.id;
    let root = state.directories.instance_game_dir(&created.instance);
    for (project, path) in [
        (1, "mods/old.jar"),
        (2, "mods/old-disabled.jar.disabled"),
        (3, "shaderpacks/old.zip"),
        (4, "mods/same.jar"),
        (5, "mods/custom.jar"),
        (6, "mods/removed.jar"),
        (8, "mods/optional.jar"),
        (9, "mods/override.jar"),
        (11, "mods/missing.jar"),
    ] {
        record(
            &state,
            id,
            path,
            project,
            10,
            ContentOwnershipKind::PackManaged,
        )
        .await;
    }
    record(
        &state,
        id,
        "mods/user.jar",
        7,
        10,
        ContentOwnershipKind::UserAdded,
    )
    .await;
    record(
        &state,
        id,
        "mods/new.jar.disabled",
        12,
        10,
        ContentOwnershipKind::UserAdded,
    )
    .await;
    crate::util::io::remove_file(root.join("mods/missing.jar"))
        .await
        .unwrap();
    let metadata = crate::state::get_instance(id, &state.pool)
        .await
        .unwrap()
        .unwrap();
    let set_id = &metadata.applied_content_set.id;
    let mut members = pack_members::get_pack_members(set_id, &state.pool)
        .await
        .unwrap();
    let mut tx = state.pool.begin().await.unwrap();
    for member in &mut members {
        if member.provider_project_id.as_deref() == Some("5") {
            member.override_kind = PackMemberOverrideKind::Version;
        }
        if member.provider_project_id.as_deref() == Some("2") {
            member.override_kind = PackMemberOverrideKind::Disabled;
        }
        pack_members::upsert_pack_member_in_transaction(member, &mut tx)
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();
    let original_files =
        files::get_instance_files(id, &state.pool).await.unwrap();
    let entries = content_entries::get_content_entries(set_id, &state.pool)
        .await
        .unwrap()
        .into_iter()
        .filter_map(|entry| entry.file_id.map(|file| (entry.id, file)))
        .collect();
    let mut job =
        InstallJobState::new(InstallRequest::UpdateManagedCurseForgeModpack {
            instance_id: id.clone(),
            file_id: 2,
        });
    job.rollback = Some(InstallRollbackState {
        instance: metadata.clone(),
        install_stage: InstanceInstallStage::Installed,
        content: None,
    });
    job.cleanup = InstallCleanup::RestoreExistingInstance {
        instance_id: id.clone(),
    };
    let job_id = uuid::Uuid::new_v4();
    store::insert(job_id, &job, InstallJobStatus::Running, &state)
        .await
        .unwrap();
    recovery::prepare_existing_pack_update_rollback(job_id, &mut job, &state)
        .await
        .unwrap();
    assert!(root.join("mods/old.jar").exists());
    assert_eq!(
        pack_members::get_pack_members(set_id, &state.pool)
            .await
            .unwrap()
            .len(),
        members.len()
    );
    let mut legacy = serde_json::to_value(
        job.rollback.as_ref().unwrap().content.as_ref().unwrap(),
    )
    .unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("retain_original_files");
    let legacy: crate::install::model::InstallContentRollbackSnapshot =
        serde_json::from_value(legacy).unwrap();
    assert!(!legacy.retain_original_files);
    let reporter = InstallProgressReporter::new(job_id, job.clone());
    let override_path = temp.join("override.zip");
    {
        use std::io::Write;
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        archive.start_file("manifest.json", options).unwrap();
        archive.write_all(br#"{"minecraft":{"version":"1.21.1","modLoaders":[]},"files":[],"overrides":"overrides"}"#).unwrap();
        archive
            .start_file("overrides/mods/override.jar", options)
            .unwrap();
        archive.write_all(b"new override").unwrap();
        let bytes = archive.finish().unwrap().into_inner();
        crate::util::io::write(&override_path, &bytes)
            .await
            .unwrap();
        let mut archive =
            zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let manifest = crate::api::curseforge::archive::read_modpack_manifest(
            &mut archive,
        )
        .unwrap();
        let identities =
            crate::api::curseforge::archive::read_modpack_override_identities(
                &mut archive,
                &manifest,
            )
            .unwrap();
        crate::api::curseforge::archive::track_modpack_override_identities(
            &reporter,
            &identities,
        )
        .await
        .unwrap();
    }
    let mut installed = Vec::new();
    for (project, path) in [
        (1, "mods/new.jar"),
        (2, "mods/new-disabled.jar.disabled"),
        (3, "shaderpacks/new.zip"),
        (4, "mods/same.jar"),
        (11, "mods/missing.jar"),
        (9, "mods/override.jar"),
    ] {
        let bytes = format!("project {project} release 20");
        let sha1 =
            crate::util::fetch::sha1_async(bytes::Bytes::from(bytes.clone()))
                .await
                .unwrap();
        reporter
            .track_pack_update_file(path, &sha1, bytes.len() as u64)
            .await
            .unwrap();
        record(
            &state,
            id,
            path,
            project,
            20,
            ContentOwnershipKind::PackManaged,
        )
        .await;
        installed.push(CurseForgeInstalledFile {
            project_id: project,
            file_id: 20,
            relative_path: path.into(),
            dependency: false,
        });
    }
    let (_, replacements) =
        crate::api::curseforge::archive::materialize_modpack_overrides(
            &override_path,
            &root,
            None,
        )
        .unwrap();
    replacements.finalize().unwrap();
    let mut optional = expected(8, 20, "mods/optional-new.jar");
    optional.required = false;
    let expected = CurseForgePackExpectedContent {
        members: vec![
            expected(1, 20, "mods/new.jar"),
            expected(2, 20, "mods/new-disabled.jar"),
            expected(3, 20, "shaderpacks/new.zip"),
            expected(4, 20, "mods/same.jar"),
            expected(5, 20, "mods/custom-new.jar"),
            expected(11, 20, "mods/missing.jar"),
            expected(9, 20, "mods/override.jar"),
            optional,
        ],
        overrides: vec![
            crate::api::curseforge::modpack::CurseForgePackExpectedOverride {
                project_type: ProjectType::Mod,
                expected_relative_path: "mods/override.jar".into(),
            },
        ],
    };
    // An external edit must not be deleted on the strength of an old index row.
    crate::util::io::write(root.join("mods/old.jar"), b"user edit")
        .await
        .unwrap();
    assert!(
        finalize_updated_content(
            id,
            set_id,
            &members,
            &entries,
            &original_files,
            &expected,
            &installed,
            &state
        )
        .await
        .is_err()
    );
    assert_eq!(
        crate::util::io::read(root.join("mods/old.jar"))
            .await
            .unwrap(),
        b"user edit"
    );
    crate::util::io::write(root.join("mods/old.jar"), b"project 1 release 10")
        .await
        .unwrap();
    finalize_updated_content(
        id,
        set_id,
        &members,
        &entries,
        &original_files,
        &expected,
        &installed,
        &state,
    )
    .await
    .unwrap();
    for path in [
        "mods/old.jar",
        "mods/old-disabled.jar.disabled",
        "shaderpacks/old.zip",
        "mods/removed.jar",
    ] {
        assert!(!root.join(path).exists(), "old file survived: {path}");
    }
    for path in [
        "mods/new.jar",
        "mods/new-disabled.jar.disabled",
        "shaderpacks/new.zip",
        "mods/same.jar",
        "mods/custom.jar",
        "mods/user.jar",
        "mods/optional.jar",
        "mods/new.jar.disabled",
    ] {
        assert!(root.join(path).exists(), "required file missing: {path}");
    }
    let after = files::get_instance_files(id, &state.pool).await.unwrap();
    assert!(
        !after
            .iter()
            .any(|file| file.relative_path == "mods/old.jar")
    );
    assert!(
        !after
            .iter()
            .find(|file| file.relative_path == "mods/new-disabled.jar.disabled")
            .unwrap()
            .enabled
    );
    assert_eq!(
        crate::util::io::read(root.join("mods/override.jar"))
            .await
            .unwrap(),
        b"new override"
    );
    let updated_members = pack_members::get_pack_members(set_id, &state.pool)
        .await
        .unwrap();
    let disabled = updated_members
        .iter()
        .find(|member| member.provider_project_id.as_deref() == Some("2"))
        .unwrap();
    assert_eq!(disabled.override_kind, PackMemberOverrideKind::Disabled);
    let custom = updated_members
        .iter()
        .find(|member| member.provider_project_id.as_deref() == Some("5"))
        .unwrap();
    assert_eq!(custom.override_kind, PackMemberOverrideKind::Version);
    let custom_refs = content_entries::get_content_provider_refs(
        custom.content_entry_id.as_deref().unwrap(),
        &state.pool,
    )
    .await
    .unwrap();
    assert!(custom_refs.iter().any(|reference| matches!(reference, ContentProviderRef::CurseForge { file_id: Some(file), .. } if file.get() == 10)));
    let new_member = updated_members
        .into_iter()
        .find(|member| member.provider_project_id.as_deref() == Some("1"))
        .unwrap();
    assert_eq!(new_member.provider_release_id.as_deref(), Some("20"));
    assert_eq!(new_member.override_kind, PackMemberOverrideKind::None);
    let updated_entries =
        content_entries::get_content_entries(set_id, &state.pool)
            .await
            .unwrap();
    assert_eq!(
        updated_entries
            .iter()
            .find(|entry| Some(entry.id.as_str())
                == new_member.content_entry_id.as_deref())
            .unwrap()
            .ownership_kind,
        ContentOwnershipKind::PackManaged
    );
    let refs = content_entries::get_content_provider_refs(
        new_member.content_entry_id.as_deref().unwrap(),
        &state.pool,
    )
    .await
    .unwrap();
    assert!(refs.iter().any(|reference| matches!(reference, ContentProviderRef::CurseForge { file_id: Some(file), .. } if file.get() == 20)));
    // Cover publication before SQLite registration and a subsequent reconcile
    // that released the new entries' ownership before process recovery.
    let bytes = b"published before database";
    let sha1 =
        crate::util::fetch::sha1_async(bytes::Bytes::copy_from_slice(bytes))
            .await
            .unwrap();
    reporter
        .track_pack_update_file("mods/unindexed.jar", &sha1, bytes.len() as u64)
        .await
        .unwrap();
    crate::util::io::write(root.join("mods/unindexed.jar"), bytes)
        .await
        .unwrap();
    let mut tx = state.pool.begin().await.unwrap();
    content_entries::release_pack_managed_entries_in_transaction(
        set_id,
        chrono::Utc::now().timestamp(),
        &mut tx,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let mut persisted =
        store::get_required(job_id, &state).await.unwrap().state;
    crate::util::io::write(root.join("mods/same.jar"), b"external change")
        .await
        .unwrap();
    let conflict = recovery::apply_cleanup(&mut persisted, &state)
        .await
        .unwrap_err();
    assert!(conflict.to_string().contains("changed after pack update"));
    assert_eq!(
        crate::util::io::read(root.join("mods/same.jar"))
            .await
            .unwrap(),
        b"external change"
    );
    persisted.rollback_error =
        Some(crate::install::model::InstallErrorView::from_error(
            "rollback_error",
            crate::install::InstallPhaseId::RollingBack,
            &conflict,
            None,
        ));
    store::update_status(job_id, InstallJobStatus::Failed, &persisted, &state)
        .await
        .unwrap();
    assert!(crate::install::retry_job(job_id).await.is_err());
    assert!(crate::install::retry_job_as_new(job_id).await.is_err());
    let saved = store::get_required(job_id, &state).await.unwrap();
    assert!(
        saved
            .state
            .rollback
            .as_ref()
            .unwrap()
            .content
            .as_ref()
            .unwrap()
            .retain_original_files
    );
    crate::util::io::write(root.join("mods/same.jar"), b"project 4 release 20")
        .await
        .unwrap();
    recovery::apply_cleanup(&mut persisted, &state)
        .await
        .unwrap();
    assert!(!root.join("mods/unindexed.jar").exists());
    assert!(!root.join("mods/new.jar").exists());
    assert!(!root.join("mods/missing.jar").exists());
    for original in &original_files {
        if original.relative_path == "mods/missing.jar" {
            continue;
        }
        let (size, sha1) = sha1_file_async(&root.join(&original.relative_path))
            .await
            .unwrap();
        assert_eq!((size, sha1), (original.size, original.sha1.clone()));
    }
    let restored = content_entries::get_content_entries(set_id, &state.pool)
        .await
        .unwrap();
    assert_eq!(restored.len(), original_files.len());
    assert_eq!(
        pack_members::get_pack_members(set_id, &state.pool)
            .await
            .unwrap()
            .len(),
        members.len()
    );
}

#[tokio::test]
async fn managed_update_recovers_download_and_override_publication_windows() {
    crate::event::EventState::init().await.unwrap();
    let temp = tempfile::tempdir().unwrap().keep();
    let state = State::init_for_test(temp.to_string_lossy().into_owned())
        .await
        .unwrap();
    for publish_override in [false, true] {
        let created = crate::api::instance::create(
            format!("CF identity {}", uuid::Uuid::new_v4()),
            "1.21.1".into(),
            ModLoader::Vanilla,
            None,
            None,
            InstanceLink::CurseForgeModpack {
                project_id: "925200".into(),
                version_id: "1".into(),
            },
            None,
            None,
        )
        .await
        .unwrap();
        let id = &created.instance.id;
        let root = state.directories.instance_game_dir(&created.instance);
        let original_sha1 = record(
            &state,
            id,
            "mods/same.jar",
            20,
            10,
            ContentOwnershipKind::PackManaged,
        )
        .await;
        record(
            &state,
            id,
            "mods/User.jar",
            30,
            10,
            ContentOwnershipKind::UserAdded,
        )
        .await;
        let metadata = crate::state::get_instance(id, &state.pool)
            .await
            .unwrap()
            .unwrap();
        let mut job = InstallJobState::new(
            InstallRequest::UpdateManagedCurseForgeModpack {
                instance_id: id.clone(),
                file_id: 2,
            },
        );
        job.rollback = Some(InstallRollbackState {
            instance: metadata,
            install_stage: InstanceInstallStage::Installed,
            content: None,
        });
        job.cleanup = InstallCleanup::RestoreExistingInstance {
            instance_id: id.clone(),
        };
        let job_id = uuid::Uuid::new_v4();
        store::insert(job_id, &job, InstallJobStatus::Running, &state)
            .await
            .unwrap();
        recovery::prepare_existing_pack_update_rollback(
            job_id, &mut job, &state,
        )
        .await
        .unwrap();
        let reporter = InstallProgressReporter::new(job_id, job);
        if cfg!(any(windows, target_os = "macos")) {
            assert!(
                reporter
                    .track_pack_update_file("mods/user.jar", "untrusted", 10)
                    .await
                    .is_err()
            );
            assert_eq!(
                crate::util::io::read(root.join("mods/User.jar"))
                    .await
                    .unwrap(),
                b"project 30 release 10"
            );
        }
        let archive_path = temp.join(format!("{}.zip", job_id));
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        {
            use std::io::Write;
            archive.start_file("manifest.json", options).unwrap();
            archive.write_all(br#"{"minecraft":{"version":"1.21.1","modLoaders":[]},"files":[],"overrides":"overrides"}"#).unwrap();
            for path in ["mods/same.jar", "mods/new.jar"] {
                archive
                    .start_file(format!("overrides/{path}"), options)
                    .unwrap();
                archive.write_all(b"archive replacement").unwrap();
            }
        }
        let bytes = archive.finish().unwrap().into_inner();
        crate::util::io::write(&archive_path, &bytes).await.unwrap();
        let mut archive =
            zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let manifest = crate::api::curseforge::archive::read_modpack_manifest(
            &mut archive,
        )
        .unwrap();
        let identities =
            crate::api::curseforge::archive::read_modpack_override_identities(
                &mut archive,
                &manifest,
            )
            .unwrap();
        crate::api::curseforge::archive::track_modpack_override_identities(
            &reporter,
            &identities,
        )
        .await
        .unwrap();
        for (project, path) in [(20, "mods/same.jar"), (21, "mods/new.jar")] {
            let bytes = format!("project {project} release 20");
            let sha1 = crate::util::fetch::sha1_async(bytes::Bytes::from(
                bytes.clone(),
            ))
            .await
            .unwrap();
            reporter
                .track_pack_update_file(path, &sha1, bytes.len() as u64)
                .await
                .unwrap();
            record(
                &state,
                id,
                path,
                project,
                20,
                ContentOwnershipKind::PackManaged,
            )
            .await;
        }
        if publish_override {
            let (_, replacements) =
                crate::api::curseforge::archive::materialize_modpack_overrides(
                    &archive_path,
                    &root,
                    None,
                )
                .unwrap();
            replacements.finalize().unwrap();
        }
        let mut persisted =
            store::get_required(job_id, &state).await.unwrap().state;
        assert_eq!(
            persisted
                .rollback
                .as_ref()
                .unwrap()
                .content
                .as_ref()
                .unwrap()
                .replacement_files
                .len(),
            4
        );
        recovery::apply_cleanup(&mut persisted, &state)
            .await
            .unwrap();
        assert_eq!(
            sha1_file_async(&root.join("mods/same.jar"))
                .await
                .unwrap()
                .1,
            original_sha1
        );
        assert!(!root.join("mods/new.jar").exists());
        assert_eq!(
            files::get_instance_files(id, &state.pool)
                .await
                .unwrap()
                .len(),
            2
        );
    }
}

#[tokio::test]
async fn managed_update_replaces_same_filename_disabled_file_and_recovers() {
    crate::event::EventState::init().await.unwrap();
    let temp = tempfile::tempdir().unwrap().keep();
    let state = State::init_for_test(temp.to_string_lossy().into_owned())
        .await
        .unwrap();
    let created = crate::api::instance::create(
        format!("CF disabled {}", uuid::Uuid::new_v4()),
        "1.21.1".into(),
        ModLoader::Vanilla,
        None,
        None,
        InstanceLink::CurseForgeModpack {
            project_id: "925200".into(),
            version_id: "1".into(),
        },
        None,
        None,
    )
    .await
    .unwrap();
    let id = &created.instance.id;
    let root = state.directories.instance_game_dir(&created.instance);
    let original_sha1 = record(
        &state,
        id,
        "mods/same.jar.disabled",
        40,
        10,
        ContentOwnershipKind::PackManaged,
    )
    .await;
    let metadata = crate::state::get_instance(id, &state.pool)
        .await
        .unwrap()
        .unwrap();
    let set_id = &metadata.applied_content_set.id;
    let mut members = pack_members::get_pack_members(set_id, &state.pool)
        .await
        .unwrap();
    let mut tx = state.pool.begin().await.unwrap();
    members[0].override_kind = PackMemberOverrideKind::Disabled;
    pack_members::upsert_pack_member_in_transaction(&members[0], &mut tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let original_files =
        files::get_instance_files(id, &state.pool).await.unwrap();
    let original_entries =
        content_entries::get_content_entries(set_id, &state.pool)
            .await
            .unwrap();
    let originals =
        crate::api::curseforge::modpack::original_disabled_pack_files(
            &members,
            &original_entries,
            &original_files,
        );
    assert!(originals.contains_key("40"));
    let entries = original_entries
        .into_iter()
        .filter_map(|entry| entry.file_id.map(|file| (entry.id, file)))
        .collect();
    let mut job =
        InstallJobState::new(InstallRequest::UpdateManagedCurseForgeModpack {
            instance_id: id.clone(),
            file_id: 2,
        });
    job.rollback = Some(InstallRollbackState {
        instance: metadata.clone(),
        install_stage: InstanceInstallStage::Installed,
        content: None,
    });
    job.cleanup = InstallCleanup::RestoreExistingInstance {
        instance_id: id.clone(),
    };
    let job_id = uuid::Uuid::new_v4();
    store::insert(job_id, &job, InstallJobStatus::Running, &state)
        .await
        .unwrap();
    recovery::prepare_existing_pack_update_rollback(job_id, &mut job, &state)
        .await
        .unwrap();
    let reporter = InstallProgressReporter::new(job_id, job);
    let bytes = b"project 40 release 20";
    let new_sha1 =
        crate::util::fetch::sha1_async(bytes::Bytes::copy_from_slice(bytes))
            .await
            .unwrap();
    reporter
        .track_pack_update_file("mods/same.jar", &new_sha1, bytes.len() as u64)
        .await
        .unwrap();
    record(
        &state,
        id,
        "mods/same.jar",
        40,
        20,
        ContentOwnershipKind::PackManaged,
    )
    .await;
    let installed = vec![CurseForgeInstalledFile {
        project_id: 40,
        file_id: 20,
        relative_path: "mods/same.jar".into(),
        dependency: false,
    }];
    let disabled = HashSet::from(["40".to_owned()]);
    crate::util::io::write(
        root.join("mods/same.jar.disabled"),
        b"external edit",
    )
    .await
    .unwrap();
    assert!(
        crate::api::curseforge::modpack::finalize_disabled_pack_files(
            id,
            &installed,
            &disabled,
            &originals,
            Some(&reporter),
            &state
        )
        .await
        .is_err()
    );
    assert_eq!(
        crate::util::io::read(root.join("mods/same.jar.disabled"))
            .await
            .unwrap(),
        b"external edit"
    );
    crate::util::io::write(
        root.join("mods/same.jar.disabled"),
        b"project 40 release 10",
    )
    .await
    .unwrap();
    crate::api::curseforge::modpack::finalize_disabled_pack_files(
        id,
        &installed,
        &disabled,
        &originals,
        Some(&reporter),
        &state,
    )
    .await
    .unwrap();
    let expected = CurseForgePackExpectedContent {
        members: vec![expected(40, 20, "mods/same.jar")],
        overrides: vec![],
    };
    finalize_updated_content(
        id,
        set_id,
        &members,
        &entries,
        &original_files,
        &expected,
        &installed,
        &state,
    )
    .await
    .unwrap();
    assert!(!root.join("mods/same.jar").exists());
    assert_eq!(
        sha1_file_async(&root.join("mods/same.jar.disabled"))
            .await
            .unwrap()
            .1,
        new_sha1
    );
    let current_files =
        files::get_instance_files(id, &state.pool).await.unwrap();
    assert_eq!(current_files.len(), 1);
    assert!(!current_files[0].enabled);
    let current_members = pack_members::get_pack_members(set_id, &state.pool)
        .await
        .unwrap();
    assert_eq!(
        current_members[0].override_kind,
        PackMemberOverrideKind::Disabled
    );
    let refs = content_entries::get_content_provider_refs(
        current_members[0].content_entry_id.as_deref().unwrap(),
        &state.pool,
    )
    .await
    .unwrap();
    assert!(refs.iter().any(|reference| matches!(reference, ContentProviderRef::CurseForge { file_id: Some(file), .. } if file.get() == 20)));
    let mut persisted =
        store::get_required(job_id, &state).await.unwrap().state;
    recovery::apply_cleanup(&mut persisted, &state)
        .await
        .unwrap();
    assert!(!root.join("mods/same.jar").exists());
    assert_eq!(
        sha1_file_async(&root.join("mods/same.jar.disabled"))
            .await
            .unwrap()
            .1,
        original_sha1
    );
    assert_eq!(
        files::get_instance_files(id, &state.pool)
            .await
            .unwrap()
            .len(),
        1
    );
    let restored = pack_members::get_pack_members(set_id, &state.pool)
        .await
        .unwrap();
    assert_eq!(restored[0].override_kind, PackMemberOverrideKind::Disabled);
    assert_eq!(restored[0].provider_release_id.as_deref(), Some("10"));
}
