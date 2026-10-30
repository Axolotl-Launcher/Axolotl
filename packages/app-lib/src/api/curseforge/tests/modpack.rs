//! CurseForge modpack tests.

use super::fixtures::{
    create_stage6_instance, stage8_curseforge_file,
    write_override_size_test_archive, write_override_test_archive,
};
use crate::api::curseforge::archive::{
    collect_modpack_override_tasks, curseforge_override_content,
    extract_modpack_overrides, materialize_modpack_overrides,
    read_modpack_manifest, safe_archive_relative_path,
};
use crate::api::curseforge::catalog::unique_metadata_chunks;
use crate::api::curseforge::dto::{
    CurseForgeFileHash, CurseForgeManifestFile, CurseForgeProject,
};
use crate::api::curseforge::mapping::project_type_for_class;
use crate::api::curseforge::modpack::{
    CurseForgePackExpectedOverride, retry_modpack_file_install,
};
use crate::api::curseforge::selection::{
    curseforge_modpack_h2_range_concurrency, install_modpack_world_file,
    managed_project_type, modpack_target_folder,
};
use crate::api::curseforge::validation::validate_world_archive_name;
use crate::state::ProjectType;
use serde_json::json;
use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[test]
fn modpack_manifest_worlds_use_the_save_importer_instead_of_the_mod_file_installer()
 {
    assert_eq!(project_type_for_class(Some(17)), "world");
    assert_eq!(modpack_target_folder("world").unwrap(), "saves");
    assert_eq!(modpack_target_folder("mod").unwrap(), "mods");
    assert_eq!(
        modpack_target_folder("resourcepack").unwrap(),
        "resourcepacks"
    );
    assert!(modpack_target_folder("modpack").is_err());
    assert!(managed_project_type("world").is_err());
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn modpack_manifest_imports_world_archives_and_preserves_existing_saves_on_retry()
 {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (state, instance_id) = create_stage6_instance("manifest world").await;
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    archive
        .start_file(
            "Wrapped world/level.dat",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    archive.write_all(b"world level data").unwrap();
    archive
        .start_file(
            "Wrapped world/region/r.0.0.mca",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    archive.write_all(b"region bytes").unwrap();
    let bytes = archive.finish().unwrap().into_inner();
    let server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/world.zip", server.local_addr().unwrap());
    let response_bytes = bytes.clone();
    let server_task = tokio::spawn(async move {
        loop {
            let (mut stream, _) = server.accept().await.unwrap();
            let bytes = response_bytes.clone();
            tokio::spawn(async move {
                let mut request = [0; 8192];
                let n = stream.read(&mut request).await.unwrap();
                let head =
                    String::from_utf8_lossy(&request[..n]).starts_with("HEAD ");
                stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/zip\r\nConnection: close\r\n\r\n", bytes.len()).as_bytes()).await.unwrap();
                if !head {
                    stream.write_all(&bytes).await.unwrap();
                }
            });
        }
    });
    let project: CurseForgeProject = serde_json::from_value(json!({
        "id": 1733396, "gameId": 432, "name": "Pack world", "slug": "pack-world", "summary": "World bundled in a pack",
        "status": 4, "downloadCount": 0, "isFeatured": false, "primaryCategoryId": 0,
        "classId": 17, "mainFileId": 9206584, "dateCreated": "", "dateModified": "", "dateReleased": "", "isAvailable": true,
    })).unwrap();
    let mut file = stage8_curseforge_file(
        project.id,
        9206584,
        "Pack world.zip",
        bytes.len() as u64,
        vec![CurseForgeFileHash {
            algo: 1,
            value: sha1_smol::Sha1::from(&bytes).hexdigest(),
        }],
        0,
    );
    file.download_url = Some(url);
    let manifest_file = CurseForgeManifestFile {
        project_id: project.id,
        file_id: file.id,
        required: true,
    };
    let installed = tokio::time::timeout(
        Duration::from_secs(10),
        retry_modpack_file_install(
            &instance_id,
            &manifest_file,
            "world",
            "1.21.1",
            Some(6),
            crate::state::instances::ManualDownloadOperationKind::PackInstall,
            None,
            &file.file_name,
            CancellationToken::new(),
            None,
            None,
            Some((project.clone(), file.clone())),
        ),
    )
    .await
    .unwrap();
    server_task.abort();
    let result = installed.0.unwrap_or_else(|| panic!("{}", installed.2));
    assert_eq!(result.installed[0].relative_path, "saves/Pack world");
    let root = crate::api::instance::get_full_path(&instance_id)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(root.join("saves/Pack world/level.dat")).unwrap(),
        b"world level data"
    );
    assert_eq!(
        std::fs::read(root.join("saves/Pack world/region/r.0.0.mca")).unwrap(),
        b"region bytes"
    );
    std::fs::write(root.join("saves/Pack world/level.dat"), b"player progress")
        .unwrap();
    install_modpack_world_file(
        &instance_id,
        &project,
        &file,
        None,
        crate::state::instances::ManualDownloadOperationKind::PackUpdate,
    )
    .await
    .unwrap();
    assert_eq!(
        std::fs::read(root.join("saves/Pack world/level.dat")).unwrap(),
        b"player progress"
    );
    assert!(
        crate::state::get_instance(&instance_id, &state.pool)
            .await
            .unwrap()
            .is_some()
    );
    crate::api::instance::remove(&instance_id).await.unwrap();
}

#[test]
fn modpack_metadata_ids_are_deduplicated_before_batching() {
    let mut ids = (1..=120).collect::<Vec<_>>();
    ids.extend([1, 50, 120]);
    let chunks = unique_metadata_chunks(ids);

    assert_eq!(
        chunks.iter().map(Vec::len).collect::<Vec<_>>(),
        vec![50, 50, 20]
    );
    assert_eq!(
        chunks
            .iter()
            .flatten()
            .copied()
            .collect::<HashSet<_>>()
            .len(),
        120
    );
}

#[test]
fn modpack_archives_use_bounded_ranges_only_for_large_files() {
    assert_eq!(
        curseforge_modpack_h2_range_concurrency(128 * 1024 * 1024),
        Some(4)
    );
    assert_eq!(curseforge_modpack_h2_range_concurrency(1024 * 1024), None);
}

#[test]
fn curseforge_overrides_publish_after_all_entries_validate() {
    let root = tempfile::tempdir().unwrap();
    let archive_path = root.path().join("pack.zip");
    let instance_path = root.path().join("instance");
    let first = instance_path.join("config/first.txt");
    let second = instance_path.join("config/second.txt");
    std::fs::create_dir_all(first.parent().unwrap()).unwrap();
    std::fs::write(&first, b"old-first").unwrap();
    std::fs::write(&second, b"old-second").unwrap();
    let corrupt_payload = b"unique-corrupt-override-payload";
    write_override_test_archive(
        &archive_path,
        &[
            ("config/first.txt", b"new-first"),
            ("config/second.txt", corrupt_payload),
        ],
    );
    let mut bytes = std::fs::read(&archive_path).unwrap();
    let offset = bytes
        .windows(corrupt_payload.len())
        .position(|window| window == corrupt_payload)
        .unwrap();
    bytes[offset] ^= 0xff;
    std::fs::write(&archive_path, bytes).unwrap();

    let error = extract_modpack_overrides(&archive_path, &instance_path, None)
        .unwrap_err();

    assert!(
        error.to_string().contains("CRC")
            || error.to_string().contains("checksum")
    );
    assert_eq!(std::fs::read(&first).unwrap(), b"old-first");
    assert_eq!(std::fs::read(&second).unwrap(), b"old-second");
    assert!(!PathBuf::from(format!("{}.installing", first.display())).exists());
    assert!(
        !PathBuf::from(format!("{}.installing", second.display())).exists()
    );
}

#[test]
fn curseforge_overrides_accept_sizes_above_two_gib_up_to_shared_limit() {
    let root = tempfile::tempdir().unwrap();
    let archive_path = root.path().join("pack.zip");
    for sizes in [
        [3 * 1024 * 1024 * 1024, 0, 0],
        [u32::MAX - 1, u32::MAX - 1, 4],
    ] {
        write_override_size_test_archive(
            &archive_path,
            &[("a", sizes[0]), ("b", sizes[1]), ("c", sizes[2])],
        );
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(&archive_path).unwrap())
                .unwrap();
        let manifest = read_modpack_manifest(&mut archive).unwrap();
        let tasks = collect_modpack_override_tasks(
            &mut archive,
            &manifest,
            root.path(),
            None,
        )
        .unwrap();
        assert_eq!(tasks.len(), 3);
        assert_eq!(
            tasks.iter().map(|task| task.size).sum::<u64>(),
            sizes.into_iter().map(u64::from).sum::<u64>(),
        );
    }
}

#[test]
fn curseforge_overrides_reject_above_shared_limit_before_writing() {
    let root = tempfile::tempdir().unwrap();
    let archive_path = root.path().join("pack.zip");
    let instance_path = root.path().join("instance");
    write_override_size_test_archive(
        &archive_path,
        &[("a", u32::MAX - 1), ("b", u32::MAX - 1), ("c", 5)],
    );
    let error =
        materialize_modpack_overrides(&archive_path, &instance_path, None)
            .err()
            .unwrap()
            .to_string();
    assert!(error.contains("overrides exceed the extraction limit"));
    assert!(error.contains("8589934593 unpacked bytes"));
    assert!(error.contains("limit is 8589934592 bytes"));
    assert!(!instance_path.exists());
}

#[test]
fn curseforge_overrides_size_counts_only_final_duplicate_targets() {
    let root = tempfile::tempdir().unwrap();
    let archive_path = root.path().join("pack.zip");
    write_override_size_test_archive(
        &archive_path,
        &[
            ("config/a", u32::MAX - 1),
            ("config\\a", 1),
            ("config/b", u32::MAX - 1),
            ("config/c", 5),
        ],
    );
    let mut archive =
        zip::ZipArchive::new(std::fs::File::open(&archive_path).unwrap())
            .unwrap();
    let manifest = read_modpack_manifest(&mut archive).unwrap();
    let tasks = collect_modpack_override_tasks(
        &mut archive,
        &manifest,
        root.path(),
        None,
    )
    .unwrap();
    assert_eq!(tasks.len(), 3);
    let winner = tasks
        .iter()
        .find(|task| task.target == root.path().join("config/a"))
        .unwrap();
    assert_eq!(winner.index, 2);
    assert_eq!(winner.size, 1);
}

#[test]
fn curseforge_overrides_commit_successful_parallel_staging() {
    let root = tempfile::tempdir().unwrap();
    let archive_path = root.path().join("pack.zip");
    let instance_path = root.path().join("instance");
    let target = instance_path.join("config/example.txt");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(&target, b"old").unwrap();
    write_override_test_archive(
        &archive_path,
        &[("config/example.txt", b"replacement")],
    );

    let written =
        extract_modpack_overrides(&archive_path, &instance_path, None).unwrap();

    assert_eq!(written, 1);
    assert_eq!(std::fs::read(&target).unwrap(), b"replacement");
    assert!(
        !PathBuf::from(format!("{}.installing", target.display())).exists()
    );
    assert!(
        !PathBuf::from(format!("{}.installing.previous", target.display()))
            .exists()
    );
}

#[test]
fn curseforge_overrides_restore_after_later_stage_failure() {
    let root = tempfile::tempdir().unwrap();
    let archive_path = root.path().join("pack.zip");
    let instance_path = root.path().join("instance");
    let existing = instance_path.join("config/existing.txt");
    let created = instance_path.join("config/created.txt");
    std::fs::create_dir_all(existing.parent().unwrap()).unwrap();
    std::fs::write(&existing, b"old-existing").unwrap();
    write_override_test_archive(
        &archive_path,
        &[
            ("config/existing.txt", b"new-existing"),
            ("config/created.txt", b"new-created"),
        ],
    );

    let (written, replacements) =
        materialize_modpack_overrides(&archive_path, &instance_path, None)
            .unwrap();
    assert_eq!(written, 2);
    assert_eq!(std::fs::read(&existing).unwrap(), b"new-existing");
    assert_eq!(std::fs::read(&created).unwrap(), b"new-created");

    replacements.rollback().unwrap();

    assert_eq!(std::fs::read(&existing).unwrap(), b"old-existing");
    assert!(!created.exists());
    assert!(
        !PathBuf::from(format!("{}.installing.previous", existing.display()))
            .exists()
    );
}

#[test]
fn archive_paths_stay_inside_the_instance() {
    assert_eq!(
        safe_archive_relative_path("config/example.toml").unwrap(),
        "config/example.toml"
    );
    assert!(safe_archive_relative_path("../options.txt").is_err());
    assert!(safe_archive_relative_path("/options.txt").is_err());
}

#[test]
fn curseforge_override_content_joins_the_pack_group() {
    assert_eq!(
        curseforge_override_content("mods/cc-tweaked.jar").unwrap(),
        Some(CurseForgePackExpectedOverride {
            project_type: ProjectType::Mod,
            expected_relative_path: "mods/cc-tweaked.jar".to_string(),
        })
    );
    assert_eq!(
        curseforge_override_content("datapacks/sawmill.zip").unwrap(),
        Some(CurseForgePackExpectedOverride {
            project_type: ProjectType::DataPack,
            expected_relative_path: "datapacks/sawmill.zip".to_string(),
        })
    );
    assert_eq!(
        curseforge_override_content("config/example.toml").unwrap(),
        None
    );
}

#[test]
fn world_archives_must_use_a_zip_file_name() {
    assert!(validate_world_archive_name("world.zip").is_ok());
    assert!(validate_world_archive_name("WORLD.ZIP").is_ok());
    assert!(validate_world_archive_name("world.rar").is_err());
    assert!(validate_world_archive_name("../world.zip").is_err());
}
