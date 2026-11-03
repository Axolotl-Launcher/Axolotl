use super::CompatibilityState;
use std::cmp::Ordering;

/// Dependency checks reject only declarations proven not to match. Callers
/// needing to distinguish unsupported syntax must retain the assessment.
pub fn matches_release_constraint(pattern: &str, version: &str) -> bool {
    assess_release_constraint(pattern, version)
        != CompatibilityState::Incompatible
}

/// Assess Mod release constraints without interpreting arbitrary qualifiers as
/// numeric versions. Numeric releases and valid SemVer are comparable; other
/// release labels remain unknown unless they match exactly.
pub fn assess_release_constraint(
    pattern: &str,
    version: &str,
) -> CompatibilityState {
    let pattern = pattern.trim();
    let version = version.trim();
    if pattern.is_empty() || version.is_empty() {
        return CompatibilityState::Unknown;
    }
    if pattern == version || matches!(pattern, "*" | "x" | "X") {
        return CompatibilityState::Compatible;
    }
    if pattern.contains("||") {
        return any(pattern
            .split("||")
            .map(|part| assess_release_constraint(part, version)));
    }
    if pattern.starts_with(['[', '(']) {
        return assess_intervals(pattern, version);
    }
    if let Some((lower, upper)) = pattern.split_once(" - ") {
        return assess_bounds(lower, true, upper, true, version);
    }
    let atoms = split_conjunction(pattern);
    if atoms.len() > 1 {
        if !atoms
            .iter()
            .all(|atom| atom.starts_with(['<', '>', '=', '!', '~', '^']))
        {
            return CompatibilityState::Unknown;
        }
        return all(atoms
            .iter()
            .map(|atom| assess_release_constraint(atom, version)));
    }
    for operator in [">=", "<=", "!=", ">", "<", "="] {
        if let Some(bound) = pattern.strip_prefix(operator) {
            return compare(version, bound.trim())
                .map(|order| match operator {
                    ">=" => order != Ordering::Less,
                    "<=" => order != Ordering::Greater,
                    ">" => order == Ordering::Greater,
                    "<" => order == Ordering::Less,
                    "!=" => order != Ordering::Equal,
                    _ => order == Ordering::Equal,
                })
                .map_or(CompatibilityState::Unknown, state);
        }
    }
    if pattern.starts_with(['~', '^'])
        || pattern
            .split('.')
            .any(|part| matches!(part, "*" | "x" | "X"))
    {
        let pattern = pattern
            .split('.')
            .map(|part| if part == "X" { "x" } else { part })
            .collect::<Vec<_>>()
            .join(".");
        return semver::VersionReq::parse(&pattern)
            .ok()
            .zip(parse_semver(version))
            .map(|(requirement, actual)| state(requirement.matches(&actual)))
            .unwrap_or(CompatibilityState::Unknown);
    }
    compare(version, pattern)
        .map(|order| state(order == Ordering::Equal))
        .unwrap_or(CompatibilityState::Unknown)
}

fn state(matches: bool) -> CompatibilityState {
    if matches {
        CompatibilityState::Compatible
    } else {
        CompatibilityState::Incompatible
    }
}

fn any(states: impl Iterator<Item = CompatibilityState>) -> CompatibilityState {
    let mut result = CompatibilityState::Incompatible;
    for state in states {
        match state {
            CompatibilityState::Compatible => return state,
            CompatibilityState::Unknown => result = state,
            CompatibilityState::Incompatible => {}
        }
    }
    result
}

fn all(states: impl Iterator<Item = CompatibilityState>) -> CompatibilityState {
    let mut result = CompatibilityState::Compatible;
    for state in states {
        match state {
            CompatibilityState::Incompatible => return state,
            CompatibilityState::Unknown => result = state,
            CompatibilityState::Compatible => {}
        }
    }
    result
}

fn split_conjunction(pattern: &str) -> Vec<String> {
    let mut atoms: Vec<String> = Vec::new();
    for token in pattern
        .split(|character: char| character == ',' || character.is_whitespace())
    {
        if token.is_empty() {
            continue;
        }
        if let Some(previous) = atoms.last_mut()
            && matches!(
                previous.as_str(),
                ">=" | "<=" | ">" | "<" | "=" | "!=" | "~" | "^"
            )
        {
            previous.push_str(token);
        } else {
            atoms.push(token.to_string());
        }
    }
    atoms
}

fn assess_intervals(pattern: &str, version: &str) -> CompatibilityState {
    let mut remaining = pattern;
    let mut intervals = Vec::new();
    loop {
        if !remaining.starts_with(['[', '(']) {
            return CompatibilityState::Unknown;
        }
        let Some(end) = remaining.find([']', ')']) else {
            return CompatibilityState::Unknown;
        };
        let interval = &remaining[..=end];
        let inner = &interval[1..end];
        let result = if let Some((lower, upper)) = inner.split_once(',') {
            if upper.contains(',') {
                return CompatibilityState::Unknown;
            }
            assess_bounds(
                lower,
                interval.starts_with('['),
                upper,
                interval.ends_with(']'),
                version,
            )
        } else if interval.starts_with('[') && interval.ends_with(']') {
            let exact = inner.trim();
            if exact.is_empty() {
                return CompatibilityState::Unknown;
            }
            if exact == version {
                CompatibilityState::Compatible
            } else {
                compare(version, exact)
                    .map(|order| state(order == Ordering::Equal))
                    .unwrap_or(CompatibilityState::Unknown)
            }
        } else {
            return CompatibilityState::Unknown;
        };
        intervals.push(result);
        remaining = remaining[end + 1..].trim();
        if remaining.is_empty() {
            return any(intervals.into_iter());
        }
        let Some(next) = remaining.strip_prefix(',') else {
            return CompatibilityState::Unknown;
        };
        remaining = next.trim();
    }
}

fn assess_bounds(
    lower: &str,
    lower_inclusive: bool,
    upper: &str,
    upper_inclusive: bool,
    version: &str,
) -> CompatibilityState {
    let lower = lower.trim();
    let upper = upper.trim();
    if lower.is_empty() && upper.is_empty()
        || matches!(compare(lower, upper), Some(Ordering::Greater))
    {
        return CompatibilityState::Unknown;
    }
    let lower_state = if lower.is_empty() {
        CompatibilityState::Compatible
    } else {
        compare(version, lower)
            .map(|order| {
                state(
                    order == Ordering::Greater
                        || lower_inclusive && order == Ordering::Equal,
                )
            })
            .unwrap_or(CompatibilityState::Unknown)
    };
    let upper_state = if upper.is_empty() {
        CompatibilityState::Compatible
    } else {
        compare(version, upper)
            .map(|order| {
                state(
                    order == Ordering::Less
                        || upper_inclusive && order == Ordering::Equal,
                )
            })
            .unwrap_or(CompatibilityState::Unknown)
    };
    all([lower_state, upper_state].into_iter())
}

fn numeric(value: &str) -> Option<Vec<u64>> {
    let value = value.strip_prefix('v').unwrap_or(value);
    value
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
        .collect()
}

fn parse_semver(value: &str) -> Option<semver::Version> {
    let value = value.strip_prefix('v').unwrap_or(value);
    if let Ok(version) = semver::Version::parse(value) {
        return Some(version);
    }
    let end = value.find(['-', '+']).unwrap_or(value.len());
    let mut core = numeric(&value[..end])?;
    if core.len() > 3 {
        return None;
    }
    core.resize(3, 0);
    let normalized =
        format!("{}.{}.{}{}", core[0], core[1], core[2], &value[end..]);
    semver::Version::parse(&normalized).ok()
}

fn compare(left: &str, right: &str) -> Option<Ordering> {
    if let (Some(mut left), Some(mut right)) = (numeric(left), numeric(right)) {
        let length = left.len().max(right.len());
        left.resize(length, 0);
        right.resize(length, 0);
        Some(left.cmp(&right))
    } else {
        // SemVer build metadata does not affect precedence. Arbitrary Maven
        // qualifiers are not stripped or assigned an invented ordering.
        Some(parse_semver(left)?.cmp_precedence(&parse_semver(right)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sodium_build_metadata_satisfies_installed_mod_requirements() {
        for pattern in [
            "[0.6.9,)",
            "[0.8.12,)",
            "[0.8.13+mc1.21.1]",
            "[0.6.0,)",
            "[0,)",
        ] {
            assert_eq!(
                assess_release_constraint(pattern, "0.8.13+mc1.21.1"),
                CompatibilityState::Compatible,
                "{pattern}"
            );
        }
        assert_eq!(
            assess_release_constraint("[0.16.2,)", "0.16.3+mc1.21.1"),
            CompatibilityState::Compatible
        );
        assert_eq!(
            assess_release_constraint("[0.16.3]", "0.16.3"),
            CompatibilityState::Compatible
        );
    }

    #[test]
    fn ranges_keep_proven_conflicts_and_semver_precedence() {
        for (pattern, version, expected) in [
            ("[0.8.14,)", "0.8.13+mc1.21.1", false),
            ("[0.8.13]", "0.8.12+mc1.21.1", false),
            ("(0.8.13,)", "0.8.13+mc1.21.1", false),
            ("(,0.8.13]", "0.8.13+mc1.21.1", true),
            ("[0.8.13,)", "0.8.13-beta.2", false),
            ("[0.8.13-beta.2,)", "0.8.13-beta.10+mc1.21.1", true),
            ("[0.8.13-beta.10,)", "0.8.13-beta.2", false),
            ("^0.8.0", "0.8.13+mc1.21.1", true),
            ("~0.8.0", "0.9.0+mc1.21.1", false),
            ("0.8.x", "0.8.13+mc1.21.1", true),
            ("0.8.X", "0.8.13+mc1.21.1", true),
            ("0.8.X", "0.9.0+mc1.21.1", false),
            ("1.20.X", "1.21.0", false),
            ("^0.8.0", "0.8.13-beta.2", false),
            ("[1.2.3.4,)", "1.2.3.5", true),
            ("[1.2.3.4,)", "1.2.3.3", false),
        ] {
            assert_eq!(
                assess_release_constraint(pattern, version),
                state(expected),
                "{pattern}: {version}"
            );
        }
    }

    #[test]
    fn compound_constraints_preserve_unknown_states() {
        for (pattern, version, expected) in [
            (
                "<0.8 || >=0.8.13",
                "0.8.13+mc1.21.1",
                CompatibilityState::Compatible,
            ),
            (
                ">=0.8.13 <0.9",
                "0.8.13+mc1.21.1",
                CompatibilityState::Compatible,
            ),
            (
                ">= 0.8.13, < 0.9",
                "0.8.12+mc1.21.1",
                CompatibilityState::Incompatible,
            ),
            (
                "(,0.8.12],[0.8.14,)",
                "0.8.13+mc1.21.1",
                CompatibilityState::Incompatible,
            ),
            (
                "(,0.8.12], [0.8.13,)",
                "0.8.13+mc1.21.1",
                CompatibilityState::Compatible,
            ),
            (
                ">=unknown || [0.8.14,)",
                "0.8.13+mc1.21.1",
                CompatibilityState::Unknown,
            ),
            (
                ">=unknown <0.8.13",
                "0.8.13+mc1.21.1",
                CompatibilityState::Incompatible,
            ),
            (
                ">=unknown <0.9",
                "0.8.13+mc1.21.1",
                CompatibilityState::Unknown,
            ),
            (
                "[1.0,2.0)",
                "custom-sodium-build",
                CompatibilityState::Unknown,
            ),
            ("[1.0,2.0)", "1.0.0.0-neoforge", CompatibilityState::Unknown),
            ("[2,1]", "1.5", CompatibilityState::Unknown),
            ("supports 1.20.4", "1.21", CompatibilityState::Unknown),
            ("named release 2", "1", CompatibilityState::Unknown),
            ("1.2 2.0", "1.3", CompatibilityState::Unknown),
            (">=1.2 2.0", "1.3", CompatibilityState::Unknown),
            ("[1,)", "", CompatibilityState::Unknown),
            (
                "[custom-build]",
                "custom-build",
                CompatibilityState::Compatible,
            ),
        ] {
            assert_eq!(
                assess_release_constraint(pattern, version),
                expected,
                "{pattern}: {version}"
            );
            assert_eq!(
                matches_release_constraint(pattern, version),
                expected != CompatibilityState::Incompatible
            );
        }
    }
}
