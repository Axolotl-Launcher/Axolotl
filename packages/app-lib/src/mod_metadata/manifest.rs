//! JAR manifest (META-INF/MANIFEST.MF) reader.
//!
//! Extracts metadata entries from a JAR file's manifest without requiring
//! full mod metadata parsing. Used by the drop classifier to identify
//! launcher JARs (e.g. HMCL).

use std::io::Read;
use std::path::Path;

/// Key-value pairs extracted from a JAR's META-INF/MANIFEST.MF.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JarManifest {
    /// Value of `Main-Class` attribute.
    pub main_class: Option<String>,
    /// Value of `Implementation-Title` attribute.
    pub implementation_title: Option<String>,
    /// Value of `Implementation-Version` attribute.
    pub implementation_version: Option<String>,
}

/// Read and parse the `META-INF/MANIFEST.MF` from a JAR (ZIP) file.
///
/// Returns `Some(JarManifest)` when the file is a valid ZIP containing
/// `META-INF/MANIFEST.MF` with at least one recognized attribute.
/// Returns `None` if the file cannot be opened, is not a ZIP, does not
/// contain the manifest entry, or parsing fails entirely.
pub fn read_jar_manifest(path: &Path) -> Option<JarManifest> {
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;

    let mut entry = archive.by_name("META-INF/MANIFEST.MF").ok()?;
    let mut content = String::new();
    entry.read_to_string(&mut content).ok()?;

    Some(parse_manifest(&content))
}

/// Read and parse `META-INF/MANIFEST.MF` from an already-opened ZIP archive.
///
/// Used by mod metadata extraction to resolve Gradle placeholders such as
/// `${file.jarVersion}` in Forge `mods.toml` files.
pub(crate) fn archive_manifest<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Option<JarManifest> {
    let mut entry = archive.by_name("META-INF/MANIFEST.MF").ok()?;
    let mut content = String::new();
    entry.read_to_string(&mut content).ok()?;

    Some(parse_manifest(&content))
}

/// Parse raw MANIFEST.MF text into a `JarManifest`.
///
/// Handles continuation lines (lines starting with a space) and
/// case-insensitive attribute names per the JAR specification.
fn parse_manifest(content: &str) -> JarManifest {
    let mut manifest = JarManifest::default();

    // First pass: normalize continuation lines.
    // Lines starting with a space or tab are continuations of the previous line.
    let mut lines: Vec<String> = Vec::new();
    for line in content.lines() {
        if line.is_empty() {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            // Continuation line — append to the last line with a single space separator.
            if let Some(last) = lines.last_mut() {
                last.push(' ');
                last.push_str(line.trim());
            }
        } else {
            lines.push(line.to_string());
        }
    }

    for line in &lines {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();

            // Case-insensitive matching per JAR spec.
            match key.to_ascii_lowercase().as_str() {
                "main-class" => manifest.main_class = Some(value.to_string()),
                "implementation-title" => {
                    manifest.implementation_title = Some(value.to_string());
                }
                "implementation-version" => {
                    manifest.implementation_version = Some(value.to_string());
                }
                _ => {}
            }
        }
    }

    manifest
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    /// Helper: create a minimal JAR (ZIP) with an optional MANIFEST.MF.
    fn create_test_jar(
        manifest_content: Option<&str>,
    ) -> (std::path::PathBuf, tempfile::TempDir) {
        let dir = tempdir().expect("temp dir");
        let jar_path = dir.path().join("test.jar");

        let file = std::fs::File::create(&jar_path).expect("create jar");
        let mut zip = zip::ZipWriter::new(file);

        if let Some(content) = manifest_content {
            zip.start_file(
                "META-INF/MANIFEST.MF",
                zip::write::FileOptions::<()>::default(),
            )
            .expect("start manifest entry");
            zip.write_all(content.as_bytes()).expect("write manifest");
        }

        zip.finish().expect("finish zip");
        (jar_path, dir)
    }

    #[test]
    fn test_hmcl_manifest() {
        let content =
            "Manifest-Version: 1.0\nMain-Class: org.jackhuang.hmcl.Main\n";
        let (path, _dir) = create_test_jar(Some(content));

        let manifest = read_jar_manifest(&path).expect("should read manifest");
        assert_eq!(
            manifest.main_class,
            Some("org.jackhuang.hmcl.Main".to_string())
        );
    }

    #[test]
    fn test_no_manifest() {
        let (path, _dir) = create_test_jar(None);
        let manifest = read_jar_manifest(&path);
        assert!(manifest.is_none(), "no manifest should return None");
    }

    #[test]
    fn test_non_jar_file() {
        let dir = tempdir().expect("temp dir");
        let not_a_jar = dir.path().join("not_a_jar.txt");
        std::fs::write(&not_a_jar, "this is not a zip").expect("write file");

        let manifest = read_jar_manifest(&not_a_jar);
        assert!(manifest.is_none(), "non-zip should return None");
    }

    #[test]
    fn test_nonexistent_file() {
        let manifest =
            read_jar_manifest(Path::new("/tmp/nonexistent_file_xyz.jar"));
        assert!(manifest.is_none(), "nonexistent file should return None");
    }

    #[test]
    fn test_continuation_lines() {
        let content = "Manifest-Version: 1.0\nImplementation-Title: Hello\n World\nMain-Class: Test\n";
        let (path, _dir) = create_test_jar(Some(content));

        let manifest = read_jar_manifest(&path).expect("should read manifest");
        assert_eq!(
            manifest.implementation_title,
            Some("Hello World".to_string())
        );
        assert_eq!(manifest.main_class, Some("Test".to_string()));
    }

    #[test]
    fn test_case_insensitive_attributes() {
        let content = "manifest-version: 1.0\nmain-class: com.example.Main\nimplementation-title: MyApp\n";
        let (path, _dir) = create_test_jar(Some(content));

        let manifest = read_jar_manifest(&path).expect("should read manifest");
        assert_eq!(manifest.main_class, Some("com.example.Main".to_string()));
        assert_eq!(manifest.implementation_title, Some("MyApp".to_string()));
    }

    #[test]
    fn test_empty_jar() {
        // A valid empty ZIP (no entries) should return None since there's no MANIFEST.MF.
        let dir = tempdir().expect("temp dir");
        let jar_path = dir.path().join("empty.jar");
        let file = std::fs::File::create(&jar_path).expect("create file");
        let zip = zip::ZipWriter::new(file);
        zip.finish().expect("finish zip");

        let manifest = read_jar_manifest(&jar_path);
        assert!(manifest.is_none(), "empty jar should return None");
    }

    #[test]
    fn test_all_fields_present() {
        let content = "Manifest-Version: 1.0\nMain-Class: org.example.Main\nImplementation-Title: Example\nImplementation-Version: 1.2.3\n";
        let (path, _dir) = create_test_jar(Some(content));

        let manifest = read_jar_manifest(&path).expect("should read manifest");
        assert_eq!(manifest.main_class, Some("org.example.Main".to_string()));
        assert_eq!(manifest.implementation_title, Some("Example".to_string()));
        assert_eq!(manifest.implementation_version, Some("1.2.3".to_string()));
    }

    #[test]
    fn test_no_known_attributes() {
        let content = "Manifest-Version: 1.0\nCreated-By: Someone\n";
        let (path, _dir) = create_test_jar(Some(content));

        let manifest = read_jar_manifest(&path).expect("should read manifest");
        assert_eq!(manifest.main_class, None);
        assert_eq!(manifest.implementation_title, None);
        assert_eq!(manifest.implementation_version, None);
    }
}
