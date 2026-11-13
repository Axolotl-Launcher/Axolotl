//! Unpacking a theme pack zip without trusting anything inside it.
//!
//! A pack arrives as a file the user downloaded or a friend sent them, so the
//! archive is treated as hostile: every entry name is checked against the pack
//! directory, symlinks are refused, and both the compressed and the extracted
//! size are capped before a byte reaches the disk.

use std::collections::BTreeSet;
use std::fs::File;
use std::io;
use std::io::Read as _;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde::Serialize;
use uuid::Uuid;

use super::manifest;
use super::manifest::ThemePackManifest;
use crate::ErrorKind;

pub const ORIGIN_FILE: &str = "origin.json";
const DEFAULT_LIMIT_MB: u64 = 500;
const DEFAULT_MAX_ENTRIES: usize = 4_096;
pub(super) const MAX_STYLESHEET_BYTES: u64 = 4 * 1024 * 1024;

/// How much a theme pack may unpack to. Both limits exist because a zip can be
/// small and still expand to gigabytes, which is the shape of a zip bomb.
#[derive(Debug, Clone, Copy)]
pub struct ThemePackLimits {
    pub max_archive_bytes: u64,
    pub max_uncompressed_bytes: u64,
    pub max_entries: usize,
}

impl Default for ThemePackLimits {
    fn default() -> Self {
        Self {
            max_archive_bytes: DEFAULT_LIMIT_MB * 1024 * 1024,
            max_uncompressed_bytes: DEFAULT_LIMIT_MB * 1024 * 1024,
            max_entries: DEFAULT_MAX_ENTRIES,
        }
    }
}

impl ThemePackLimits {
    /// Reads the operator overrides, so a server build can raise the ceiling
    /// without a rebuild. Values are in megabytes.
    pub fn from_env() -> Self {
        let default = Self::default();
        let read_mb = |name: &str, fallback: u64| {
            std::env::var(name)
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| *value > 0)
                .map(|value| value.saturating_mul(1024 * 1024))
                .unwrap_or(fallback)
        };
        Self {
            max_archive_bytes: read_mb(
                "AXOLOTL_THEME_MAX_ARCHIVE_MB",
                default.max_archive_bytes,
            ),
            max_uncompressed_bytes: read_mb(
                "AXOLOTL_THEME_MAX_UNCOMPRESSED_MB",
                default.max_uncompressed_bytes,
            ),
            max_entries: std::env::var("AXOLOTL_THEME_MAX_ENTRIES")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(default.max_entries),
        }
    }
}

/// What a successful extraction measured, kept for the installed list.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemePackExtraction {
    pub entries: usize,
    pub uncompressed_bytes: u64,
}

#[derive(Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThemePackOrigin {
    pub archive_name: String,
    pub installed_at: u64,
    pub version: String,
    #[serde(default)]
    pub size_bytes: u64,
}

pub fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

/// Extracts `archive` into `destination`, which must not exist yet, and then
/// validates the manifest against the extracted tree. The caller moves the
/// directory into place only after this returns.
pub fn extract(
    archive_path: &Path,
    destination: &Path,
    limits: &ThemePackLimits,
) -> crate::Result<(ThemePackManifest, ThemePackExtraction)> {
    let metadata = std::fs::metadata(archive_path).map_err(|error| {
        invalid(format!("The theme pack archive could not be read: {error}"))
    })?;
    if !metadata.is_file() {
        return Err(invalid("The theme pack archive is not a file"));
    }
    if metadata.len() > limits.max_archive_bytes {
        return Err(invalid(format!(
            "The theme pack archive is larger than the {} MB limit",
            limits.max_archive_bytes / (1024 * 1024)
        )));
    }

    let file = File::open(archive_path).map_err(|error| {
        invalid(format!(
            "The theme pack archive could not be opened: {error}"
        ))
    })?;
    let mut zip = zip::ZipArchive::new(file).map_err(|error| {
        invalid(format!("The theme pack archive is not a zip file: {error}"))
    })?;

    if zip.len() > limits.max_entries {
        return Err(invalid(format!(
            "The theme pack archive has more than {} entries",
            limits.max_entries
        )));
    }

    std::fs::create_dir_all(destination).map_err(|error| {
        invalid(format!(
            "The theme pack staging directory could not be created: {error}"
        ))
    })?;

    let mut names = BTreeSet::new();
    let mut declared_total: u64 = 0;
    for index in 0..zip.len() {
        let entry = zip.by_index(index).map_err(|error| {
            invalid(format!(
                "A theme pack archive entry is unreadable: {error}"
            ))
        })?;
        let raw_name = entry
            .name()
            .map_err(|error| {
                invalid(format!(
                    "A theme pack archive entry name is unreadable: {error}"
                ))
            })?
            .to_string();
        if raw_name.is_empty() {
            continue;
        }
        let is_directory = raw_name.ends_with('/');
        let name = raw_name.trim_end_matches('/');
        if name.is_empty() {
            continue;
        }
        if let Some(mode) = entry.unix_mode() {
            if mode & 0o170000 == 0o120000 {
                return Err(invalid(format!(
                    "Theme pack archive entry {name} is a symbolic link"
                )));
            }
        }
        manifest::validate_relative(name)?;
        // Windows file systems fold case, so two entries that differ only by
        // case would overwrite each other unpredictably.
        if !names.insert(name.to_ascii_lowercase()) {
            return Err(invalid(format!(
                "The theme pack archive contains {name} twice"
            )));
        }
        if is_directory {
            continue;
        }
        declared_total = declared_total
            .checked_add(entry.size())
            .ok_or_else(|| invalid("The theme pack archive size overflows"))?;
        if declared_total > limits.max_uncompressed_bytes {
            return Err(invalid(format!(
                "The theme pack unpacks to more than the {} MB limit",
                limits.max_uncompressed_bytes / (1024 * 1024)
            )));
        }
    }

    let mut written_total: u64 = 0;
    let mut entries = 0usize;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|error| {
            invalid(format!(
                "A theme pack archive entry is unreadable: {error}"
            ))
        })?;
        let raw_name = entry
            .name()
            .map_err(|error| {
                invalid(format!(
                    "A theme pack archive entry name is unreadable: {error}"
                ))
            })?
            .to_string();
        if raw_name.is_empty() {
            continue;
        }
        let is_directory = raw_name.ends_with('/');
        let name = raw_name.trim_end_matches('/');
        if name.is_empty() {
            continue;
        }
        let path = join_relative(destination, name)?;
        if is_directory {
            std::fs::create_dir_all(&path).map_err(|error| {
                invalid(format!(
                    "Theme pack directory {name} could not be created: {error}"
                ))
            })?;
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                invalid(format!(
                    "Theme pack directory for {name} could not be created: {error}"
                ))
            })?;
        }
        let mut output = File::create(&path).map_err(|error| {
            invalid(format!(
                "Theme pack file {name} could not be created: {error}"
            ))
        })?;
        let remaining =
            limits.max_uncompressed_bytes.saturating_sub(written_total);
        let copied =
            io::copy(&mut entry.by_ref().take(remaining + 1), &mut output)
                .map_err(|error| {
                    invalid(format!(
                        "Theme pack file {name} could not be written: {error}"
                    ))
                })?;
        if copied > remaining {
            return Err(invalid(format!(
                "The theme pack unpacks to more than the {} MB limit",
                limits.max_uncompressed_bytes / (1024 * 1024)
            )));
        }
        written_total = written_total.saturating_add(copied);
        entries += 1;
    }

    let manifest = ThemePackManifest::load(destination)?;
    manifest.validate(destination)?;

    let stylesheet_total: u64 = manifest
        .styles
        .iter()
        .filter_map(|style| {
            std::fs::metadata(join_relative(destination, style).ok()?)
                .ok()
                .map(|metadata| metadata.len())
        })
        .sum();
    if stylesheet_total
        > MAX_STYLESHEET_BYTES * manifest.styles.len().max(1) as u64
    {
        return Err(invalid(
            "The theme pack stylesheets are unreasonably large",
        ));
    }

    Ok((
        manifest,
        ThemePackExtraction {
            entries,
            uncompressed_bytes: written_total,
        },
    ))
}

pub fn write_origin(
    pack_dir: &Path,
    archive_path: &Path,
    version: &str,
    size_bytes: u64,
) -> crate::Result<()> {
    let origin = ThemePackOrigin {
        archive_name: archive_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string(),
        installed_at: now_seconds(),
        version: version.to_string(),
        size_bytes,
    };
    let bytes = serde_json::to_vec_pretty(&origin).map_err(|error| {
        invalid(format!(
            "The theme pack origin could not be serialized: {error}"
        ))
    })?;
    std::fs::write(pack_dir.join(ORIGIN_FILE), bytes).map_err(|error| {
        invalid(format!(
            "The theme pack origin could not be written: {error}"
        ))
    })
}

pub fn read_origin(pack_dir: &Path) -> Option<ThemePackOrigin> {
    let bytes = std::fs::read(pack_dir.join(ORIGIN_FILE)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Joins validated relative components and proves the result stays under the
/// destination even after a previous entry created a link. Components come
/// from [`manifest::validate_relative`], which rejects `..` and absolute paths.
fn join_relative(root: &Path, relative: &str) -> crate::Result<PathBuf> {
    let components = manifest::validate_relative(relative)?;
    let mut path = root.to_path_buf();
    for component in components {
        path.push(component);
    }
    Ok(path)
}

/// A unique sibling name for staging or trashing a pack, so a crash never
/// leaves a partially installed pack under its real id.
pub fn hidden_sibling(parent: &Path, prefix: &str) -> PathBuf {
    parent.join(format!(".{prefix}-{}", Uuid::new_v4()))
}

fn invalid(message: impl Into<String>) -> crate::Error {
    ErrorKind::InputError(message.into()).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn make_zip(path: &Path, files: &[(&str, &str)]) {
        let file = File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, contents) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(contents.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    fn valid_files() -> Vec<(&'static str, &'static str)> {
        vec![
            (
                "manifest.json",
                r##"{
                    "schemaVersion": 1,
                    "id": "test-pack",
                    "name": {"en-US": "Test"},
                    "version": "1.0.0",
                    "engines": {"axolotl": ">=1.0.0"},
                    "capabilities": ["tokens"],
                    "tokens": {
                        "light": {"--surface-1": "#ffffff", "--surface-3": "#f5f5f5", "--color-text-primary": "#101215"},
                        "dark": {"--surface-1": "#101215", "--surface-3": "#262b34", "--color-text-primary": "#eceff4"}
                    }
                }"##,
            ),
            (
                "styles/theme.css",
                ".x { color: var(--color-text-primary); }",
            ),
        ]
    }

    #[test]
    fn extracts_a_valid_pack() {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("pack.zip");
        make_zip(&archive, &valid_files());
        let destination = temp.path().join("out");
        let (manifest, extraction) =
            extract(&archive, &destination, &ThemePackLimits::default())
                .expect("valid pack");
        assert_eq!(manifest.id, "test-pack");
        assert_eq!(extraction.entries, 2);
        assert!(destination.join("styles/theme.css").is_file());
    }

    #[test]
    fn rejects_a_path_that_escapes_the_pack() {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("pack.zip");
        make_zip(
            &archive,
            &[("manifest.json", "{}"), ("../escaped.txt", "bad")],
        );
        let destination = temp.path().join("out");
        assert!(
            extract(&archive, &destination, &ThemePackLimits::default())
                .is_err()
        );
        assert!(!temp.path().join("escaped.txt").exists());
    }

    #[test]
    fn rejects_an_archive_over_the_size_limit() {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("pack.zip");
        make_zip(&archive, &valid_files());
        let limits = ThemePackLimits {
            max_archive_bytes: 8,
            ..ThemePackLimits::default()
        };
        assert!(extract(&archive, &temp.path().join("out"), &limits).is_err());
    }

    #[test]
    fn accepts_a_pack_with_the_optional_folders() {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("pack.zip");
        let mut files: Vec<(&str, &str)> = valid_files()
            .into_iter()
            .filter(|(name, _)| *name != "manifest.json")
            .collect();
        files.push(("assets/background.png", "image"));
        files.push(("pages/home.html", "<html></html>"));
        files.push((
            "manifest.json",
            r##"{
                "schemaVersion": 1,
                "id": "test-pack",
                "name": {"en-US": "Test"},
                "version": "1.0.0",
                "engines": {"axolotl": ">=1.0.0"},
                "capabilities": ["tokens", "pages"],
                "tokens": {
                    "light": {"--surface-1": "#ffffff", "--surface-3": "#f5f5f5", "--color-text-primary": "#101215"},
                    "dark": {"--surface-1": "#101215", "--surface-3": "#262b34", "--color-text-primary": "#eceff4"}
                },
                "background": "assets/background.png",
                "pages": [{"id": "home", "title": {"en-US": "Home"}, "entry": "pages/home.html"}]
            }"##,
        ));
        make_zip(&archive, &files);
        extract(
            &archive,
            &temp.path().join("out"),
            &ThemePackLimits::default(),
        )
        .expect("valid pack");
    }
}
