//! Theme packs: the launcher's built-in palettes, extended by a signed-off
//! folder of tokens, stylesheets, assets and (optionally) sandboxed pages.
//!
//! This module owns the on-disk contract. It does not know about Tauri: the
//! desktop layer resolves the directories, exposes commands and registers the
//! `axolotl-theme` protocol, and every entry point here assumes the caller
//! handed it a path it can trust as a directory, never as user content.

pub mod archive;
pub mod manifest;
pub mod network;
pub mod store;

use std::path::Path;
use std::path::PathBuf;

pub use archive::ThemePackLimits;
pub use manifest::ThemePackManifest;
pub use manifest::ThemePackPage;
pub use manifest::ThemePackPermission;
pub use store::InstalledThemePack;

pub const THEMES_DIRECTORY: &str = "themes";
pub const DATA_DIRECTORY: &str = "theme-pack-data";

/// The two directories a theme pack touches.
///
/// The pack itself lives under `app_data/themes/<id>` and is replaced wholesale
/// on update. Whatever a pack writes with the storage permission lives under
/// `app_data/theme-pack-data/<id>`, so an uninstall can remove the first
/// without silently discarding the second's ownership story.
#[derive(Debug, Clone)]
pub struct ThemePackStorage {
    themes: PathBuf,
    data: PathBuf,
}

impl ThemePackStorage {
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            themes: app_data_dir.join(THEMES_DIRECTORY),
            data: app_data_dir.join(DATA_DIRECTORY),
        }
    }

    pub fn themes_dir(&self) -> &Path {
        &self.themes
    }

    pub fn data_dir(&self) -> &Path {
        &self.data
    }

    pub fn pack_dir(&self, pack_id: &str) -> crate::Result<PathBuf> {
        manifest::validate_id(pack_id)?;
        Ok(self.themes.join(pack_id))
    }

    pub fn pack_data_dir(&self, pack_id: &str) -> crate::Result<PathBuf> {
        manifest::validate_id(pack_id)?;
        Ok(self.data.join(pack_id))
    }
}

/// The launcher version packs are checked against.
pub fn launcher_version() -> semver::Version {
    semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .unwrap_or_else(|_| semver::Version::new(0, 0, 0))
}
