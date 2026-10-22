//! Theseus directory information
use crate::LoadingBarType;
use crate::event::emit::{emit_loading, init_loading};
use crate::state::LAUNCHER_STATE;
use crate::state::{JavaVersion, Settings};
use crate::util::fetch::IoSemaphore;
use dashmap::DashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;

pub const CACHES_FOLDER_NAME: &str = "caches";
pub const LAUNCHER_LOGS_FOLDER_NAME: &str = "launcher_logs";
pub const INSTANCES_FOLDER_NAME: &str = "profiles";
pub const SERVERS_FOLDER_NAME: &str = "servers";
pub const INSTALL_ROLLBACKS_FOLDER_NAME: &str = "install-rollbacks";
pub const METADATA_FOLDER_NAME: &str = "meta";

#[derive(Debug)]
pub struct DirectoryInfo {
    pub settings_dir: PathBuf, // Base settings directory- app database
    pub config_dir: PathBuf, // Base config directory- instances, minecraft downloads, etc. Changeable as a setting.
    pub app_identifier: String,
}

impl DirectoryInfo {
    pub(crate) fn synced_options_dir(&self) -> std::path::PathBuf {
        self.config_dir.join("synced-options")
    }
    pub fn global_handle_if_ready() -> Option<&'static Self> {
        LAUNCHER_STATE.get().map(|x| &x.directories)
    }

    pub fn get_initial_settings_dir(&self) -> Option<PathBuf> {
        Self::initial_settings_dir_path(&self.app_identifier)
    }

    // Get the settings directory
    // init() is not needed for this function
    pub fn initial_settings_dir_path(app_identifier: &str) -> Option<PathBuf> {
        Self::env_path("THESEUS_CONFIG_DIR").or_else(|| {
            Some(
                dirs::data_dir()?.join(crate::brand::app_data_dir_identifier(
                    app_identifier,
                )),
            )
        })
    }

    /// Get all paths needed for Theseus to operate properly
    #[tracing::instrument]
    pub async fn init(
        config_dir: Option<String>,
        app_identifier: &str,
    ) -> crate::Result<Self> {
        let settings_dir = Self::initial_settings_dir_path(app_identifier)
            .ok_or(crate::ErrorKind::FSError(
                "Could not find valid settings dir".to_string(),
            ))?;

        fs::create_dir_all(&settings_dir).await.map_err(|err| {
            crate::ErrorKind::FSError(format!(
                "Error creating Theseus config directory: {err}"
            ))
        })?;

        let config_dir =
            config_dir.map_or_else(|| settings_dir.clone(), PathBuf::from);

        Ok(Self {
            settings_dir,
            config_dir,
            app_identifier: app_identifier.to_owned(),
        })
    }

    /// Get the Minecraft instance metadata directory
    #[inline]
    pub fn metadata_dir(&self) -> PathBuf {
        self.config_dir.join(METADATA_FOLDER_NAME)
    }

    /// Get the Minecraft java versions metadata directory
    #[inline]
    pub fn java_versions_dir(&self) -> PathBuf {
        self.metadata_dir().join("java_versions")
    }

    /// Get the Minecraft versions metadata directory
    #[inline]
    pub fn versions_dir(&self) -> PathBuf {
        self.metadata_dir().join("versions")
    }

    /// Get the metadata directory for a given version
    #[inline]
    pub fn version_dir(&self, version: &str) -> PathBuf {
        self.versions_dir().join(version)
    }

    /// Get the Minecraft libraries metadata directory
    #[inline]
    pub fn libraries_dir(&self) -> PathBuf {
        self.metadata_dir().join("libraries")
    }

    /// Get the Minecraft assets metadata directory
    #[inline]
    pub fn assets_dir(&self) -> PathBuf {
        self.metadata_dir().join("assets")
    }

    /// Get the assets index directory
    #[inline]
    pub fn assets_index_dir(&self) -> PathBuf {
        self.assets_dir().join("indexes")
    }

    /// Get the assets objects directory
    #[inline]
    pub fn objects_dir(&self) -> PathBuf {
        self.assets_dir().join("objects")
    }

    /// Get the directory for a specific object
    #[inline]
    pub fn object_dir(&self, hash: &str) -> PathBuf {
        self.objects_dir().join(&hash[..2]).join(hash)
    }

    /// Get the Minecraft log config's directory
    #[inline]
    pub fn log_configs_dir(&self) -> PathBuf {
        self.metadata_dir().join("log_configs")
    }

    /// Get the Minecraft legacy assets metadata directory
    #[inline]
    pub fn legacy_assets_dir(&self) -> PathBuf {
        self.metadata_dir().join("resources")
    }

    /// Get the Minecraft legacy assets metadata directory
    #[inline]
    pub fn natives_dir(&self) -> PathBuf {
        self.metadata_dir().join("natives")
    }

    /// Get the natives directory for a version of Minecraft
    #[inline]
    pub fn version_natives_dir(&self, version: &str) -> PathBuf {
        self.natives_dir().join(version)
    }

    /// Get the directory containing instance icons
    #[inline]
    pub fn icon_dir(&self) -> PathBuf {
        self.config_dir.join("icons")
    }

    /// Get the instances directory
    #[inline]
    pub fn instances_dir(&self) -> PathBuf {
        self.config_dir.join(INSTANCES_FOLDER_NAME)
    }

    /// Get the directory containing managed dedicated servers
    #[inline]
    pub fn servers_dir(&self) -> PathBuf {
        self.config_dir.join(SERVERS_FOLDER_NAME)
    }

    /// Gets the directory of a managed dedicated server by id
    #[inline]
    pub fn server_dir(&self, server_id: &str) -> crate::Result<PathBuf> {
        crate::util::io::safe_relative_join(&self.servers_dir(), server_id)
    }

    #[inline]
    pub fn install_rollbacks_dir(&self) -> PathBuf {
        self.config_dir.join(INSTALL_ROLLBACKS_FOLDER_NAME)
    }

    /// Gets the logs dir for a given instance path
    #[inline]
    pub fn instance_logs_dir(&self, instance_path: &str) -> PathBuf {
        self.instances_dir().join(instance_path).join("logs")
    }

    /// Gets the logs dir for a resolved game directory (honours a per-instance
    /// `game_dir_override`), so launcher-captured logs follow the game dir.
    #[inline]
    pub fn game_logs_dir(&self, game_dir: &std::path::Path) -> PathBuf {
        game_dir.join("logs")
    }

    /// Gets the crash reports dir for a given instance path
    #[inline]
    pub fn crash_reports_dir(&self, instance_path: &str) -> PathBuf {
        self.instances_dir()
            .join(instance_path)
            .join("crash-reports")
    }

    /// Gets the crash reports dir for a resolved game directory (honours a
    /// per-instance `game_dir_override`), so game-written crash reports are
    /// read from the same place the game writes them.
    #[inline]
    pub fn game_crash_reports_dir(
        &self,
        game_dir: &std::path::Path,
    ) -> PathBuf {
        game_dir.join("crash-reports")
    }

    /// Resolve the game working directory (the "content" directory the game
    /// actually reads and writes: mods, saves, config, logs, crash-reports,
    /// options.txt, resourcepacks, datapacks, shaders, worlds) for an instance.
    ///
    /// Returns the per-instance override when set (an external /
    /// non-version-isolated folder), otherwise the managed folder under the
    /// instances directory.
    ///
    /// This is the SOLE resolver for anything that represents the game's own
    /// content. Launcher-owned bookkeeping (instance config file, icon, install
    /// rollbacks, content-backup metadata) must keep using `instances_dir()` and
    /// not go through this function.
    pub fn resolve_game_dir(
        &self,
        instance_path: &str,
        game_dir_override: Option<&str>,
    ) -> PathBuf {
        match game_dir_override {
            // The override must be an absolute path; a relative override would
            // make the game's working directory depend on the process cwd, so
            // treat it as unset and fall back to the managed folder.
            Some(override_dir)
                if !override_dir.is_empty()
                    && (Path::new(override_dir).is_absolute()
                        || Self::is_absolute_override(override_dir)) =>
            {
                PathBuf::from(override_dir)
            }
            _ => self.instances_dir().join(instance_path),
        }
    }

    /// Convenience: `resolve_game_dir` for an `Instance`, borrowing its
    /// relative `path` and optional `game_dir_override`.
    pub fn instance_game_dir(
        &self,
        instance: &crate::state::instances::Instance,
    ) -> PathBuf {
        // Symlink imports persist the exact external target independently of
        // the optional game-dir override. Use it as a fallback so content
        // management keeps operating on the real directory even for older
        // imports that predate the override field.
        let game_dir = instance
            .game_dir_override
            .as_deref()
            .or(instance.symlink_target.as_deref());
        self.resolve_game_dir(&instance.path, game_dir)
    }

    /// `Path::is_absolute` treats a Windows drive-letter path (e.g.
    /// `D:\Games\.minecraft`) as relative on non-Windows builds. Overrides are
    /// persisted and round-trip across OSes, so a drive-letter path must be
    /// honored as absolute on every platform.
    fn is_absolute_override(value: &str) -> bool {
        let bytes = value.as_bytes();
        bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\')
    }

    #[inline]
    pub fn launcher_logs_dir(&self) -> Option<PathBuf> {
        self.get_initial_settings_dir()
            .map(|d| d.join(LAUNCHER_LOGS_FOLDER_NAME))
    }

    #[inline]
    pub fn launcher_logs_dir_path(app_identifier: &str) -> Option<PathBuf> {
        Self::initial_settings_dir_path(app_identifier)
            .map(|d| d.join(LAUNCHER_LOGS_FOLDER_NAME))
    }

    /// Get the cache directory for Theseus
    #[inline]
    pub fn caches_dir(&self) -> PathBuf {
        self.config_dir.join(CACHES_FOLDER_NAME)
    }

    pub fn resource_cache_db(&self, channel: &str) -> crate::Result<PathBuf> {
        if !matches!(channel, "release" | "beta") {
            return Err(crate::ErrorKind::InputError(
                "Invalid resource cache channel".into(),
            )
            .into());
        }
        Ok(self
            .caches_dir()
            .join("resources")
            .join(channel)
            .join("resource-cache.db"))
    }

    pub fn content_store_dir(&self) -> PathBuf {
        self.caches_dir().join("content-store")
    }

    /// Get path from environment variable
    #[inline]
    fn env_path(name: &str) -> Option<PathBuf> {
        std::env::var_os(name).map(PathBuf::from)
    }

    #[tracing::instrument(skip(settings, exec, io_semaphore))]
    pub async fn move_launcher_directory<'a, E>(
        settings: &mut Settings,
        exec: E,
        io_semaphore: &IoSemaphore,
        app_identifier: &str,
    ) -> crate::Result<()>
    where
        E: sqlx::Executor<'a, Database = sqlx::Sqlite>
            + sqlx::Acquire<'a, Database = sqlx::Sqlite>
            + Copy,
    {
        let app_dir = DirectoryInfo::initial_settings_dir_path(app_identifier)
            .ok_or(crate::ErrorKind::FSError(
                "Could not find valid config dir".to_string(),
            ))?;

        if let Some(ref prev_custom_dir) = settings.prev_custom_dir {
            let prev_dir = PathBuf::from(prev_custom_dir);

            let move_dir = settings
                .custom_dir
                .as_ref()
                .map_or_else(|| app_dir.clone(), PathBuf::from);

            async fn is_dir_writable(
                new_config_dir: &Path,
            ) -> crate::Result<bool> {
                let temp_path = new_config_dir.join(".tmp");
                match fs::write(temp_path.clone(), "test").await {
                    Ok(_) => {
                        fs::remove_file(temp_path).await?;
                        Ok(true)
                    }
                    Err(e) => {
                        tracing::error!(
                            "Error writing to new config dir: {}",
                            e
                        );
                        Ok(false)
                    }
                }
            }

            fn get_disk_usage(path: &Path) -> crate::Result<Option<u64>> {
                let path = crate::util::io::canonicalize(path)?;

                let disks = sysinfo::Disks::new_with_refreshed_list();

                for disk in &disks {
                    if path.starts_with(disk.mount_point()) {
                        return Ok(Some(disk.available_space()));
                    }
                }

                Ok(None)
            }

            let new_dir = move_dir.to_string_lossy().to_string();

            if prev_dir != move_dir {
                let loader_bar_id = init_loading(
                    LoadingBarType::DirectoryMove {
                        old: prev_dir.clone(),
                        new: move_dir.clone(),
                    },
                    100.0,
                    "Moving launcher directory",
                )
                .await?;

                if !is_dir_writable(&move_dir).await? {
                    return Err(crate::ErrorKind::DirectoryMoveError(format!("Cannot move directory to {}: directory is not writable", move_dir.display())).into());
                }

                if settings.backup_repository_path.is_none() {
                    let old_repository =
                        prev_dir.join("Backups").join("instances");
                    let new_repository =
                        move_dir.join("Backups").join("instances");
                    if let Some(path) = crate::api::instance::move_default_repository_for_launcher_directory(
                        &old_repository,
                        &new_repository,
                    )
                    .await?
                    {
                        sqlx::query(
                            "INSERT INTO pending_backup_repository_cleanups (path, created_at)
                             VALUES (?, unixepoch('subsec') * 1000)
                             ON CONFLICT(path) DO UPDATE SET created_at = excluded.created_at",
                        )
                        .bind(path.to_string_lossy().to_string())
                        .execute(exec)
                        .await?;
                    }
                }

                const MOVE_DIRS: &[&str] = &[
                    CACHES_FOLDER_NAME,
                    INSTANCES_FOLDER_NAME,
                    METADATA_FOLDER_NAME,
                ];

                struct MovePath {
                    old: PathBuf,
                    new: PathBuf,
                    size: u64,
                }

                async fn add_paths(
                    source: &Path,
                    destination: &Path,
                    paths: &mut Vec<MovePath>,
                    total_size: &mut u64,
                ) -> crate::Result<()> {
                    if !source.exists() {
                        crate::util::io::create_dir_all(source).await?;
                    }

                    if !destination.exists() {
                        crate::util::io::create_dir_all(destination).await?;
                    }

                    for entry_path in
                        crate::pack::import::get_all_subfiles(source, false)
                            .await?
                    {
                        let relative_path = entry_path.strip_prefix(source)?;
                        let new_path = destination.join(relative_path);
                        let path_size =
                            entry_path.metadata().map(|x| x.len()).unwrap_or(0);

                        *total_size += path_size;

                        paths.push(MovePath {
                            old: entry_path,
                            new: new_path,
                            size: path_size,
                        });
                    }

                    Ok(())
                }

                let mut paths: Vec<MovePath> = vec![];
                let mut total_size = 0;

                for dir in MOVE_DIRS {
                    add_paths(
                        &prev_dir.join(dir),
                        &move_dir.join(dir),
                        &mut paths,
                        &mut total_size,
                    )
                    .await?;
                    emit_loading(
                        &loader_bar_id,
                        10.0 / (MOVE_DIRS.len() as f64),
                        None,
                    )?;
                }

                let paths_len = paths.len();

                if crate::util::io::is_same_disk(&prev_dir, &move_dir)
                    .unwrap_or(false)
                {
                    let success_idxs = Arc::new(DashSet::new());

                    let loader_bar_id = Arc::new(&loader_bar_id);
                    let res =
                        futures::future::try_join_all(paths.iter().enumerate().map(|(idx, x)| {
                            let loader_bar_id = loader_bar_id.clone();
                            let success_idxs = success_idxs.clone();

                            async move {
                                let _permit = io_semaphore.0.acquire().await?;

                                if let Some(parent) = x.new.parent() {
                                    crate::util::io::create_dir_all(parent).await.map_err(|e| {
                                        crate::Error::from(crate::ErrorKind::DirectoryMoveError(
                                            format!(
                                                "Failed to create directory {}: {}",
                                                parent.display(),
                                                e
                                            )
                                        ))
                                    })?;
                                }

                                crate::util::io::rename_or_move(
                                    &x.old,
                                    &x.new,
                                )
                                .await
                                    .map_err(|e| {
                                        crate::Error::from(crate::ErrorKind::DirectoryMoveError(
                                            format!(
                                                "Failed to move directory from {} to {}: {e:?}",
                                                x.old.display(),
                                                x.new.display(),
                                            ),
                                        ))
                                    })?;

                                let _ = emit_loading(
                                    &loader_bar_id,
                                    90.0 / paths_len as f64,
                                    None,
                                );

                                success_idxs.insert(idx);

                                Ok::<(), crate::Error>(())
                            }
                        }))
                        .await;

                    if let Err(e) = res {
                        for idx in success_idxs.iter() {
                            let path = &paths[*idx.key()];

                            let res =
                                tokio::fs::rename(&path.new, &path.old).await;

                            if let Err(e) = res {
                                tracing::warn!(
                                    "Failed to rollback directory {}: {}",
                                    path.new.display(),
                                    e
                                );
                            }
                        }

                        return Err(e);
                    }
                } else {
                    if let Some(disk_usage) = get_disk_usage(&move_dir)?
                        && total_size > disk_usage
                    {
                        return Err(crate::ErrorKind::DirectoryMoveError(format!("Not enough space to move directory to {}: only {} bytes available", app_dir.display(), disk_usage)).into());
                    }

                    let loader_bar_id = Arc::new(&loader_bar_id);
                    futures::future::try_join_all(paths.iter().map(|x| {
                        let loader_bar_id = loader_bar_id.clone();

                        async move {
                            crate::util::fetch::copy(
                                &x.old,
                                &x.new,
                                io_semaphore,
                            )
                            .await.map_err(|e| { crate::Error::from(
                                crate::ErrorKind::DirectoryMoveError(format!("Failed to move directory from {} to {}: {e:?}", x.old.display(), x.new.display())))
                            })?;

                            let _ = emit_loading(
                                &loader_bar_id,
                                ((x.size as f64) / (total_size as f64)) * 60.0,
                                None,
                            );

                            Ok::<(), crate::Error>(())
                        }
                    }))
                    .await?;

                    futures::future::join_all(paths.iter().map(|x| {
                        let loader_bar_id = loader_bar_id.clone();

                        async move {
                            let res = async {
                                let _permit = io_semaphore.0.acquire().await?;
                                crate::util::io::remove_file(&x.old).await?;

                                emit_loading(
                                    &loader_bar_id,
                                    30.0 / paths_len as f64,
                                    None,
                                )?;

                                Ok::<(), crate::Error>(())
                            };

                            if let Err(e) = res.await {
                                tracing::warn!(
                                    "Failed to remove old file {}: {}",
                                    x.old.display(),
                                    e
                                );
                            }
                        }
                    }))
                    .await;
                }

                let java_versions = JavaVersion::get_all(exec).await?;
                for java_version in java_versions {
                    let new_java_path = java_version.path.replace(
                        prev_custom_dir,
                        new_dir.trim_end_matches('/').trim_end_matches('\\'),
                    );
                    if crate::util::jre::is_java_install_staging_path(
                        Path::new(&new_java_path),
                    ) {
                        tracing::warn!(
                            java = %new_java_path,
                            "Dropping incomplete Java installation during directory migration"
                        );
                        JavaVersion::delete(&java_version.path, exec).await?;
                        continue;
                    }
                    if new_java_path != java_version.path {
                        JavaVersion::update_path(
                            &java_version.path,
                            &new_java_path,
                            exec,
                        )
                        .await?;
                    }
                }
                sqlx::query(
                    "
                    UPDATE discovered_javas
                    SET path = replace(path, $1, $2)
                    WHERE path LIKE $1 || '%'
                    ",
                )
                .bind(prev_custom_dir)
                .bind(new_dir.trim_end_matches('/').trim_end_matches('\\'))
                .execute(exec)
                .await?;

                let new_dir = new_dir
                    .trim_end_matches('/')
                    .trim_end_matches('\\')
                    .to_string();
                let new_dir = new_dir.as_str();
                sqlx::query!(
                    "
                    UPDATE instances
                    SET icon_path = replace(icon_path, ?, ?)
                    WHERE icon_path IS NOT NULL
                    ",
                    prev_custom_dir,
                    new_dir,
                )
                .execute(exec)
                .await?;
                sqlx::query!(
                    "
                    UPDATE instance_files
                    SET icon_path = replace(icon_path, ?, ?)
                    WHERE icon_path IS NOT NULL AND icon_path != ''
                    ",
                    prev_custom_dir,
                    new_dir,
                )
                .execute(exec)
                .await?;
                sqlx::query!(
                    "
                    UPDATE instance_launch_overrides
                    SET overrides = jsonb(json_set(
                        overrides,
                        '$.java_path',
                        replace(json_extract(overrides, '$.java_path'), ?, ?)
                    ))
                    WHERE json_type(overrides, '$.java_path') = 'text'
                    ",
                    prev_custom_dir,
                    new_dir,
                )
                .execute(exec)
                .await?;
                crate::state::instances::adapters::sqlite::config_sync_rows::mark_all_config_dirty(
                    exec,
                )
                .await?;
            }

            settings.custom_dir = Some(new_dir);
        }

        settings.prev_custom_dir.clone_from(&settings.custom_dir);
        if settings.custom_dir.is_none() {
            settings.custom_dir = Some(app_dir.to_string_lossy().to_string());
        }

        settings.update(exec).await?;

        Ok(())
    }
}

#[cfg(test)]
mod resolve_game_dir_tests {
    use super::DirectoryInfo;
    use std::path::PathBuf;

    fn dirs() -> DirectoryInfo {
        DirectoryInfo {
            settings_dir: PathBuf::from(r"C:\launcher\settings"),
            config_dir: PathBuf::from(r"C:\launcher"),
            app_identifier: "test".to_string(),
        }
    }

    #[test]
    fn server_directory_rejects_absolute_and_windows_alias_paths() {
        let dirs = dirs();
        assert_eq!(
            dirs.server_dir("server-1234").unwrap(),
            dirs.servers_dir().join("server-1234")
        );
        for id in [
            "../outside",
            "C:outside",
            "C:/outside",
            r"\outside",
            r"\\server\share",
            ".",
            "NUL",
            "server:stream",
            "server.",
        ] {
            assert!(dirs.server_dir(id).is_err(), "{id:?}");
        }
    }

    #[test]
    fn override_used_when_set_and_absolute() {
        let dirs = dirs();
        assert_eq!(
            dirs.resolve_game_dir("inst", Some(r"D:\Games\.minecraft")),
            PathBuf::from(r"D:\Games\.minecraft")
        );
    }

    #[test]
    fn managed_folder_used_without_override() {
        let dirs = dirs();
        assert_eq!(
            dirs.resolve_game_dir("inst", None),
            dirs.instances_dir().join("inst")
        );
    }

    #[test]
    fn managed_folder_used_for_empty_override() {
        let dirs = dirs();
        assert_eq!(
            dirs.resolve_game_dir("inst", Some("")),
            dirs.instances_dir().join("inst")
        );
    }

    #[test]
    fn managed_folder_used_for_relative_override() {
        // A relative override must be treated as unset to avoid a cwd-dependent
        // game working directory.
        let dirs = dirs();
        assert_eq!(
            dirs.resolve_game_dir("inst", Some(r"relative\dir")),
            dirs.instances_dir().join("inst")
        );
    }
}
