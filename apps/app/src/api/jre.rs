use crate::api::Result;
use std::path::PathBuf;
use tauri::plugin::TauriPlugin;
use theseus::prelude::*;

pub fn init<R: tauri::Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("jre")
        .invoke_handler(tauri::generate_handler![
            get_java_versions,
            get_java_default_versions,
            set_java_version,
            set_java_default_version,
            remove_java_default_version,
            remove_java_version,
            jre_find_filtered_jres,
            jre_get_jre,
            jre_test_jre,
            jre_auto_install_java,
            jre_get_max_memory,
            jre_get_memory_status,
            jre_optimize_memory,
            list_java_distribution_versions,
            list_java_feed_vendors,
            list_java_feed_versions,
            download_java_from_feed,
            download_java,
        ])
        .build()
}

#[tauri::command]
pub async fn get_java_versions() -> Result<Vec<JavaVersion>> {
    Ok(jre::get_java_versions().await?)
}

#[tauri::command]
pub async fn get_java_default_versions() -> Result<Vec<JavaVersion>> {
    Ok(jre::get_java_default_versions().await?)
}

#[tauri::command]
pub async fn set_java_version(java_version: JavaVersion) -> Result<()> {
    jre::set_java_version(java_version).await?;
    Ok(())
}

#[tauri::command]
pub async fn set_java_default_version(
    major_version: u32,
    path: String,
) -> Result<JavaVersion> {
    Ok(jre::set_java_default_version(major_version, path).await?)
}

#[tauri::command]
pub async fn remove_java_default_version(major_version: u32) -> Result<()> {
    jre::remove_java_default_version(major_version).await?;
    Ok(())
}

#[tauri::command]
pub async fn remove_java_version(path: String) -> Result<()> {
    jre::remove_java_version(path).await?;
    Ok(())
}

// Finds the installation of Java 8, if it exists
#[tauri::command]
pub async fn jre_find_filtered_jres(
    version: Option<u32>,
    full_scan: bool,
    force_fresh: bool,
    exhaustive: bool,
) -> Result<Vec<JavaVersion>> {
    Ok(
        jre::find_filtered_jres(version, full_scan, force_fresh, exhaustive)
            .await?,
    )
}

// Validates JRE at a given path
// Returns None if the path is not a valid JRE
#[tauri::command]
pub async fn jre_get_jre(path: PathBuf) -> Result<JavaVersion> {
    Ok(jre::check_jre(path).await?)
}

// Tests JRE of a certain version
#[tauri::command]
pub async fn jre_test_jre(path: PathBuf, major_version: u32) -> Result<bool> {
    Ok(jre::test_jre(path, major_version).await?)
}

// Auto installs java for the given java version
#[tauri::command]
pub async fn jre_auto_install_java(
    java_version: u32,
) -> Result<Option<PathBuf>> {
    Ok(jre::auto_install_java(java_version).await?)
}

#[tauri::command]
pub async fn list_java_distribution_versions(
    distribution: String,
) -> Result<Vec<u32>> {
    Ok(jre::list_java_distribution_versions(distribution).await?)
}

// Gets the maximum memory a system has available.
#[tauri::command]
pub async fn jre_get_max_memory() -> Result<u64> {
    Ok(jre::get_max_memory().await?)
}

#[tauri::command]
pub async fn jre_get_memory_status(
    instance_id: Option<String>,
    requested_memory_mb: u32,
    automatic: bool,
) -> Result<jre::MemoryStatus> {
    Ok(jre::get_memory_status(
        instance_id.as_deref(),
        requested_memory_mb,
        automatic,
    )
    .await?)
}

#[tauri::command]
pub async fn jre_optimize_memory()
-> Result<theseus::memory::MemoryOptimizationResult> {
    Ok(theseus::memory::optimize().await?)
}

#[tauri::command]
pub async fn list_java_feed_vendors() -> Result<Vec<String>> {
    Ok(jre::list_java_feed_vendors().await?)
}

#[tauri::command]
pub async fn list_java_feed_versions(
    vendor: String,
) -> Result<Vec<JdkVersionInfo>> {
    Ok(jre::list_java_feed_versions(&vendor).await?)
}

#[tauri::command]
pub async fn download_java_from_feed(
    vendor: String,
    jdk_version_major: u32,
) -> Result<PathBuf> {
    Ok(jre::download_java_from_feed(&vendor, jdk_version_major).await?)
}

#[tauri::command]
pub async fn download_java(
    vendor: String,
    version: u32,
) -> Result<theseus::install::InstallJobSnapshot> {
    Ok(theseus::install::download_java(vendor, version).await?)
}
