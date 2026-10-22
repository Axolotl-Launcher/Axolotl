#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use content_management::{
        ContentMetadataProvider, ContentType, Dependency, DependencyPolicy,
        DependencyType, Error, ResolutionPreferences, ResolveContentRequest,
        SkippedReason, Version, resolve_content, resolve_content_with_policy,
    };

    const QUILT_FABRIC_API_EXCEPTION_PROJECT_ID: &str = "P7dR8mSH";

    #[derive(Clone, Default)]
    struct MemoryProvider {
        versions: HashMap<String, Version>,
        project_versions: HashMap<String, Vec<String>>,
        requests: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl ContentMetadataProvider for MemoryProvider {
        async fn get_version(
            &mut self,
            version_id: &str,
        ) -> Result<Option<Version>, Error> {
            self.requests.lock().unwrap().push(version_id.to_string());
            Ok(self.versions.get(version_id).cloned())
        }

        async fn get_project_versions(
            &mut self,
            project_id: &str,
        ) -> Result<Vec<Version>, Error> {
            self.requests.lock().unwrap().push(project_id.to_string());
            Ok(self
                .project_versions
                .get(project_id)
                .into_iter()
                .flatten()
                .filter_map(|id| self.versions.get(id))
                .cloned()
                .collect())
        }
    }

    impl MemoryProvider {
        fn with_versions(mut self, versions: Vec<Version>) -> Self {
            for version in versions {
                self.project_versions
                    .entry(version.project_id.clone())
                    .or_default()
                    .push(version.id.clone());
                self.versions.insert(version.id.clone(), version);
            }

            self
        }

        fn with_project_versions(
            mut self,
            project_id: &str,
            version_ids: &[&str],
        ) -> Self {
            self.project_versions.insert(
                project_id.to_string(),
                version_ids.iter().map(|id| id.to_string()).collect(),
            );
            self
        }
    }

    fn version(
        id: &str,
        project_id: &str,
        date: &str,
        game_versions: &[&str],
        loaders: &[&str],
        dependencies: Vec<Dependency>,
    ) -> Version {
        version_with_channel(
            id,
            project_id,
            date,
            "release",
            game_versions,
            loaders,
            dependencies,
        )
    }

    fn version_with_channel(
        id: &str,
        project_id: &str,
        date: &str,
        version_type: &str,
        game_versions: &[&str],
        loaders: &[&str],
        dependencies: Vec<Dependency>,
    ) -> Version {
        Version {
            id: id.to_string(),
            project_id: project_id.to_string(),
            date_published: DateTime::parse_from_rfc3339(date)
                .unwrap()
                .with_timezone(&Utc),
            version_type: version_type.to_string(),
            dependencies,
            game_versions: game_versions
                .iter()
                .map(|v| v.to_string())
                .collect(),
            loaders: loaders.iter().map(|v| v.to_string()).collect(),
            version_number: None,
            filename: None,
            sha1: None,
        }
    }

    fn dependency(
        project_id: Option<&str>,
        version_id: Option<&str>,
        dependency_type: DependencyType,
    ) -> Dependency {
        Dependency {
            version_id: version_id.map(str::to_string),
            project_id: project_id.map(str::to_string),
            file_name: None,
            dependency_type,
        }
    }

    fn required_project_dependency(project_id: &str) -> Dependency {
        dependency(Some(project_id), None, DependencyType::Required)
    }

    fn required_version_dependency(version_id: &str) -> Dependency {
        dependency(None, Some(version_id), DependencyType::Required)
    }

    fn request(project_id: &str) -> ResolveContentRequest {
        ResolveContentRequest {
            project_id: project_id.to_string(),
            version_id: None,
            content_type: ContentType::Mod,
            selected: ResolutionPreferences::default(),
            target: ResolutionPreferences {
                game_versions: vec!["1.20.1".to_string()],
                loaders: vec!["fabric".to_string()],
            },
            existing_project_ids: Vec::new(),
            excluded_project_ids: Vec::new(),
            force_project_ids: Vec::new(),
        }
    }

    fn mixed_dependency_provider() -> MemoryProvider {
        let make_version = |id, dependencies| {
            version(
                id,
                id,
                "2026-10-01T00:00:00Z",
                &["1.20.1"],
                &["fabric"],
                dependencies,
            )
        };
        MemoryProvider::default().with_versions(vec![
            make_version(
                "root",
                vec![
                    required_project_dependency("required"),
                    dependency(
                        Some("optional"),
                        None,
                        DependencyType::Optional,
                    ),
                    dependency(Some("shared"), None, DependencyType::Optional),
                ],
            ),
            make_version(
                "required",
                vec![
                    required_version_dependency("required-leaf"),
                    dependency(
                        None,
                        Some("optional-leaf"),
                        DependencyType::Optional,
                    ),
                ],
            ),
            make_version(
                "required-leaf",
                vec![required_project_dependency("shared")],
            ),
            make_version(
                "optional",
                vec![
                    required_project_dependency("optional-child"),
                    required_project_dependency("shared"),
                ],
            ),
            make_version("optional-child", vec![]),
            make_version("optional-leaf", vec![]),
            make_version("shared", vec![]),
        ])
    }

    #[tokio::test]
    async fn updates_skip_optional_branches_before_fetching_metadata() {
        let provider = mixed_dependency_provider();
        let requests = provider.requests.clone();
        let plan = resolve_content_with_policy(
            provider,
            request("root"),
            DependencyPolicy::RequiredOnly,
        )
        .await
        .unwrap();

        let projects = plan
            .dependencies
            .iter()
            .map(|dependency| dependency.project_id.as_str())
            .collect::<HashSet<_>>();
        assert_eq!(
            projects,
            HashSet::from(["required", "required-leaf", "shared"])
        );
        assert!(
            plan.dependencies
                .iter()
                .all(|dependency| dependency.required)
        );
        let requests = requests.lock().unwrap();
        for optional in ["optional", "optional-child", "optional-leaf"] {
            assert!(!requests.iter().any(|id| id == optional));
        }
    }

    #[tokio::test]
    async fn install_previews_still_offer_optional_dependencies() {
        let plan =
            resolve_content(mixed_dependency_provider(), request("root"))
                .await
                .unwrap();

        for optional in ["optional", "optional-leaf"] {
            let dependency = plan
                .dependencies
                .iter()
                .find(|dependency| dependency.project_id == optional)
                .unwrap();
            assert!(!dependency.required);
        }
        assert!(
            plan.dependencies
                .iter()
                .any(|dependency| dependency.project_id == "optional-child")
        );
        let optional_child = plan
            .dependencies
            .iter()
            .find(|dependency| dependency.project_id == "optional-child")
            .unwrap();
        assert!(optional_child.required);
        assert_eq!(
            optional_child.dependent_on_version_id.as_deref(),
            Some("optional")
        );
    }

    #[tokio::test]
    async fn installed_optional_mods_can_still_be_updated_as_primary_content() {
        let provider = mixed_dependency_provider();
        for project_id in ["root", "optional"] {
            let mut update_request = request(project_id);
            update_request.version_id = Some(project_id.to_string());
            update_request.existing_project_ids =
                vec!["root".into(), "optional".into()];
            let plan = resolve_content_with_policy(
                provider.clone(),
                update_request,
                DependencyPolicy::RequiredOnly,
            )
            .await
            .unwrap();

            assert_eq!(plan.primary.project_id, project_id);
            assert!(
                plan.dependencies
                    .iter()
                    .all(|dependency| dependency.required)
            );
            assert_eq!(
                plan.dependencies.iter().any(|dependency| dependency.project_id == "optional-child"),
                project_id == "optional",
            );
            assert!(
                !plan
                    .dependencies
                    .iter()
                    .any(|dependency| dependency.project_id == "optional")
            );
        }
    }

    #[tokio::test]
    async fn quilt_replacement_preserves_optional_dependency_kind() {
        let provider = MemoryProvider::default().with_versions(vec![
            version(
                "root",
                "root",
                "2026-10-01T00:00:00Z",
                &["1.20.1"],
                &["quilt"],
                vec![dependency(
                    Some("P7dR8mSH"),
                    None,
                    DependencyType::Optional,
                )],
            ),
            version(
                "quilted-api",
                "qvIfYCYJ",
                "2026-10-01T00:00:00Z",
                &["1.20.1"],
                &["quilt"],
                vec![],
            ),
        ]);
        let mut request = request("root");
        request.target.loaders = vec!["quilt".into()];
        let preview = resolve_content(provider.clone(), request.clone())
            .await
            .unwrap();
        assert_eq!(preview.dependencies[0].project_id, "qvIfYCYJ");
        assert!(!preview.dependencies[0].required);

        let update = resolve_content_with_policy(
            provider,
            request,
            DependencyPolicy::RequiredOnly,
        )
        .await
        .unwrap();
        assert!(update.dependencies.is_empty());
    }

    #[tokio::test]
    async fn explicit_primary_version_is_used() {
        let provider = MemoryProvider::default().with_versions(vec![version(
            "v1",
            "p1",
            "2024-01-01T00:00:00Z",
            &["1.20.1"],
            &["fabric"],
            vec![],
        )]);
        let mut request = request("p1");
        request.version_id = Some("v1".to_string());

        let plan = resolve_content(provider, request).await.unwrap();

        assert_eq!(plan.primary.version_id, "v1");
    }

    #[tokio::test]
    async fn explicit_primary_version_bypasses_target_filters() {
        let provider = MemoryProvider::default().with_versions(vec![version(
            "v1",
            "p1",
            "2024-01-01T00:00:00Z",
            &["1.21.1"],
            &["neoforge"],
            vec![],
        )]);
        let mut request = request("p1");
        request.version_id = Some("v1".to_string());

        let plan = resolve_content(provider, request).await.unwrap();

        assert_eq!(plan.primary.version_id, "v1");
    }

    #[tokio::test]
    async fn newest_matching_primary_version_is_selected() {
        let provider = MemoryProvider::default().with_versions(vec![
            version(
                "old",
                "p1",
                "2024-01-01T00:00:00Z",
                &["1.20.1"],
                &["fabric"],
                vec![],
            ),
            version(
                "new",
                "p1",
                "2024-02-01T00:00:00Z",
                &["1.20.1"],
                &["fabric"],
                vec![],
            ),
        ]);

        let plan = resolve_content(provider, request("p1")).await.unwrap();

        assert_eq!(plan.primary.version_id, "new");
    }

    #[tokio::test]
    async fn automatic_primary_prefers_a_release_over_a_newer_beta() {
        let provider = MemoryProvider::default().with_versions(vec![
            version_with_channel(
                "iris-release",
                "iris",
                "2026-05-01T00:00:00Z",
                "release",
                &["1.21.1"],
                &["neoforge"],
                vec![dependency(
                    Some("sodium"),
                    Some("sodium-release"),
                    DependencyType::Required,
                )],
            ),
            version_with_channel(
                "iris-newer-beta",
                "iris",
                "2026-06-01T00:00:00Z",
                "beta",
                &["1.21.1"],
                &["neoforge"],
                vec![],
            ),
            version_with_channel(
                "sodium-release",
                "sodium",
                "2026-04-01T00:00:00Z",
                "release",
                &["1.21.1"],
                &["neoforge"],
                vec![],
            ),
        ]);
        let mut install_request = request("iris");
        install_request.target.game_versions = vec!["1.21.1".to_string()];
        install_request.target.loaders = vec!["neoforge".to_string()];

        let plan = resolve_content(provider, install_request).await.unwrap();

        assert_eq!(plan.primary.version_id, "iris-release");
        assert_eq!(plan.dependencies.len(), 1);
        assert_eq!(plan.dependencies[0].version_id, "sodium-release");
    }

    #[tokio::test]
    async fn project_only_dependency_uses_the_target_runtime() {
        let provider = MemoryProvider::default().with_versions(vec![
            version_with_channel(
                "primary",
                "primary",
                "2026-06-01T00:00:00Z",
                "release",
                &["1.21.1"],
                &["neoforge"],
                vec![required_project_dependency("dependency")],
            ),
            version_with_channel(
                "wrong-game-version",
                "dependency",
                "2026-07-01T00:00:00Z",
                "beta",
                &["1.21.2"],
                &["neoforge"],
                vec![],
            ),
            version_with_channel(
                "wrong-loader",
                "dependency",
                "2026-06-15T00:00:00Z",
                "beta",
                &["1.21.1"],
                &["fabric"],
                vec![],
            ),
            version_with_channel(
                "matching-runtime",
                "dependency",
                "2026-05-01T00:00:00Z",
                "release",
                &["1.21.1"],
                &["neoforge"],
                vec![],
            ),
        ]);
        let mut install_request = request("primary");
        install_request.version_id = Some("primary".to_string());
        install_request.target.game_versions = vec!["1.21.1".to_string()];
        install_request.target.loaders = vec!["neoforge".to_string()];

        let plan = resolve_content(provider, install_request).await.unwrap();

        assert_eq!(plan.primary.version_id, "primary");
        assert_eq!(plan.dependencies.len(), 1);
        assert_eq!(plan.dependencies[0].version_id, "matching-runtime");
    }

    #[tokio::test]
    async fn project_only_dependency_selects_matching_version() {
        let provider = MemoryProvider::default().with_versions(vec![
            version(
                "p1v1",
                "p1",
                "2024-01-01T00:00:00Z",
                &["1.20.1"],
                &["fabric"],
                vec![required_project_dependency("dep")],
            ),
            version(
                "depv1",
                "dep",
                "2024-01-01T00:00:00Z",
                &["1.20.1"],
                &["fabric"],
                vec![],
            ),
        ]);

        let plan = resolve_content(provider, request("p1")).await.unwrap();

        assert_eq!(plan.dependencies[0].project_id, "dep");
        assert_eq!(
            plan.dependencies[0].dependent_on_version_id.as_deref(),
            Some("p1v1")
        );
    }

    #[tokio::test]
    async fn exact_version_dependency_bypasses_target_filters() {
        let provider = MemoryProvider::default().with_versions(vec![
            version(
                "p1v1",
                "p1",
                "2024-01-01T00:00:00Z",
                &["1.20.1"],
                &["fabric"],
                vec![required_version_dependency("depv1")],
            ),
            version(
                "depv1",
                "dep",
                "2024-01-01T00:00:00Z",
                &["1.19.4"],
                &["forge"],
                vec![],
            ),
        ]);

        let plan = resolve_content(provider, request("p1")).await.unwrap();

        assert_eq!(plan.dependencies.len(), 1);
        assert_eq!(plan.dependencies[0].project_id, "dep");
        assert_eq!(plan.dependencies[0].version_id, "depv1");
        assert!(plan.skipped.is_empty());
    }

    #[tokio::test]
    async fn already_installed_dependencies_are_skipped() {
        let provider = MemoryProvider::default().with_versions(vec![
            version(
                "p1v1",
                "p1",
                "2024-01-01T00:00:00Z",
                &["1.20.1"],
                &["fabric"],
                vec![required_project_dependency("dep")],
            ),
            version(
                "depv1",
                "dep",
                "2024-01-01T00:00:00Z",
                &["1.20.1"],
                &["fabric"],
                vec![],
            ),
        ]);
        let mut request = request("p1");
        request.existing_project_ids = vec!["dep".to_string()];

        let plan = resolve_content(provider, request).await.unwrap();

        assert!(plan.dependencies.is_empty());
        assert_eq!(plan.skipped[0].reason, SkippedReason::AlreadyInstalled);
    }

    #[tokio::test]
    async fn duplicate_dependency_projects_are_skipped() {
        let provider = MemoryProvider::default().with_versions(vec![
            version(
                "p1v1",
                "p1",
                "2024-01-01T00:00:00Z",
                &["1.20.1"],
                &["fabric"],
                vec![
                    required_project_dependency("dep"),
                    required_project_dependency("dep"),
                ],
            ),
            version(
                "depv1",
                "dep",
                "2024-01-01T00:00:00Z",
                &["1.20.1"],
                &["fabric"],
                vec![],
            ),
        ]);

        let plan = resolve_content(provider, request("p1")).await.unwrap();

        assert_eq!(plan.dependencies.len(), 1);
        assert_eq!(plan.skipped[0].reason, SkippedReason::DuplicateProject);
    }

    #[tokio::test]
    async fn conflicting_dependency_versions_are_skipped() {
        let provider = MemoryProvider::default().with_versions(vec![
            version(
                "p1v1",
                "p1",
                "2024-01-01T00:00:00Z",
                &["1.20.1"],
                &["fabric"],
                vec![
                    dependency(
                        Some("dep"),
                        Some("depv1"),
                        DependencyType::Required,
                    ),
                    dependency(
                        Some("dep"),
                        Some("depv2"),
                        DependencyType::Required,
                    ),
                ],
            ),
            version(
                "depv1",
                "dep",
                "2024-01-01T00:00:00Z",
                &["1.20.1"],
                &["fabric"],
                vec![],
            ),
            version(
                "depv2",
                "dep",
                "2024-02-01T00:00:00Z",
                &["1.20.1"],
                &["fabric"],
                vec![],
            ),
        ]);

        let plan = resolve_content(provider, request("p1")).await.unwrap();

        assert_eq!(plan.dependencies.len(), 1);
        assert_eq!(
            plan.skipped[0].reason,
            SkippedReason::ConflictingDependency
        );
    }

    #[tokio::test]
    async fn quilt_instances_replace_fabric_api_with_quilted_fabric_api() {
        let provider = MemoryProvider::default().with_versions(vec![
            version(
                "p1v1",
                "p1",
                "2024-01-01T00:00:00Z",
                &["1.20.1"],
                &["quilt"],
                vec![required_project_dependency(
                    QUILT_FABRIC_API_EXCEPTION_PROJECT_ID,
                )],
            ),
            version(
                "qfabv1",
                "qvIfYCYJ",
                "2024-01-01T00:00:00Z",
                &["1.20.1"],
                &["quilt"],
                vec![],
            ),
        ]);
        let mut request = request("p1");
        request.target.loaders = vec!["quilt".to_string()];

        let plan = resolve_content(provider, request).await.unwrap();

        assert_eq!(plan.dependencies.len(), 1);
        assert_eq!(plan.dependencies[0].project_id, "qvIfYCYJ");
        assert!(plan.skipped.is_empty());
    }

    #[tokio::test]
    async fn iris_metadata_gap_injects_missing_sodium_dependency() {
        let provider = MemoryProvider::default().with_versions(vec![
            version(
                "Cjwm9s3i",
                "YL57xq9U",
                "2023-12-13T02:48:08Z",
                &["1.20.2"],
                &["fabric"],
                vec![],
            ),
            version(
                "pmgeU5yX",
                "AANobbMI",
                "2023-12-09T04:07:46Z",
                &["1.20.2"],
                &["fabric"],
                vec![],
            ),
        ]);
        let mut request = request("YL57xq9U");
        request.target.game_versions = vec!["1.20.2".to_string()];

        let plan = resolve_content(provider, request).await.unwrap();

        assert_eq!(plan.dependencies.len(), 1);
        assert_eq!(plan.dependencies[0].project_id, "AANobbMI");
        assert_eq!(plan.dependencies[0].version_id, "pmgeU5yX");
        assert!(plan.skipped.is_empty());
    }

    #[tokio::test]
    async fn iris_metadata_gap_is_not_duplicated_once_upstream_is_fixed() {
        let provider = MemoryProvider::default().with_versions(vec![
            version(
                "Cjwm9s3i",
                "YL57xq9U",
                "2023-12-13T02:48:08Z",
                &["1.20.2"],
                &["fabric"],
                vec![required_project_dependency("AANobbMI")],
            ),
            version(
                "pmgeU5yX",
                "AANobbMI",
                "2023-12-09T04:07:46Z",
                &["1.20.2"],
                &["fabric"],
                vec![],
            ),
        ]);
        let mut request = request("YL57xq9U");
        request.target.game_versions = vec!["1.20.2".to_string()];

        let plan = resolve_content(provider, request).await.unwrap();

        assert_eq!(plan.dependencies.len(), 1);
        assert_eq!(plan.dependencies[0].version_id, "pmgeU5yX");
        assert!(plan.skipped.is_empty());
    }

    #[tokio::test]
    async fn iris_metadata_gap_does_not_apply_to_other_versions() {
        let provider = MemoryProvider::default().with_versions(vec![version(
            "other",
            "YL57xq9U",
            "2024-01-01T00:00:00Z",
            &["1.20.2"],
            &["fabric"],
            vec![],
        )]);
        let mut request = request("YL57xq9U");
        request.target.game_versions = vec!["1.20.2".to_string()];

        let plan = resolve_content(provider, request).await.unwrap();

        assert!(plan.dependencies.is_empty());
        assert!(plan.skipped.is_empty());
    }

    #[tokio::test]
    async fn mods_reject_datapack_versions_when_a_loader_is_requested() {
        let provider = MemoryProvider::default().with_versions(vec![version(
            "p1v1",
            "p1",
            "2024-01-01T00:00:00Z",
            &["1.20.1"],
            &["datapack"],
            vec![],
        )]);

        assert!(matches!(
            resolve_content(provider, request("p1")).await,
            Err(Error::NoCompatibleVersion(project_id)) if project_id == "p1"
        ));
    }

    #[tokio::test]
    async fn neoforge_rejects_neo_loader_alias() {
        let provider = MemoryProvider::default().with_versions(vec![version(
            "p1v1",
            "p1",
            "2024-01-01T00:00:00Z",
            &["1.20.1"],
            &["neo"],
            vec![],
        )]);
        let mut request = request("p1");
        request.target.loaders = vec!["neoforge".to_string()];

        assert!(matches!(
            resolve_content(provider, request).await,
            Err(Error::NoCompatibleVersion(project_id)) if project_id == "p1"
        ));
    }

    #[tokio::test]
    async fn paper_rejects_bukkit_loader_alias() {
        let provider = MemoryProvider::default().with_versions(vec![version(
            "p1v1",
            "p1",
            "2024-01-01T00:00:00Z",
            &["1.20.1"],
            &["bukkit"],
            vec![],
        )]);
        let mut request = request("p1");
        request.content_type = ContentType::Plugin;
        request.target.loaders = vec!["paper".to_string()];

        assert!(matches!(
            resolve_content(provider, request).await,
            Err(Error::NoCompatibleVersion(project_id)) if project_id == "p1"
        ));
    }

    #[tokio::test]
    async fn modpack_embedded_dependencies_are_not_installed() {
        let provider = MemoryProvider::default()
            .with_versions(vec![version(
                "Ur9uoHH5",
                "shFhR8Vx",
                "2025-01-19T06:39:41.349487Z",
                &["1.20.1"],
                &["fabric"],
                vec![
                    dependency(
                        Some("embedded-a"),
                        None,
                        DependencyType::Embedded,
                    ),
                    dependency(
                        Some("embedded-b"),
                        None,
                        DependencyType::Embedded,
                    ),
                    dependency(
                        Some("embedded-c"),
                        None,
                        DependencyType::Embedded,
                    ),
                ],
            )])
            .with_project_versions("better-mc-fabric-bmc2", &["Ur9uoHH5"]);

        let plan = resolve_content(
            provider,
            ResolveContentRequest {
                project_id: "better-mc-fabric-bmc2".to_string(),
                version_id: None,
                content_type: ContentType::ModPack,
                selected: ResolutionPreferences::default(),
                target: ResolutionPreferences::default(),
                existing_project_ids: Vec::new(),
                excluded_project_ids: Vec::new(),
                force_project_ids: Vec::new(),
            },
        )
        .await
        .unwrap();

        assert_eq!(plan.primary.project_id, "shFhR8Vx");
        assert_eq!(plan.primary.version_id, "Ur9uoHH5");
        assert!(plan.dependencies.is_empty());
        assert!(plan.skipped.is_empty());
    }

    #[tokio::test]
    async fn required_dependencies_resolve_transitively() {
        let provider = MemoryProvider::default().with_versions(vec![
            version(
                "pKFEfjEB",
                "DdAlVT8M",
                "2026-06-07T21:21:24.772638Z",
                &["1.21.1", "1.21.2"],
                &["neoforge"],
                vec![
                    dependency(
                        Some("LNytGWDc"),
                        Some("UjX6dr61"),
                        DependencyType::Required,
                    ),
                    dependency(
                        Some("oWaK0Q19"),
                        Some("YhZLrAFC"),
                        DependencyType::Required,
                    ),
                ],
            ),
            version(
                "UjX6dr61",
                "LNytGWDc",
                "2026-04-21T22:20:03.579201Z",
                &["1.21.1"],
                &["neoforge"],
                vec![],
            ),
            version(
                "YhZLrAFC",
                "oWaK0Q19",
                "2026-05-21T22:20:03.579201Z",
                &["1.21.1"],
                &["neoforge"],
                vec![
                    dependency(
                        Some("T9PomCSv"),
                        None,
                        DependencyType::Required,
                    ),
                    dependency(
                        Some("LNytGWDc"),
                        Some("UjX6dr61"),
                        DependencyType::Required,
                    ),
                ],
            ),
            version(
                "hyQUls27",
                "T9PomCSv",
                "2026-06-17T04:32:50.469591Z",
                &["1.21.1"],
                &["fabric"],
                vec![],
            ),
            version(
                "1L6XJqnY",
                "T9PomCSv",
                "2026-06-17T04:32:49.279302Z",
                &["1.21.1"],
                &["neoforge"],
                vec![],
            ),
        ]);

        let plan = resolve_content(
            provider,
            ResolveContentRequest {
                project_id: "DdAlVT8M".to_string(),
                version_id: Some("pKFEfjEB".to_string()),
                content_type: ContentType::Mod,
                selected: ResolutionPreferences::default(),
                target: ResolutionPreferences {
                    game_versions: vec![
                        "1.21.1".to_string(),
                        "1.21.2".to_string(),
                    ],
                    loaders: vec!["neoforge".to_string()],
                },
                existing_project_ids: Vec::new(),
                excluded_project_ids: Vec::new(),
                force_project_ids: Vec::new(),
            },
        )
        .await
        .unwrap();

        assert_eq!(plan.primary.project_id, "DdAlVT8M");
        assert_eq!(plan.primary.version_id, "pKFEfjEB");
        assert_eq!(plan.dependencies.len(), 3);
        assert!(plan.dependencies.iter().any(|dependency| {
            dependency.project_id == "LNytGWDc"
                && dependency.version_id == "UjX6dr61"
                && dependency.dependent_on_version_id.as_deref()
                    == Some("pKFEfjEB")
        }));
        assert!(plan.dependencies.iter().any(|dependency| {
            dependency.project_id == "oWaK0Q19"
                && dependency.version_id == "YhZLrAFC"
                && dependency.dependent_on_version_id.as_deref()
                    == Some("pKFEfjEB")
        }));
        assert!(plan.dependencies.iter().any(|dependency| {
            dependency.project_id == "T9PomCSv"
                && dependency.version_id == "1L6XJqnY"
                && dependency.dependent_on_version_id.as_deref()
                    == Some("YhZLrAFC")
        }));
        assert_eq!(plan.skipped.len(), 1);
        assert_eq!(
            plan.skipped[0].reason,
            content_management::SkippedReason::DuplicateProject
        );
        assert_eq!(plan.skipped[0].project_id, "LNytGWDc");
    }
}
