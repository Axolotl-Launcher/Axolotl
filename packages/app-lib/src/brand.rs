pub const PRODUCT_NAME: &str = "Axolotl Launcher";
pub const SHORT_PRODUCT_NAME: &str = "Axolotl";
pub const WEBSITE: &str = "https://www.ghs.red";
pub const BUNDLE_IDENTIFIER: &str = "red.ghs.axolotl";
pub const DEEP_LINK_SCHEME: &str = "axolotl";

/// Longest accepted data directory suffix; enough to name a branch or a build.
const MAX_DATA_DIR_SUFFIX_LEN: usize = 32;

pub fn user_agent(version: &str, os: &str) -> String {
    format!("Axolotl-Launcher/{version} ({os})")
}

/// Name of the directory the launcher keeps its settings, databases and logs in.
///
/// A build can opt into its own directory by setting `AXOLOTL_DATA_DIR_SUFFIX`
/// at build time (see `build.rs`). Local and test builds use this to stay away
/// from the database of an installed launcher: the two otherwise share one
/// directory, and a database touched by a newer build can no longer be opened
/// by an older one. Releases leave the variable unset, so they resolve to the
/// plain identifier.
pub fn app_data_dir_identifier(app_identifier: &str) -> String {
    data_dir_identifier(app_identifier, option_env!("AXOLOTL_DATA_DIR_SUFFIX"))
}

/// Appends a sanitized suffix to the identifier.
///
/// The result becomes a directory name, so anything that could escape the data
/// directory (path separators, drive letters, traversal) is dropped rather than
/// escaped. A suffix that leaves nothing usable behind is ignored entirely -
/// `build.rs` refuses such a value before it can reach a build, so this fallback
/// is only reachable from a direct call.
fn data_dir_identifier(base: &str, suffix: Option<&str>) -> String {
    let Some(suffix) = suffix else {
        return base.to_string();
    };

    let filtered: String = suffix
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_' | '.')
        })
        .collect();

    // Trim on both sides of the length cap. A suffix of dots alone would
    // otherwise spend the whole cap and silently drop what follows it.
    let trimmed = filtered.trim_matches(['.', '-']);
    let capped = &trimmed[..trimmed.len().min(MAX_DATA_DIR_SUFFIX_LEN)];
    let sanitized = capped.trim_matches(['.', '-']);

    if sanitized.is_empty() {
        base.to_string()
    } else {
        format!("{base}-{sanitized}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_agent_is_unique_and_contains_no_contact_information() {
        let user_agent = user_agent("1.2.3", "windows");

        assert_eq!(user_agent, "Axolotl-Launcher/1.2.3 (windows)");
        assert!(!user_agent.contains("ghs.red"));
        assert!(!user_agent.contains("http"));
        assert!(!user_agent.contains('@'));
    }

    #[test]
    fn data_dir_identifier_without_suffix_keeps_the_identifier() {
        assert_eq!(
            data_dir_identifier("red.ghs.axolotl", None),
            "red.ghs.axolotl"
        );
    }

    #[test]
    fn data_dir_identifier_appends_a_readable_suffix() {
        for (suffix, expected) in [
            ("pr538", "red.ghs.axolotl-pr538"),
            ("fix_issue_538", "red.ghs.axolotl-fix_issue_538"),
            ("v1.2.3", "red.ghs.axolotl-v1.2.3"),
        ] {
            assert_eq!(
                data_dir_identifier("red.ghs.axolotl", Some(suffix)),
                expected,
                "unexpected identifier for suffix {suffix:?}"
            );
        }
    }

    #[test]
    fn data_dir_identifier_never_escapes_the_data_directory() {
        for suffix in [
            "../evil",
            "..\\evil",
            "a/b",
            "a\\b",
            "..",
            "/",
            "\\",
            "C:\\Users",
            "\\\\server\\share",
            ".....",
            "..-.-",
        ] {
            let identifier =
                data_dir_identifier("red.ghs.axolotl", Some(suffix));

            assert!(
                !identifier.contains(['/', '\\', ':']),
                "suffix {suffix:?} produced a path separator or drive letter in {identifier:?}"
            );
            assert!(
                !identifier.ends_with(['.', '-']),
                "suffix {suffix:?} left a trailing dot or dash in {identifier:?}, which Windows would strip"
            );

            let remainder = identifier
                .strip_prefix("red.ghs.axolotl-")
                .map_or(identifier.as_str(), |remainder| remainder);
            assert!(
                !remainder.is_empty() && remainder != "." && remainder != "..",
                "suffix {suffix:?} produced the bare component {remainder:?}"
            );
        }
    }

    #[test]
    fn data_dir_identifier_keeps_the_prefix_so_device_names_cannot_win() {
        // Windows treats a path component as a device when the whole name is
        // one. The identifier always carries the prefix, so it never is.
        for suffix in ["CON", "NUL", "PRN", "AUX", "COM1", "LPT1"] {
            assert_eq!(
                data_dir_identifier("red.ghs.axolotl", Some(suffix)),
                format!("red.ghs.axolotl-{suffix}")
            );
        }
    }

    #[test]
    fn data_dir_identifier_drops_characters_that_are_unsafe_in_a_path() {
        for (suffix, expected) in [
            ("a b", "red.ghs.axolotl-ab"),
            ("a:b", "red.ghs.axolotl-ab"),
            // Fullwidth solidus: not a separator the filesystem honours, but no
            // more welcome in a directory name than any other non-ASCII.
            ("a\u{ff0f}b", "red.ghs.axolotl-ab"),
            // Every non-ASCII character goes, so a suffix typed in another
            // script keeps only its ASCII part - which may be nothing at all.
            ("版本v2", "red.ghs.axolotl-v2"),
            ("版本", "red.ghs.axolotl"),
        ] {
            assert_eq!(
                data_dir_identifier("red.ghs.axolotl", Some(suffix)),
                expected,
                "unexpected identifier for suffix {suffix:?}"
            );
        }
    }

    #[test]
    fn data_dir_identifier_trims_before_capping_the_length() {
        // Capping first would let a run of dots spend the whole allowance and
        // drop the characters after it, so the suffix would vanish silently.
        let suffix = format!("{}abc", ".".repeat(40));

        assert_eq!(
            data_dir_identifier("red.ghs.axolotl", Some(&suffix)),
            "red.ghs.axolotl-abc"
        );
    }

    #[test]
    fn data_dir_identifier_ignores_unusable_suffixes() {
        for suffix in ["", "   ", "..", "--", "///", "..-.-"] {
            assert_eq!(
                data_dir_identifier("red.ghs.axolotl", Some(suffix)),
                "red.ghs.axolotl",
                "suffix {suffix:?} should have been ignored"
            );
        }
    }

    #[test]
    fn data_dir_identifier_bounds_the_suffix_length() {
        let identifier =
            data_dir_identifier("red.ghs.axolotl", Some(&"a".repeat(200)));

        assert_eq!(
            identifier,
            format!("red.ghs.axolotl-{}", "a".repeat(MAX_DATA_DIR_SUFFIX_LEN))
        );
    }
}
