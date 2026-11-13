//! Keeps the tools an AppImage carries from shadowing the host's own.
//!
//! The AppImage's `AppRun` puts `$APPDIR/usr/bin` first on `PATH`, and the
//! `xdg-open` bundled there comes from the older build host. On a desktop newer
//! than that script knows — Plasma 6, for one — it does nothing and exits 0, so
//! the opener reports success and no browser ever appears. The host's copy
//! matches the host's desktop, so it has to be found first; the bundled one stays
//! on `PATH` for a host that has none.

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::Path;

/// Moves the AppImage's own directories behind the host's on this process's
/// `PATH`. Does nothing outside an AppImage.
///
/// Must run before any thread is spawned, because it changes the environment.
#[cfg(target_os = "linux")]
pub fn prefer_host_tools() {
    let Some(app_dir) = env::var_os("APPDIR").filter(|dir| !dir.is_empty())
    else {
        return;
    };
    let Some(path) = env::var_os("PATH") else {
        return;
    };
    if let Some(reordered) = host_tools_first(&path, Path::new(&app_dir)) {
        // SAFETY: the caller runs this before any threads are spawned in main()
        unsafe { env::set_var("PATH", reordered) }
    }
}

/// `path` with every entry inside `app_dir` moved behind the others, or `None`
/// when there is nothing to move past.
fn host_tools_first(path: &OsStr, app_dir: &Path) -> Option<OsString> {
    let (bundled, host): (Vec<_>, Vec<_>) =
        env::split_paths(path).partition(|entry| entry.starts_with(app_dir));
    if bundled.is_empty() || host.is_empty() {
        return None;
    }
    env::join_paths(host.into_iter().chain(bundled)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn joined(entries: &[&str]) -> OsString {
        env::join_paths(entries).unwrap()
    }

    fn split(path: OsString) -> Vec<PathBuf> {
        env::split_paths(&path).collect()
    }

    #[test]
    fn host_directories_come_before_the_bundled_ones() {
        let path = joined(&[
            "/tmp/.mount_x/usr/bin",
            "/tmp/.mount_x/usr/sbin",
            "/usr/local/bin",
            "/usr/bin",
        ]);
        let reordered =
            host_tools_first(&path, Path::new("/tmp/.mount_x")).unwrap();
        assert_eq!(
            split(reordered),
            [
                "/usr/local/bin",
                "/usr/bin",
                "/tmp/.mount_x/usr/bin",
                "/tmp/.mount_x/usr/sbin",
            ]
            .map(PathBuf::from)
        );
    }

    #[test]
    fn a_path_without_bundled_directories_is_left_alone() {
        let path = joined(&["/usr/local/bin", "/usr/bin"]);
        assert_eq!(host_tools_first(&path, Path::new("/tmp/.mount_x")), None);
    }

    #[test]
    fn a_path_with_only_bundled_directories_is_left_alone() {
        let path = joined(&["/tmp/.mount_x/usr/bin"]);
        assert_eq!(host_tools_first(&path, Path::new("/tmp/.mount_x")), None);
    }

    #[test]
    fn a_sibling_directory_is_not_mistaken_for_the_appimage() {
        let path = joined(&["/tmp/.mount_xy/usr/bin", "/usr/bin"]);
        assert_eq!(host_tools_first(&path, Path::new("/tmp/.mount_x")), None);
    }
}
