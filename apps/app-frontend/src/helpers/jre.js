/**
 * All theseus API calls return serialized values (both return values and errors);
 * So, for example, addDefaultInstance creates a blank instance object, where the Rust struct is serialized,
 *  and deserialized into a usable JS object.
 */
import { invoke } from '@tauri-apps/api/core'

/*

JavaVersion {
    path: Path
    version: String
}

*/

export async function get_java_versions() {
    return await invoke('plugin:jre|get_java_versions')
}

export async function get_java_default_versions() {
    return await invoke('plugin:jre|get_java_default_versions')
}

export async function get_java_selection_info(path) {
    return await invoke('plugin:jre|get_java_selection_info', { path })
}

export async function approve_java_selection(path, volumeId) {
    return await invoke('plugin:jre|approve_java_selection', { path, volumeId })
}

export async function set_java_version(javaVersion) {
    return await invoke('plugin:jre|set_java_version', { javaVersion })
}

export async function set_java_default_version(majorVersion, path) {
    return await invoke('plugin:jre|set_java_default_version', { majorVersion, path })
}

export async function remove_java_default_version(majorVersion) {
    return await invoke('plugin:jre|remove_java_default_version', { majorVersion })
}

export async function remove_java_version(path) {
    return await invoke('plugin:jre|remove_java_version', { path })
}

// Finds all the installation of the given Java version, if it exists
// Returns [JavaVersion]
export async function find_filtered_jres(
    version,
    fullScan = false,
    forceFresh = false,
    exhaustive = false,
) {
    return await invoke('plugin:jre|jre_find_filtered_jres', {
        version,
        fullScan,
        forceFresh,
        exhaustive,
    })
}

// Gets java version from a specific path by trying to run 'java -version' on it.
// This also validates it, as it returns null if no valid java version is found at the path
export async function get_jre(path) {
    return await invoke('plugin:jre|jre_get_jre', { path })
}

// Tests JRE version by running 'java -version' on it.
// Returns true if the version is valid, and matches given (after extraction)
export async function test_jre(path, majorVersion) {
    return await invoke('plugin:jre|jre_test_jre', { path, majorVersion })
}

// Automatically installs specified java version
export async function auto_install_java(javaVersion) {
    return await invoke('plugin:jre|jre_auto_install_java', { javaVersion })
}

export async function list_java_distribution_versions(distribution) {
    return await invoke('plugin:jre|list_java_distribution_versions', { distribution })
}

// Get max memory in KiB
export async function get_max_memory() {
    return await invoke('plugin:jre|jre_get_max_memory')
}

export async function get_memory_status(instanceId, requestedMemoryMb, automatic) {
    return await invoke('plugin:jre|jre_get_memory_status', {
        instanceId,
        requestedMemoryMb,
        automatic,
    })
}

export async function optimize_memory() {
    return await invoke('plugin:jre|jre_optimize_memory')
}

export async function list_java_feed_vendors() {
    return await invoke('plugin:jre|list_java_feed_vendors')
}

export async function list_java_feed_versions(vendor) {
    return await invoke('plugin:jre|list_java_feed_versions', { vendor })
}

export async function download_java_from_feed(vendor, jdkVersionMajor) {
    return await invoke('plugin:jre|download_java_from_feed', { vendor, jdkVersionMajor })
}

export async function download_java(vendor, version) {
    return await invoke('plugin:jre|download_java', { vendor, version })
}
