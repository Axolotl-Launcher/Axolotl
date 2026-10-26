//! Download URLs, CDN validation and progress formatting.

use crate::ErrorKind;
use crate::api::curseforge::dto::CurseForgeFile;
use crate::event::emit::emit_loading;
use crate::install::{
    InstallJobEventKind, InstallPhaseDetails, InstallPhaseId, InstallProgress,
    InstallProgressReporter, InstallProgressSecondary,
};
use crate::util::download::constants::{
    CURSEFORGE_H2_RANGE_THRESHOLD, CURSEFORGE_REQUESTED_H2_RANGES,
};
use crate::util::fetch::{ContentValidation, Integrity};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub(crate) async fn report_modpack_progress(
    loading_bar: Option<&crate::event::LoadingBarId>,
    reporter: Option<&InstallProgressReporter>,
    details: InstallPhaseDetails,
    files_done: &AtomicU64,
    bytes_done: &AtomicU64,
    active_downloads: &AtomicU64,
    total_files: u64,
    total_bytes: u64,
    file_bytes: u64,
    event: InstallJobEventKind,
) -> crate::Result<()> {
    let current_files = files_done.fetch_add(1, Ordering::Relaxed) + 1;
    let current_bytes =
        bytes_done.fetch_add(file_bytes, Ordering::Relaxed) + file_bytes;
    let active = active_downloads.load(Ordering::Relaxed);
    let message = if total_bytes > 0 {
        format!(
            "{current_files}/{total_files} files · {} / {} · {active} downloading in parallel",
            format_bytes(current_bytes.min(total_bytes)),
            format_bytes(total_bytes)
        )
    } else {
        format!(
            "{current_files}/{total_files} files · {active} downloading in parallel"
        )
    };
    if let Some(loading_bar) = loading_bar {
        emit_loading(loading_bar, 1.0, Some(&message))?;
    }
    if let Some(reporter) = reporter {
        reporter
            .update_with_events(
                InstallPhaseId::DownloadingContent,
                Some(InstallProgress {
                    current: current_files,
                    total: total_files,
                    secondary: Some(InstallProgressSecondary {
                        current: current_bytes.min(total_bytes),
                        total: total_bytes,
                    }),
                }),
                details,
                vec![event],
            )
            .await?;
    }
    Ok(())
}

pub(crate) fn is_forge_cdn_url(url: &reqwest::Url) -> bool {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    host == "forgecdn.net" || host.ends_with(".forgecdn.net")
}

pub(crate) fn curseforge_file_page_url(
    website_url: Option<&str>,
    file_id: u32,
) -> Option<String> {
    let website_url = website_url?;
    let Ok(mut url) = reqwest::Url::parse(website_url) else {
        return Some(website_url.to_owned());
    };
    if !matches!(
        url.host_str(),
        Some("curseforge.com" | "www.curseforge.com" | "legacy.curseforge.com")
    ) {
        return Some(website_url.to_owned());
    }

    let mut path = url.path().trim_end_matches('/').to_string();
    for marker in ["/files/", "/download/"] {
        if let Some(index) = path.rfind(marker) {
            path.truncate(index);
            break;
        }
    }
    url.set_path(&format!("{path}/download/{file_id}"));
    url.set_query(None);
    url.set_fragment(None);
    Some(url.into())
}

pub(crate) fn validate_cdn_url(url: &reqwest::Url) -> crate::Result<()> {
    #[cfg(debug_assertions)]
    if url.scheme() == "http"
        && matches!(
            url.host_str()
                .unwrap_or_default()
                .to_ascii_lowercase()
                .as_str(),
            "127.0.0.1" | "localhost"
        )
    {
        return Ok(());
    }
    if url.scheme() != "https" || !is_forge_cdn_url(url) {
        return Err(ErrorKind::InputError(
            "CurseForge returned a download URL outside its CDN".to_string(),
        )
        .into());
    }
    Ok(())
}

pub(crate) fn curseforge_content_validation(
    file_name: &str,
) -> ContentValidation {
    match Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jar" | "zip" | "mrpack") => ContentValidation::Jar,
        _ => ContentValidation::None,
    }
}

pub(crate) fn curseforge_integrity(
    file: &CurseForgeFile,
    validation: ContentValidation,
) -> Integrity {
    Integrity {
        size: Some(file.file_length),
        sha1: file
            .hashes
            .iter()
            .find(|hash| hash.algo == 1)
            .map(|hash| hash.value.clone()),
        md5: file
            .hashes
            .iter()
            .find(|hash| hash.algo == 2)
            .map(|hash| hash.value.clone()),
        content: validation,
        ..Integrity::default()
    }
}

pub(crate) const fn curseforge_modpack_h2_range_concurrency(
    size: u64,
) -> Option<usize> {
    if size >= CURSEFORGE_H2_RANGE_THRESHOLD {
        Some(CURSEFORGE_REQUESTED_H2_RANGES)
    } else {
        None
    }
}

pub(crate) fn curseforge_candidate_urls(
    url: &str,
) -> crate::Result<Vec<String>> {
    let parsed = reqwest::Url::parse(url).map_err(|_| {
        ErrorKind::InputError(
            "CurseForge returned an invalid download URL".to_string(),
        )
    })?;
    validate_cdn_url(&parsed)?;
    if !is_forge_cdn_url(&parsed) {
        return Ok(Vec::new());
    }

    let original_host = parsed.host_str().unwrap_or_default();
    let mut candidates = Vec::new();
    for host in [
        "edge.forgecdn.net",
        "media.forgecdn.net",
        "mediafilez.forgecdn.net",
    ] {
        if host == original_host {
            continue;
        }
        let mut candidate = parsed.clone();
        candidate.set_host(Some(host)).map_err(|_| {
            ErrorKind::InputError(
                "CurseForge returned an invalid CDN URL".to_string(),
            )
        })?;
        candidates.push(candidate.to_string());
    }
    Ok(candidates)
}
