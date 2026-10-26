//! Launcher-dialect resolution and launch glue for directly linked Minecraft
//! installations. Linked folders are read in place; native archives are the
//! only data copied, and they are extracted into Axolotl's own cache.

use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use daedalus::minecraft::{
    AssetIndex, Library, LibraryDownload, Os, VersionInfo, VersionType,
};
use daedalus::modded::normalize_loader_libraries;

use super::local_version::{
    HmclVersionSettings, LinkedLibrary, MergedVersion, discover_version_json,
    load_version_for_dialect,
};
use crate::state::{Instance, MemorySettings, ModLoader, WindowSize};

/// Selects where a conventional external `.minecraft` installation stores
/// instance-owned content. `Automatic` is retained only for legacy links and
/// follows the owning launcher's original configuration.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ExternalGameDirMode {
    Automatic,
    Isolated,
    Shared,
}

impl ExternalGameDirMode {
    pub fn parse(value: &str) -> crate::Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "automatic" => Ok(Self::Automatic),
            "isolated" => Ok(Self::Isolated),
            "shared" => Ok(Self::Shared),
            value => Err(crate::ErrorKind::LauncherError(format!(
                "Unsupported external Minecraft directory mode: {value}"
            ))
            .into()),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Isolated => "isolated",
            Self::Shared => "shared",
        }
    }
}

/// Returns the conventional version-metadata directory for an ordinary
/// instance created against an external `.minecraft` root. The game working
/// directory may be either this version directory or the root itself.
pub(crate) fn external_version_dir_for_game_override(
    instance: &Instance,
) -> Option<(PathBuf, ExternalGameDirMode)> {
    let game_dir_override = instance.game_dir_override.as_deref()?;
    let game_dir = PathBuf::from(game_dir_override);

    if game_dir
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("versions"))
    {
        return Some((game_dir, ExternalGameDirMode::Isolated));
    }

    game_dir
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(".minecraft"))
        .then(|| {
            (
                game_dir.join("versions").join(&instance.name),
                ExternalGameDirMode::Shared,
            )
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkedLauncherDialect {
    Hmcl,
    Pcl,
    PclCe,
    Generic,
}

impl LinkedLauncherDialect {
    pub fn parse(value: &str) -> crate::Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "hmcl" => Ok(Self::Hmcl),
            "pcl" | "pcl2" => Ok(Self::Pcl),
            "pcl_ce" | "pcl2_ce" | "pcl2ce" => Ok(Self::PclCe),
            "generic" => Ok(Self::Generic),
            value => Err(crate::ErrorKind::LauncherError(format!(
                "Unsupported linked launcher dialect: {value}"
            ))
            .into()),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DirectLinkedLaunch {
    pub dot_minecraft: PathBuf,
    pub launcher_root: Option<PathBuf>,
    pub version_id: String,
    pub version_json: Option<PathBuf>,
    pub dialect: LinkedLauncherDialect,
    pub game_dir_mode: Option<ExternalGameDirMode>,
}

pub(crate) struct ResolvedLinkedLaunch {
    pub merged: MergedVersion,
    pub game_dir: PathBuf,
}

impl DirectLinkedLaunch {
    /// Builds a generic direct link for an instance created in
    /// .minecraft/versions/<id>. This is intentionally derived at runtime
    /// instead of persisted in the database, preserving compatibility for
    /// instances created before external direct management was added.
    pub(crate) fn from_external_version_dir(
        version_dir: &Path,
    ) -> crate::Result<Option<Self>> {
        let Some(folder_name) =
            version_dir.file_name().and_then(|name| name.to_str())
        else {
            return Ok(None);
        };
        if version_dir
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .is_none_or(|name| !name.eq_ignore_ascii_case("versions"))
        {
            return Ok(None);
        }
        let Some(dot_minecraft) = version_dir.parent().and_then(Path::parent)
        else {
            return Ok(None);
        };
        // Older externally-created records contain only `game_dir_override`.
        // Do not assume their display folder and manifest stem match: PCL and
        // copied installations commonly keep a user-facing folder name while
        // the actual JSON is named after the version ID. Resolving the exact
        // manifest here keeps those rows on the external runtime adapter,
        // instead of later falling back to Axolotl's managed libraries.
        let version_json = discover_version_json(dot_minecraft, folder_name)?;
        let version_id = version_json
            .file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|stem| !stem.is_empty())
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Version JSON has no usable file stem: {}",
                    version_json.display()
                ))
            })?;
        Ok(Some(Self {
            dot_minecraft: crate::util::io::canonicalize(dot_minecraft)?,
            launcher_root: None,
            version_id: version_id.to_string(),
            version_json: Some(crate::util::io::canonicalize(version_json)?),
            dialect: LinkedLauncherDialect::Generic,
            game_dir_mode: Some(ExternalGameDirMode::Isolated),
        }))
    }

    /// Builds a direct link for a newly created external instance before its
    /// link metadata has been persisted. This handles both the conventional
    /// version-isolated override and the shared `.minecraft` root override.
    pub(crate) fn from_game_dir_override(
        instance: &Instance,
    ) -> crate::Result<Option<Self>> {
        let Some((version_dir, game_dir_mode)) =
            external_version_dir_for_game_override(instance)
        else {
            return Ok(None);
        };
        let has_version_json = version_dir
            .read_dir()
            .ok()
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .any(|entry| {
                entry.path().extension().and_then(|ext| ext.to_str())
                    == Some("json")
            });
        let mut direct = match Self::from_external_version_dir(&version_dir) {
            Ok(Some(direct)) => direct,
            Ok(None) => return Ok(None),
            Err(error) if has_version_json => return Err(error),
            Err(_) => {
                let Some(version_id) =
                    version_dir.file_name().and_then(|name| name.to_str())
                else {
                    return Ok(None);
                };
                let Some(dot_minecraft) =
                    version_dir.parent().and_then(Path::parent)
                else {
                    return Ok(None);
                };
                Self {
                    dot_minecraft: crate::util::io::canonicalize(
                        dot_minecraft,
                    )?,
                    launcher_root: None,
                    version_id: version_id.to_string(),
                    version_json: None,
                    dialect: LinkedLauncherDialect::Generic,
                    game_dir_mode: Some(game_dir_mode),
                }
            }
        };
        direct.game_dir_mode = Some(game_dir_mode);
        Ok(Some(direct))
    }

    pub(crate) fn from_instance(
        instance: &Instance,
    ) -> crate::Result<Option<Self>> {
        let fields = (
            instance.linked_launcher.as_deref(),
            instance.linked_dot_minecraft.as_deref(),
            instance.linked_version_id.as_deref(),
        );
        let has_any_linked_metadata = fields.0.is_some()
            || fields.1.is_some()
            || fields.2.is_some()
            || instance.linked_launcher_root.is_some()
            || instance.linked_version_json_path.is_some();
        if !has_any_linked_metadata {
            return Ok(None);
        }
        let (launcher, dot_minecraft, version_id) = match fields {
            (Some(launcher), Some(dot_minecraft), Some(version_id))
                if !launcher.trim().is_empty()
                    && !dot_minecraft.trim().is_empty()
                    && !version_id.trim().is_empty() =>
            {
                (launcher, dot_minecraft, version_id)
            }
            _ => {
                return Err(crate::ErrorKind::LauncherError(
                    "Directly linked instance metadata is incomplete"
                        .to_string(),
                )
                .into());
            }
        };

        Ok(Some(Self {
            dot_minecraft: PathBuf::from(dot_minecraft),
            launcher_root: instance
                .linked_launcher_root
                .as_deref()
                .map(PathBuf::from),
            version_id: version_id.to_string(),
            version_json: instance
                .linked_version_json_path
                .as_deref()
                .map(PathBuf::from),
            dialect: LinkedLauncherDialect::parse(launcher)?,
            game_dir_mode: instance
                .linked_game_dir_mode
                .as_deref()
                .map(ExternalGameDirMode::parse)
                .transpose()?,
        }))
    }

    pub(crate) fn resolve(&self) -> crate::Result<ResolvedLinkedLaunch> {
        let merged = load_version_for_dialect(
            &self.dot_minecraft,
            &self.version_id,
            self.version_json.as_deref(),
            self.dialect,
        )?;
        let game_dir = match self.game_dir_mode {
            Some(ExternalGameDirMode::Isolated) => self.version_dir(),
            Some(ExternalGameDirMode::Shared) => self.dot_minecraft.clone(),
            Some(ExternalGameDirMode::Automatic) | None => match self.dialect {
                LinkedLauncherDialect::Pcl | LinkedLauncherDialect::PclCe => {
                    resolve_pcl_game_dir(
                        &self.dot_minecraft,
                        &self.version_dir(),
                        self.dialect,
                        &merged,
                        self.launcher_root.as_deref(),
                    )?
                }
                LinkedLauncherDialect::Hmcl => resolve_hmcl_game_dir(
                    self.launcher_root.as_deref(),
                    &self.dot_minecraft,
                    &self.version_dir(),
                )?,
                // A generic direct link is the `.minecraft/versions/<id>` format;
                // its content is always isolated beside the version metadata,
                // including when the directory is currently empty.
                LinkedLauncherDialect::Generic => self.version_dir(),
            },
        };
        Ok(ResolvedLinkedLaunch { merged, game_dir })
    }

    pub(crate) fn libraries_dir(&self) -> PathBuf {
        self.dot_minecraft.join("libraries")
    }

    pub(crate) fn assets_dir(&self) -> PathBuf {
        self.dot_minecraft.join("assets")
    }

    pub(crate) fn log_configs_dir(&self) -> PathBuf {
        self.assets_dir().join("log_configs")
    }

    pub(crate) fn version_dir(&self) -> PathBuf {
        self.version_json
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| {
                self.dot_minecraft.join("versions").join(&self.version_id)
            })
    }

    pub(crate) fn client_jar(&self, jar_id: &str) -> PathBuf {
        if jar_id == self.version_id {
            let version_dir = self.version_dir();
            let by_id = version_dir.join(format!("{jar_id}.jar"));
            if by_id.is_file() {
                return by_id;
            }
            if let Some(folder_name) =
                version_dir.file_name().and_then(|name| name.to_str())
            {
                let by_folder = version_dir.join(format!("{folder_name}.jar"));
                if by_folder.is_file() {
                    return by_folder;
                }
            }
            by_id
        } else {
            let inherited = self
                .dot_minecraft
                .join("versions")
                .join(jar_id)
                .join(format!("{jar_id}.jar"));
            if inherited.is_file() {
                return inherited;
            }
            let version_dir = self.version_dir();
            if let Some(folder_name) =
                version_dir.file_name().and_then(|name| name.to_str())
            {
                let by_folder = version_dir.join(format!("{folder_name}.jar"));
                if by_folder.is_file() {
                    return by_folder;
                }
            }
            inherited
        }
    }

    pub(crate) fn natives_cache_key(&self) -> String {
        let identity = format!(
            "{}\0{}\0{}",
            self.dot_minecraft.display(),
            self.version_id,
            self.version_json
                .as_deref()
                .unwrap_or(self.dot_minecraft.as_path())
                .display()
        );
        format!(
            "linked-{}",
            sha1_smol::Sha1::from(identity.as_bytes()).hexdigest()
        )
    }

    pub(crate) fn library_path(
        &self,
        library: &LinkedLibrary,
    ) -> crate::Result<PathBuf> {
        if library.hint.as_deref() == Some("local") {
            let relative =
                library.filename.as_deref().map(PathBuf::from).unwrap_or(
                    maven_artifact_filename(&library.library.name)?.into(),
                );
            Ok(self.version_dir().join("libraries").join(relative))
        } else {
            Ok(self
                .libraries_dir()
                .join(library.classpath_relative_path()?))
        }
    }
}

/// HMCL commonly keeps an isolated instance's content beside its version JSON
/// without a launcher-specific config file. Prefer that directory when it
/// contains instance-owned content; otherwise use the shared `.minecraft`
/// root. Generic direct links use the isolated directory unconditionally.
fn resolve_content_game_dir(
    dot_minecraft: &Path,
    version_dir: &Path,
) -> crate::Result<PathBuf> {
    for name in [
        "mods",
        "config",
        "saves",
        "resourcepacks",
        "shaderpacks",
        "datapacks",
        "schematics",
        "screenshots",
    ] {
        let path = version_dir.join(name);
        if path.is_dir() {
            return Ok(version_dir.to_path_buf());
        }
    }
    Ok(dot_minecraft.to_path_buf())
}

fn resolve_hmcl_game_dir(
    launcher_root: Option<&Path>,
    dot_minecraft: &Path,
    version_dir: &Path,
) -> crate::Result<PathBuf> {
    if let Some(launcher_root) = launcher_root
        && let Some(game_dir) =
            crate::api::pack::import::hmcl::configured_game_dir(
                launcher_root,
                dot_minecraft,
                version_dir,
            )
    {
        return Ok(game_dir);
    }
    resolve_content_game_dir(dot_minecraft, version_dir)
}

/// Applies the managed-install loader normalization to a directly linked
/// merge, reusing the shared `daedalus::modded::normalize_loader_libraries`
/// rule without duplicating its coordinates.
///
/// Only Cleanroom-linked versions are affected: the shared rule drops the
/// whole vanilla LWJGL 2 family at `2.9.4-nightly-20150209` from the merged
/// list — the `org.lwjgl.lwjgl:lwjgl` binding, `org.lwjgl.lwjgl:lwjgl_util`,
/// and the natives-only carrier `org.lwjgl.lwjgl:lwjgl-platform` whose
/// root-level extracted `lwjgl.dll` / `liblwjgl.so` / `liblwjgl.dylib` is
/// what `System.loadLibrary("lwjgl")` (org.lwjgl.Sys) resolves first in the
/// natives directory, while the Cleanroom LWJGL 3 natives are nested.
/// HMCL never classpaths native jars and its first-wins unzip protects its
/// natives directory; a direct link merges the vanilla parent after the
/// loader (last-writer-wins extraction) and ships the old root-level
/// LWJGL 2 native, so the carrier must not reach the extraction pass at
/// all. The merged list drives the direct ensure pass, native extraction,
/// the launch classpath, and the `VersionInfo` projection, so normalizing
/// it before those consumers run covers direct ensure, classpath, and
/// launch at once. The Cleanroom LWJGL 3 line and `com.cleanroommc:lwjglxx`
/// are untouched; the vanilla JNA platform and the Mojang ICU bundle are
/// removed by the pre-existing Axolotl Cleanroom rule (not HMCL logic).
/// Returns the removed coordinates for logging.
pub(crate) fn normalize_merged_loader_libraries(
    loader: ModLoader,
    merged: &mut MergedVersion,
) -> Vec<String> {
    // The frozen content-set label can mislabel a directly linked Cleanroom
    // installation as vanilla when the association predates Cleanroom loader
    // detection. Fall back only for a strictly vanilla label: a merged
    // library carrying the Cleanroom loader jar itself (`com.cleanroommc:cleanroom`)
    // is then authoritative. Any other frozen label (Forge and friends) keeps
    // the launcher's own classification untouched.
    let is_cleanroom = loader == ModLoader::Cleanroom
        || (loader == ModLoader::Vanilla
            && merged.libraries.iter().any(|library| {
                library
                    .library
                    .name
                    .starts_with("com.cleanroommc:cleanroom:")
            }));
    if !is_cleanroom {
        return Vec::new();
    }

    // The linked list wraps daedalus libraries with community-launcher
    // fields; evaluate the shared rule against a plain projection, then
    // drop the same coordinates from the linked list in place.
    let mut projection: Vec<Library> = merged
        .libraries
        .iter()
        .map(|library| library.library.clone())
        .collect();
    let removed =
        normalize_loader_libraries("cleanroom", &merged.id, &mut projection);
    if !removed.is_empty() {
        let removed_names: std::collections::HashSet<&str> =
            removed.iter().map(String::as_str).collect();
        merged.libraries.retain(|library| {
            !removed_names.contains(library.library.name.as_str())
        });
    }
    removed
}

pub(crate) fn merged_to_version_info(
    mut merged: MergedVersion,
) -> crate::Result<VersionInfo> {
    let main_class = merged.main_class.take().ok_or_else(|| {
        crate::ErrorKind::LauncherError(format!(
            "Local version \"{}\" does not declare a mainClass",
            merged.id
        ))
    })?;
    let asset_id = merged
        .assets
        .clone()
        .or_else(|| merged.asset_index.as_ref().map(|index| index.id.clone()))
        .unwrap_or_else(|| "legacy".to_string());
    let asset_index = merged.asset_index.take().unwrap_or_else(|| AssetIndex {
        id: asset_id.clone(),
        sha1: String::new(),
        size: 0,
        total_size: 0,
        url: String::new(),
    });

    Ok(VersionInfo {
        arguments: merged.arguments.take(),
        assets: asset_id,
        asset_index,
        downloads: merged.downloads.take().unwrap_or_default(),
        id: merged.id,
        java_version: merged.java_version.take(),
        libraries: std::mem::take(&mut merged.libraries)
            .into_iter()
            .map(|library| library.library)
            .collect(),
        logging: merged.logging.take(),
        main_class,
        minecraft_arguments: merged.minecraft_arguments.take(),
        minimum_launcher_version: merged.minimum_launcher_version.unwrap_or(0),
        release_time: merged.release_time.unwrap_or(DateTime::<Utc>::MIN_UTC),
        time: merged.time.unwrap_or(DateTime::<Utc>::MIN_UTC),
        type_: merged.type_.take().unwrap_or(VersionType::Release),
        data: None,
        processors: None,
    })
}

pub(crate) fn conservative_launch_facts(
    merged: &MergedVersion,
) -> (bool, super::QuickPlayVersion) {
    use super::quick_play_version::{
        QuickPlayServerVersion, QuickPlaySingleplayerVersion,
    };

    let modern = merged
        .java_version
        .as_ref()
        .is_none_or(|java| java.major_version >= 17);
    (
        modern,
        super::QuickPlayVersion {
            server: QuickPlayServerVersion::Unsupported,
            singleplayer: QuickPlaySingleplayerVersion::Unsupported,
        },
    )
}

/// Rust port of HMCL `StringUtils.tokenize` (commit
/// `083dbb18ade1c935e2e56d0bdefcd718be1e2ed6`, invoked without variables),
/// which is how HMCL splits its javaArgs string: arguments separate on spaces,
/// single- and double-quoted segments stay one argument with the quotes
/// stripped, an unclosed quote runs to the end of the string, backslashes are
/// always literal, and inside double quotes a backtick escapes the next
/// character (letters expanding to their C-style control characters).
fn hmcl_tokenize(input: &str) -> Vec<String> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut has_value = false;
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\'' => {
                has_value = true;
                let end = chars[i + 1..]
                    .iter()
                    .position(|&ch| ch == '\'')
                    .map_or(chars.len(), |offset| i + 1 + offset);
                current.extend(&chars[i + 1..end]);
                i = end + 1;
            }
            '"' => {
                has_value = true;
                i += 1;
                while i < chars.len() {
                    let character = chars[i];
                    i += 1;
                    match character {
                        '"' => break,
                        '`' if i < chars.len() => {
                            let escaped = chars[i];
                            i += 1;
                            current.push(match escaped {
                                'a' => '\u{7}',
                                'b' => '\u{8}',
                                'f' => '\u{c}',
                                'n' => '\n',
                                'r' => '\r',
                                't' => '\t',
                                'v' => '\u{b}',
                                other => other,
                            });
                        }
                        _ => current.push(character),
                    }
                }
            }
            ' ' => {
                if has_value {
                    tokens.push(std::mem::take(&mut current));
                    has_value = false;
                }
                i += 1;
            }
            character => {
                has_value = true;
                current.push(character);
                i += 1;
            }
        }
    }
    if has_value {
        tokens.push(current);
    }
    tokens
}

/// Applies the HMCL private per-version settings that map onto Axolotl's
/// launch configuration. `java_major` is the major version of the JVM chosen
/// for this launch; `None` means it could not be determined.
///
/// Returns extra *game* arguments that must be appended after the vanilla
/// game arguments (currently the auto-connect server mapping).
pub(crate) fn apply_hmcl_settings(
    settings: &HmclVersionSettings,
    memory: &mut MemorySettings,
    resolution: &mut WindowSize,
    java_args: &mut Vec<String>,
    java_major: Option<u32>,
) -> Vec<String> {
    if let Some(java_args_string) = settings.java_args.as_deref() {
        java_args.extend(hmcl_tokenize(java_args_string));
    }
    let effective_max_memory = settings.max_memory.map(|memory| memory.max(1));
    if let Some(max_memory) = effective_max_memory
        && let Ok(max_memory) = u32::try_from(max_memory)
    {
        memory.maximum = max_memory;
    }
    if let Some(min_memory) = settings.min_memory {
        // HMCL clamps -Xms to -Xmx (DefaultLauncher only emits -Xms when
        // min <= max); we clamp the value so both bounds stay consistent.
        let mut min_memory = min_memory.max(1);
        if let Some(max_memory) = effective_max_memory
            && min_memory > max_memory
        {
            tracing::debug!(
                min_memory,
                max_memory,
                "Clamping linked HMCL min memory to the configured maximum"
            );
            min_memory = max_memory;
        }
        if let Ok(min_memory) = u32::try_from(min_memory) {
            java_args.push(format!("-Xms{min_memory}M"));
        }
    }
    if let Some(perm_size) = settings.perm_size.as_deref()
        && !perm_size.trim().is_empty()
    {
        // HMCL stores permSize as a number in MiB and always concatenates the
        // "m" unit when building the flag (DefaultLauncher:
        // options.getMetaspace() + "m"); honor a manually written unit too.
        let perm_size = perm_size.trim();
        let perm_size = if !perm_size.is_empty()
            && perm_size
                .chars()
                .all(|character| character.is_ascii_digit())
        {
            format!("{perm_size}m")
        } else {
            perm_size.to_string()
        };
        // HMCL never sends a PermGen flag to Java 8+: DefaultLauncher adds
        // -XX:PermSize only when the JVM's parsed version is below 8 and maps
        // this setting to -XX:MetaspaceSize otherwise (PermGen was removed in
        // Java 8). An undetermined major version is treated as modern so a
        // legacy flag can never reach an unknown JVM.
        if java_major.is_some_and(|major| major < 8) {
            java_args.push(format!("-XX:PermSize={perm_size}"));
        } else {
            java_args.push(format!("-XX:MetaspaceSize={perm_size}"));
        }
    }

    let width = settings.width.and_then(|width| u16::try_from(width).ok());
    let height = settings
        .height
        .and_then(|height| u16::try_from(height).ok());
    if let (Some(width), Some(height)) = (width, height) {
        *resolution = WindowSize(width, height);
    } else if let Some(width) = width {
        resolution.0 = width;
    } else if let Some(height) = height {
        resolution.1 = height;
    }

    // Auto-connect server: HMCL's DefaultLauncher maps a configured multiplayer
    // address to `--server <host> --port <port>` for versions without Quick
    // Play (directly associated versions are treated conservatively as such),
    // falling back to port 25565 when unset.
    let mut game_args = Vec::new();
    if let Some(server_ip) = settings
        .server_ip
        .as_deref()
        .map(str::trim)
        .filter(|ip| !ip.is_empty())
    {
        game_args.push("--server".to_string());
        game_args.push(server_ip.to_string());
        game_args.push("--port".to_string());
        game_args.push(
            settings
                .server_port
                .map_or("25565".to_string(), |port| port.to_string()),
        );
    }

    // TODO(direct-link): fullscreen would require mutating options.txt in the
    // externally managed game directory, which direct association forbids.
    game_args
}

pub(crate) fn hmcl_java_candidates(
    settings: &HmclVersionSettings,
    dot_minecraft: &Path,
) -> Vec<PathBuf> {
    const JAVA_BIN: &str = if cfg!(windows) { "java.exe" } else { "java" };
    let mut candidates = Vec::new();
    if let Some(default_java_path) = settings.default_java_path.as_deref() {
        candidates.push(PathBuf::from(default_java_path));
    }
    if let Some(java_dir) = settings.java_dir.as_deref() {
        let java_dir = PathBuf::from(java_dir);
        let root = if java_dir.is_absolute() {
            java_dir
        } else {
            dot_minecraft.join(java_dir)
        };
        candidates.push(root.join("bin").join(JAVA_BIN));
        candidates.push(root.join(JAVA_BIN));
    }
    candidates
}

// ---------------------------------------------------------------------------
// PCL / PCL-CE instance-level launch preferences (P1 direct-link support)
//
// Upstream references, both pinned:
// - PCL `639de1b48a44326cbd5465579295cecf23d9056a`: instance-scope keys in
//   `Pages/PageSetup/Settings.vb` (`Sources.Instance`, persisted to
//   `versions/<id>/PCL/Setup.ini`), launch consumption in
//   `Modules/Minecraft/ModJava.vb` (`SelectOrDownloadJava`,
//   `GetJavaRequirement`) and `ModLaunch.vb`.
// - PCL-CE `aa3b81c6afb3cd1896dda271578b002066512177`:
//   `PCL.Core/App/Config.cs` (`InstanceConfigGroup` / `LaunchConfigGroup`
//   config item names), `Modules/Minecraft/ModJava.cs`
//   (`GetInstanceJavaPreference`) and `Pages/PageInstance/
//   PageInstanceSetup.xaml.cs` / `Pages/PageSetup/PageSetupLaunch.xaml.cs`
//   (`GetRam`).
//
// Both dialects use the same camel-case flat key names; they only differ in
// the container file (`PCL/Setup.ini` vs `PCL/config.v1.yml`), which
// `read_flat_config` already normalizes.
// ---------------------------------------------------------------------------

/// The PCL/PCL-CE per-instance launch preferences relevant to a launch.
///
/// Instance values come from `<version dir>/PCL/{Setup.ini|config.v1.yml}`;
/// global values come from `<launcher root>/PCL/...` and may be empty when no
/// launcher root is linked or its file is unreadable.
#[derive(Debug, Clone)]
pub(crate) struct PclLaunchSettings {
    dialect: LinkedLauncherDialect,
    instance_values: std::collections::HashMap<String, String>,
    global_values: std::collections::HashMap<String, String>,
}

impl DirectLinkedLaunch {
    /// Loads the PCL/PCL-CE preference files for this instance. Missing files
    /// yield empty maps; unreadable files are logged and treated as missing so
    /// a broken launcher config can never block a launch.
    pub(crate) fn pcl_launch_settings(&self) -> PclLaunchSettings {
        let file_name = match self.dialect {
            LinkedLauncherDialect::Pcl => "Setup.ini",
            LinkedLauncherDialect::PclCe => "config.v1.yml",
            LinkedLauncherDialect::Hmcl | LinkedLauncherDialect::Generic => {
                return PclLaunchSettings {
                    dialect: self.dialect,
                    instance_values: std::collections::HashMap::default(),
                    global_values: std::collections::HashMap::default(),
                };
            }
        };
        // PCL keeps instance-scope settings under <version>/PCL/ and
        // launcher-global settings under <launcher>/PCL/.
        let read = |path: PathBuf| {
            read_flat_config(&path).unwrap_or_else(|error| {
                tracing::debug!(
                    %error,
                    path = %path.display(),
                    "Ignoring unreadable linked PCL configuration"
                );
                std::collections::HashMap::default()
            })
        };
        PclLaunchSettings {
            dialect: self.dialect,
            instance_values: read(
                self.version_dir().join("PCL").join(file_name),
            ),
            global_values: self
                .launcher_root
                .as_ref()
                .map_or(std::collections::HashMap::default(), |root| {
                    read(root.join("PCL").join(file_name))
                }),
        }
    }
}

const PCL_JAVA_BIN: &str = if cfg!(windows) { "java.exe" } else { "java" };

/// Case-insensitive JSON object field lookup (System.Text.Json deserializes
/// with `PropertyNameCaseInsensitive = true`; see pinned PCL-CE
/// `PCL.Core/Utils/JsonCompat.cs`).
fn json_field_ci<'a>(
    value: &'a serde_json::Value,
    key: &str,
) -> Option<&'a str> {
    value.as_object()?.iter().find_map(|(name, field)| {
        (name.eq_ignore_ascii_case(key))
            .then_some(field.as_str())
            .flatten()
    })
}

impl PclLaunchSettings {
    /// Ordered Java candidates derived from the instance's Java preference,
    /// consumed by the same availability check as HMCL's candidates
    /// (`select_hmcl_java` in launcher/mod.rs).
    ///
    /// If no candidate validates there, Axolotl falls back to its own Java
    /// discovery. Upstream instead aborts or prompts; silently continuing is
    /// deliberate so a stale linked-launcher preference never blocks a launch.
    pub(crate) fn java_candidates(
        &self,
        version_dir: &Path,
        launcher_root: Option<&Path>,
    ) -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        let fall_through_to_global = self.add_instance_candidates(
            &mut candidates,
            version_dir,
            launcher_root,
        );
        if fall_through_to_global {
            // Priority 2 upstream: the globally selected Java
            // (`LaunchArgumentJavaSelect`; PCL Settings.vb / PCL-CE Config.cs).
            if let Some(global_java) =
                config_string(&self.global_values, "LaunchArgumentJavaSelect")
                    .as_deref()
                    .map(str::trim)
                    .filter(|path| !path.is_empty() && *path != "使用全局设置")
            {
                candidates.push(PathBuf::from(global_java));
            }
        }
        candidates.dedup();
        candidates
    }

    /// Returns `true` when the global selection should also be considered.
    fn add_instance_candidates(
        &self,
        candidates: &mut Vec<PathBuf>,
        version_dir: &Path,
        launcher_root: Option<&Path>,
    ) -> bool {
        let raw_select =
            config_string(&self.instance_values, "VersionArgumentJavaSelect")
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());

        // PCL classic gates its newer enum behind VersionArgumentJavaV2
        // (ModJava.vb SelectOrDownloadJava): 0 auto, 1 explicit range,
        // 2 Java bundled in the version folder, 3 forced Java.
        if self.dialect == LinkedLauncherDialect::Pcl
            && let Some(v2) =
                config_i64(&self.instance_values, "VersionArgumentJavaV2")
        {
            match v2 {
                2 => {
                    // 使用版本文件夹中的 Java: search the version folder for a
                    // bundled runtime (upstream searches Instance.PathVersion).
                    candidates.extend(pcl_bundled_java_candidates(version_dir));
                    return false;
                }
                3 => {
                    // 强制指定特定 Java: the forced path is stored either as
                    // legacy JSON {"Path": ...} or as a plain absolute path.
                    if let Some(path) = raw_select
                        .as_deref()
                        .and_then(pcl_parse_legacy_java_select)
                    {
                        candidates.push(path);
                    } else {
                        tracing::debug!(
                            "Linked PCL version forces a specific Java but \
                             stores it outside Setup.ini; ignoring"
                        );
                    }
                    return false;
                }
                1 => {
                    // Explicit Java version range (VersionArgumentJavaRange):
                    // Axolotl cannot constrain its own discovery by that range
                    // yet, so behave like automatic selection.
                    // TODO(direct-link): honor VersionArgumentJavaRange by
                    // filtering discovered runtimes through the parsed range.
                    return true;
                }
                _ => return true, // 0/auto: continue below
            }
        }

        // PCL-CE GetInstanceJavaPreference (ModJava.cs): an empty value means
        // auto, "使用全局设置" means global, everything else is either a
        // polymorphic JSON preference or a plain Java executable path.
        match raw_select.as_deref() {
            None | Some("使用全局设置") => true,
            Some(raw) => {
                let preference = serde_json::from_str::<serde_json::Value>(raw)
                    .ok()
                    .filter(|value| value.get("kind").is_some());
                if let Some(preference) = preference {
                    match preference
                        .get("kind")
                        .and_then(serde_json::Value::as_str)
                    {
                        // {"kind":"exist","javaExePath": "..."}
                        Some("exist") => {
                            if let Some(path) =
                                json_field_ci(&preference, "javaExePath")
                                    .map(PathBuf::from)
                            {
                                candidates.push(path);
                            }
                            false
                        }
                        // {"kind":"relative","relativePath": "..."} resolves
                        // against the launcher directory (upstream
                        // Basics.ExecutableDirectory), NOT the .minecraft
                        // folder; paths escaping it degrade to the global
                        // preference.
                        Some("relative") => {
                            match json_field_ci(&preference, "relativePath")
                                .and_then(|relative| {
                                    pcl_resolve_relative_java(
                                        relative,
                                        launcher_root,
                                    )
                                }) {
                                Some(path) => {
                                    candidates.push(path);
                                    false
                                }
                                None => true,
                            }
                        }
                        // {"kind":"global"} / {"kind":"auto"}
                        _ => true,
                    }
                } else if Path::new(raw).is_absolute() {
                    // Legacy plain-path entry.
                    candidates.push(PathBuf::from(raw));
                    false
                } else {
                    // Relative or unrecognized strings degrade to the global
                    // preference (pinned ModJava.cs GetInstanceJavaPreference).
                    true
                }
            }
        }
    }

    /// Resolves the RAM setting to a heap size in MiB, mirroring pinned
    /// PCL-CE `PageInstanceSetup.GetRam` (and classic
    /// `PageInstanceSetup.xaml.vb`). `None` keeps Axolotl's own memory
    /// setting.
    pub(crate) fn resolve_ram_mb(
        &self,
        game_dir: &Path,
        modable: bool,
        opti_fine: bool,
        available_memory_gb: f64,
        is_32_bit_java: bool,
    ) -> Option<u32> {
        // VersionRamType: 2 follow global (upstream default), 0 automatic,
        // 1 manual slider (VersionRamCustom).
        let ram_type =
            config_i64(&self.instance_values, "VersionRamType").unwrap_or(2);
        let ram_gb = match ram_type {
            0 => Some(pcl_auto_ram_gb(
                available_memory_gb,
                pcl_mod_count(game_dir),
                modable,
                opti_fine,
            )),
            1 => Some(pcl_slider_ram_gb(
                config_i64(&self.instance_values, "VersionRamCustom")
                    .unwrap_or(15),
            )),
            _ => {
                // 跟随全局: resolve through the launcher-global keys; when the
                // global configuration is unavailable keep Axolotl's default.
                match config_i64(&self.global_values, "LaunchRamType") {
                    Some(0) => Some(pcl_auto_ram_gb(
                        available_memory_gb,
                        pcl_mod_count(game_dir),
                        modable,
                        opti_fine,
                    )),
                    Some(_) => Some(pcl_slider_ram_gb(
                        config_i64(&self.global_values, "LaunchRamCustom")
                            .unwrap_or(15),
                    )),
                    None => {
                        tracing::debug!(
                            "Linked PCL instance follows the global memory \
                             setting but no launcher-global configuration was \
                             found; keeping the Axolotl memory setting"
                        );
                        None
                    }
                }
            }
        }?;

        // 若使用 32 位 Java，则限制为 1G (pinned PCL-CE GetRam tail).
        let mut ram_gb = ram_gb;
        if is_32_bit_java {
            ram_gb = ram_gb.min(1.0);
        }
        // ModLaunch.vb: TargetRam = Math.Floor(GetRam(...) * 1024) MiB.
        Some((ram_gb * 1024.0).floor().max(1.0) as u32)
    }

    /// Custom JVM arguments (VersionAdvanceJvm, falling back to the global
    /// LaunchAdvanceJvm when the instance leaves it empty — ModLaunch.vb
    /// line ~1255 / PCL-CE equivalent).
    pub(crate) fn extra_jvm_args(&self) -> Vec<String> {
        let custom = config_string(&self.instance_values, "VersionAdvanceJvm")
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                config_string(&self.global_values, "LaunchAdvanceJvm")
                    .filter(|value| !value.trim().is_empty())
            });
        custom
            .map(|value| pcl_split_arguments(&value))
            .unwrap_or_default()
    }

    /// Custom game arguments (VersionAdvanceGame with the same global
    /// fallback; appended after the vanilla game arguments like PCL's
    /// `Arg &= " " & CustomArg`).
    pub(crate) fn extra_game_args(&self) -> Vec<String> {
        let custom = config_string(&self.instance_values, "VersionAdvanceGame")
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                config_string(&self.global_values, "LaunchAdvanceGame")
                    .filter(|value| !value.trim().is_empty())
            });
        custom
            .map(|value| pcl_split_arguments(&value))
            .unwrap_or_default()
    }
}

/// Parses the legacy PCL Java selection value: either a JSON object carrying a
/// "Path" member (old PCL serialized `New Java(folder)` this way) or a plain
/// absolute path.
fn pcl_parse_legacy_java_select(raw: &str) -> Option<PathBuf> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
        return json_field_ci(&value, "Path")
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(PathBuf::from);
    }
    let trimmed = raw.trim();
    (!trimmed.is_empty() && Path::new(trimmed).is_absolute())
        .then(|| PathBuf::from(trimmed))
}

/// Resolves a PCL-CE `UseRelativePath` preference against the launcher root.
/// Paths that escape the launcher directory are rejected (pinned ModJava.cs
/// guards with `Files.IsPathWithinDirectory(absPath, ExecutableDirectory)`),
/// which makes the caller fall back to the global preference.
fn pcl_resolve_relative_java(
    relative: &str,
    launcher_root: Option<&Path>,
) -> Option<PathBuf> {
    let relative = relative.trim();
    let launcher_root = launcher_root?;
    if relative.is_empty() || Path::new(relative).is_absolute() {
        return None;
    }
    let base_components: Vec<_> =
        launcher_root.components().collect::<Vec<_>>();
    let candidate = launcher_root.join(relative);
    // Lexical containment: walking the joined path must never climb above the
    // launcher root via `..` (upstream IsPathWithinDirectory).
    let mut depth = base_components.len();
    for component in candidate.components().skip(base_components.len()) {
        match component {
            std::path::Component::Normal(_) => depth += 1,
            std::path::Component::ParentDir => {
                if depth == base_components.len() {
                    return None;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    // The preference may name the runtime folder (upstream stores the java
    // folder) or the binary itself; accept both spellings.
    let lower = relative.to_ascii_lowercase();
    if lower.ends_with(PCL_JAVA_BIN) {
        Some(candidate)
    } else {
        Some(candidate.join("bin").join(PCL_JAVA_BIN))
    }
}

/// Finds Java executables bundled inside a version folder (PCL classic
/// `VersionArgumentJavaV2 = 2`, ModJava.vb `SelectOrDownloadJava` case 2).
fn pcl_bundled_java_candidates(version_dir: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut stack = vec![(version_dir.to_path_buf(), 0usize)];
    while let Some((directory, depth)) = stack.pop() {
        if candidates.len() >= 4 {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        let mut sub_directories = Vec::new();
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name == "bin") {
                    let binary = path.join(PCL_JAVA_BIN);
                    if binary.is_file() {
                        candidates.push(binary);
                    }
                } else if depth < 3 {
                    sub_directories.push((path, depth + 1));
                }
            }
        }
        // Depth-first order keeps the first discovered runtime first.
        stack.extend(sub_directories.into_iter().rev());
    }
    candidates
}

/// Number of files in the resolved game directory's mods folder (PCL-CE
/// GetRam counts every file; classic filters .jar/.zip/.litemod).
fn pcl_mod_count(game_dir: &Path) -> u32 {
    std::fs::read_dir(game_dir.join("mods"))
        .map(|entries| entries.filter_map(Result::ok).count() as u32)
        .unwrap_or(0)
}

/// PCL's staged automatic memory estimate, in GB. Faithful port of pinned
/// PCL-CE `PageInstanceSetup.GetRam` automatic branch (identical algorithm in
/// classic `PageInstanceSetup.xaml.vb` / global `PageSetupLaunch.GetRam`):
/// targets grow with the mod count, each stage consumes a shrinking share of
/// the remaining available physical memory, and the result never drops below
/// the guaranteed minimum.
pub(crate) fn pcl_auto_ram_gb(
    available_memory_gb: f64,
    mod_count: u32,
    modable: bool,
    opti_fine: bool,
) -> f64 {
    // 可安装 Mod 的版本 scale every target with the mod count; OptiFine-only
    // versions get fixed targets; vanilla uses the smallest table.
    let (ram_minimum, target1, target2, target3) = if modable {
        (
            0.5 + f64::from(mod_count) / 150.0,
            1.5 + f64::from(mod_count) / 90.0,
            2.7 + f64::from(mod_count) / 50.0,
            4.5 + f64::from(mod_count) / 25.0,
        )
    } else if opti_fine {
        (0.5, 1.5, 3.0, 5.0)
    } else {
        (0.5, 1.5, 2.5, 4.0)
    };

    let mut ram_available = (available_memory_gb * 10.0).round() / 10.0;
    let mut ram_give = 0.0;
    for (delta, ratio) in [
        (target1, 1.0),
        (target2 - target1, 0.7),
        (target3 - target2, 0.4),
        (target3, 0.15),
    ] {
        ram_give += (ram_available * ratio).min(delta);
        ram_available -= delta / ratio;
        if ram_available < 0.1 {
            break;
        }
    }

    (ram_give.max(ram_minimum) * 10.0).round() / 10.0
}

/// Maps PCL's manual slider value (VersionRamCustom/LaunchRamCustom) to GB.
fn pcl_slider_ram_gb(value: i64) -> f64 {
    // Upstream slider steps: 0-12 → 0.1 GiB increments from 0.3, then coarser
    // bands up to (value-33)*2+16.
    if value <= 12 {
        f64::from(value as i32) * 0.1 + 0.3
    } else if value <= 25 {
        f64::from((value - 12) as i32) * 0.5 + 1.5
    } else if value <= 33 {
        f64::from((value - 25) as i32) + 8.0
    } else {
        f64::from((value - 33) as i32) * 2.0 + 16.0
    }
}

/// Rust port of PCL `SplitJavaArguments` (ModLaunch.vb at pinned commit
/// `639de1b48a44326cbd5465579295cecf23d9056a`): splits on spaces outside
/// double quotes, keeps the quote characters inside tokens (PCL re-quotes
/// whole arguments downstream), treats `\"` literally, and collapses line
/// breaks into separators.
fn pcl_split_arguments(input: &str) -> Vec<String> {
    // Line endings become separators, collapsed like PCL's
    // ReplaceLineEndings(" ", mergeMultiple:=True); empty tokens are dropped
    // below, so consecutive separators behave identically.
    let normalized: String = input
        .chars()
        .map(|character| {
            if character == '\n' || character == '\r' {
                ' '
            } else {
                character
            }
        })
        .collect();
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = normalized.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\\' && chars.peek() == Some(&'"') {
            current.push_str("\\\"");
            chars.next();
        } else if character == '"' {
            in_quotes = !in_quotes;
            current.push('"');
        } else if character == ' ' && !in_quotes {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else {
            current.push(character);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Reads HMCL's global game settings used when a version declares
/// `"usesGlobal": true`.
///
/// Upstream reference (pinned commit `083dbb18ade1c935e2e56d0bdefcd718be1e2ed6`):
/// `SettingsManager.GAME_SETTINGS_LOCATION` is `.hmcl/config/game-settings.json`,
/// a [`GameSettingsPresets`](source: GameSettingsPresets.java) document whose
/// `presets` array carries `GameSettings` properties (`jvmOptions`, `minMemory`,
/// `maxMemory`, ...); the preset an ordinary instance inherits is chosen by
/// `LauncherSettings.PROPERTY_DEFAULT_GAME_SETTINGS_PRESET`
/// (`defaultGameSettingsPreset`) inside `.hmcl/config/launcher-settings.json`.
/// When neither file exists the caller keeps the per-version values untouched.
pub(crate) struct HmclGlobalGameSettings {
    pub jvm_options: Option<String>,
    pub min_memory: Option<i64>,
    pub max_memory: Option<i64>,
}

pub(crate) fn hmcl_global_game_settings(
    launcher_root: &Path,
) -> Option<HmclGlobalGameSettings> {
    let presets_raw = std::fs::read_to_string(
        launcher_root
            .join(".hmcl")
            .join("config")
            .join("game-settings.json"),
    )
    .ok()?;
    let presets =
        serde_json::from_str::<serde_json::Value>(&presets_raw).ok()?;
    let presets = presets.get("presets")?.as_array()?;
    let default_id = std::fs::read_to_string(
        launcher_root
            .join(".hmcl")
            .join("config")
            .join("launcher-settings.json"),
    )
    .ok()
    .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
    .and_then(|settings| {
        settings
            .get("defaultGameSettingsPreset")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    });

    let preset = presets
        .iter()
        .find(|preset| {
            default_id.as_deref().is_some_and(|default_id| {
                preset.get("id").and_then(serde_json::Value::as_str)
                    == Some(default_id)
            })
        })
        .or_else(|| presets.first())?;

    let number =
        |key: &str| preset.get(key).and_then(serde_json::Value::as_i64);
    Some(HmclGlobalGameSettings {
        jvm_options: preset
            .get("jvmOptions")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        min_memory: number("minMemory"),
        max_memory: number("maxMemory"),
    })
}

/// Overlays HMCL's global game settings onto per-version settings for
/// versions with `usesGlobal: true`. Per-version values win; the global file
/// only fills fields the version left unset ("作回退").
pub(crate) fn hmcl_with_global_fallback(
    settings: &HmclVersionSettings,
    launcher_root: Option<&Path>,
) -> HmclVersionSettings {
    let mut effective = settings.clone();
    if effective.uses_global != Some(true) {
        return effective;
    }
    let Some(launcher_root) = launcher_root else {
        tracing::debug!(
            "Linked HMCL version uses global settings but has no launcher \
             root recorded; keeping per-version settings"
        );
        return effective;
    };
    let Some(global) = hmcl_global_game_settings(launcher_root) else {
        tracing::debug!(
            "Linked HMCL version uses global settings but {} could not be \
             read; keeping per-version settings",
            launcher_root
                .join(".hmcl")
                .join("config")
                .join("game-settings.json")
                .display()
        );
        return effective;
    };
    if effective.java_args.is_none() {
        effective.java_args = global.jvm_options;
    }
    if effective.min_memory.is_none() {
        effective.min_memory = global.min_memory;
    }
    if effective.max_memory.is_none() {
        effective.max_memory = global.max_memory;
    }
    effective
}

/// PCL `McInstance.PathIndie` at commits
/// `639de1b48a44326cbd5465579295cecf23d9056a` and
/// `aa3b81c6afb3cd1896dda271578b002066512177`.
///
/// Older rows may not have `launcher_root`; when unavailable, the upstream
/// default `LaunchArgumentIndieV2 = 4` (isolate all) is used after the per-version
/// and content-presence fallbacks.
pub(crate) fn resolve_pcl_game_dir(
    dot_minecraft: &Path,
    version_dir: &Path,
    dialect: LinkedLauncherDialect,
    merged: &MergedVersion,
    launcher_root: Option<&Path>,
) -> crate::Result<PathBuf> {
    let version_dir = version_dir.to_path_buf();
    let pcl_dir = version_dir.join("PCL");
    let instance_values = match dialect {
        LinkedLauncherDialect::Pcl => {
            read_flat_config(&pcl_dir.join("Setup.ini"))?
        }
        LinkedLauncherDialect::PclCe => {
            read_flat_config(&pcl_dir.join("config.v1.yml"))?
        }
        _ => return Ok(dot_minecraft.to_path_buf()),
    };

    if let Some(value) = config_bool(&instance_values, "VersionArgumentIndieV2")
    {
        return Ok(if value {
            version_dir
        } else {
            dot_minecraft.to_path_buf()
        });
    }
    if let Some(value) = config_i64(&instance_values, "VersionArgumentIndie")
        && value > 0
    {
        return Ok(if value == 1 {
            version_dir
        } else {
            dot_minecraft.to_path_buf()
        });
    }

    if directory_has_files(&version_dir.join("mods"))?
        || directory_has_directories(&version_dir.join("saves"))?
    {
        return Ok(version_dir);
    }

    let global_values = if let Some(launcher_root) = launcher_root {
        match dialect {
            LinkedLauncherDialect::Pcl => {
                read_flat_config(&launcher_root.join("PCL").join("Setup.ini"))?
            }
            LinkedLauncherDialect::PclCe => read_flat_config(
                &launcher_root.join("PCL").join("config.v1.yml"),
            )?,
            _ => Default::default(),
        }
    } else {
        Default::default()
    };
    // TODO(direct-link): PCL/PCL-CE fullscreen and other unmapped per-version
    // settings remain P1 follow-up; Java selection, RAM, and custom JVM/game
    // arguments are consumed via pcl_launch_settings() at launch time.
    let policy =
        config_i64(&global_values, "LaunchArgumentIndieV2").unwrap_or(4);
    let modded = pcl_is_modded(merged);
    let release = matches!(merged.type_, None | Some(VersionType::Release));
    let isolate = match policy {
        0 => false,
        1 => modded,
        2 => !release,
        3 => modded || !release,
        _ => true,
    };
    Ok(if isolate {
        version_dir
    } else {
        dot_minecraft.to_path_buf()
    })
}

fn pcl_is_modded(merged: &MergedVersion) -> bool {
    if merged.main_class.as_deref().is_some_and(|main| {
        main != "net.minecraft.client.main.Main"
            && main != "net.minecraft.client.Minecraft"
    }) {
        return true;
    }

    merged
        .libraries
        .iter()
        .any(|library| pcl_coordinate_is_modded_loader(&library.library.name))
}

/// Coordinate check shared by the isolation policy (`pcl_is_modded`) and the
/// RAM profile.
fn pcl_coordinate_is_modded_loader(name: &str) -> bool {
    let mut coordinate = name.split(':');
    let group = coordinate.next().unwrap_or_default().to_ascii_lowercase();
    let artifact = coordinate.next().unwrap_or_default().to_ascii_lowercase();
    matches!(
        (group.as_str(), artifact.as_str()),
        ("net.fabricmc", "fabric-loader")
            | ("org.quiltmc", "quilt-loader")
            | ("net.minecraftforge", "forge" | "fmlloader")
            | ("net.neoforged", "neoforge")
            | ("net.neoforged.fancymodloader", "loader")
            | ("com.cleanroommc", "cleanroom")
            | ("com.mumfrey", "liteloader")
            | ("optifine", "optifine")
    )
}

/// Classifies a merged version for PCL's automatic RAM estimate
/// (pinned PCL-CE `GetRam`): `(installable-mods version, OptiFine version)`.
/// OptiFine-only versions use their own fixed targets upstream, so OptiFine
/// alone does not count as a mod loader here.
pub(crate) fn pcl_ram_profile(
    libraries: &[daedalus::minecraft::Library],
) -> (bool, bool) {
    let mut modable = false;
    let mut opti_fine = false;
    for library in libraries {
        let mut coordinate = library.name.split(':');
        let group = coordinate.next().unwrap_or_default().to_ascii_lowercase();
        let artifact =
            coordinate.next().unwrap_or_default().to_ascii_lowercase();
        match (group.as_str(), artifact.as_str()) {
            ("optifine", "optifine") => opti_fine = true,
            ("net.fabricmc", "fabric-loader")
            | ("org.quiltmc", "quilt-loader")
            | ("net.minecraftforge", "forge" | "fmlloader")
            | ("net.neoforged", "neoforge")
            | ("net.neoforged.fancymodloader", "loader")
            | ("com.cleanroommc", "cleanroom")
            | ("com.mumfrey", "liteloader") => modable = true,
            _ => {}
        }
    }
    (modable, opti_fine)
}

/// Available physical memory in GiB, rounded to one decimal like PCL does
/// before running its allocation stages (pinned GetRam implementations).
pub(crate) fn pcl_available_memory_gb() -> f64 {
    let mut system = sysinfo::System::new_with_specifics(
        sysinfo::RefreshKind::nothing()
            .with_memory(sysinfo::MemoryRefreshKind::nothing().with_ram()),
    );
    system.refresh_memory();
    let available_gib =
        system.available_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    (available_gib * 10.0).round() / 10.0
}

fn read_flat_config(
    path: &Path,
) -> crate::Result<std::collections::HashMap<String, String>> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Default::default());
        }
        Err(error) => {
            return Err(crate::ErrorKind::FSError(format!(
                "Failed to read linked launcher config {}: {error}",
                path.display()
            ))
            .into());
        }
    };
    let mut values = std::collections::HashMap::new();
    for line in content.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with(['#', ';', '[']) {
            continue;
        }
        // Prefer the separator that appears first: YAML configs (`config.v1.yml`)
        // separate with ':', PCL's Setup.ini with '=', and INI values may
        // themselves contain colons (e.g. JSON selections).
        let pair = match (line.find(':'), line.find('=')) {
            (Some(colon), Some(equals)) => {
                if equals < colon {
                    line.split_once('=')
                } else {
                    line.split_once(':')
                }
            }
            (Some(_), None) => line.split_once(':'),
            (None, Some(_)) => line.split_once('='),
            (None, None) => None,
        };
        if let Some((key, value)) = pair {
            values.insert(
                key.trim().to_ascii_lowercase(),
                value.trim().trim_matches(['\'', '"']).to_string(),
            );
        }
    }
    Ok(values)
}

fn config_bool(
    values: &std::collections::HashMap<String, String>,
    key: &str,
) -> Option<bool> {
    match values
        .get(&key.to_ascii_lowercase())?
        .to_ascii_lowercase()
        .as_str()
    {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

fn config_i64(
    values: &std::collections::HashMap<String, String>,
    key: &str,
) -> Option<i64> {
    values.get(&key.to_ascii_lowercase())?.parse().ok()
}

fn config_string(
    values: &std::collections::HashMap<String, String>,
    key: &str,
) -> Option<String> {
    values.get(&key.to_ascii_lowercase()).cloned()
}

fn directory_has_files(path: &Path) -> crate::Result<bool> {
    directory_has(path, |entry| {
        entry.file_type().is_ok_and(|kind| kind.is_file())
    })
}

fn directory_has_directories(path: &Path) -> crate::Result<bool> {
    directory_has(path, |entry| {
        entry.file_type().is_ok_and(|kind| kind.is_dir())
    })
}

fn directory_has(
    path: &Path,
    predicate: impl Fn(&std::fs::DirEntry) -> bool,
) -> crate::Result<bool> {
    match std::fs::read_dir(path) {
        Ok(entries) => Ok(entries
            .filter_map(Result::ok)
            .any(|entry| predicate(&entry))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(crate::ErrorKind::FSError(format!(
            "Failed to inspect linked directory {}: {error}",
            path.display()
        ))
        .into()),
    }
}

pub(crate) fn extract_linked_natives(
    direct: &DirectLinkedLaunch,
    libraries: &[LinkedLibrary],
    target: &Path,
    java_arch: &str,
    minecraft_updated: bool,
) -> crate::Result<()> {
    std::fs::create_dir_all(target).map_err(|error| {
        crate::ErrorKind::FSError(format!(
            "Failed to create native cache {}: {error}",
            target.display()
        ))
    })?;

    for library in libraries {
        if let Some(rules) = library.library.rules.as_deref()
            && !super::parse_rules(
                rules,
                java_arch,
                &crate::instance::QuickPlayType::None,
                minecraft_updated,
            )
        {
            continue;
        }
        let Some((classifier, download)) = native_download(library, java_arch)
        else {
            continue;
        };
        let archive = if library.hint.as_deref() == Some("local") {
            direct.library_path(library)?
        } else {
            let relative = download
                .and_then(|download| download.path.as_deref())
                .map(PathBuf::from)
                .unwrap_or(
                    classified_artifact_path(
                        &library.library.name,
                        &classifier,
                    )?
                    .into(),
                );
            direct.libraries_dir().join(relative)
        };
        let excludes = library
            .library
            .extract
            .as_ref()
            .and_then(|extract| extract.exclude.as_deref())
            .unwrap_or(&[]);
        extract_native_archive(&archive, target, excludes)?;
    }
    Ok(())
}

/// Whether a library declares only native classifier jars and no main
/// artifact. Vanilla publishes no plain jar for such libraries (e.g. the
/// 1.12.2 `net.java.jinput:jinput-platform:2.0.5`, which only ships
/// `natives-<os>` classifiers), so neither the vanilla launcher nor HMCL ever
/// downloads or class-paths one; the selected classifier is handled by native
/// extraction instead.
pub(crate) fn is_native_only_library(library: &Library) -> bool {
    let downloads = library.downloads.as_ref();
    downloads
        .and_then(|downloads| downloads.artifact.as_ref())
        .is_none()
        && (library.natives.is_some()
            || downloads
                .and_then(|downloads| downloads.classifiers.as_ref())
                .is_some_and(|classifiers| {
                    classifiers.keys().any(|key| key.starts_with("native"))
                }))
}

pub(crate) fn native_download<'a>(
    library: &'a LinkedLibrary,
    java_arch: &str,
) -> Option<(String, Option<&'a LibraryDownload>)> {
    let normalized_arch = normalize_architecture(java_arch);
    let native_os = Os::native_arch(normalized_arch);
    let base_os = native_os.get_os();
    let classifiers = library
        .library
        .downloads
        .as_ref()
        .and_then(|downloads| downloads.classifiers.as_ref());
    let classifier = if let Some(natives) = library.library.natives.as_ref() {
        natives
            .get(&native_os)
            .or_else(|| natives.get(&base_os))?
            .replace("${arch}", linked_architecture_width(java_arch))
    } else {
        let classifiers = classifiers?;
        native_classifier_candidates(normalized_arch)
            .into_iter()
            .find(|candidate| classifiers.contains_key(candidate))?
    };
    let download =
        classifiers.and_then(|classifiers| classifiers.get(&classifier));
    Some((classifier, download))
}

fn native_classifier_candidates(java_arch: &str) -> Vec<String> {
    let os = match std::env::consts::OS {
        "macos" => "osx",
        other => other,
    };
    let arch = java_arch.to_ascii_lowercase();
    let width = linked_architecture_width(java_arch);
    let mut candidates = Vec::new();
    for key in ["", arch.as_str(), width] {
        for variant in ["", "native", "natives"] {
            let mut classifier = String::new();
            if !variant.is_empty() {
                classifier.push_str(variant);
                classifier.push('-');
            }
            classifier.push_str(os);
            if !key.is_empty() {
                classifier.push('-');
                classifier.push_str(key);
            }
            candidates.push(classifier);
        }
    }
    candidates
}

pub(crate) fn classified_artifact_path(
    name: &str,
    classifier: &str,
) -> crate::Result<String> {
    let path = daedalus::get_path_from_artifact(name)?;
    if name.split(':').nth(3).is_some() {
        return Ok(path);
    }
    Ok(if let Some((prefix, extension)) = path.rsplit_once('.') {
        format!("{prefix}-{classifier}.{extension}")
    } else {
        format!("{path}-{classifier}")
    })
}

fn maven_artifact_filename(name: &str) -> crate::Result<String> {
    let path = daedalus::get_path_from_artifact(name)?;
    Ok(Path::new(&path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            crate::ErrorKind::LauncherError(format!(
                "Invalid local library coordinate: {name}"
            ))
        })?
        .to_string())
}

fn extract_native_archive(
    archive_path: &Path,
    target: &Path,
    excludes: &[String],
) -> crate::Result<()> {
    let file = std::fs::File::open(archive_path).map_err(|error| {
        crate::ErrorKind::LauncherError(format!(
            "Failed to open linked native archive {}: {error}",
            archive_path.display()
        ))
    })?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| {
        crate::ErrorKind::LauncherError(format!(
            "Failed to read linked native archive {}: {error}",
            archive_path.display()
        ))
    })?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| {
            crate::ErrorKind::LauncherError(format!(
                "Failed to read entry from linked native archive {}: {error}",
                archive_path.display()
            ))
        })?;
        let Some(relative) = entry.enclosed_name() else {
            return Err(crate::ErrorKind::LauncherError(format!(
                "Unsafe path in linked native archive {}",
                archive_path.display()
            ))
            .into());
        };
        let normalized = relative.to_string_lossy().replace('\\', "/");
        if excludes.iter().any(|exclude| {
            let exclude = exclude.replace('\\', "/");
            normalized.starts_with(exclude.trim_start_matches('/'))
        }) {
            continue;
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(crate::ErrorKind::LauncherError(format!(
                "Symlink entry in linked native archive {}",
                archive_path.display()
            ))
            .into());
        }
        let destination = target.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&destination)?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut output = std::fs::File::create(&destination)?;
        std::io::copy(&mut entry, &mut output)?;
        output.flush()?;
    }
    Ok(())
}

fn normalize_architecture(java_arch: &str) -> &str {
    if java_arch.eq_ignore_ascii_case("amd64") {
        "x86_64"
    } else if java_arch.eq_ignore_ascii_case("i386")
        || java_arch.eq_ignore_ascii_case("i686")
    {
        "x86"
    } else if java_arch.eq_ignore_ascii_case("arm64") {
        "aarch64"
    } else if java_arch.eq_ignore_ascii_case("arm32") {
        "arm"
    } else if java_arch.eq_ignore_ascii_case("loongarch64")
        || java_arch.eq_ignore_ascii_case("loongarch64el")
        || java_arch.eq_ignore_ascii_case("loong64")
    {
        "loongarch64"
    } else if java_arch.eq_ignore_ascii_case("riscv64")
        || java_arch.eq_ignore_ascii_case("riscv64gc")
    {
        "riscv64"
    } else {
        java_arch
    }
}

pub(crate) fn linked_architecture_width(java_arch: &str) -> &'static str {
    match normalize_architecture(java_arch)
        .to_ascii_lowercase()
        .as_str()
    {
        "x86" | "arm" => "32",
        "x86_64" | "aarch64" | "loongarch64" | "riscv64" => "64",
        _ => "64",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn loongarch64_and_riscv64_use_64_bit_native_classifiers() {
        for arch in ["loongarch64", "loong64", "riscv64", "riscv64gc"] {
            assert_eq!(linked_architecture_width(arch), "64");
        }
        assert_eq!(normalize_architecture("LOONG64"), "loongarch64");
        assert_eq!(normalize_architecture("RISCV64GC"), "riscv64");
        let os = match std::env::consts::OS {
            "macos" => "osx",
            other => other,
        };
        let candidates = native_classifier_candidates("loongarch64");
        assert!(candidates.contains(&format!("natives-{os}-loongarch64")));
        let candidates = native_classifier_candidates("riscv64");
        assert!(candidates.contains(&format!("natives-{os}-riscv64")));
    }

    fn minimal_merged() -> MergedVersion {
        let root = tempfile::tempdir().unwrap();
        let version = root.path().join("versions/base");
        std::fs::create_dir_all(&version).unwrap();
        std::fs::write(
            version.join("base.json"),
            serde_json::to_vec(&json!({
                "id":"base", "mainClass":"net.minecraft.client.main.Main",
                "assetIndex":{"id":"1", "sha1":"", "size":0, "totalSize":0, "url":""}
            })).unwrap(),
        ).unwrap();
        super::super::local_version::load_version_chain(root.path(), "base")
            .unwrap()
    }

    #[test]
    fn parses_dialects_without_falling_back_to_hmcl() {
        assert_eq!(
            LinkedLauncherDialect::parse("hmcl").unwrap(),
            LinkedLauncherDialect::Hmcl
        );
        assert_eq!(
            LinkedLauncherDialect::parse("pcl2").unwrap(),
            LinkedLauncherDialect::Pcl
        );
        assert_eq!(
            LinkedLauncherDialect::parse("pcl2_ce").unwrap(),
            LinkedLauncherDialect::PclCe
        );
        assert!(LinkedLauncherDialect::parse("unknown").is_err());
    }

    #[test]
    fn legacy_external_version_override_uses_the_actual_manifest_stem() {
        let root = tempfile::tempdir().unwrap();
        let version_dir = root.path().join("versions").join("My Instance");
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(
            version_dir.join("1.20.1-fabric.json"),
            serde_json::to_vec(&json!({
                "id": "1.20.1-fabric",
                "mainClass": "net.minecraft.client.main.Main"
            }))
            .unwrap(),
        )
        .unwrap();

        let direct =
            DirectLinkedLaunch::from_external_version_dir(&version_dir)
                .unwrap()
                .expect("a conventional versions directory is external");

        assert_eq!(direct.version_id, "1.20.1-fabric");
        assert_eq!(
            direct.version_json,
            Some(
                crate::util::io::canonicalize(
                    version_dir.join("1.20.1-fabric.json")
                )
                .unwrap()
            )
        );
        assert_eq!(
            direct.dot_minecraft,
            crate::util::io::canonicalize(root.path()).unwrap()
        );
    }

    #[test]
    fn external_version_override_can_resolve_before_minecraft_is_installed() {
        let root = tempfile::tempdir().unwrap();
        let version_dir = root.path().join("versions").join("New Instance");
        std::fs::create_dir_all(&version_dir).unwrap();
        let now = chrono::Utc::now();
        let instance = Instance {
            id: "local:test".to_string(),
            path: "managed".to_string(),
            applied_content_set_id: None,
            install_stage: crate::state::InstanceInstallStage::NotInstalled,
            launcher_feature_version:
                crate::state::LauncherFeatureVersion::MOST_RECENT,
            update_channel: crate::state::ReleaseChannel::Release,
            name: "New Instance".to_string(),
            icon_path: None,
            symlink_target: None,
            linked_launcher: None,
            linked_launcher_root: None,
            linked_dot_minecraft: None,
            linked_version_id: None,
            linked_version_json_path: None,
            linked_game_dir_mode: None,
            game_dir_override: Some(version_dir.to_string_lossy().into_owned()),
            created: now,
            modified: now,
            last_played: None,
            pinned_at: None,
            submitted_time_played: 0,
            recent_time_played: 0,
        };

        let direct = DirectLinkedLaunch::from_game_dir_override(&instance)
            .unwrap()
            .expect("external version directory should be resolvable");

        assert_eq!(direct.version_id, "New Instance");
        assert_eq!(direct.version_json, None);
        assert_eq!(direct.version_dir(), version_dir);
    }

    #[test]
    fn pcl_instance_configs_select_exact_game_directory() {
        let root = tempfile::tempdir().unwrap();
        let version = root.path().join("versions/demo");
        std::fs::create_dir_all(version.join("PCL")).unwrap();
        std::fs::write(
            version.join("PCL/Setup.ini"),
            "VersionArgumentIndieV2: true\n",
        )
        .unwrap();
        let isolated = resolve_pcl_game_dir(
            root.path(),
            &version,
            LinkedLauncherDialect::Pcl,
            &minimal_merged(),
            None,
        )
        .unwrap();
        assert_eq!(isolated, version);

        std::fs::write(
            version.join("PCL/config.v1.yml"),
            "VersionArgumentIndieV2: false\n",
        )
        .unwrap();
        let shared = resolve_pcl_game_dir(
            root.path(),
            &version,
            LinkedLauncherDialect::PclCe,
            &minimal_merged(),
            None,
        )
        .unwrap();
        assert_eq!(shared, root.path());
    }

    #[test]
    fn generic_version_with_content_folders_uses_its_isolated_game_dir() {
        let root = tempfile::tempdir().unwrap();
        let version = root.path().join("versions/demo");
        std::fs::create_dir_all(version.join("mods")).unwrap();

        assert_eq!(
            resolve_content_game_dir(root.path(), &version).unwrap(),
            version
        );
    }

    #[test]
    fn screenshots_folder_marks_version_as_isolated_game_dir() {
        let root = tempfile::tempdir().unwrap();
        let version = root.path().join("versions/demo");
        std::fs::create_dir_all(version.join("screenshots")).unwrap();

        assert_eq!(
            resolve_content_game_dir(root.path(), &version).unwrap(),
            version
        );
    }

    #[test]
    fn generic_version_isolated_even_without_content_folders() {
        let root = tempfile::tempdir().unwrap();
        let version = root.path().join("versions/demo");
        std::fs::create_dir_all(&version).unwrap();
        std::fs::write(
            version.join("demo.json"),
            br#"{"id":"demo","mainClass":"net.minecraft.client.main.Main"}"#,
        )
        .unwrap();

        assert_eq!(
            resolve_content_game_dir(root.path(), &version).unwrap(),
            root.path()
        );

        let direct = DirectLinkedLaunch {
            dot_minecraft: root.path().to_path_buf(),
            launcher_root: None,
            version_id: "demo".to_string(),
            version_json: Some(version.join("demo.json")),
            dialect: LinkedLauncherDialect::Generic,
            game_dir_mode: None,
        };
        assert_eq!(direct.resolve().unwrap().game_dir, version);
    }

    #[test]
    fn explicit_external_root_mode_overrides_pcl_configuration() {
        let root = tempfile::tempdir().unwrap();
        let version = root.path().join("versions/demo");
        std::fs::create_dir_all(version.join("PCL")).unwrap();
        std::fs::write(
            version.join("PCL/config.v1.yml"),
            "VersionArgumentIndieV2: true\n",
        )
        .unwrap();
        std::fs::write(
            version.join("demo.json"),
            serde_json::to_vec(&json!({
                "id": "demo",
                "mainClass": "net.minecraft.client.main.Main"
            }))
            .unwrap(),
        )
        .unwrap();

        let shared = DirectLinkedLaunch {
            dot_minecraft: root.path().to_path_buf(),
            launcher_root: None,
            version_id: "demo".to_string(),
            version_json: Some(version.join("demo.json")),
            dialect: LinkedLauncherDialect::PclCe,
            game_dir_mode: Some(ExternalGameDirMode::Shared),
        };
        let isolated = DirectLinkedLaunch {
            game_dir_mode: Some(ExternalGameDirMode::Isolated),
            ..shared.clone()
        };

        assert_eq!(shared.resolve().unwrap().game_dir, root.path());
        assert_eq!(isolated.resolve().unwrap().game_dir, version);
    }

    #[test]
    fn hmcl_explicitly_configured_empty_version_is_isolated() {
        let launcher = tempfile::tempdir().unwrap();
        let dot_minecraft = launcher.path().join("game");
        let version = dot_minecraft.join("versions").join("demo");
        std::fs::create_dir_all(&version).unwrap();
        let hmcl_dir = launcher.path().join(".hmcl");
        std::fs::create_dir_all(&hmcl_dir).unwrap();
        std::fs::write(
            hmcl_dir.join("hmcl.json"),
            serde_json::to_vec(&serde_json::json!({
                "configurations": {
                    "demo": { "gameDir": version.to_string_lossy() }
                }
            }))
            .unwrap(),
        )
        .unwrap();

        assert_eq!(
            resolve_hmcl_game_dir(
                Some(launcher.path()),
                &dot_minecraft,
                &version
            )
            .unwrap(),
            version
        );
    }

    #[test]
    fn pcl_fallbacks_apply_before_global_isolation_policy() {
        let root = tempfile::tempdir().unwrap();
        let launcher = tempfile::tempdir().unwrap();
        let version = root.path().join("versions/demo");
        std::fs::create_dir_all(&version).unwrap();
        std::fs::create_dir_all(launcher.path().join("PCL")).unwrap();
        std::fs::write(
            launcher.path().join("PCL/Setup.ini"),
            "LaunchArgumentIndieV2=0\n",
        )
        .unwrap();

        let shared = resolve_pcl_game_dir(
            root.path(),
            &version,
            LinkedLauncherDialect::Pcl,
            &minimal_merged(),
            Some(launcher.path()),
        )
        .unwrap();
        assert_eq!(shared, root.path());

        std::fs::create_dir_all(version.join("mods")).unwrap();
        std::fs::write(version.join("mods/example.jar"), b"mod").unwrap();
        let isolated = resolve_pcl_game_dir(
            root.path(),
            &version,
            LinkedLauncherDialect::Pcl,
            &minimal_merged(),
            Some(launcher.path()),
        )
        .unwrap();
        assert_eq!(isolated, version);
    }

    #[test]
    fn pcl_modded_policy_does_not_treat_vanilla_libraries_as_a_loader() {
        let root = tempfile::tempdir().unwrap();
        let launcher = tempfile::tempdir().unwrap();
        let version = root.path().join("versions/demo");
        std::fs::create_dir_all(&version).unwrap();
        std::fs::create_dir_all(launcher.path().join("PCL")).unwrap();
        std::fs::write(
            launcher.path().join("PCL/Setup.ini"),
            "LaunchArgumentIndieV2=1\n",
        )
        .unwrap();
        let mut merged = minimal_merged();
        merged.libraries.push(
            serde_json::from_value(json!({"name":"org.lwjgl:lwjgl:3.3.3"}))
                .unwrap(),
        );

        let shared = resolve_pcl_game_dir(
            root.path(),
            &version,
            LinkedLauncherDialect::Pcl,
            &merged,
            Some(launcher.path()),
        )
        .unwrap();
        assert_eq!(shared, root.path());
    }

    #[test]
    fn explicit_version_json_controls_the_version_directory() {
        let root = tempfile::tempdir().unwrap();
        let version_json = root.path().join("versions/folder/manifest.json");
        std::fs::create_dir_all(version_json.parent().unwrap()).unwrap();
        let direct = DirectLinkedLaunch {
            dot_minecraft: root.path().to_path_buf(),
            launcher_root: None,
            version_id: "logical-id".to_string(),
            version_json: Some(version_json),
            dialect: LinkedLauncherDialect::Hmcl,
            game_dir_mode: None,
        };

        assert_eq!(direct.version_dir(), root.path().join("versions/folder"));
        assert_eq!(
            direct.client_jar("logical-id"),
            root.path().join("versions/folder/logical-id.jar")
        );

        std::fs::write(root.path().join("versions/folder/folder.jar"), b"jar")
            .unwrap();
        assert_eq!(
            direct.client_jar("logical-id"),
            root.path().join("versions/folder/folder.jar")
        );
    }

    #[test]
    fn native_extraction_respects_excludes() {
        let root = tempfile::tempdir().unwrap();
        let libraries = root.path().join("libraries/x/native/1");
        std::fs::create_dir_all(&libraries).unwrap();
        let archive_path = libraries.join("native-1-natives-linux.jar");
        let file = std::fs::File::create(&archive_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("libnative.so", options).unwrap();
        zip.write_all(b"native").unwrap();
        zip.start_file("META-INF/manifest", options).unwrap();
        zip.write_all(b"excluded").unwrap();
        zip.finish().unwrap();

        let os = serde_json::to_value(Os::native().get_os()).unwrap();
        let mut natives = serde_json::Map::new();
        natives.insert(os.as_str().unwrap().to_string(), json!("natives-test"));
        let library: LinkedLibrary = serde_json::from_value(json!({
            "name":"x:native:1",
            "natives":natives,
            "extract":{"exclude":["META-INF/"]},
            "downloads":{"classifiers":{"natives-test":{"path":"x/native/1/native-1-natives-linux.jar", "sha1":"", "size":0, "url":""}}}
        })).unwrap();
        let direct = DirectLinkedLaunch {
            dot_minecraft: root.path().to_path_buf(),
            launcher_root: None,
            version_id: "demo".to_string(),
            version_json: None,
            dialect: LinkedLauncherDialect::Hmcl,
            game_dir_mode: None,
        };
        let target = root.path().join("axolotl-cache");
        extract_linked_natives(
            &direct,
            &[library],
            &target,
            std::env::consts::ARCH,
            true,
        )
        .unwrap();
        assert_eq!(
            std::fs::read(target.join("libnative.so")).unwrap(),
            b"native"
        );
        assert!(!target.join("META-INF/manifest").exists());
    }

    #[test]
    fn legacy_native_without_download_metadata_uses_maven_classifier_path() {
        let root = tempfile::tempdir().unwrap();
        let classifier = "natives-test";
        let relative =
            classified_artifact_path("x:native:1", classifier).unwrap();
        let archive_path = root.path().join("libraries").join(relative);
        std::fs::create_dir_all(archive_path.parent().unwrap()).unwrap();
        let file = std::fs::File::create(&archive_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("legacy-native.bin", options).unwrap();
        zip.write_all(b"legacy").unwrap();
        zip.finish().unwrap();

        let os = serde_json::to_value(Os::native().get_os()).unwrap();
        let mut natives = serde_json::Map::new();
        natives.insert(os.as_str().unwrap().to_string(), json!(classifier));
        let library: LinkedLibrary = serde_json::from_value(json!({
            "name":"x:native:1",
            "natives":natives
        }))
        .unwrap();
        let direct = DirectLinkedLaunch {
            dot_minecraft: root.path().to_path_buf(),
            launcher_root: None,
            version_id: "demo".to_string(),
            version_json: None,
            dialect: LinkedLauncherDialect::Hmcl,
            game_dir_mode: None,
        };
        let target = root.path().join("axolotl-cache");
        extract_linked_natives(
            &direct,
            &[library],
            &target,
            std::env::consts::ARCH,
            true,
        )
        .unwrap();
        assert_eq!(
            std::fs::read(target.join("legacy-native.bin")).unwrap(),
            b"legacy"
        );
    }

    #[test]
    fn native_archive_rejects_parent_directory_entries() {
        let root = tempfile::tempdir().unwrap();
        let archive_path = root.path().join("unsafe.jar");
        let file = std::fs::File::create(&archive_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file(
            "../outside.bin",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(b"unsafe").unwrap();
        zip.finish().unwrap();

        let target = root.path().join("target");
        assert!(extract_native_archive(&archive_path, &target, &[]).is_err());
        assert!(!root.path().join("outside.bin").exists());
    }

    #[test]
    fn hmcl_java_args_keep_quoted_arguments_together() {
        let mut args = Vec::new();
        apply_hmcl_settings(
            &HmclVersionSettings {
                java_args: Some(
                    "-Dlog4j.configurationFile=\"C:/my dir/log.xml\" -Xmx2G"
                        .to_string(),
                ),
                ..Default::default()
            },
            &mut MemorySettings {
                maximum: 1024,
                automatic: false,
                optimize_before_launch: false,
            },
            &mut WindowSize(856, 481),
            &mut args,
            Some(17),
        );
        assert_eq!(
            args,
            ["-Dlog4j.configurationFile=C:/my dir/log.xml", "-Xmx2G"]
        );

        let single_quoted = hmcl_tokenize("'a b' c");
        assert_eq!(single_quoted, ["a b", "c"]);

        // Backslashes are literal in HMCL's tokenizer: an unquoted argument
        // containing spaces still splits, so users must quote it.
        assert_eq!(
            hmcl_tokenize("-Dx=C:\\path with space"),
            ["-Dx=C:\\path", "with", "space"]
        );
        assert_eq!(
            hmcl_tokenize("\"-Dx=C:\\path with space\""),
            ["-Dx=C:\\path with space"]
        );
        assert_eq!(hmcl_tokenize("-Dy=\"unterminated"), ["-Dy=unterminated"]);
    }

    #[test]
    fn hmcl_perm_size_never_reaches_modern_jvms() {
        let mut base_memory = MemorySettings {
            maximum: 1024,
            automatic: false,
            optimize_before_launch: false,
        };
        let mut resolution = WindowSize(856, 481);

        let legacy_args = {
            let mut args = Vec::new();
            apply_hmcl_settings(
                &HmclVersionSettings {
                    min_memory: Some(512),
                    perm_size: Some("256m".to_string()),
                    ..Default::default()
                },
                &mut base_memory,
                &mut resolution,
                &mut args,
                Some(7),
            );
            args
        };
        assert!(legacy_args.contains(&"-XX:PermSize=256m".to_string()));
        assert!(legacy_args.contains(&"-Xms512M".to_string()));

        for java_major in [Some(8), Some(17), None] {
            let mut args = Vec::new();
            apply_hmcl_settings(
                &HmclVersionSettings {
                    perm_size: Some("256m".to_string()),
                    ..Default::default()
                },
                &mut base_memory,
                &mut resolution,
                &mut args,
                java_major,
            );
            assert!(!args.iter().any(|arg| arg.starts_with("-XX:MaxPermSize")));
            assert_eq!(args, ["-XX:MetaspaceSize=256m"]);
        }

        let mut empty_args = Vec::new();
        apply_hmcl_settings(
            &HmclVersionSettings {
                perm_size: Some(" ".to_string()),
                ..Default::default()
            },
            &mut base_memory,
            &mut resolution,
            &mut empty_args,
            None,
        );
        assert!(empty_args.is_empty());
    }

    // -----------------------------------------------------------------------
    // P1: PCL/PCL-CE instance-level Java, memory, and custom arguments
    // -----------------------------------------------------------------------

    fn pcl_direct(
        dot_minecraft: &Path,
        launcher_root: Option<&Path>,
        version_id: &str,
        dialect: LinkedLauncherDialect,
    ) -> DirectLinkedLaunch {
        DirectLinkedLaunch {
            dot_minecraft: dot_minecraft.to_path_buf(),
            launcher_root: launcher_root.map(Path::to_path_buf),
            version_id: version_id.to_string(),
            version_json: None,
            dialect,
            game_dir_mode: None,
        }
    }

    fn write_version_config(
        root: &Path,
        version_id: &str,
        dialect: LinkedLauncherDialect,
        content: &str,
    ) -> PathBuf {
        let version = root.join("versions").join(version_id);
        std::fs::create_dir_all(version.join("PCL")).unwrap();
        let file = match dialect {
            LinkedLauncherDialect::Pcl => "PCL/Setup.ini",
            _ => "PCL/config.v1.yml",
        };
        std::fs::write(version.join(file), content).unwrap();
        version
    }

    fn write_global_config(
        launcher_root: &Path,
        dialect: LinkedLauncherDialect,
        content: &str,
    ) {
        std::fs::create_dir_all(launcher_root.join("PCL")).unwrap();
        let file = match dialect {
            LinkedLauncherDialect::Pcl => "Setup.ini",
            _ => "config.v1.yml",
        };
        std::fs::write(launcher_root.join("PCL").join(file), content).unwrap();
    }

    #[test]
    fn pcl_ce_relative_java_resolves_against_launcher_root() {
        let mc = tempfile::tempdir().unwrap();
        let launcher = tempfile::tempdir().unwrap();
        write_version_config(
            mc.path(),
            "demo",
            LinkedLauncherDialect::PclCe,
            "VersionArgumentJavaSelect: '{\"kind\":\"relative\",\
             \"RelativePath\":\"runtime/jdk17\"}'\n",
        );
        write_global_config(
            launcher.path(),
            LinkedLauncherDialect::PclCe,
            "LaunchArgumentJavaSelect: /opt/global/java\n",
        );
        let direct = pcl_direct(
            mc.path(),
            Some(launcher.path()),
            "demo",
            LinkedLauncherDialect::PclCe,
        );
        let settings = direct.pcl_launch_settings();

        let candidates = settings.java_candidates(
            &direct.version_dir(),
            direct.launcher_root.as_deref(),
        );
        assert_eq!(
            candidates,
            vec![launcher.path().join("runtime/jdk17").join(
                if cfg!(windows) {
                    "bin/java.exe"
                } else {
                    "bin/java"
                }
            )]
        );

        // A relative path escaping the launcher directory degrades to the
        // global preference (pinned ModJava.cs containment check).
        write_version_config(
            mc.path(),
            "demo",
            LinkedLauncherDialect::PclCe,
            "VersionArgumentJavaSelect: '{\"kind\":\"relative\",\
             \"RelativePath\":\"../../outside\"}'\n",
        );
        let settings = direct.pcl_launch_settings();
        let candidates = settings.java_candidates(
            &direct.version_dir(),
            direct.launcher_root.as_deref(),
        );
        assert_eq!(candidates, vec![PathBuf::from("/opt/global/java")]);
    }

    #[test]
    fn pcl_ce_exist_and_global_preferences_order_candidates() {
        let mc = tempfile::tempdir().unwrap();
        let launcher = tempfile::tempdir().unwrap();

        // {"kind":"exist"} selects the exact executable; no global fallback.
        write_version_config(
            mc.path(),
            "demo",
            LinkedLauncherDialect::PclCe,
            r#"VersionArgumentJavaSelect: '{"kind":"exist","JavaExePath":"/usr/lib/jvm/17/bin/java"}'"#,
        );
        write_global_config(
            launcher.path(),
            LinkedLauncherDialect::PclCe,
            "LaunchArgumentJavaSelect: /opt/global/java\n",
        );
        let direct = pcl_direct(
            mc.path(),
            Some(launcher.path()),
            "demo",
            LinkedLauncherDialect::PclCe,
        );
        assert_eq!(
            direct.pcl_launch_settings().java_candidates(
                &direct.version_dir(),
                direct.launcher_root.as_deref(),
            ),
            vec![PathBuf::from("/usr/lib/jvm/17/bin/java")]
        );

        // 使用全局设置 (and an empty value) falls back to the global entry.
        for value in ["使用全局设置", ""] {
            write_version_config(
                mc.path(),
                "demo",
                LinkedLauncherDialect::PclCe,
                &format!("VersionArgumentJavaSelect: {value}\n"),
            );
            assert_eq!(
                direct.pcl_launch_settings().java_candidates(
                    &direct.version_dir(),
                    direct.launcher_root.as_deref(),
                ),
                vec![PathBuf::from("/opt/global/java")]
            );
        }
    }

    #[test]
    fn pcl_classic_v2_selects_forced_and_bundled_java() {
        let mc = tempfile::tempdir().unwrap();
        let launcher = tempfile::tempdir().unwrap();
        let direct = pcl_direct(
            mc.path(),
            Some(launcher.path()),
            "demo",
            LinkedLauncherDialect::Pcl,
        );

        // V2=3 with the legacy JSON selection.
        write_version_config(
            mc.path(),
            "demo",
            LinkedLauncherDialect::Pcl,
            "VersionArgumentJavaV2=3\n\
             VersionArgumentJavaSelect={\"Path\":\"C:/Program Files/Java/17\"}\n",
        );
        assert_eq!(
            direct.pcl_launch_settings().java_candidates(
                &direct.version_dir(),
                direct.launcher_root.as_deref(),
            ),
            vec![PathBuf::from("C:/Program Files/Java/17")]
        );

        // V2=2 searches the version folder for a bundled runtime.
        let bundled = direct.version_dir().join("runtime/jdk-17").join("bin");
        std::fs::create_dir_all(&bundled).unwrap();
        std::fs::write(bundled.join(PCL_JAVA_BIN), b"").unwrap();
        write_version_config(
            mc.path(),
            "demo",
            LinkedLauncherDialect::Pcl,
            "VersionArgumentJavaV2=2\n",
        );
        assert_eq!(
            direct.pcl_launch_settings().java_candidates(
                &direct.version_dir(),
                direct.launcher_root.as_deref(),
            ),
            vec![bundled.join(PCL_JAVA_BIN)]
        );

        // V2=0 keeps only the globally selected Java as the candidate.
        write_global_config(
            launcher.path(),
            LinkedLauncherDialect::Pcl,
            "LaunchArgumentJavaSelect=/opt/global/java\n",
        );
        write_version_config(
            mc.path(),
            "demo",
            LinkedLauncherDialect::Pcl,
            "VersionArgumentJavaV2=0\n",
        );
        assert_eq!(
            direct.pcl_launch_settings().java_candidates(
                &direct.version_dir(),
                direct.launcher_root.as_deref(),
            ),
            vec![PathBuf::from("/opt/global/java")]
        );
    }

    #[test]
    fn pcl_auto_ram_formula_matches_upstream_tables() {
        // Vanilla targets 0.5/1.5/2.5/4 GB exhaust all four stages on a large
        // machine: 8.0 GB total.
        assert_eq!(pcl_auto_ram_gb(100.0, 0, false, false), 8.0);
        // OptiFine-only versions use the fixed 0.5/1.5/3/5 table: 10.0 GB.
        assert_eq!(pcl_auto_ram_gb(100.0, 0, false, true), 10.0);
        // Modded targets grow with the mod count (50 mods → 13.0 GB).
        assert_eq!(pcl_auto_ram_gb(100.0, 50, true, false), 13.0);
        // Scarce memory caps the estimate at what is available.
        assert_eq!(pcl_auto_ram_gb(0.4, 0, false, false), 0.5);

        // Slider mapping (VersionRamCustom/LaunchRamCustom bands); compare
        // with an epsilon because the first band multiplies by 0.1.
        let slider = |value: i64| (pcl_slider_ram_gb(value) * 10.0).round();
        assert_eq!(slider(12), 15.0);
        assert_eq!(slider(25), 80.0);
        assert_eq!(slider(33), 160.0);
        assert_eq!(slider(40), 300.0);
    }

    #[test]
    fn pcl_ram_modes_resolve_through_instance_and_global_files() {
        let mc = tempfile::tempdir().unwrap();
        let launcher = tempfile::tempdir().unwrap();
        let direct = pcl_direct(
            mc.path(),
            Some(launcher.path()),
            "demo",
            LinkedLauncherDialect::PclCe,
        );

        // Manual instance value maps through the slider formula to MiB.
        write_version_config(
            mc.path(),
            "demo",
            LinkedLauncherDialect::PclCe,
            "VersionRamType: 1\nVersionRamCustom: 20\n",
        );
        assert_eq!(
            direct.pcl_launch_settings().resolve_ram_mb(
                mc.path(),
                false,
                false,
                100.0,
                false
            ),
            Some((5.5f64 * 1024.0).floor() as u32)
        );

        // A 32-bit JVM caps the allocation at 1 GiB (pinned PCL-CE GetRam).
        write_version_config(
            mc.path(),
            "demo",
            LinkedLauncherDialect::PclCe,
            "VersionRamType: 1\nVersionRamCustom: 33\n",
        );
        assert_eq!(
            direct.pcl_launch_settings().resolve_ram_mb(
                mc.path(),
                false,
                false,
                100.0,
                true
            ),
            Some(1024)
        );

        // 跟随全局 (the default) reads the launcher-global keys.
        write_version_config(
            mc.path(),
            "demo",
            LinkedLauncherDialect::PclCe,
            "",
        );
        write_global_config(
            launcher.path(),
            LinkedLauncherDialect::PclCe,
            "LaunchRamType: 1\nLaunchRamCustom: 12\n",
        );
        assert_eq!(
            direct.pcl_launch_settings().resolve_ram_mb(
                mc.path(),
                false,
                false,
                100.0,
                false
            ),
            Some(1536)
        );

        // Without any global configuration Axolotl keeps its own setting.
        let orphan =
            pcl_direct(mc.path(), None, "demo", LinkedLauncherDialect::PclCe);
        assert_eq!(
            orphan.pcl_launch_settings().resolve_ram_mb(
                mc.path(),
                false,
                false,
                100.0,
                false
            ),
            None
        );

        // Automatic estimation counts files in the resolved game directory's
        // mods folder (150 mods → 21.0 GB).
        let mods = mc.path().join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        for index in 0..150 {
            std::fs::write(mods.join(format!("mod{index}.jar")), b"").unwrap();
        }
        write_global_config(
            launcher.path(),
            LinkedLauncherDialect::PclCe,
            "LaunchRamType: 0\n",
        );
        assert_eq!(
            direct.pcl_launch_settings().resolve_ram_mb(
                mc.path(),
                true,
                false,
                100.0,
                false
            ),
            Some((21.0f64 * 1024.0).floor() as u32)
        );
    }

    #[test]
    fn pcl_custom_jvm_and_game_arguments_are_tokenized_with_global_fallback() {
        let mc = tempfile::tempdir().unwrap();
        let launcher = tempfile::tempdir().unwrap();
        let direct = pcl_direct(
            mc.path(),
            Some(launcher.path()),
            "demo",
            LinkedLauncherDialect::Pcl,
        );

        write_version_config(
            mc.path(),
            "demo",
            LinkedLauncherDialect::Pcl,
            "VersionAdvanceJvm=-Xmx2G -XX:+UseZGC\n\
             VersionAdvanceGame=--tweakClass optifine.OptiFineTweaker\n",
        );
        let settings = direct.pcl_launch_settings();
        assert_eq!(
            settings.extra_jvm_args(),
            ["-Xmx2G".to_string(), "-XX:+UseZGC".to_string()]
        );
        assert_eq!(
            settings.extra_game_args(),
            [
                "--tweakClass".to_string(),
                "optifine.OptiFineTweaker".to_string()
            ]
        );

        // An empty instance value falls back to the global key (ModLaunch.vb:
        // `If CustomArg = "" Then CustomArg = LaunchAdvanceJvm`); absent keys
        // inject nothing.
        write_version_config(
            mc.path(),
            "demo",
            LinkedLauncherDialect::Pcl,
            "VersionAdvanceJvm=\n",
        );
        write_global_config(
            launcher.path(),
            LinkedLauncherDialect::Pcl,
            "LaunchAdvanceJvm=-Dglobal=1\n",
        );
        assert_eq!(
            direct.pcl_launch_settings().extra_jvm_args(),
            ["-Dglobal=1".to_string()]
        );
        assert!(direct.pcl_launch_settings().extra_game_args().is_empty());
    }

    #[test]
    fn pcl_split_arguments_keeps_quoted_tokens_intact() {
        assert_eq!(
            pcl_split_arguments("-Xmx2G -XX:+UseZGC"),
            ["-Xmx2G", "-XX:+UseZGC"]
        );
        assert_eq!(
            pcl_split_arguments("-Dlog4j.configurationFile=\"my dir/log.xml\""),
            ["-Dlog4j.configurationFile=\"my dir/log.xml\""]
        );
        // An escaped quote does not toggle quote state (SplitJavaArguments).
        assert_eq!(pcl_split_arguments("a\\\"b c"), ["a\\\"b", "c"]);
        assert_eq!(pcl_split_arguments("\n-Xa \r\n-Xb "), ["-Xa", "-Xb"]);
    }

    #[test]
    fn hmcl_uses_global_falls_back_to_launcher_settings_files() {
        let launcher = tempfile::tempdir().unwrap();
        let config_dir = launcher.path().join(".hmcl/config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("game-settings.json"),
            r#"{
                "schema": {"id": "game-settings", "version": {"major": 1, "minor": 0, "patch": 0}},
                "presets": [
                    {"id": "game-settings-preset:a", "jvmOptions": "-Xmx8G"},
                    {"id": "game-settings-preset:b", "jvmOptions": "-Xmx1G", "minMemory": 512, "maxMemory": 1024}
                ]
            }"#,
        )
        .unwrap();
        std::fs::write(
            config_dir.join("launcher-settings.json"),
            r#"{"defaultGameSettingsPreset": "game-settings-preset:b"}"#,
        )
        .unwrap();

        // Per-version values win; unset fields fall back to the default preset.
        let mut java_args = Vec::new();
        let mut memory = MemorySettings {
            maximum: 2048,
            automatic: false,
            optimize_before_launch: false,
        };
        let effective = hmcl_with_global_fallback(
            &HmclVersionSettings {
                uses_global: Some(true),
                java_args: Some("-XX:PerVersion".to_string()),
                ..Default::default()
            },
            Some(launcher.path()),
        );
        apply_hmcl_settings(
            &effective,
            &mut memory,
            &mut WindowSize(854, 480),
            &mut java_args,
            Some(17),
        );
        assert!(java_args.contains(&"-XX:PerVersion".to_string()));
        assert!(!java_args.contains(&"-Xmx1G".to_string()));
        assert!(java_args.contains(&"-Xms512M".to_string()));
        assert_eq!(memory.maximum, 1024);

        // Without usesGlobal the file is never consulted.
        let untouched = hmcl_with_global_fallback(
            &HmclVersionSettings::default(),
            Some(launcher.path()),
        );
        assert_eq!(untouched.min_memory, None);
        assert_eq!(untouched.max_memory, None);

        // A missing global file keeps the per-version values untouched.
        let missing = hmcl_with_global_fallback(
            &HmclVersionSettings {
                uses_global: Some(true),
                ..Default::default()
            },
            Some(tempfile::tempdir().unwrap().path()),
        );
        assert_eq!(missing.java_args, None);
    }

    #[test]
    fn hmcl_server_ip_maps_to_legacy_server_game_arguments() {
        let mut java_args = Vec::new();
        let game_args = apply_hmcl_settings(
            &HmclVersionSettings {
                server_ip: Some("mc.example.com".to_string()),
                server_port: None,
                ..Default::default()
            },
            &mut MemorySettings {
                maximum: 1024,
                automatic: false,
                optimize_before_launch: false,
            },
            &mut WindowSize(854, 480),
            &mut java_args,
            Some(17),
        );
        // DefaultLauncher emits --server/--port for versions without Quick
        // Play and defaults the port to 25565.
        assert_eq!(
            game_args,
            ["--server", "mc.example.com", "--port", "25565"]
        );

        let mut java_args = Vec::new();
        let game_args = apply_hmcl_settings(
            &HmclVersionSettings {
                server_ip: Some("mc.example.com".to_string()),
                server_port: Some(25566),
                ..Default::default()
            },
            &mut MemorySettings {
                maximum: 1024,
                automatic: false,
                optimize_before_launch: false,
            },
            &mut WindowSize(854, 480),
            &mut java_args,
            Some(17),
        );
        assert_eq!(
            game_args,
            ["--server", "mc.example.com", "--port", "25566"]
        );

        let mut java_args = Vec::new();
        let game_args = apply_hmcl_settings(
            &HmclVersionSettings {
                server_ip: Some("  ".to_string()),
                ..Default::default()
            },
            &mut MemorySettings {
                maximum: 1024,
                automatic: false,
                optimize_before_launch: false,
            },
            &mut WindowSize(854, 480),
            &mut java_args,
            Some(17),
        );
        assert!(game_args.is_empty());
        assert!(java_args.is_empty());
    }

    #[test]
    fn hmcl_perm_size_numbers_gain_unit_and_memory_bounds_are_clamped() {
        let mut java_args = Vec::new();
        let mut memory = MemorySettings {
            maximum: 4096,
            automatic: false,
            optimize_before_launch: false,
        };
        apply_hmcl_settings(
            &HmclVersionSettings {
                max_memory: Some(1024),
                min_memory: Some(4096),
                perm_size: Some("256".to_string()),
                ..Default::default()
            },
            &mut memory,
            &mut WindowSize(854, 480),
            &mut java_args,
            Some(17),
        );
        // Purely numeric permSize gains HMCL's lowercase unit...
        assert!(java_args.contains(&"-XX:MetaspaceSize=256m".to_string()));
        // ...and min memory is clamped down to the configured maximum.
        assert_eq!(memory.maximum, 1024);
        assert!(java_args.contains(&"-Xms1024M".to_string()));
        assert!(!java_args.contains(&"-Xms4096M".to_string()));

        // Values that already carry a unit pass through unchanged.
        let mut java_args = Vec::new();
        apply_hmcl_settings(
            &HmclVersionSettings {
                perm_size: Some("512M".to_string()),
                ..Default::default()
            },
            &mut memory,
            &mut WindowSize(854, 480),
            &mut java_args,
            Some(17),
        );
        assert_eq!(java_args, ["-XX:MetaspaceSize=512M"]);
    }

    fn resolve_linked_fixture(
        root: &Path,
        version_id: &str,
        libraries: serde_json::Value,
    ) -> MergedVersion {
        let directory = root.join("versions").join(version_id);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join(format!("{version_id}.json")),
            serde_json::to_vec(
                &json!({ "id": version_id, "libraries": libraries }),
            )
            .unwrap(),
        )
        .unwrap();
        DirectLinkedLaunch {
            dot_minecraft: root.to_path_buf(),
            launcher_root: None,
            version_id: version_id.to_string(),
            version_json: None,
            dialect: LinkedLauncherDialect::Generic,
            game_dir_mode: None,
        }
        .resolve()
        .unwrap()
        .merged
    }

    fn cleanroom_mixed_fixture() -> serde_json::Value {
        // The reported 1.12.2 + Cleanroom 0.6.11-alpha direct-link failure:
        // the vanilla parent contributes the LWJGL 2 family (`lwjgl`,
        // `lwjgl_util`, and the native carrier `lwjgl-platform`) while
        // Cleanroom contributes the LWJGL 3 line; no LWJGL 2 jar may ever
        // share a launch with the LWJGL 3 files. The vanilla JNA platform
        // and Mojang ICU bundle are removed by the pre-existing Axolotl
        // Cleanroom rule as well.
        json!([
            {"name": "com.cleanroommc:cleanroom:0.6.11-alpha"},
            {"name": "org.lwjgl.lwjgl:lwjgl:2.9.4-nightly-20150209"},
            {"name": "org.lwjgl.lwjgl:lwjgl_util:2.9.4-nightly-20150209"},
            {"name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209", "natives": {"windows": "natives-windows"}},
            {"name": "org.lwjgl:lwjgl:3.4.1-unsafe"},
            {"name": "org.lwjgl:lwjgl:3.4.1-unsafe", "natives": {"windows": "natives-windows"}},
            {"name": "com.cleanroommc:lwjglxx:1.1.22"},
            {"name": "net.java.dev.jna:jna:5.19.1"},
            {"name": "net.java.dev.jna:platform:3.4.0"},
            {"name": "com.ibm.icu:icu4j-core-mojang:51.2"}
        ])
    }

    #[test]
    fn hmcl_child_before_parent_merge_and_cleanroom_normalization_drop_lwjgl2_family()
     {
        // Realistic HMCL 1.12.2 + Cleanroom layout: the Cleanroom version
        // document inherits the vanilla `1.12.2` document. The merger puts
        // the child (loader) libraries before the parent (vanilla) ones, so
        // without normalization the vanilla LWJGL 2 family would sit after
        // the Cleanroom LWJGL 3 line.
        let root = tempfile::tempdir().unwrap();
        let vanilla_dir = root.path().join("versions/1.12.2");
        std::fs::create_dir_all(&vanilla_dir).unwrap();
        std::fs::write(
            vanilla_dir.join("1.12.2.json"),
            serde_json::to_vec(&json!({
                "id": "1.12.2",
                "mainClass": "net.minecraft.client.main.Main",
                "libraries": [
                    {"name": "org.lwjgl.lwjgl:lwjgl:2.9.4-nightly-20150209"},
                    {"name": "org.lwjgl.lwjgl:lwjgl_util:2.9.4-nightly-20150209"},
                    {"name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209", "natives": {"windows": "natives-windows"}},
                    {"name": "net.java.dev.jna:platform:3.4.0"},
                    {"name": "com.ibm.icu:icu4j-core-mojang:51.2"}
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        let cleanroom_dir =
            root.path().join("versions/1.12.2-Cleanroom-0.6.11-alpha");
        std::fs::create_dir_all(&cleanroom_dir).unwrap();
        std::fs::write(
            cleanroom_dir.join("1.12.2-Cleanroom-0.6.11-alpha.json"),
            serde_json::to_vec(&json!({
                "id": "1.12.2-Cleanroom-0.6.11-alpha",
                "inheritsFrom": "1.12.2",
                "mainClass": "net.minecraft.launchwrapper.Launch",
                "libraries": [
                    {"name": "com.cleanroommc:cleanroom:0.6.11-alpha"},
                    {"name": "org.lwjgl:lwjgl:3.4.1-unsafe"},
                    {"name": "org.lwjgl:lwjgl:3.4.1-unsafe", "natives": {"windows": "natives-windows"}},
                    {"name": "com.cleanroommc:lwjglxx:1.1.22"},
                    {"name": "net.java.dev.jna:jna:5.19.1"}
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        let resolved = DirectLinkedLaunch {
            dot_minecraft: root.path().to_path_buf(),
            launcher_root: None,
            version_id: "1.12.2-Cleanroom-0.6.11-alpha".to_string(),
            version_json: None,
            dialect: LinkedLauncherDialect::Hmcl,
            game_dir_mode: None,
        }
        .resolve()
        .unwrap();
        let mut merged = resolved.merged;
        let names = merged
            .libraries
            .iter()
            .map(|library| library.library.name.clone())
            .collect::<Vec<_>>();
        let lwjgl3_pos = names
            .iter()
            .position(|name| name == "org.lwjgl:lwjgl:3.4.1-unsafe")
            .expect("Cleanroom LWJGL 3 line merged");
        let lwjgl2_pos = names
            .iter()
            .position(|name| {
                name == "org.lwjgl.lwjgl:lwjgl:2.9.4-nightly-20150209"
            })
            .expect("vanilla LWJGL 2 binding merged");
        let platform_pos = names
            .iter()
            .position(|name| {
                name == "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209"
            })
            .expect("vanilla LWJGL 2 native carrier merged");
        // Child-first/parent-last: the LWJGL 2 family sits after the
        // Cleanroom LWJGL 3 natives, which is exactly the shadowing order
        // the normalization below must remove.
        assert!(
            lwjgl3_pos < lwjgl2_pos && lwjgl2_pos < platform_pos,
            "unexpected merge order: {names:?}"
        );
        assert_eq!(
            names
                .iter()
                .filter(|name| { *name == "org.lwjgl:lwjgl:3.4.1-unsafe" })
                .count(),
            2,
            "HMCL keeps the LWJGL 3 main jar and its native classifier"
        );

        let removed = normalize_merged_loader_libraries(
            ModLoader::Cleanroom,
            &mut merged,
        );
        assert_eq!(
            removed,
            [
                "org.lwjgl.lwjgl:lwjgl:2.9.4-nightly-20150209",
                "org.lwjgl.lwjgl:lwjgl_util:2.9.4-nightly-20150209",
                "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209",
                "net.java.dev.jna:platform:3.4.0",
                "com.ibm.icu:icu4j-core-mojang:51.2",
            ]
        );
        let names = merged
            .libraries
            .iter()
            .map(|library| library.library.name.as_str())
            .collect::<Vec<_>>();
        for kept in [
            "com.cleanroommc:cleanroom:0.6.11-alpha",
            "org.lwjgl:lwjgl:3.4.1-unsafe",
            "com.cleanroommc:lwjglxx:1.1.22",
            "net.java.dev.jna:jna:5.19.1",
        ] {
            assert!(names.contains(&kept), "{kept}");
        }
        assert_eq!(names.len(), 5);
    }

    #[test]
    fn cleanroom_linked_merge_drops_legacy_conflicts_at_launch_normalization() {
        let root = tempfile::tempdir().unwrap();
        let mut merged = resolve_linked_fixture(
            root.path(),
            "1.12.2",
            cleanroom_mixed_fixture(),
        );

        let removed = normalize_merged_loader_libraries(
            ModLoader::Cleanroom,
            &mut merged,
        );
        assert_eq!(
            removed,
            [
                "org.lwjgl.lwjgl:lwjgl:2.9.4-nightly-20150209",
                "org.lwjgl.lwjgl:lwjgl_util:2.9.4-nightly-20150209",
                "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209",
                "net.java.dev.jna:platform:3.4.0",
                "com.ibm.icu:icu4j-core-mojang:51.2",
            ]
        );

        let names = merged
            .libraries
            .iter()
            .map(|library| library.library.name.as_str())
            .collect::<Vec<_>>();
        for removed in &removed {
            assert!(!names.contains(&removed.as_str()), "{removed}");
        }
        for kept in [
            "com.cleanroommc:cleanroom:0.6.11-alpha",
            "org.lwjgl:lwjgl:3.4.1-unsafe",
            "com.cleanroommc:lwjglxx:1.1.22",
            "net.java.dev.jna:jna:5.19.1",
        ] {
            assert!(names.contains(&kept), "{kept}");
        }
        assert_eq!(names.len(), 5);
    }

    #[test]
    fn non_cleanroom_loaders_leave_linked_libraries_untouched() {
        // No `com.cleanroommc:cleanroom` coordinate: the launcher label alone
        // decides, so other loaders (and vanilla) keep the legacy Mojang
        // LWJGL 2/@JNA/ICU libraries exactly as the linked installation
        // defines them.
        let non_cleanroom_fixture = || {
            json!([
                {"name": "net.minecraftforge:forge:1.12.2-14.23.5.2864"},
                {"name": "org.lwjgl.lwjgl:lwjgl:2.9.4-nightly-20150209"},
                {"name": "org.lwjgl.lwjgl:lwjgl_util:2.9.4-nightly-20150209"},
                {"name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209", "natives": {"windows": "natives-windows"}},
                {"name": "net.java.dev.jna:platform:3.4.0"},
                {"name": "com.ibm.icu:icu4j-core-mojang:51.2"}
            ])
        };
        for loader in [ModLoader::Vanilla, ModLoader::Forge] {
            let root = tempfile::tempdir().unwrap();
            let mut merged = resolve_linked_fixture(
                root.path(),
                "1.12.2",
                non_cleanroom_fixture(),
            );
            let original = merged
                .libraries
                .iter()
                .map(|library| library.library.name.clone())
                .collect::<Vec<_>>();

            let removed =
                normalize_merged_loader_libraries(loader, &mut merged);
            assert!(removed.is_empty(), "{loader:?}");
            assert_eq!(
                merged
                    .libraries
                    .iter()
                    .map(|library| library.library.name.clone())
                    .collect::<Vec<_>>(),
                original
            );
        }
    }

    #[test]
    fn cleanroom_library_coordinate_falls_back_when_vanilla_label_misses() {
        // The frozen content-set loader label can mislabel a directly linked
        // Cleanroom installation as vanilla (e.g. associations created before
        // loader detection covered Cleanroom); the merged
        // `com.cleanroommc:cleanroom` coordinate is then authoritative.
        let root = tempfile::tempdir().unwrap();
        let mut merged = resolve_linked_fixture(
            root.path(),
            "1.12.2",
            cleanroom_mixed_fixture(),
        );

        let removed =
            normalize_merged_loader_libraries(ModLoader::Vanilla, &mut merged);
        assert_eq!(
            removed,
            [
                "org.lwjgl.lwjgl:lwjgl:2.9.4-nightly-20150209",
                "org.lwjgl.lwjgl:lwjgl_util:2.9.4-nightly-20150209",
                "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209",
                "net.java.dev.jna:platform:3.4.0",
                "com.ibm.icu:icu4j-core-mojang:51.2",
            ]
        );
        let names = merged
            .libraries
            .iter()
            .map(|library| library.library.name.as_str())
            .collect::<Vec<_>>();
        for name in &removed {
            assert!(!names.contains(&name.as_str()), "{name}");
        }
    }

    #[test]
    fn non_cleanroom_loader_labels_never_fall_back_on_cleanroom_coordinate() {
        // A Forge/other frozen label is authoritative even when the merged
        // list happens to carry the Cleanroom coordinate: the fallback is
        // strictly limited to the vanilla label.
        for loader in [
            ModLoader::Forge,
            ModLoader::NeoForge,
            ModLoader::Fabric,
            ModLoader::OptiFine,
        ] {
            let root = tempfile::tempdir().unwrap();
            let mut merged = resolve_linked_fixture(
                root.path(),
                "1.12.2",
                cleanroom_mixed_fixture(),
            );
            let original = merged
                .libraries
                .iter()
                .map(|library| library.library.name.clone())
                .collect::<Vec<_>>();

            let removed =
                normalize_merged_loader_libraries(loader, &mut merged);
            assert!(removed.is_empty(), "{loader:?}");
            assert_eq!(
                merged
                    .libraries
                    .iter()
                    .map(|library| library.library.name.clone())
                    .collect::<Vec<_>>(),
                original
            );
        }
    }

    #[test]
    fn cleanroom_normalization_keeps_lwjgl3_line_and_lwjglxx() {
        // Cleanroom normalization removes the LWJGL 2 carrier but keeps the
        // Cleanroom LWJGL 3 line (main plus native classifier) and lwjglxx.
        let root = tempfile::tempdir().unwrap();
        let mut merged = resolve_linked_fixture(
            root.path(),
            "1.12.2",
            json!([
                {"name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209", "natives": {"windows": "natives-windows"}},
                {"name": "org.lwjgl:lwjgl:3.4.1-unsafe"},
                {"name": "org.lwjgl:lwjgl:3.4.1-unsafe", "natives": {"windows": "natives-windows"}},
                {"name": "com.cleanroommc:lwjglxx:1.1.22"}
            ]),
        );

        let removed = normalize_merged_loader_libraries(
            ModLoader::Cleanroom,
            &mut merged,
        );
        assert_eq!(
            removed,
            ["org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209"]
        );
        let names = merged
            .libraries
            .iter()
            .map(|library| library.library.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "org.lwjgl:lwjgl:3.4.1-unsafe",
                "org.lwjgl:lwjgl:3.4.1-unsafe",
                "com.cleanroommc:lwjglxx:1.1.22",
            ]
        );
    }

    #[test]
    fn cleanroom_normalization_keeps_vanilla_lwjgl2_native_out_of_extraction() {
        // The real 1.12.2 hazard: the vanilla LWJGL 2 carrier
        // (`lwjgl-platform-2.9.4-nightly-20150209-natives-windows.jar`) ships
        // a root-level `lwjgl.dll`, while Cleanroom's LWJGL 3 natives are
        // nested (`windows/x64/org/lwjgl/lwjgl.dll`), so the two generations
        // never overwrite each other — but the carrier's root-level
        // `lwjgl.dll` is exactly the library name `System.loadLibrary("lwjgl")`
        // (org.lwjgl.Sys) resolves first in the natives directory. Normalizing
        // the merge for Cleanroom removes the carrier so the vanilla native
        // never reaches the natives directory.
        let root = tempfile::tempdir().unwrap();
        let os = serde_json::to_value(Os::native().get_os()).unwrap();
        let natives = |classifier: &str| {
            let mut map = serde_json::Map::new();
            map.insert(os.as_str().unwrap().to_string(), json!(classifier));
            map
        };
        let lwjgl3_natives =
            root.path().join("libraries/org/lwjgl/lwjgl/3.4.1-unsafe");
        let lwjgl2_natives = root.path().join(
            "libraries/org/lwjgl/lwjgl/lwjgl-platform/2.9.4-nightly-20150209",
        );
        std::fs::create_dir_all(&lwjgl3_natives).unwrap();
        std::fs::create_dir_all(&lwjgl2_natives).unwrap();
        write_native_jar(
            &lwjgl3_natives.join("native3.jar"),
            "windows/x64/org/lwjgl/lwjgl.dll",
            b"LWJGL3",
        );
        write_native_jar(
            &lwjgl2_natives.join("native2.jar"),
            "lwjgl.dll",
            b"LWJGL2",
        );

        let direct = DirectLinkedLaunch {
            dot_minecraft: root.path().to_path_buf(),
            launcher_root: None,
            version_id: "1.12.2".to_string(),
            version_json: None,
            dialect: LinkedLauncherDialect::Hmcl,
            game_dir_mode: None,
        };
        let lwjgl3_library: LinkedLibrary = serde_json::from_value(json!({
            "name": "org.lwjgl:lwjgl:3.4.1-unsafe",
            "natives": natives("natives-lwjgl3"),
            "downloads": {"classifiers": {"natives-lwjgl3": {"path": "org/lwjgl/lwjgl/3.4.1-unsafe/native3.jar", "sha1": "", "size": 0, "url": ""}}}
        }))
        .unwrap();
        let lwjgl2_library: LinkedLibrary = serde_json::from_value(json!({
            "name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209",
            "natives": natives("natives-lwjgl2"),
            "downloads": {"classifiers": {"natives-lwjgl2": {"path": "org/lwjgl/lwjgl/lwjgl-platform/2.9.4-nightly-20150209/native2.jar", "sha1": "", "size": 0, "url": ""}}}
        }))
        .unwrap();

        // Unnormalized merge order (loader libraries first, vanilla last):
        // the LWJGL 2 carrier's root-level `lwjgl.dll` lands in the natives
        // directory after the LWJGL 3 nested files were written.
        let unnormalized = vec![lwjgl3_library.clone(), lwjgl2_library.clone()];
        let shadowed = root.path().join("shadowed");
        extract_linked_natives(
            &direct,
            &unnormalized,
            &shadowed,
            std::env::consts::ARCH,
            true,
        )
        .unwrap();
        assert_eq!(
            std::fs::read(shadowed.join("lwjgl.dll")).unwrap(),
            b"LWJGL2",
            "root-level LWJGL 2 dll is what System.loadLibrary(\"lwjgl\") resolves"
        );
        assert_eq!(
            std::fs::read(shadowed.join("windows/x64/org/lwjgl/lwjgl.dll"))
                .unwrap(),
            b"LWJGL3"
        );

        // Cleanroom normalization drops the carrier from the merged list, so
        // the extraction pass that follows the normalize point in the launch
        // pipeline never writes the vanilla native.
        let mut merged = minimal_merged();
        merged.libraries = vec![lwjgl3_library, lwjgl2_library];
        normalize_merged_loader_libraries(ModLoader::Cleanroom, &mut merged);
        let normalized = merged.libraries.to_vec();
        assert_eq!(normalized.len(), 1);
        let target = root.path().join("normalized");
        extract_linked_natives(
            &direct,
            &normalized,
            &target,
            std::env::consts::ARCH,
            true,
        )
        .unwrap();
        assert!(
            !target.join("lwjgl.dll").exists(),
            "no vanilla LWJGL 2 dll may sit in the natives directory root"
        );
        assert_eq!(
            std::fs::read(target.join("windows/x64/org/lwjgl/lwjgl.dll"))
                .unwrap(),
            b"LWJGL3"
        );
    }

    fn write_native_jar(path: &std::path::Path, entry: &str, content: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file(entry, options).unwrap();
        zip.write_all(content).unwrap();
        zip.finish().unwrap();
    }
}
