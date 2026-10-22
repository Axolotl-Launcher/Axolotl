use content_management::resources::{ipc::*, *};
use serde_json::{Value, json};

fn project(source: SourceId) -> SourceProjectRef {
    SourceProjectRef {
        source,
        project_id: SourceProjectId::new("42").unwrap(),
    }
}

fn artifact(source: SourceId, file: &str) -> SourceArtifactRef {
    SourceArtifactRef {
        release: SourceReleaseRef {
            project: project(source),
            release_id: SourceReleaseId::new("7").unwrap(),
        },
        artifact_id: SourceArtifactId::new(file).unwrap(),
    }
}

fn resource_artifact() -> ResourceArtifact {
    ResourceArtifact {
        reference: artifact(SourceId::modrinth(), "primary-file"),
        file_name: Some("example.jar".into()),
        role: ArtifactRole::Primary,
        available: None,
        integrity: ArtifactIntegrity {
            size: Some(42),
            hashes: vec![ContentHash {
                algorithm: HashAlgorithm::Sha1,
                value: "f".repeat(40),
            }],
        },
        acquisition: ArtifactAcquisition::PublicDownload {
            urls: vec!["https://example.com/example.jar".into()],
        },
        dependencies: vec![],
    }
}

#[test]
fn source_release_dependencies_and_command_arguments_match_camel_case_ipc() {
    let dependency = DependencyTarget::SourceRelease {
        source: SourceId::modrinth(),
        release_id: SourceReleaseId::new("release1").unwrap(),
    };
    let encoded = serde_json::to_value(&dependency).unwrap();
    assert_eq!(encoded["releaseId"], "release1");
    assert!(encoded.get("release_id").is_none());
    assert_eq!(
        serde_json::from_value::<DependencyTarget>(encoded).unwrap(),
        dependency
    );
    for name in [
        "resource_get",
        "resource_get_description",
        "resource_get_release",
    ] {
        let reference = if name == "resource_get_release" {
            json!({"project":{"source":"modrinth","projectId":"project1"},"releaseId":"release1"})
        } else {
            json!({"kind":"source","project":{"source":"modrinth","projectId":"project1"}})
        };
        let mut value = json!({"command":name,"args":{"reference":reference,"source":"modrinth", "requestId":"request1"}});
        assert!(
            serde_json::from_value::<ResourceCommand>(value.clone()).is_ok()
        );
        value["args"].as_object_mut().unwrap().remove("requestId");
        assert!(serde_json::from_value::<ResourceCommand>(value).is_err());
    }
    let cancel: ResourceCommand = serde_json::from_value(json!({"command":"resource_export_cancel","args":{"requestIdOrJobId":"request1"}})).unwrap();
    assert!(
        matches!(cancel, ResourceCommand::ResourceExportCancel { request_id_or_job_id } if request_id_or_job_id == "request1")
    );
}

#[test]
fn numeric_release_versions_compare_across_component_counts() {
    fn release(version: &str, published: &str) -> ResourceRelease {
        ResourceRelease {
            reference: SourceReleaseRef {
                project: project(SourceId::modrinth()),
                release_id: SourceReleaseId::new(version).unwrap(),
            },
            version: version.into(),
            channel: ReleaseChannel::Release,
            published_at: Some(published.parse().unwrap()),
            compatibility: ReleaseCompatibility {
                game_versions: vec![],
                loaders: vec![],
            },
            artifacts: vec![],
            equivalent_releases: vec![],
        }
    }
    let current = release("2.0", "2020-01-01T00:00:00Z");
    let older = release("1.9.9", "2025-01-01T00:00:00Z");
    assert!(release_is_known_lower(&older, &current));
    assert!(!release_is_newer(&older, &current));
    let next = release("2.0.1", "2019-01-01T00:00:00Z");
    assert!(release_is_newer(&next, &current));
    let (chosen, _) = select_automatic_release(
        vec![older, current, next],
        &SourceId::modrinth(),
    )
    .unwrap();
    assert_eq!(chosen.version, "2.0.1");
}

fn installed(id: &str, scope: InstallScope, path: &str) -> InstalledResource {
    InstalledResource {
        entry_id: InstalledEntryId::new(id).unwrap(),
        instance_id: "instance".into(),
        resource_id: Some(ResourceId::new("resource").unwrap()),
        artifact: Some(artifact(SourceId::curseforge(), "current-file")),
        known_artifacts: vec![artifact(
            SourceId::modrinth(),
            "equivalent-file",
        )],
        integrity: resource_artifact().integrity,
        relative_path: path.into(),
        original_file_name: Some("original.jar".into()),
        resource_type: ResourceType::DataPack,
        scope,
        enabled: true,
        missing: false,
        ownership: ResourceOwnership::UserAdded,
    }
}

#[test]
fn source_project_release_and_file_identity_are_distinct() {
    assert_ne!(
        project(SourceId::modrinth()),
        project(SourceId::curseforge())
    );
    let primary = artifact(SourceId::modrinth(), "primary");
    let attachment = artifact(SourceId::modrinth(), "attachment");
    assert_eq!(primary.release, attachment.release);
    assert_ne!(primary, attachment);
    let third_party = SourceId::new("third_party").unwrap();
    let encoded = serde_json::to_value(project(third_party.clone())).unwrap();
    assert_eq!(encoded["source"], "third_party");
    assert_eq!(encoded["projectId"], "42");
    assert_eq!(
        serde_json::from_value::<SourceProjectRef>(encoded)
            .unwrap()
            .source,
        third_party
    );
}

#[test]
fn invalid_identifiers_are_rejected_at_the_wire_boundary() {
    for invalid in ["", " ", " 42", "42 ", "42\n", "x\0y"] {
        assert!(SourceProjectId::new(invalid).is_err());
        assert!(
            serde_json::from_value::<SourceProjectId>(json!(invalid)).is_err()
        );
    }
    assert!(serde_json::from_value::<SourceProjectId>(json!(42)).is_err());
}

#[test]
fn search_candidates_round_trip_without_claiming_stable_identity() {
    let group = ResourceSearchEntry {
        query_entry_id: "query-1/group-1".into(),
        resource_id: None,
        default_project: project(SourceId::modrinth()),
        candidates: vec![
            project(SourceId::modrinth()),
            project(SourceId::curseforge()),
        ],
        status: IdentityStatus::Unconfirmed,
        basis: vec![CandidateBasis::ExactTitle],
    };
    let encoded = serde_json::to_value(&group).unwrap();
    assert_eq!(encoded["status"], "unconfirmed");
    assert!(encoded["resourceId"].is_null());
    assert_eq!(
        serde_json::from_value::<ResourceSearchEntry>(encoded.clone()).unwrap(),
        group
    );
    assert!(serde_json::from_value::<ResourceRef>(encoded).is_err());
}

#[test]
fn duplicate_entries_and_world_scopes_survive_a_frozen_plan() {
    let scope = |world: &str| InstallScope::World {
        relative_path: format!("saves/{world}"),
    };
    let first =
        installed("entry-1", scope("one"), "saves/one/datapacks/custom.zip");
    let second =
        installed("entry-2", scope("two"), "saves/two/datapacks/custom.zip");
    assert_eq!(first.resource_id, second.resource_id);
    assert_eq!(first.integrity, second.integrity);
    assert_ne!(first.entry_id, second.entry_id);
    assert_ne!(first.scope, second.scope);
    let node_id = SelectionId::new("root").unwrap();
    let actions = [first.clone(), second.clone()]
        .into_iter()
        .map(|current| ResourceEntryAction {
            entry_id: current.entry_id.clone(),
            node_id: node_id.clone(),
            kind: ResourceActionKind::Replace,
            target_relative_path: current.relative_path.clone(),
            current: Some(current),
            target: resource_artifact(),
        })
        .collect();
    let plan = ResourceOperationPlan {
        schema_version: ResourceContractVersion,
        id: PlanId::new("plan").unwrap(),
        revision: 3,
        instance_id: "instance".into(),
        instance_revision: 17,
        intent: ResourceOperationIntent::Update,
        selections: vec![],
        active_roots: vec![node_id.clone()],
        nodes: vec![],
        edges: vec![],
        actions,
        issues: vec![],
        exceptions: vec![CompatibilityException {
            selection_id: node_id,
            release: resource_artifact().reference.release,
            dimensions: vec![CompatibilityDimension::Loader],
        }],
        selection_reasons: vec![],
    };
    let mut encoded = serde_json::to_value(&plan).unwrap();
    let restored: ResourceOperationPlan =
        serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(restored, plan);
    assert_eq!(restored.actions.len(), 2);
    assert_eq!(restored.actions[0].current.as_ref().unwrap(), &first);
    assert_eq!(restored.actions[1].current.as_ref().unwrap(), &second);
    assert_eq!(
        restored.actions[0].target.reference.release.project.source,
        SourceId::modrinth()
    );
    assert_eq!(
        restored.actions[0]
            .current
            .as_ref()
            .unwrap()
            .artifact
            .as_ref()
            .unwrap()
            .release
            .project
            .source,
        SourceId::curseforge()
    );
    encoded["schemaVersion"] = json!(99);
    assert!(serde_json::from_value::<ResourceOperationPlan>(encoded).is_err());
}

#[test]
fn active_dependency_edges_preserve_every_parent_and_optional_decisions() {
    let child = SelectionId::new("shared").unwrap();
    let edge = |parent: &str, kind| ResourceDependencyEdge {
        release_selection: None,
        parent: SelectionId::new(parent).unwrap(),
        child: child.clone(),
        declared_by: artifact(SourceId::modrinth(), parent),
        kind,
        version_constraint: Some(">=1.0".into()),
    };
    let edges = vec![
        edge("root-a", ResourceDependencyKind::Required),
        edge("root-b", ResourceDependencyKind::Optional),
    ];
    let restored: Vec<ResourceDependencyEdge> =
        serde_json::from_value(serde_json::to_value(&edges).unwrap()).unwrap();
    assert_eq!(restored, edges);
    assert_ne!(restored[0].parent, restored[1].parent);
    let decision = ResourcePlanDecision::AddOptional { node_id: child };
    assert_eq!(
        serde_json::to_value(decision).unwrap(),
        json!({"kind": "addOptional", "nodeId": "shared"})
    );
    let required = ResourcePlanDecision::AddRequired {
        node_id: SelectionId::new("required").unwrap(),
    };
    let encoded = serde_json::to_value(&required).unwrap();
    assert_eq!(
        encoded,
        json!({"kind": "addRequired", "nodeId": "required"})
    );
    assert_eq!(
        serde_json::from_value::<ResourcePlanDecision>(encoded).unwrap(),
        required
    );
    for (decision, kind) in [
        (
            ResourcePlanDecision::IgnoreDependencies,
            "ignoreDependencies",
        ),
        (
            ResourcePlanDecision::SkipUnwritableFiles,
            "skipUnwritableFiles",
        ),
    ] {
        let encoded = serde_json::to_value(&decision).unwrap();
        assert_eq!(encoded, json!({"kind": kind}));
        assert_eq!(
            serde_json::from_value::<ResourcePlanDecision>(encoded).unwrap(),
            decision
        );
    }
}

#[test]
fn ipc_arguments_are_camel_case_and_revisions_are_explicit() {
    let command = ResourceCommand::ResourceRevisePlan {
        plan_id: PlanId::new("plan").unwrap(),
        revision: 7,
        decisions: vec![],
        request_id: RequestId::new("request").unwrap(),
    };
    let encoded = serde_json::to_value(&command).unwrap();
    assert_eq!(
        encoded,
        json!({"command": "resource_revise_plan", "args": {"planId": "plan", "revision": 7, "decisions": [], "requestId": "request"}})
    );
    assert_eq!(
        serde_json::from_value::<ResourceCommand>(encoded.clone()).unwrap(),
        command
    );
    let mut without_revision = encoded;
    without_revision["args"]
        .as_object_mut()
        .unwrap()
        .remove("revision");
    assert!(
        serde_json::from_value::<ResourceCommand>(without_revision).is_err()
    );
    assert!(serde_json::from_value::<ResourceCommand>(json!({"command": "resource_queue_plan", "args": {"plan_id": "plan", "revision": 7}})).is_err());
}

#[test]
fn export_no_match_failure_and_embedding_decision_are_distinct() {
    let missing = ExportSourceResolution::NoMatch {
        source: SourceId::modrinth(),
    };
    let failed = ExportSourceResolution::Failed {
        source: SourceId::curseforge(),
        code: ResourceIssueCode::SourceUnavailable,
        message: "offline".into(),
    };
    assert_ne!(
        serde_json::to_value(missing).unwrap()["kind"],
        serde_json::to_value(failed).unwrap()["kind"]
    );
    let command = ResourceCommand::ResourceExportPlan {
        session_id: SessionId::new("session").unwrap(),
        selection_revision: 8,
        format: ResourceExportFormat::Modrinth,
        package_info: ExportPackageInfo {
            name: "包".into(),
            version: "1.0.0".into(),
            summary: "".into(),
            author: None,
        },
        embedded_failure_choices: vec![ExportEmbeddedFailureChoice {
            snapshot: ExportFileSnapshot {
                relative_path: "mods/中文.jar.disabled".into(),
                integrity: resource_artifact().integrity,
                revision: 2,
                identifiable: true,
                modified_at: None,
                category_id: "mods".into(),
                category_selected: true,
            },
            source: SourceId::curseforge(),
        }],
    };
    let restored: ResourceCommand =
        serde_json::from_value(serde_json::to_value(&command).unwrap())
            .unwrap();
    assert_eq!(restored, command);
}

#[test]
fn unknown_compatibility_and_environment_are_preserved() {
    assert_eq!(
        serde_json::to_value(CompatibilityState::Unknown).unwrap(),
        json!("unknown")
    );
    let environment = ArtifactEnvironment {
        client: ArtifactEnvironmentRequirement::Unknown,
        server: ArtifactEnvironmentRequirement::Unsupported,
    };
    assert_eq!(
        serde_json::to_value(environment).unwrap(),
        json!({"client": "unknown", "server": "unsupported"})
    );
    let page: ResourcePage<Value> = ResourcePage {
        request_id: RequestId::new("request").unwrap(),
        entries: vec![],
        cursor: Some("opaque-cursor".into()),
        has_more: true,
        total: ResourceTotal {
            value: None,
            precision: TotalPrecision::Unknown,
        },
        source_failures: vec![ResourceSourceFailure {
            source: SourceId::curseforge(),
            code: ResourceIssueCode::SourceUnavailable,
            message: "offline".into(),
            retryable: true,
        }],
        exhausted_sources: vec![],
    };
    assert!(serde_json::to_value(page).unwrap()["total"]["value"].is_null());
}

#[test]
fn history_is_self_contained_and_keeps_the_original_source_and_path() {
    let previous = installed(
        "entry",
        InstallScope::Instance,
        "mods/中文-old.jar.disabled",
    );
    let mut target = previous.clone();
    target.artifact = Some(resource_artifact().reference);
    target.relative_path = "mods/中文-new.jar.disabled".into();
    let dependency = ResourceDependencyNode {
        resource_type: None,
        root_selection_ids: vec![],
        declared_compatibility: None,
        id: SelectionId::new("dependency").unwrap(),
        resource: ResourceRef::Source {
            project: project(SourceId::curseforge()),
        },
        scope: InstallScope::Instance,
        state: DependencyNodeState::Satisfied,
        artifact: Some(resource_artifact()),
        installed_entries: vec![
            InstalledEntryId::new("dependency-entry").unwrap(),
        ],
        compatibility: CompatibilityAssessment {
            game_version: CompatibilityState::Compatible,
            loader: CompatibilityState::Compatible,
            reasons: vec![],
        },
    };
    let edge = ResourceDependencyEdge {
        release_selection: None,
        parent: SelectionId::new("root").unwrap(),
        child: dependency.id.clone(),
        declared_by: previous.artifact.clone().unwrap(),
        kind: ResourceDependencyKind::Required,
        version_constraint: Some(">=1.0".into()),
    };
    let mut historical_node = serde_json::to_value(&dependency).unwrap();
    historical_node
        .as_object_mut()
        .unwrap()
        .remove("rootSelectionIds");
    historical_node
        .as_object_mut()
        .unwrap()
        .remove("resourceType");
    let decoded: ResourceDependencyNode =
        serde_json::from_value(historical_node).unwrap();
    assert!(decoded.root_selection_ids.is_empty());
    assert!(decoded.resource_type.is_none());
    let mut historical_edge = serde_json::to_value(&edge).unwrap();
    historical_edge
        .as_object_mut()
        .unwrap()
        .remove("releaseSelection");
    let decoded: ResourceDependencyEdge =
        serde_json::from_value(historical_edge).unwrap();
    assert!(decoded.release_selection.is_none());
    let mut root = dependency.clone();
    root.id = edge.parent.clone();
    root.resource = ResourceRef::Resource {
        id: previous.resource_id.clone().unwrap(),
    };
    root.installed_entries = vec![previous.entry_id.clone()];
    root.artifact.as_mut().unwrap().reference =
        previous.artifact.clone().unwrap();
    let history = ResourceHistoryEntry {
        id: HistoryId::new("history").unwrap(),
        entry_id: previous.entry_id.clone(),
        instance_id: "instance".into(),
        recorded_at: "2026-10-11T00:00:00Z".parse().unwrap(),
        previous: previous.clone(),
        target,
        previous_nodes: vec![root, dependency],
        target_nodes: vec![],
        previous_dependencies: vec![edge],
        target_dependencies: vec![],
        backup_relative_path: Some("mods/中文-old.jar.disabled.old".into()),
        backup_integrity: Some(previous.integrity.clone()),
        backup_available: true,
        backup_retired: false,
    };
    let restored: ResourceHistoryEntry =
        serde_json::from_value(serde_json::to_value(&history).unwrap())
            .unwrap();
    assert_eq!(restored, history);
    let mut legacy = serde_json::to_value(&history).unwrap();
    legacy.as_object_mut().unwrap().remove("backupRetired");
    assert_eq!(
        serde_json::from_value::<ResourceHistoryEntry>(legacy).unwrap(),
        history
    );
    assert_eq!(restored.previous, previous);
    assert_eq!(restored.previous.entry_id, restored.target.entry_id);
    assert_eq!(
        restored.previous_dependencies[0].child,
        restored.previous_nodes[1].id
    );
    assert_eq!(
        restored.previous_nodes[1].installed_entries[0].as_str(),
        "dependency-entry"
    );
}
