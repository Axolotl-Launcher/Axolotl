use super::*;
use std::collections::HashSet;

pub fn safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':'])
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|part| !part.is_empty() && !matches!(part, "." | ".."))
}

pub fn artifact_path(
    scope: &InstallScope,
    kind: ResourceType,
    artifact: &ResourceArtifact,
) -> Result<String, String> {
    let file = artifact
        .file_name
        .as_ref()
        .ok_or_else(|| "artifact has no filename".to_string())?;
    if !safe_relative_path(file) || file.contains('/') {
        return Err("invalid artifact filename".into());
    }
    let base = match scope {
        InstallScope::Instance => match kind {
            ResourceType::Mod => "mods",
            ResourceType::Plugin => "plugins",
            ResourceType::Shader => "shaderpacks",
            ResourceType::ResourcePack => "resourcepacks",
            _ => {
                return Err("unsupported content scope".into());
            }
        },
        InstallScope::World { relative_path } => {
            if kind != ResourceType::DataPack
                || !safe_relative_path(relative_path)
            {
                return Err("unsupported content scope".into());
            }
            return Ok(format!("{relative_path}/datapacks/{file}"));
        }
    };
    Ok(format!("{base}/{file}"))
}

/// Compatibility remains unknown when the source did not declare a dimension.
pub fn assess_compatibility(
    release: &ResourceRelease,
    target: &ipc::ResourceTarget,
    kind: ResourceType,
) -> CompatibilityAssessment {
    assess_declared_compatibility(&release.compatibility, target, kind)
}

pub fn assess_declared_compatibility(
    declared: &ReleaseCompatibility,
    target: &ipc::ResourceTarget,
    kind: ResourceType,
) -> CompatibilityAssessment {
    let game_version =
        if declared.game_versions.is_empty() || target.game_versions.is_empty()
        {
            CompatibilityState::Unknown
        } else {
            let states: Vec<_> = declared
                .game_versions
                .iter()
                .flat_map(|pattern| {
                    target.game_versions.iter().map(move |version| {
                        assess_game_version(pattern, version)
                    })
                })
                .collect();
            if states.contains(&CompatibilityState::Compatible) {
                CompatibilityState::Compatible
            } else if states.contains(&CompatibilityState::Unknown) {
                CompatibilityState::Unknown
            } else {
                CompatibilityState::Incompatible
            }
        };
    let loader = if matches!(
        kind,
        ResourceType::ResourcePack
            | ResourceType::Shader
            | ResourceType::DataPack
            | ResourceType::World
    ) {
        CompatibilityState::Compatible
    } else {
        dimension(&declared.loaders, &target.loaders, |a, b| {
            a.eq_ignore_ascii_case(b)
                || a.eq_ignore_ascii_case("fabric")
                    && b.eq_ignore_ascii_case("quilt")
        })
    };
    let mut reasons = vec![];
    if game_version != CompatibilityState::Compatible {
        reasons.push("gameVersion".into());
    }
    if loader != CompatibilityState::Compatible {
        reasons.push("loader".into());
    }
    CompatibilityAssessment {
        game_version,
        loader,
        reasons,
    }
}

fn dimension(
    values: &[String],
    targets: &[String],
    matches: impl Fn(&str, &str) -> bool,
) -> CompatibilityState {
    if values.is_empty() || targets.is_empty() {
        CompatibilityState::Unknown
    } else if values.iter().any(|a| targets.iter().any(|b| matches(a, b))) {
        CompatibilityState::Compatible
    } else {
        CompatibilityState::Incompatible
    }
}

pub fn matches_game_version(pattern: &str, version: &str) -> bool {
    assess_game_version(pattern, version) == CompatibilityState::Compatible
}

pub fn matches_release_constraint(pattern: &str, version: &str) -> bool {
    if pattern.contains("||") {
        return false;
    }
    if pattern.trim() == version {
        return true;
    }
    let Some(mut actual) = numeric_version(version) else {
        return false;
    };
    if actual.len() > 3 {
        return false;
    }
    actual.resize(3, 0);
    let actual = actual
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(".");
    let mut normalized = String::new();
    let mut number = String::new();
    for character in pattern.chars().chain(std::iter::once(' ')) {
        if character.is_ascii_digit() || character == '.' {
            number.push(character);
        } else {
            if !number.is_empty() {
                if number.ends_with('.') {
                    normalized.push_str(&number);
                } else {
                    let Some(mut parts) = numeric_version(&number) else {
                        return false;
                    };
                    if parts.len() > 3 {
                        return false;
                    }
                    parts.resize(3, 0);
                    normalized.push_str(
                        &parts
                            .iter()
                            .map(u64::to_string)
                            .collect::<Vec<_>>()
                            .join("."),
                    );
                }
                number.clear();
            }
            normalized.push(character);
        }
    }
    matches_game_version(normalized.trim(), &actual)
}

/// Unsupported range syntax is unknown rather than evidence of compatibility.
pub fn assess_game_version(pattern: &str, version: &str) -> CompatibilityState {
    fn numeric(value: &str) -> Option<[u64; 3]> {
        if value.trim().starts_with('v') {
            return None;
        }
        let parts = numeric_version(value.trim())?;
        if !(1..=3).contains(&parts.len()) {
            return None;
        }
        Some([
            parts[0],
            *parts.get(1).unwrap_or(&0),
            *parts.get(2).unwrap_or(&0),
        ])
    }
    fn evaluate(pattern: &str, actual: [u64; 3]) -> Option<bool> {
        for operator in [">=", "<=", ">", "<"] {
            if let Some(bound) = pattern.strip_prefix(operator) {
                let bound = numeric(bound)?;
                return Some(match operator {
                    ">=" => actual >= bound,
                    "<=" => actual <= bound,
                    ">" => actual > bound,
                    _ => actual < bound,
                });
            }
        }
        if matches!(pattern.as_bytes().first(), Some(b'[' | b'(')) {
            if pattern.starts_with('[')
                && pattern.ends_with(']')
                && !pattern.contains(',')
            {
                return Some(
                    actual == numeric(&pattern[1..pattern.len() - 1])?,
                );
            }
            let lower_inclusive = pattern.starts_with('[');
            let upper_inclusive = pattern.ends_with(']');
            if !upper_inclusive && !pattern.ends_with(')') {
                return None;
            }
            let (lower, upper) =
                pattern[1..pattern.len() - 1].split_once(',')?;
            let lower = if lower.trim().is_empty() {
                None
            } else {
                Some(numeric(lower)?)
            };
            let upper = if upper.trim().is_empty() {
                None
            } else {
                Some(numeric(upper)?)
            };
            if lower.is_none() && upper.is_none()
                || matches!((lower, upper), (Some(a), Some(b)) if a > b)
            {
                return None;
            }
            return Some(
                lower.is_none_or(|bound| {
                    actual > bound || lower_inclusive && actual == bound
                }) && upper.is_none_or(|bound| {
                    actual < bound || upper_inclusive && actual == bound
                }),
            );
        }
        if let Some((lower, upper)) = pattern.split_once('-') {
            let lower = numeric(lower)?;
            let upper = numeric(upper)?;
            return (lower <= upper)
                .then_some(actual >= lower && actual <= upper);
        }
        let parts: Vec<_> = pattern.split('.').collect();
        if parts
            .last()
            .is_some_and(|part| matches!(*part, "x" | "X" | "*"))
        {
            if !(2..=3).contains(&parts.len()) {
                return None;
            }
            let prefix: Option<Vec<u64>> = parts[..parts.len() - 1]
                .iter()
                .map(|part| part.parse().ok())
                .collect();
            let prefix = prefix?;
            return Some(actual[..prefix.len()] == prefix);
        }
        Some(actual == numeric(pattern)?)
    }
    let pattern = pattern.trim();
    if matches!(pattern, "*" | "x" | "X") && numeric(version).is_some() {
        return CompatibilityState::Compatible;
    }
    if pattern.contains("||") {
        let states: Vec<_> = pattern
            .split("||")
            .map(|part| assess_game_version(part.trim(), version))
            .collect();
        return if states.contains(&CompatibilityState::Compatible) {
            CompatibilityState::Compatible
        } else if states.contains(&CompatibilityState::Unknown) {
            CompatibilityState::Unknown
        } else {
            CompatibilityState::Incompatible
        };
    }
    let parts: Vec<_> = pattern.split_whitespace().collect();
    if parts.len() > 1 && parts.iter().all(|part| part.starts_with(['<', '>']))
    {
        let states: Vec<_> = parts
            .iter()
            .map(|part| assess_game_version(part, version))
            .collect();
        return if states.contains(&CompatibilityState::Incompatible) {
            CompatibilityState::Incompatible
        } else if states.contains(&CompatibilityState::Unknown) {
            CompatibilityState::Unknown
        } else {
            CompatibilityState::Compatible
        };
    }
    if let Some(actual) = numeric(version) {
        match evaluate(pattern, actual) {
            Some(true) => CompatibilityState::Compatible,
            Some(false) => CompatibilityState::Incompatible,
            None => CompatibilityState::Unknown,
        }
    } else if pattern == version && !pattern.is_empty() {
        CompatibilityState::Compatible
    } else {
        CompatibilityState::Unknown
    }
}

fn numeric_version(value: &str) -> Option<Vec<u64>> {
    let value = value.strip_prefix('v').unwrap_or(value);
    let parts: Option<Vec<_>> = value
        .split('.')
        .map(|part| {
            if part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
            {
                None
            } else {
                part.parse().ok()
            }
        })
        .collect();
    parts.filter(|parts| !parts.is_empty()).map(|mut parts| {
        while parts.len() > 1 && parts.last() == Some(&0) {
            parts.pop();
        }
        parts
    })
}

pub fn release_is_newer(
    target: &ResourceRelease,
    current: &ResourceRelease,
) -> bool {
    match (
        numeric_version(&target.version),
        numeric_version(&current.version),
    ) {
        (Some(mut target_version), Some(mut current_version)) => {
            let len = target_version.len().max(current_version.len());
            target_version.resize(len, 0);
            current_version.resize(len, 0);
            target_version > current_version
                || (target_version == current_version
                    && target.published_at > current.published_at)
        }
        _ => {
            matches!((target.published_at, current.published_at), (Some(target), Some(current)) if target > current)
        }
    }
}

pub fn release_is_known_lower(
    target: &ResourceRelease,
    current: &ResourceRelease,
) -> bool {
    if let (Some(mut target), Some(mut current)) = (
        numeric_version(&target.version),
        numeric_version(&current.version),
    ) {
        let len = target.len().max(current.len());
        target.resize(len, 0);
        current.resize(len, 0);
        target < current
    } else {
        false
    }
}

/// Use one ordering basis for the complete set, never pairwise mixed ordering.
pub fn select_automatic_release(
    mut releases: Vec<ResourceRelease>,
    preferred: &SourceId,
) -> Option<(ResourceRelease, ReleaseSelectionBasis)> {
    if releases.is_empty() {
        return None;
    }
    let versions: Option<Vec<_>> = releases
        .iter()
        .map(|release| numeric_version(&release.version))
        .collect();
    let comparable = versions.is_some();
    releases.sort_by(|a, b| {
        let order = if comparable {
            let mut av = numeric_version(&a.version).unwrap();
            let mut bv = numeric_version(&b.version).unwrap();
            let len = av.len().max(bv.len());
            av.resize(len, 0);
            bv.resize(len, 0);
            av.cmp(&bv)
        } else {
            a.published_at.cmp(&b.published_at)
        };
        order
            .then(a.published_at.cmp(&b.published_at))
            .then(a.reference.project.source.cmp(&b.reference.project.source))
            .then(a.reference.release_id.cmp(&b.reference.release_id))
    });
    let selected = releases.pop()?;
    let equivalent = releases.into_iter().rev().find(|release| {
        selected.reference.project.source != *preferred
            && release.reference.project.source == *preferred
            && !release_is_known_lower(release, &selected)
            && unique_primary(release)
                .zip(unique_primary(&selected))
                .is_some_and(|(a, b)| equal_bytes(&a.integrity, &b.integrity))
    });
    if let Some(equivalent) = equivalent {
        Some((equivalent, ReleaseSelectionBasis::EquivalentPreferredSource))
    } else {
        Some((
            selected,
            if comparable {
                ReleaseSelectionBasis::ComparableVersion
            } else {
                ReleaseSelectionBasis::PublishedAt
            },
        ))
    }
}

fn unique_primary(release: &ResourceRelease) -> Option<&ResourceArtifact> {
    let mut artifacts = release
        .artifacts
        .iter()
        .filter(|artifact| artifact.role == ArtifactRole::Primary);
    let primary = artifacts.next()?;
    artifacts.next().is_none().then_some(primary)
}

pub fn equal_bytes(
    left: &ArtifactIntegrity,
    right: &ArtifactIntegrity,
) -> bool {
    if left.size.is_none() || left.size != right.size {
        return false;
    }
    let mut shared = false;
    for a in &left.hashes {
        for b in right.hashes.iter().filter(|b| b.algorithm == a.algorithm) {
            if !a.value.eq_ignore_ascii_case(&b.value) {
                return false;
            }
            shared |= a.algorithm != HashAlgorithm::Md5;
        }
    }
    shared
}

/// Only required edges propagate activity; each optional resource is an explicit root.
pub fn active_dependency_nodes(
    roots: &[SelectionId],
    edges: &[ResourceDependencyEdge],
) -> HashSet<SelectionId> {
    let mut active: HashSet<_> = roots.iter().cloned().collect();
    let mut queue = roots.to_vec();
    while let Some(parent) = queue.pop() {
        for edge in edges.iter().filter(|edge| {
            edge.parent == parent
                && edge.kind == ResourceDependencyKind::Required
        }) {
            if active.insert(edge.child.clone()) {
                queue.push(edge.child.clone());
            }
        }
    }
    active
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(
        version: &str,
        source: SourceId,
        time: i64,
        hash: char,
    ) -> ResourceRelease {
        let reference = SourceReleaseRef {
            project: SourceProjectRef {
                source,
                project_id: SourceProjectId::new("abcdefgh").unwrap(),
            },
            release_id: SourceReleaseId::new(time.to_string()).unwrap(),
        };
        ResourceRelease {
            reference: reference.clone(),
            version: version.into(),
            channel: ReleaseChannel::Release,
            published_at: chrono::DateTime::from_timestamp(time, 0),
            compatibility: ReleaseCompatibility {
                game_versions: vec![],
                loaders: vec![],
            },
            artifacts: vec![ResourceArtifact {
                reference: SourceArtifactRef {
                    release: reference,
                    artifact_id: SourceArtifactId::new("primary").unwrap(),
                },
                file_name: Some("file.jar".into()),
                role: ArtifactRole::Primary,
                available: Some(true),
                integrity: ArtifactIntegrity {
                    size: Some(100),
                    hashes: vec![ContentHash {
                        algorithm: HashAlgorithm::Sha1,
                        value: hash.to_string().repeat(40),
                    }],
                },
                acquisition: ArtifactAcquisition::Unavailable,
                dependencies: vec![],
            }],
            equivalent_releases: vec![],
        }
    }

    #[test]
    fn shared_attachments_never_replace_the_selected_primary_release() {
        let mut old = release("2", SourceId::modrinth(), 1, 'a');
        let mut new = release("2", SourceId::curseforge(), 2, 'b');
        let mut attachment = old.artifacts[0].clone();
        attachment.role = ArtifactRole::Attachment;
        attachment.reference.artifact_id =
            SourceArtifactId::new("attachment").unwrap();
        old.artifacts.push(attachment.clone());
        attachment.reference.release = new.reference.clone();
        new.artifacts.push(attachment);
        assert_eq!(
            select_automatic_release(
                vec![old, new.clone()],
                &SourceId::modrinth()
            )
            .unwrap()
            .0
            .reference,
            new.reference
        );
    }

    #[test]
    fn preferred_source_only_replaces_equivalent_unique_primary_artifacts() {
        let oldest = release("2", SourceId::modrinth(), 1, 'a');
        let newest = release("2", SourceId::modrinth(), 2, 'a');
        let selected = release("2", SourceId::curseforge(), 3, 'a');
        let (result, basis) = select_automatic_release(
            vec![oldest, newest.clone(), selected.clone()],
            &SourceId::modrinth(),
        )
        .unwrap();
        assert_eq!(result.reference, newest.reference);
        assert_eq!(basis, ReleaseSelectionBasis::EquivalentPreferredSource);
        let lower = release("1", SourceId::modrinth(), 2, 'a');
        assert_eq!(
            select_automatic_release(
                vec![lower, selected.clone()],
                &SourceId::modrinth()
            )
            .unwrap()
            .0
            .reference,
            selected.reference
        );
        let mut ambiguous = newest;
        ambiguous.artifacts.push(ambiguous.artifacts[0].clone());
        assert_eq!(
            select_automatic_release(
                vec![ambiguous, selected.clone()],
                &SourceId::modrinth()
            )
            .unwrap()
            .0
            .reference,
            selected.reference
        );
    }

    #[test]
    fn preferred_source_does_not_replace_itself_with_an_older_release() {
        let old = release("1", SourceId::modrinth(), 1, 'a');
        let new = release("2", SourceId::modrinth(), 2, 'a');
        assert_eq!(
            select_automatic_release(
                vec![old, new.clone()],
                &SourceId::modrinth()
            )
            .unwrap()
            .0
            .reference,
            new.reference
        );
    }
    #[test]
    fn wildcard_preserves_major_and_minor_boundaries() {
        assert!(matches_game_version("1.20.x", "1.20.4"));
        assert!(!matches_game_version("1.20.x", "1.12"));
        assert!(!matches_game_version("1.20.x", "1.21"));
        assert!(matches_game_version("1.x", "1.21.1"));
        assert!(!matches_game_version("1.x", "2.0"));
        assert!(!matches_game_version("1.x.2", "1.20.2"));
        for wildcard in ["1.20.x", "1.20.X", "1.20.*"] {
            assert!(matches_game_version(wildcard, "1.20.0"));
            assert!(!matches_game_version(wildcard, "2.20.0"));
        }
    }

    #[test]
    fn release_constraints_keep_numeric_precision_and_reject_unknown_syntax() {
        for (constraint, version, expected) in [
            (">=1", "2", true),
            ("<3", "2", true),
            ("<3", "3", false),
            ("[1,3)", "2", true),
            ("1.x", "1.2.3", true),
            ("1.2.*", "1.3", false),
            ("2", "2.0.0", true),
            ("named release", "named release", true),
            (">=unknown", "2", false),
            ("1 || 2", "2", false),
        ] {
            assert_eq!(
                matches_release_constraint(constraint, version),
                expected,
                "{constraint}: {version}"
            );
        }
    }

    #[test]
    fn game_ranges_preserve_open_bounds_and_unknown_syntax() {
        assert_eq!(
            assess_game_version(">=1.20 <1.21", "1.20.4"),
            CompatibilityState::Compatible
        );
        assert_eq!(
            assess_game_version(">=1.20 <1.21", "1.21"),
            CompatibilityState::Incompatible
        );
        assert_eq!(
            assess_game_version("1.20.x || 1.21", "1.21"),
            CompatibilityState::Compatible
        );
        assert_eq!(
            assess_game_version("*", "1.21"),
            CompatibilityState::Compatible
        );
        for (range, version, expected) in [
            ("1.20", "1.20.0", true),
            ("<1.20", "1.19.4", true),
            ("<1.20", "1.20.0", false),
            (">1.20.9", "1.21.0", true),
            ("[1.20,1.21)", "1.20.0", true),
            ("[1.20,1.21)", "1.21.0", false),
            ("(1.20,1.21]", "1.20.0", false),
            ("(1.20,1.21]", "1.21.0", true),
            ("(,1.20]", "1.19.4", true),
            ("[1.20,)", "1.21.0", true),
            ("[1.21.1]", "1.21.1", true),
            ("[1.21.1]", "1.21.0", false),
            ("[1,)", "1.21.1", true),
            (">=1 <2", "1.21.1", true),
            ("[2,)", "1.21.1", false),
            ("(,1]", "1.21.1", false),
            ("[1.21,1.21.1)", "1.21.1", false),
            ("1.20 - 1.21", "1.21.0", true),
        ] {
            assert_eq!(
                matches_game_version(range, version),
                expected,
                "{range}: {version}"
            );
        }
        for range in ["supports 1.20.4", "1.20.x.1", ">=unknown", "[1.21,1.20]"]
        {
            assert_eq!(
                assess_game_version(range, "1.20.4"),
                CompatibilityState::Unknown
            );
        }
    }

    #[test]
    fn optional_edges_do_not_activate_until_the_optional_root_is_added() {
        let edge = ResourceDependencyEdge {
            release_selection: None,
            parent: SelectionId::new("a").unwrap(),
            child: SelectionId::new("b").unwrap(),
            declared_by: SourceArtifactRef {
                release: SourceReleaseRef {
                    project: SourceProjectRef {
                        source: SourceId::new("modrinth").unwrap(),
                        project_id: SourceProjectId::new("abcdefgh").unwrap(),
                    },
                    release_id: SourceReleaseId::new("release").unwrap(),
                },
                artifact_id: SourceArtifactId::new("file").unwrap(),
            },
            kind: ResourceDependencyKind::Required,
            version_constraint: None,
        };
        assert!(
            !active_dependency_nodes(
                &[SelectionId::new("root").unwrap()],
                &[edge.clone()]
            )
            .contains(&edge.child)
        );
        assert!(
            active_dependency_nodes(&[SelectionId::new("a").unwrap()], &[edge])
                .contains(&SelectionId::new("b").unwrap())
        );
    }

    #[test]
    fn target_paths_reject_traversal_and_preserve_scope() {
        let artifact = ResourceArtifact {
            reference: SourceArtifactRef {
                release: SourceReleaseRef {
                    project: SourceProjectRef {
                        source: SourceId::new("modrinth").unwrap(),
                        project_id: SourceProjectId::new("abcdefgh").unwrap(),
                    },
                    release_id: SourceReleaseId::new("release").unwrap(),
                },
                artifact_id: SourceArtifactId::new("file").unwrap(),
            },
            file_name: Some("example.jar".into()),
            role: ArtifactRole::Primary,
            available: Some(true),
            integrity: ArtifactIntegrity {
                size: Some(1),
                hashes: vec![],
            },
            acquisition: ArtifactAcquisition::Unavailable,
            dependencies: vec![],
        };
        assert_eq!(
            artifact_path(
                &InstallScope::Instance,
                ResourceType::Mod,
                &artifact
            )
            .unwrap(),
            "mods/example.jar"
        );
        assert!(!safe_relative_path("../escape.jar"));
    }
}
