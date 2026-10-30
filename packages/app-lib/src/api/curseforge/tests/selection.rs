//! CurseForge selection tests.

use super::fixtures::stage8_curseforge_file;
use crate::api::curseforge::archive::{
    loader_family, loader_type, looks_like_game_version, modpack_target,
};
use crate::api::curseforge::dto::{
    CurseForgeManifestLoader, CurseForgeManifestMinecraft,
    CurseForgeModpackManifest, CurseForgeModpackTarget,
    CurseForgeSortableGameVersion,
};
use crate::api::curseforge::install::{
    dependency_policy, should_resolve_dependency,
};
use crate::api::curseforge::mapping::{
    project_type_for_class, recognized_project_type,
};
use crate::api::curseforge::selection::{
    dependency_target_game_version, file_matches_dependency_target,
    select_best_dependency_file,
};
use crate::state::instances::DependencyRelation;
use crate::state::{ModLoader, ProjectType};
use content_management::DependencyPolicy;

#[test]
fn content_updates_only_resolve_required_dependencies() {
    use crate::state::instances::ManualDownloadOperationKind;

    let operation = ManualDownloadOperationKind::ContentUpdate;
    assert!(should_resolve_dependency(
        DependencyRelation::Required,
        operation
    ));
    for relation in [
        DependencyRelation::Optional,
        DependencyRelation::Include,
        DependencyRelation::Embedded,
        DependencyRelation::Tool,
        DependencyRelation::Incompatible,
    ] {
        assert!(!should_resolve_dependency(relation, operation));
    }
    assert_eq!(dependency_policy(operation), DependencyPolicy::RequiredOnly);
}

#[test]
fn content_installs_still_resolve_optional_dependencies_for_selection() {
    use crate::state::instances::ManualDownloadOperationKind;

    for operation in [
        ManualDownloadOperationKind::ContentInstall,
        ManualDownloadOperationKind::PackInstall,
        ManualDownloadOperationKind::PackUpdate,
    ] {
        for relation in [
            DependencyRelation::Required,
            DependencyRelation::Optional,
            DependencyRelation::Include,
        ] {
            assert!(should_resolve_dependency(relation, operation));
        }
        assert_eq!(
            dependency_policy(operation),
            DependencyPolicy::IncludeOptional
        );
    }
}

#[test]
fn dependency_fallback_requires_the_target_game_and_loader() {
    let mut target_file =
        stage8_curseforge_file(1, 101, "target.jar", 0, Vec::new(), 0);
    target_file.game_versions =
        vec!["1.21.1".to_string(), "NeoForge".to_string()];

    let mut newer_game_file =
        stage8_curseforge_file(1, 102, "newer-game.jar", 0, Vec::new(), 0);
    newer_game_file.game_versions =
        vec!["26.1.2".to_string(), "NeoForge".to_string()];

    let mut wrong_loader_file =
        stage8_curseforge_file(1, 103, "wrong-loader.jar", 0, Vec::new(), 0);
    wrong_loader_file.game_versions =
        vec!["1.21.1".to_string(), "Fabric".to_string()];

    assert!(file_matches_dependency_target(
        &target_file,
        Some("1.21.1"),
        Some(6),
    ));
    assert!(!file_matches_dependency_target(
        &newer_game_file,
        Some("1.21.1"),
        Some(6),
    ));
    assert!(!file_matches_dependency_target(
        &wrong_loader_file,
        Some("1.21.1"),
        Some(6),
    ));
}

#[test]
fn dependency_fallback_rejects_conflicting_game_version_metadata() {
    let mut conflicting_file =
        stage8_curseforge_file(1, 101, "conflicting.jar", 0, Vec::new(), 0);
    conflicting_file.game_versions =
        vec!["1.21.1".to_string(), "NeoForge".to_string()];
    conflicting_file.sortable_game_versions =
        vec![CurseForgeSortableGameVersion {
            game_version_name: "26.1.2".to_string(),
            game_version_padded: None,
            game_version: Some("26.1.2".to_string()),
            game_version_release_date: None,
            game_version_type_id: None,
        }];

    assert!(!file_matches_dependency_target(
        &conflicting_file,
        Some("1.21.1"),
        Some(6),
    ));
}

#[test]
fn dependency_selection_requires_a_minecraft_target() {
    assert_eq!(dependency_target_game_version(None), None);
    assert_eq!(dependency_target_game_version(Some("")), None);
    assert_eq!(dependency_target_game_version(Some("  ")), None);
    assert_eq!(
        dependency_target_game_version(Some("1.21.1")),
        Some("1.21.1"),
    );
}

#[test]
fn dependency_selection_prefers_a_stable_target_match_over_newer_beta() {
    let mut stable =
        stage8_curseforge_file(1, 101, "stable.jar", 0, Vec::new(), 0);
    stable.game_versions = vec!["1.21.1".to_string(), "NeoForge".to_string()];
    stable.release_type = 1;
    stable.file_date = "2026-08-01T00:00:00Z".to_string();

    let mut newer_beta =
        stage8_curseforge_file(1, 102, "newer-beta.jar", 0, Vec::new(), 0);
    newer_beta.game_versions =
        vec!["1.21.1".to_string(), "NeoForge".to_string()];
    newer_beta.release_type = 2;
    newer_beta.file_date = "2026-08-02T00:00:00Z".to_string();

    let selected = select_best_dependency_file(
        vec![newer_beta, stable],
        Some("1.21.1"),
        Some(6),
    )
    .unwrap();

    assert_eq!(selected.id, 101);
}

#[test]
fn waystones_fixture_selects_neoforge_balm_for_minecraft_1_21_1() {
    let mut balm_1_21_1 = stage8_curseforge_file(
        531_761,
        21_065,
        "balm-neoforge-1.21.1-21.0.65.jar",
        0,
        Vec::new(),
        0,
    );
    balm_1_21_1.game_versions =
        vec!["1.21.1".to_string(), "NeoForge".to_string()];
    balm_1_21_1.release_type = 1;

    let mut balm_26_1_2 = stage8_curseforge_file(
        531_761,
        26_102,
        "balm-neoforge-26.1.2-26.1.2.jar",
        0,
        Vec::new(),
        0,
    );
    balm_26_1_2.game_versions =
        vec!["26.1.2".to_string(), "NeoForge".to_string()];
    balm_26_1_2.release_type = 1;

    let selected = select_best_dependency_file(
        vec![balm_26_1_2, balm_1_21_1],
        Some("1.21.1"),
        Some(6),
    )
    .unwrap();
    assert_eq!(selected.file_name, "balm-neoforge-1.21.1-21.0.65.jar");
}

#[test]
fn project_types_are_provider_qualified() {
    assert_eq!(project_type_for_class(Some(6)), "mod");
    assert_eq!(project_type_for_class(Some(4471)), "modpack");
    assert_eq!(project_type_for_class(Some(6552)), "shader");
    assert_eq!(project_type_for_class(Some(6945)), "datapack");
}

#[test]
fn curseforge_loader_ids_map_to_instance_loaders() {
    assert_eq!(loader_family("forge-47.4.0"), "forge");
    assert_eq!(loader_family("fabric-0.16.10"), "fabric");
    assert_eq!(loader_type("cleanroom"), Some(1));
    assert_eq!(loader_type("Fabric"), Some(4));
    assert_eq!(loader_type("neoforge"), Some(6));
}

#[test]
fn curseforge_game_versions_ignore_loader_labels() {
    assert!(looks_like_game_version("1.21.8"));
    assert!(!looks_like_game_version("Fabric"));
    assert!(!looks_like_game_version("forge"));
}

#[test]
fn modpack_target_uses_primary_forge_loader_and_version() {
    let manifest = CurseForgeModpackManifest {
        minecraft: CurseForgeManifestMinecraft {
            version: "1.12.2".to_string(),
            mod_loaders: vec![CurseForgeManifestLoader {
                id: "forge-14.23.5.2860".to_string(),
                primary: true,
            }],
        },
        files: Vec::new(),
        overrides: "overrides".to_string(),
        name: None,
        version: None,
    };

    assert_eq!(
        modpack_target(&manifest).unwrap(),
        CurseForgeModpackTarget {
            game_version: "1.12.2".to_string(),
            loader: ModLoader::Forge,
            loader_version: Some("14.23.5.2860".to_string()),
        }
    );
}

#[test]
fn modpack_target_recognizes_pcl_cleanroom_version_convention() {
    let manifest = CurseForgeModpackManifest {
        minecraft: CurseForgeManifestMinecraft {
            version: "1.12.2".to_string(),
            mod_loaders: vec![CurseForgeManifestLoader {
                id: "forge-0.3.0-alpha".to_string(),
                primary: true,
            }],
        },
        files: Vec::new(),
        overrides: "overrides".to_string(),
        name: None,
        version: None,
    };

    assert_eq!(
        modpack_target(&manifest).unwrap(),
        CurseForgeModpackTarget {
            game_version: "1.12.2".to_string(),
            loader: ModLoader::Cleanroom,
            loader_version: Some("0.3.0-alpha".to_string()),
        }
    );
}

#[test]
fn modpack_target_without_loader_is_vanilla() {
    let manifest = CurseForgeModpackManifest {
        minecraft: CurseForgeManifestMinecraft {
            version: "1.20.1".to_string(),
            mod_loaders: Vec::new(),
        },
        files: Vec::new(),
        overrides: "overrides".to_string(),
        name: None,
        version: None,
    };

    assert_eq!(
        modpack_target(&manifest).unwrap(),
        CurseForgeModpackTarget {
            game_version: "1.20.1".to_string(),
            loader: ModLoader::Vanilla,
            loader_version: None,
        }
    );
}

#[test]
fn recognized_project_types_map_curseforge_classes() {
    assert_eq!(recognized_project_type(Some(6)), Some(ProjectType::Mod));
    assert_eq!(
        recognized_project_type(Some(12)),
        Some(ProjectType::ResourcePack),
    );
    assert_eq!(
        recognized_project_type(Some(6552)),
        Some(ProjectType::ShaderPack),
    );
    assert_eq!(
        recognized_project_type(Some(6945)),
        Some(ProjectType::DataPack),
    );
    assert_eq!(
        recognized_project_type(Some(17)),
        Some(ProjectType::WorldSave),
    );
    assert_eq!(recognized_project_type(Some(4471)), None);
    assert_eq!(recognized_project_type(None), None);
}

#[test]
fn quilted_fabric_api_dependency_uses_the_resolved_project() {
    use crate::api::curseforge::constants::{
        FABRIC_API_CURSEFORGE_PROJECT_ID,
        QUILTED_FABRIC_API_CURSEFORGE_PROJECT_ID,
    };
    use crate::api::curseforge::dto::{
        CurseForgeFileDependency, CurseForgeInstallRequest,
    };
    use crate::api::curseforge::install::{
        DependencyAction, dependency_action,
    };
    use crate::state::instances::{
        ContentOwnershipKind, ManualDownloadOperationKind,
    };

    let request = CurseForgeInstallRequest {
        instance_id: String::new(),
        project_id: 1,
        file_id: 2,
        project_type: "mod".to_string(),
        ownership_kind: ContentOwnershipKind::UserAdded,
        manual_operation_kind: ManualDownloadOperationKind::ContentInstall,
        game_version: Some("1.21.1".to_string()),
        mod_loader_type: Some(5),
        world_name: None,
        install_dependencies: true,
        excluded_dependency_project_ids: vec![
            QUILTED_FABRIC_API_CURSEFORGE_PROJECT_ID,
        ],
        force_dependency_project_ids: Vec::new(),
        dependency_plan_id: None,
        defer_persistence: false,
        verification_tx: None,
        pre_resolved_relative_path: None,
        expected_file_name: None,
    };
    let dependency = CurseForgeFileDependency::new(
        FABRIC_API_CURSEFORGE_PROJECT_ID,
        DependencyRelation::Required,
    );

    match dependency_action(&dependency, &request, &[]) {
        DependencyAction::Skip {
            reason,
            target_project_id,
        } => {
            assert_eq!(reason, "excluded_by_user");
            assert_eq!(
                target_project_id, QUILTED_FABRIC_API_CURSEFORGE_PROJECT_ID,
                "the skip must name the project the exclusion was keyed by"
            );
        }
        _ => panic!("an excluded dependency must be skipped"),
    }
}
#[test]
fn quilted_fabric_api_dependency_records_are_reported_by_the_shared_step() {
    use crate::api::curseforge::constants::{
        FABRIC_API_CURSEFORGE_PROJECT_ID,
        QUILTED_FABRIC_API_CURSEFORGE_PROJECT_ID,
    };
    use crate::api::curseforge::dto::{
        CurseForgeFileDependency, CurseForgeInstallRequest,
    };
    use crate::api::curseforge::install::{
        DependencyStep, dependency_action, dependency_step,
    };
    use crate::state::instances::{
        ContentOwnershipKind, ManualDownloadOperationKind,
    };

    let request = |excluded: Vec<u32>| CurseForgeInstallRequest {
        instance_id: String::new(),
        project_id: 1,
        file_id: 2,
        project_type: "mod".to_string(),
        ownership_kind: ContentOwnershipKind::UserAdded,
        manual_operation_kind: ManualDownloadOperationKind::ContentInstall,
        game_version: Some("1.21.1".to_string()),
        mod_loader_type: Some(5),
        world_name: None,
        install_dependencies: true,
        excluded_dependency_project_ids: excluded,
        force_dependency_project_ids: Vec::new(),
        dependency_plan_id: None,
        defer_persistence: false,
        verification_tx: None,
        pre_resolved_relative_path: None,
        expected_file_name: None,
    };
    let required = CurseForgeFileDependency::new(
        FABRIC_API_CURSEFORGE_PROJECT_ID,
        DependencyRelation::Required,
    );
    let incompatible = CurseForgeFileDependency::new(
        FABRIC_API_CURSEFORGE_PROJECT_ID,
        DependencyRelation::Incompatible,
    );

    let excluded = request(vec![QUILTED_FABRIC_API_CURSEFORGE_PROJECT_ID]);
    match dependency_step(
        &required,
        dependency_action(&required, &excluded, &[]),
    ) {
        DependencyStep::Skipped(skipped) => {
            assert_eq!(skipped.reason, "excluded_by_user");
            assert_eq!(
                skipped.project_id, QUILTED_FABRIC_API_CURSEFORGE_PROJECT_ID,
                "the reported skip must name the project the exclusion was keyed by"
            );
        }
        _ => panic!("an excluded dependency must be skipped"),
    }

    let kept = request(Vec::new());
    match dependency_step(&required, dependency_action(&required, &kept, &[])) {
        DependencyStep::Resolve { target_project_id } => {
            assert_eq!(
                target_project_id,
                QUILTED_FABRIC_API_CURSEFORGE_PROJECT_ID
            );
        }
        _ => panic!("a required dependency must be resolved"),
    }

    match dependency_step(
        &incompatible,
        dependency_action(&incompatible, &kept, &[]),
    ) {
        DependencyStep::Incompatible(project_id) => {
            assert_eq!(
                project_id, FABRIC_API_CURSEFORGE_PROJECT_ID,
                "incompatible records name the declared project"
            );
        }
        _ => panic!("an incompatible dependency must be recorded"),
    }
}
