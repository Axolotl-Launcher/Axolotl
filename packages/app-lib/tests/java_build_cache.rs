#[path = "../build_support/java_cache.rs"]
mod java_cache;

use std::fs;
use std::time::{Duration, SystemTime};
use tempfile::TempDir;

fn fixture() -> (TempDir, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    for input in java_cache::INPUTS {
        let path = root.path().join(input);
        if *input == "java/src" || *input == "java/gradle" {
            fs::create_dir_all(path).unwrap();
        } else {
            fs::write(path, input).unwrap();
        }
    }
    fs::write(root.path().join("java/src/Example.java"), "original").unwrap();
    let out = root.path().join("out");
    fs::create_dir_all(out.join("java/libs")).unwrap();
    for jar in java_cache::JARS {
        fs::write(out.join(jar), jar).unwrap();
    }
    (root, out)
}

#[test]
fn successful_build_stays_fresh_with_old_unchanged_jars() {
    let (root, out) = fixture();
    for jar in java_cache::JARS {
        fs::File::options()
            .write(true)
            .open(out.join(jar))
            .unwrap()
            .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1))
            .unwrap();
    }
    let inputs = java_cache::input_fingerprint(root.path()).unwrap();
    assert!(!java_cache::is_fresh(&out, &inputs));
    java_cache::record_success(&out, &inputs).unwrap();
    assert!(java_cache::is_fresh(&out, &inputs));
}

#[test]
fn input_contents_and_deletions_invalidate_even_without_newer_mtimes() {
    let (root, out) = fixture();
    let source = root.path().join("java/src/Example.java");
    let original_time = fs::metadata(&source).unwrap().modified().unwrap();
    let inputs = java_cache::input_fingerprint(root.path()).unwrap();
    java_cache::record_success(&out, &inputs).unwrap();
    fs::write(&source, "modified").unwrap();
    fs::File::options()
        .write(true)
        .open(&source)
        .unwrap()
        .set_modified(original_time)
        .unwrap();
    let changed = java_cache::input_fingerprint(root.path()).unwrap();
    assert_ne!(inputs, changed);
    assert!(!java_cache::is_fresh(&out, &changed));
    java_cache::record_success(&out, &changed).unwrap();
    fs::remove_file(source).unwrap();
    let deleted = java_cache::input_fingerprint(root.path()).unwrap();
    assert_ne!(changed, deleted);
    assert!(!java_cache::is_fresh(&out, &deleted));
}

#[test]
fn wrapper_changes_invalidate_the_build() {
    let (root, out) = fixture();
    let inputs = java_cache::input_fingerprint(root.path()).unwrap();
    java_cache::record_success(&out, &inputs).unwrap();
    fs::write(root.path().join("java/gradlew.bat"), "updated wrapper").unwrap();
    assert!(!java_cache::is_fresh(
        &out,
        &java_cache::input_fingerprint(root.path()).unwrap()
    ));
}

#[test]
fn missing_or_corrupt_outputs_and_failed_builds_cannot_hit_the_cache() {
    let (root, out) = fixture();
    let inputs = java_cache::input_fingerprint(root.path()).unwrap();
    java_cache::record_success(&out, &inputs).unwrap();
    fs::write(out.join(java_cache::JARS[1]), "corrupt").unwrap();
    assert!(!java_cache::is_fresh(&out, &inputs));
    java_cache::record_success(&out, &inputs).unwrap();
    java_cache::invalidate(&out).unwrap();
    assert!(!java_cache::is_fresh(&out, &inputs));
    java_cache::invalidate(&out).unwrap();
    fs::remove_file(out.join(java_cache::JARS[0])).unwrap();
    assert!(java_cache::record_success(&out, &inputs).is_err());
    assert!(!java_cache::is_fresh(&out, &inputs));
}
