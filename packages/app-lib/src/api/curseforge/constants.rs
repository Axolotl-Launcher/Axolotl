//! Shared constants for the CurseForge submodules.

use std::time::Duration;

pub(super) const API_BASE_URL: &str = "https://api.curseforge.com";
pub(super) const MINECRAFT_GAME_ID: u32 = 432;
pub(super) const MAX_PAGE_SIZE: u32 = 50;

pub(super) const FABRIC_API_CURSEFORGE_PROJECT_ID: u32 = 306612;
pub(super) const QUILTED_FABRIC_API_CURSEFORGE_PROJECT_ID: u32 = 634179;
pub(super) const CURSEFORGE_LOADER_QUILT: u32 = 5;
pub(super) const MAX_DEPENDENCY_DEPTH: usize = 32;
pub(super) const DEPENDENCY_PLAN_TTL: Duration = Duration::from_secs(10 * 60);
