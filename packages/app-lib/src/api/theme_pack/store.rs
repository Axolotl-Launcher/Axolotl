//! Installing, listing, resolving and removing theme packs on disk.

use std::path::Path;
use std::path::PathBuf;

use serde::Serialize;

use super::ThemePackLimits;
use super::ThemePackStorage;
use super::archive;
use super::manifest;
use super::manifest::ThemePackManifest;
use super::manifest::ThemePackPage;
use crate::ErrorKind;

/// Serializes installs so two archives cannot race over the same target
/// directory or its backup.
static INSTALL_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstalledThemePack {
    pub manifest: ThemePackManifest,
    /// Absolute path to the installed pack directory.
    pub path: String,
    pub installed_at: u64,
    pub size_bytes: u64,
}

impl InstalledThemePack {
    pub fn from_directory(pack_dir: &Path) -> crate::Result<Self> {
        let manifest = ThemePackManifest::load(pack_dir)?;
        let origin = archive::read_origin(pack_dir);
        Ok(Self {
            manifest,
            path: pack_dir.to_string_lossy().to_string(),
            installed_at: origin
                .as_ref()
                .map(|origin| origin.installed_at)
                .unwrap_or_default(),
            size_bytes: origin
                .as_ref()
                .map(|origin| origin.size_bytes)
                .unwrap_or_default(),
        })
    }
}

/// Unpacks `archive_path` into the themes directory.
///
/// The pack is extracted into a hidden sibling first, validated there and only
/// then moved into `themes/<id>`, so a crash or a bad archive never leaves a
/// half-installed pack that the launcher would try to load.
pub async fn install(
    storage: &ThemePackStorage,
    archive_path: &Path,
    limits: ThemePackLimits,
    replace: bool,
) -> crate::Result<InstalledThemePack> {
    let _install = INSTALL_LOCK.lock().await;
    tokio::fs::create_dir_all(storage.themes_dir())
        .await
        .map_err(io)?;
    let staging = archive::hidden_sibling(storage.themes_dir(), "install");
    let archive_path = archive_path.to_path_buf();
    let staging_for_task = staging.clone();

    let extracted = tokio::task::spawn_blocking(move || {
        let result =
            archive::extract(&archive_path, &staging_for_task, &limits);
        match result {
            Ok((manifest, extraction)) => {
                if !manifest.supports_launcher(&super::launcher_version()) {
                    let _ = std::fs::remove_dir_all(&staging_for_task);
                    return Err(ErrorKind::InputError(format!(
                        "Theme pack {} requires Axolotl {} and this launcher does not satisfy it",
                        manifest.id, manifest.engines.axolotl
                    ))
                    .into());
                }
                if let Err(error) = archive::write_origin(
                    &staging_for_task,
                    &archive_path,
                    &manifest.version,
                    extraction.uncompressed_bytes,
                ) {
                    let _ = std::fs::remove_dir_all(&staging_for_task);
                    return Err(error);
                }
                Ok((manifest, extraction))
            }
            Err(error) => {
                let _ = std::fs::remove_dir_all(&staging_for_task);
                Err(error)
            }
        }
    })
    .await;
    let extracted = match extracted {
        Ok(result) => result?,
        Err(error) => {
            let _ = tokio::fs::remove_dir_all(&staging).await;
            return Err(ErrorKind::OtherError(format!(
                "The theme pack install task failed: {error}"
            ))
            .into());
        }
    };

    let (manifest, extraction) = extracted;
    let port = manifest::origin_port(&manifest.id);
    let installed = match list(storage).await {
        Ok(packs) => packs,
        Err(error) => {
            let _ = tokio::fs::remove_dir_all(&staging).await;
            return Err(error);
        }
    };
    if installed.iter().any(|pack| {
        pack.manifest.id != manifest.id
            && manifest::origin_port(&pack.manifest.id) == port
    }) {
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(ErrorKind::InputError(format!(
            "Theme pack {} would share a browser origin with another installed pack",
            manifest.id
        ))
        .into());
    }
    let target = storage.pack_dir(&manifest.id)?;
    let target_exists = tokio::fs::try_exists(&target).await.unwrap_or(false);
    if target_exists && !replace {
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(ErrorKind::InputError(format!(
            "Theme pack {} is already installed",
            manifest.id
        ))
        .into());
    }

    let backup = target_exists
        .then(|| archive::hidden_sibling(storage.themes_dir(), "old"));
    if let Some(backup) = &backup {
        if let Err(error) = tokio::fs::rename(&target, backup).await {
            let _ = tokio::fs::remove_dir_all(&staging).await;
            return Err(io(error));
        }
    }
    if let Err(error) = tokio::fs::rename(&staging, &target).await {
        if let Some(backup) = &backup {
            if let Err(restore) = tokio::fs::rename(backup, &target).await {
                let _ = tokio::fs::remove_dir_all(&staging).await;
                return Err(ErrorKind::OtherError(format!(
                    "The theme pack could not be installed ({error}); the \
                     previous copy could not be restored ({restore}) and is \
                     preserved at {}",
                    backup.display()
                ))
                .into());
            }
        }
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(io(error));
    }
    if let Some(backup) = &backup {
        let _ = tokio::fs::remove_dir_all(backup).await;
    }

    Ok(InstalledThemePack {
        manifest,
        path: target.to_string_lossy().to_string(),
        installed_at: archive::now_seconds(),
        size_bytes: extraction.uncompressed_bytes,
    })
}

/// Every installed pack that still parses. A directory a user broke by hand is
/// skipped with a warning instead of failing the whole list.
pub async fn list(
    storage: &ThemePackStorage,
) -> crate::Result<Vec<InstalledThemePack>> {
    let themes_dir = storage.themes_dir().to_path_buf();
    if !tokio::fs::try_exists(&themes_dir).await.unwrap_or(false) {
        return Ok(Vec::new());
    }
    tokio::task::spawn_blocking(move || {
        let mut packs = Vec::new();
        let entries = std::fs::read_dir(&themes_dir).map_err(io)?;
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || !path.is_dir() {
                continue;
            }
            match InstalledThemePack::from_directory(&path) {
                Ok(pack) if pack.manifest.id == name => packs.push(pack),
                Ok(pack) => {
                    tracing::warn!(
                        path = %path.display(),
                        manifest_id = %pack.manifest.id,
                        "Skipping a theme pack whose folder does not match its id"
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        path = %path.display(),
                        %error,
                        "Skipping a theme pack that does not load"
                    );
                }
            }
        }
        packs.sort_by(|left, right| left.manifest.id.cmp(&right.manifest.id));
        Ok(packs)
    })
    .await
    .map_err(|error| -> crate::Error {
        ErrorKind::OtherError(format!(
            "The theme pack list task failed: {error}"
        ))
        .into()
    })?
}

pub async fn get(
    storage: &ThemePackStorage,
    pack_id: &str,
) -> crate::Result<Option<InstalledThemePack>> {
    let path = storage.pack_dir(pack_id)?;
    if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
        return Ok(None);
    }
    let path_for_task = path.clone();
    let result = tokio::task::spawn_blocking(move || {
        InstalledThemePack::from_directory(&path_for_task)
    })
    .await
    .map_err(|error| -> crate::Error {
        ErrorKind::OtherError(format!(
            "The theme pack read task failed: {error}"
        ))
        .into()
    })?;
    result.map(Some)
}

/// Removes a pack by moving it aside first, so the delete cannot leave a
/// directory the launcher would still list next to the uninstall UI.
pub async fn remove(
    storage: &ThemePackStorage,
    pack_id: &str,
    remove_data: bool,
) -> crate::Result<bool> {
    let path = storage.pack_dir(pack_id)?;
    if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
        return Ok(false);
    }
    let trash = archive::hidden_sibling(storage.themes_dir(), "trash");
    tokio::fs::rename(&path, &trash).await.map_err(io)?;
    if let Err(error) = tokio::fs::remove_dir_all(&trash).await {
        tracing::warn!(
            %error,
            path = %trash.display(),
            "A trashed theme pack could not be removed"
        );
    }
    if remove_data {
        let data = storage.pack_data_dir(pack_id)?;
        if tokio::fs::try_exists(&data).await.unwrap_or(false) {
            if let Err(error) = tokio::fs::remove_dir_all(&data).await {
                tracing::warn!(
                    %error,
                    path = %data.display(),
                    "Theme pack data could not be removed"
                );
            }
        }
    }
    Ok(true)
}

/// Resolves a file the pack is allowed to serve over `axolotl-theme`, and
/// reports whether it is one of the pack's declared entry points.
pub fn asset_path(
    storage: &ThemePackStorage,
    pack_id: &str,
    relative: &str,
) -> crate::Result<PathBuf> {
    let pack_dir = storage.pack_dir(pack_id)?;
    manifest::checked_file(&pack_dir, relative)
}

pub struct ResolvedThemePackPage {
    pub page: ThemePackPage,
    pub path: PathBuf,
}

pub fn resolve_page(
    storage: &ThemePackStorage,
    pack_id: &str,
    page_id: &str,
) -> crate::Result<ResolvedThemePackPage> {
    let pack_dir = storage.pack_dir(pack_id)?;
    let manifest = ThemePackManifest::load(&pack_dir)?;
    let page = manifest.page(page_id).cloned().ok_or_else(|| {
        ErrorKind::InputError(format!(
            "Theme pack {pack_id} has no page named {page_id}"
        ))
    })?;
    let path = manifest::checked_file(&pack_dir, &page.entry)?;
    Ok(ResolvedThemePackPage { page, path })
}

fn io(error: std::io::Error) -> crate::Error {
    ErrorKind::StdIOError(error).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Runtime::new()
            .expect("a test runtime")
            .block_on(future)
    }

    fn storage(directory: &Path) -> ThemePackStorage {
        ThemePackStorage::new(directory)
    }

    fn write_pack(directory: &Path, id: &str) {
        let pack = directory.join("themes").join(id);
        std::fs::create_dir_all(pack.join("styles")).unwrap();
        std::fs::write(
            pack.join("manifest.json"),
            format!(
                r##"{{
                    "schemaVersion": 1,
                    "id": "{id}",
                    "name": {{"en-US": "Test"}},
                    "version": "1.0.0",
                    "engines": {{"axolotl": ">=1.0.0"}},
                    "capabilities": ["tokens"]
                }}"##
            ),
        )
        .unwrap();
    }

    #[test]
    fn skips_hidden_directories_and_broken_packs() {
        let temp = tempfile::tempdir().unwrap();
        let storage = storage(temp.path());
        write_pack(temp.path(), "good-pack");
        std::fs::create_dir_all(
            temp.path().join("themes").join(".install-leftover"),
        )
        .unwrap();
        std::fs::create_dir_all(temp.path().join("themes").join("broken-pack"))
            .unwrap();
        let packs = block_on(list(&storage)).unwrap();
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].manifest.id, "good-pack");
    }

    #[test]
    fn removes_a_pack_without_touching_its_data_by_default() {
        let temp = tempfile::tempdir().unwrap();
        let storage = storage(temp.path());
        write_pack(temp.path(), "good-pack");
        std::fs::create_dir_all(
            temp.path().join("theme-pack-data").join("good-pack"),
        )
        .unwrap();
        assert!(block_on(remove(&storage, "good-pack", false)).unwrap());
        assert!(!temp.path().join("themes/good-pack").exists());
        assert!(
            temp.path().join("theme-pack-data/good-pack").exists(),
            "pack storage survives a reinstall"
        );
    }

    #[test]
    fn resolves_only_files_inside_the_pack() {
        let temp = tempfile::tempdir().unwrap();
        let storage = storage(temp.path());
        write_pack(temp.path(), "good-pack");
        std::fs::write(
            temp.path().join("themes/good-pack/styles/theme.css"),
            ".x{}",
        )
        .unwrap();
        assert!(asset_path(&storage, "good-pack", "styles/theme.css").is_ok());
        assert!(
            asset_path(&storage, "good-pack", "../good-pack/manifest.json")
                .is_err()
        );
        assert!(asset_path(&storage, "good-pack", "/etc/passwd").is_err());
    }

    #[test]
    fn rejects_packs_that_would_share_a_browser_origin() {
        fn write_archive(path: &Path, id: &str) {
            let file = std::fs::File::create(path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            zip.start_file("manifest.json", SimpleFileOptions::default())
                .unwrap();
            zip.write_all(
                format!(
                    r##"{{"schemaVersion":1,"id":"{id}","name":{{"en-US":"Test"}},"version":"1.0.0","engines":{{"axolotl":">=1.0.0"}},"capabilities":["tokens"],"tokens":{{"light":{{"--surface-1":"#ffffff","--surface-3":"#f5f5f5","--color-text-primary":"#101215"}},"dark":{{"--surface-1":"#101215","--surface-3":"#262b34","--color-text-primary":"#eceff4"}}}}}}"##
                )
                .as_bytes(),
            )
            .unwrap();
            zip.finish().unwrap();
        }

        let temp = tempfile::tempdir().unwrap();
        let storage = storage(temp.path());
        let first = temp.path().join("first.zip");
        let second = temp.path().join("second.zip");
        write_archive(&first, "demo-theme");
        write_archive(&second, "pack-37858");
        block_on(install(&storage, &first, ThemePackLimits::default(), false))
            .unwrap();
        let error = block_on(install(
            &storage,
            &second,
            ThemePackLimits::default(),
            false,
        ))
        .unwrap_err();
        assert!(
            error.to_string().contains("share a browser origin"),
            "{error}"
        );
    }

    #[test]
    fn installs_and_replaces_an_archive() {
        let temp = tempfile::tempdir().unwrap();
        let storage = storage(temp.path());
        let archive_path = temp.path().join("pack.zip");
        let write_archive = || {
            let file = std::fs::File::create(&archive_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = SimpleFileOptions::default();
            zip.start_file("manifest.json", options).unwrap();
            zip.write_all(
                br##"{
                    "schemaVersion": 1,
                    "id": "archive-pack",
                    "name": {"en-US": "Archive"},
                    "version": "1.0.0",
                    "engines": {"axolotl": ">=1.0.0"},
                    "capabilities": ["tokens"],
                    "tokens": {
                        "light": {"--surface-1": "#ffffff", "--surface-3": "#f5f5f5", "--color-text-primary": "#101215"},
                        "dark": {"--surface-1": "#101215", "--surface-3": "#262b34", "--color-text-primary": "#eceff4"}
                    }
                }"##,
            )
            .unwrap();
            zip.finish().unwrap();
        };
        write_archive();
        let installed = block_on(install(
            &storage,
            &archive_path,
            ThemePackLimits::default(),
            false,
        ))
        .unwrap();
        assert_eq!(installed.manifest.id, "archive-pack");
        assert!(
            block_on(install(
                &storage,
                &archive_path,
                ThemePackLimits::default(),
                false,
            ))
            .is_err(),
            "a second install without replace is rejected"
        );
        assert!(
            block_on(install(
                &storage,
                &archive_path,
                ThemePackLimits::default(),
                true,
            ))
            .is_ok()
        );
    }
}
