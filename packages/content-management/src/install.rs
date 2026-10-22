use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

use crate::model::{
    ContentMetadata, ContentType, Dependency, DependencyType, Error,
    ResolutionPreferences, ResolveContentPlan, ResolveContentRequest,
    ResolvedContent, SkippedContent, SkippedReason, Version,
};
use crate::provider::ContentMetadataProvider;

// Fabric API is replaced by Quilted Fabric API when the install target is
// quilt. See Prism Launcher's override table for the equivalent mapping.
const FABRIC_API_PROJECT_ID: &str = "P7dR8mSH";
const QUILTED_FABRIC_API_PROJECT_ID: &str = "qvIfYCYJ";
const IRIS_PROJECT_ID: &str = "YL57xq9U";
const SODIUM_PROJECT_ID: &str = "AANobbMI";
const MAX_DEPENDENCY_DEPTH: usize = 32;

// Some Modrinth versions omit required dependencies from their metadata even
// though the mod JAR itself declares them (e.g. several Iris releases omit
// Sodium). Correct those records locally so install plans match what the mod
// actually requires; entries can be removed once upstream metadata is fixed.
struct MissingDependencyCorrection {
    project_id: &'static str,
    version_id: &'static str,
    dependency_project_id: &'static str,
}

const MISSING_DEPENDENCY_CORRECTIONS: &[MissingDependencyCorrection] = &[
    MissingDependencyCorrection {
        project_id: IRIS_PROJECT_ID,
        version_id: "Cjwm9s3i", // Iris 1.6.14 for Minecraft 1.20.2
        dependency_project_id: SODIUM_PROJECT_ID,
    },
    MissingDependencyCorrection {
        project_id: IRIS_PROJECT_ID,
        version_id: "G5dd9TM4", // Iris 1.7.1 for Minecraft 1.20.1
        dependency_project_id: SODIUM_PROJECT_ID,
    },
    MissingDependencyCorrection {
        project_id: IRIS_PROJECT_ID,
        version_id: "keLlmlCc", // Iris 1.7.1 for Minecraft 1.20.5/1.20.6
        dependency_project_id: SODIUM_PROJECT_ID,
    },
    MissingDependencyCorrection {
        project_id: IRIS_PROJECT_ID,
        version_id: "Kdz76qQt", // Iris 1.8.0-beta.3 for Fabric 1.21
        dependency_project_id: SODIUM_PROJECT_ID,
    },
    MissingDependencyCorrection {
        project_id: IRIS_PROJECT_ID,
        version_id: "di7sM681", // Iris 1.8.0-beta.2 for Fabric 1.21
        dependency_project_id: SODIUM_PROJECT_ID,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyPolicy {
    RequiredOnly,
    IncludeOptional,
}

impl DependencyPolicy {
    pub fn allows(self, dependency_type: DependencyType) -> bool {
        dependency_type == DependencyType::Required
            || (self == Self::IncludeOptional
                && dependency_type == DependencyType::Optional)
    }
}

pub async fn resolve_content<P: ContentMetadataProvider>(
    provider: P,
    request: ResolveContentRequest,
) -> Result<ResolveContentPlan, Error> {
    resolve_content_with_policy(
        provider,
        request,
        DependencyPolicy::IncludeOptional,
    )
    .await
}

pub async fn resolve_content_with_policy<P: ContentMetadataProvider>(
    mut provider: P,
    request: ResolveContentRequest,
    dependency_policy: DependencyPolicy,
) -> Result<ResolveContentPlan, Error> {
    let primary_version =
        resolve_primary_version(&mut provider, &request).await?;
    let primary = ResolvedContent {
        project_id: primary_version.project_id.clone(),
        version_id: primary_version.id.clone(),
        dependent_on_version_id: None,
        required: true,
        metadata: Some(version_metadata(&primary_version)),
    };
    let mut resolver =
        InstallResolver::new(provider, &request, dependency_policy);
    resolver
        .resolve_dependencies_for_version(primary_version)
        .await?;

    Ok(ResolveContentPlan {
        primary,
        dependencies: resolver.dependencies,
        skipped: resolver.skipped,
    })
}

/// Resolves exact versions by identity and applies target filters only when
/// choosing a version automatically.
async fn resolve_primary_version<P: ContentMetadataProvider>(
    provider: &mut P,
    request: &ResolveContentRequest,
) -> Result<Version, Error> {
    if let Some(version_id) = &request.version_id {
        let version = provider
            .get_version(version_id)
            .await?
            .ok_or_else(|| Error::VersionNotFound(version_id.clone()))?;

        if version.project_id != request.project_id {
            return Err(Error::VersionProjectMismatch {
                version_id: version.id,
                project_id: request.project_id.clone(),
            });
        }

        return Ok(version);
    }

    let versions = provider.get_project_versions(&request.project_id).await?;
    if versions.is_empty() {
        return Err(Error::ProjectNotFound(request.project_id.clone()));
    }

    select_newest_matching_version(
        versions,
        request.content_type,
        &request.selected,
        &request.target,
    )
    .ok_or_else(|| Error::NoCompatibleVersion(request.project_id.clone()))
}

struct InstallResolver<'a, P> {
    provider: P,
    content_type: ContentType,
    dependency_policy: DependencyPolicy,
    selected: &'a ResolutionPreferences,
    target: &'a ResolutionPreferences,
    existing_project_ids: HashSet<String>,
    force_project_ids: HashSet<String>,
    excluded_project_ids: HashSet<String>,
    planned_project_versions: HashMap<String, String>,
    visited_versions: HashSet<String>,
    dependencies: Vec<ResolvedContent>,
    skipped: Vec<SkippedContent>,
}

impl<'a, P: ContentMetadataProvider> InstallResolver<'a, P> {
    fn new(
        provider: P,
        request: &'a ResolveContentRequest,
        dependency_policy: DependencyPolicy,
    ) -> Self {
        let mut planned_project_versions = HashMap::new();
        planned_project_versions.insert(
            request.project_id.clone(),
            request.version_id.clone().unwrap_or_default(),
        );

        Self {
            provider,
            content_type: request.content_type,
            dependency_policy,
            selected: &request.selected,
            target: &request.target,
            existing_project_ids: request
                .existing_project_ids
                .iter()
                .cloned()
                .collect(),
            force_project_ids: request
                .force_project_ids
                .iter()
                .cloned()
                .collect(),
            excluded_project_ids: request
                .excluded_project_ids
                .iter()
                .cloned()
                .collect(),
            planned_project_versions,
            visited_versions: HashSet::new(),
            dependencies: Vec::new(),
            skipped: Vec::new(),
        }
    }

    async fn resolve_dependencies_for_version(
        &mut self,
        version: Version,
    ) -> Result<(), Error> {
        let mut stack = vec![(version, 0_usize)];

        while let Some((version, depth)) = stack.pop() {
            if !self.visited_versions.insert(version.id.clone()) {
                self.skipped.push(SkippedContent {
                    project_id: version.project_id,
                    version_id: Some(version.id),
                    dependent_on_version_id: None,
                    reason: SkippedReason::DependencyCycle,
                });
                continue;
            }
            if depth >= MAX_DEPENDENCY_DEPTH {
                self.skipped.push(SkippedContent {
                    project_id: version.project_id,
                    version_id: Some(version.id),
                    dependent_on_version_id: None,
                    reason: SkippedReason::DependencyDepthExceeded,
                });
                continue;
            }

            let corrected_dependencies =
                dependency_metadata_corrections(&version);
            for original_dependency in version
                .dependencies
                .iter()
                .chain(corrected_dependencies.iter())
            {
                if !self
                    .dependency_policy
                    .allows(original_dependency.dependency_type)
                {
                    continue;
                }
                let overridden_dependency;
                let dependency = if should_use_quilted_fabric_api(
                    original_dependency,
                    self.target,
                ) {
                    overridden_dependency = Dependency {
                        project_id: Some(
                            QUILTED_FABRIC_API_PROJECT_ID.to_string(),
                        ),
                        version_id: None,
                        file_name: original_dependency.file_name.clone(),
                        dependency_type: original_dependency.dependency_type,
                    };
                    &overridden_dependency
                } else {
                    original_dependency
                };

                let Some(dependency_version) =
                    self.resolve_dependency_version(dependency).await?
                else {
                    continue;
                };

                let project_id = dependency
                    .project_id
                    .clone()
                    .unwrap_or_else(|| dependency_version.project_id.clone());
                let required = matches!(
                    dependency.dependency_type,
                    DependencyType::Required
                );

                if self.excluded_project_ids.contains(&project_id) {
                    self.skipped.push(SkippedContent {
                        project_id,
                        version_id: Some(dependency_version.id),
                        dependent_on_version_id: Some(version.id.clone()),
                        reason: SkippedReason::ExcludedByUser,
                    });
                    continue;
                }

                if self.existing_project_ids.contains(&project_id)
                    && !self.force_project_ids.contains(&project_id)
                {
                    self.skipped.push(SkippedContent {
                        project_id,
                        version_id: Some(dependency_version.id),
                        dependent_on_version_id: Some(version.id.clone()),
                        reason: SkippedReason::AlreadyInstalled,
                    });
                    continue;
                }

                if let Some(planned_version_id) =
                    self.planned_project_versions.get(&project_id)
                {
                    if required {
                        if let Some(existing) = self
                            .dependencies
                            .iter_mut()
                            .find(|existing| existing.project_id == project_id)
                        {
                            existing.required = true;
                        }
                    }
                    let reason = if planned_version_id.is_empty()
                        || planned_version_id == &dependency_version.id
                    {
                        SkippedReason::DuplicateProject
                    } else {
                        SkippedReason::ConflictingDependency
                    };

                    self.skipped.push(SkippedContent {
                        project_id,
                        version_id: Some(dependency_version.id),
                        dependent_on_version_id: Some(version.id.clone()),
                        reason,
                    });
                    continue;
                }

                self.planned_project_versions
                    .insert(project_id.clone(), dependency_version.id.clone());
                self.dependencies.push(ResolvedContent {
                    project_id,
                    version_id: dependency_version.id.clone(),
                    dependent_on_version_id: Some(version.id.clone()),
                    required,
                    metadata: Some(version_metadata(&dependency_version)),
                });
                stack.push((dependency_version, depth + 1));
            }
        }

        Ok(())
    }

    async fn resolve_dependency_version(
        &mut self,
        dependency: &Dependency,
    ) -> Result<Option<Version>, Error> {
        if let Some(version_id) = &dependency.version_id {
            let version = self.provider.get_version(version_id).await?;
            if version.is_none() {
                self.skipped.push(SkippedContent {
                    project_id: dependency
                        .project_id
                        .clone()
                        .unwrap_or_default(),
                    version_id: Some(version_id.clone()),
                    dependent_on_version_id: None,
                    reason: SkippedReason::MissingVersion,
                });
            }
            return Ok(version);
        }

        let Some(project_id) = &dependency.project_id else {
            return Ok(None);
        };
        let versions = self.provider.get_project_versions(project_id).await?;
        let version = select_newest_matching_version(
            versions,
            self.content_type,
            self.selected,
            self.target,
        );

        if version.is_none() {
            self.skipped.push(SkippedContent {
                project_id: project_id.clone(),
                version_id: None,
                dependent_on_version_id: None,
                reason: SkippedReason::NoCompatibleVersion,
            });
        }

        Ok(version)
    }
}

fn dependency_metadata_corrections(version: &Version) -> Vec<Dependency> {
    MISSING_DEPENDENCY_CORRECTIONS
        .iter()
        .filter(|correction| {
            correction.project_id == version.project_id
                && correction.version_id == version.id.as_str()
        })
        .map(|correction| Dependency {
            project_id: Some(correction.dependency_project_id.to_string()),
            version_id: None,
            file_name: None,
            dependency_type: DependencyType::Required,
        })
        .filter(|dependency| {
            !version.dependencies.iter().any(|declared| {
                declared.project_id.as_deref()
                    == dependency.project_id.as_deref()
            })
        })
        .collect()
}

fn version_metadata(v: &Version) -> ContentMetadata {
    ContentMetadata {
        title: None,
        version_number: v.version_number.clone(),
        filename: v.filename.clone(),
        sha1: v.sha1.clone(),
        icon_url: None,
    }
}

fn select_newest_matching_version(
    versions: Vec<Version>,
    content_type: ContentType,
    selected: &ResolutionPreferences,
    target: &ResolutionPreferences,
) -> Option<Version> {
    select_matching_version(
        versions,
        content_type,
        selected,
        target,
        &[
            ReleaseChannel::Release,
            ReleaseChannel::Beta,
            ReleaseChannel::Alpha,
        ],
    )
}

fn select_matching_version(
    mut versions: Vec<Version>,
    content_type: ContentType,
    selected: &ResolutionPreferences,
    target: &ResolutionPreferences,
    channel_order: &[ReleaseChannel],
) -> Option<Version> {
    versions.sort_by_key(|version| {
        (
            channel_order
                .iter()
                .position(|channel| {
                    *channel == release_channel(&version.version_type)
                })
                .unwrap_or(channel_order.len()),
            Reverse(version.date_published),
        )
    });
    let merged = selected.merge(target);

    versions
        .iter()
        .find(|version| version_matches(version, content_type, &merged))
        .or_else(|| {
            versions
                .iter()
                .find(|version| version_matches(version, content_type, target))
        })
        .cloned()
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ReleaseChannel {
    Release,
    Beta,
    Alpha,
}

fn release_channel(version_type: &str) -> ReleaseChannel {
    if version_type.eq_ignore_ascii_case("beta") {
        ReleaseChannel::Beta
    } else if version_type.eq_ignore_ascii_case("alpha") {
        ReleaseChannel::Alpha
    } else {
        ReleaseChannel::Release
    }
}

trait MergePreferences {
    fn merge(&self, target: &Self) -> Self;
}

impl MergePreferences for ResolutionPreferences {
    fn merge(&self, target: &Self) -> Self {
        Self {
            game_versions: if self.game_versions.is_empty() {
                target.game_versions.clone()
            } else {
                self.game_versions.clone()
            },
            loaders: if self.loaders.is_empty() {
                target.loaders.clone()
            } else {
                self.loaders.clone()
            },
        }
    }
}

fn version_matches(
    version: &Version,
    content_type: ContentType,
    preferences: &ResolutionPreferences,
) -> bool {
    matches_game_versions(version, preferences)
        && matches_loaders(version, content_type, preferences)
}

fn matches_game_versions(
    version: &Version,
    preferences: &ResolutionPreferences,
) -> bool {
    preferences.game_versions.is_empty()
        || preferences.game_versions.iter().any(|game_version| {
            version
                .game_versions
                .iter()
                .any(|candidate| candidate == game_version)
        })
}

fn matches_loaders(
    version: &Version,
    _content_type: ContentType,
    preferences: &ResolutionPreferences,
) -> bool {
    if preferences.loaders.is_empty() {
        return true;
    }

    preferences.loaders.iter().any(|loader| {
        version
            .loaders
            .iter()
            .any(|candidate| loaders_match(loader, candidate))
    })
}

fn loaders_match(expected: &str, candidate: &str) -> bool {
    expected.eq_ignore_ascii_case(candidate)
}

fn should_use_quilted_fabric_api(
    dependency: &Dependency,
    target: &ResolutionPreferences,
) -> bool {
    dependency.project_id.as_deref() == Some(FABRIC_API_PROJECT_ID)
        && target
            .loaders
            .iter()
            .any(|loader| loaders_match(loader, "quilt"))
}
