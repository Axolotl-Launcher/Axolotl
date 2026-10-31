use super::*;

pub(super) fn entries() -> Vec<Entry> {
    use Access::*;
    let major = json!({"type":"integer","minimum":1,"maximum":255});
    let runtime = json!({"registered_path":text(4096),"major_version":major});
    let mut versions = page();
    versions["vendor"] = text(128);
    vec![
        entry(
            "java_list_vendors",
            "List exact Java vendor names from the launcher's managed JDK feed, filtered for this machine's OS/architecture. Does not scan the filesystem or execute Java. Use returned vendor strings with java_list_versions/java_install; feed data may be cached.",
            Controlled,
            true,
            true,
            json!({}),
            &[],
        ),
        entry(
            "java_list_versions",
            "List available managed JDK major versions and feed-selected full version/product for one exact vendor. Newest major first, paginated. The feed currently offers one selected build per major; arbitrary full-version pinning and download URLs are not accepted.",
            Controlled,
            true,
            true,
            versions,
            &["vendor"],
        ),
        entry(
            "java_install",
            "Queue a managed JDK installation for an exact vendor and major_version offered by java_list_versions on this machine. Returns operation_id; poll operations_get for install job_id, then install_job_get until complete. Runtime verifies feed size/hash before extraction and registers the runtime. Installation may set that major's default. Job continues after control ends; do not retry blindly or provide URLs/paths.",
            Controlled,
            false,
            false,
            json!({"vendor":text(128),"major_version":major}),
            &["vendor", "major_version"],
        ),
        entry(
            "java_test",
            "Inspect a registered runtime using native release metadata or executing the launcher's Java probe to verify its current major version. Requires exact registered_path from java_list. Returns operation_id; poll operations_get for matches_expected_major. Does not register arbitrary executables or modify defaults. A false result means invalid/unavailable runtime or version mismatch, not a successful test.",
            Controlled,
            false,
            true,
            runtime.clone(),
            &["registered_path", "major_version"],
        ),
        entry(
            "java_set_default",
            "Set an existing registered runtime as the global default for one Java major. Native inspection revalidates release metadata or runs Java, rejecting actual major mismatch. Returns operation_id; poll operations_get for the validated runtime. Does not change explicit per-instance Java overrides or running games.",
            Controlled,
            false,
            true,
            runtime,
            &["registered_path", "major_version"],
        ),
        entry(
            "java_remove_default",
            "Clear one major's configured Java default. Requires full access. Does not remove registrations or files; runtime discovery may select a runtime again at a future launch. Does not change explicit instance overrides.",
            FullAccess,
            false,
            true,
            json!({"major_version":major}),
            &["major_version"],
        ),
        entry(
            "java_remove_registration",
            "Remove an exact registered Java path from the launcher registry, including defaults naming it. Requires full access. Does not delete Java files or change explicit instance overrides; those may still reference the path. Managed discovery can re-register it later. Use only exact registered_path from java_list.",
            FullAccess,
            false,
            false,
            json!({"registered_path":text(4096)}),
            &["registered_path"],
        ),
    ]
}
