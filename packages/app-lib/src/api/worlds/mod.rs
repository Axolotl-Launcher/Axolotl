use crate::data::ModLoader;
use crate::event::InstancePayloadType;
use crate::event::emit::emit_instance;
use crate::launcher::get_loader_version_from_profile;
use crate::launcher::instance_runtime::InstanceRuntimeAdapter;
use crate::server_address::{parse_server_address, resolve_server_address};
use crate::state::attached_world_data::AttachedWorldData;
use crate::state::{
    InstanceInstallStage, attached_world_data, server_join_log,
};
use crate::util::protocol_version::OLD_PROTOCOL_VERSIONS;
pub use crate::util::protocol_version::ProtocolVersion;

mod datapacks;
mod level_data;
pub use crate::util::server_ping::{
    ServerGameProfile, ServerPlayers, ServerStatus, ServerVersion,
};
use crate::util::{io, server_ping};
use crate::{Error, ErrorKind, Result, State, launcher};
use async_minecraft_ping::ServerDescription;
use async_walkdir::WalkDir;
use async_zip::{Compression, ZipEntryBuilder};
use chrono::{DateTime, Local, TimeZone, Utc};
pub use datapacks::*;
use either::Either;
use enumset::{EnumSet, EnumSetType};
use fs4::tokio::AsyncFileExt;
use futures::StreamExt;
pub use level_data::*;
use quartz_nbt::{NbtCompound, NbtList, NbtTag};
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::cmp::Reverse;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tokio::task::JoinSet;
use tokio_util::compat::FuturesAsyncWriteCompatExt;
use url::Url;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct WorldWithInstance {
    pub instance_id: String,
    #[serde(flatten)]
    pub world: World,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct World {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_played: Option<DateTime<Utc>>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        with = "either::serde_untagged_optional"
    )]
    pub icon: Option<Either<PathBuf, Url>>,
    pub display_status: DisplayStatus,
    #[serde(flatten)]
    pub details: WorldDetails,
}

impl World {
    pub fn world_type(&self) -> WorldType {
        match self.details {
            WorldDetails::Singleplayer { .. } => WorldType::Singleplayer,
            WorldDetails::Server { .. } => WorldType::Server,
        }
    }

    pub fn world_id(&self) -> &str {
        match &self.details {
            WorldDetails::Singleplayer { path, .. } => path,
            WorldDetails::Server { address, .. } => address,
        }
    }
}

#[derive(
    Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Hash, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum WorldType {
    #[default]
    Singleplayer,
    Server,
}

impl WorldType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Singleplayer => "singleplayer",
            Self::Server => "server",
        }
    }

    pub fn from_string(string: &str) -> Self {
        match string {
            "singleplayer" => Self::Singleplayer,
            "server" => Self::Server,
            _ => Self::Singleplayer,
        }
    }
}

#[derive(Deserialize, Serialize, EnumSetType, Debug, Default)]
#[serde(rename_all = "snake_case")]
#[enumset(serialize_repr = "list")]
pub enum DisplayStatus {
    #[default]
    Normal,
    Hidden,
    Favorite,
}

impl DisplayStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Hidden => "hidden",
            Self::Favorite => "favorite",
        }
    }

    pub fn from_string(string: &str) -> Self {
        match string {
            "normal" => Self::Normal,
            "hidden" => Self::Hidden,
            "favorite" => Self::Favorite,
            _ => Self::Normal,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorldDetails {
    Singleplayer {
        path: String,
        game_mode: SingleplayerGameMode,
        hardcore: bool,
        locked: bool,
    },
    Server {
        index: usize,
        address: String,
        pack_status: ServerPackStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        project_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        content_kind: Option<String>,
    },
}

#[derive(Deserialize, Serialize, Debug, Copy, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub enum SingleplayerGameMode {
    #[default]
    Survival,
    Creative,
    Adventure,
    Spectator,
}

#[derive(Deserialize, Serialize, Debug, Copy, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServerPackStatus {
    Enabled,
    Disabled,
    #[default]
    Prompt,
}

impl From<Option<bool>> for ServerPackStatus {
    fn from(value: Option<bool>) -> Self {
        match value {
            Some(true) => ServerPackStatus::Enabled,
            Some(false) => ServerPackStatus::Disabled,
            None => ServerPackStatus::Prompt,
        }
    }
}

impl From<ServerPackStatus> for Option<bool> {
    fn from(val: ServerPackStatus) -> Self {
        match val {
            ServerPackStatus::Enabled => Some(true),
            ServerPackStatus::Disabled => Some(false),
            ServerPackStatus::Prompt => None,
        }
    }
}

pub async fn get_recent_worlds(
    limit: usize,
    display_statuses: EnumSet<DisplayStatus>,
) -> Result<Vec<WorldWithInstance>> {
    if limit == 0 || display_statuses.is_empty() {
        return Ok(Vec::new());
    }
    let state = State::get().await?;

    let instances = crate::state::list_instances(&state.pool).await?;

    let mut result = Vec::with_capacity(limit.min(256));

    for instance in instances {
        let instance_id = &instance.instance.id;
        // Other launchers can update world timestamps without updating our
        // instance timestamp, so every instance must be considered.
        let instance_worlds =
            get_worlds_for_instance(&instance.instance, &state).await;
        if let Err(e) = instance_worlds {
            if matches!(e.raw.as_ref(), ErrorKind::IOError(error)
                if error.kind() == std::io::ErrorKind::NotFound)
            {
                continue;
            }
            tracing::error!(
                "Failed to get worlds for instance {}: {}",
                instance_id,
                e
            );
            continue;
        }
        for world in instance_worlds? {
            if !display_statuses.contains(world.display_status) {
                continue;
            }
            result.push(WorldWithInstance {
                instance_id: instance_id.clone(),
                world,
            });
        }
        if result.len() > limit {
            result.sort_by_key(|x| Reverse(x.world.last_played));
            result.truncate(limit);
        }
    }

    result.sort_by_key(|x| Reverse(x.world.last_played));
    Ok(result)
}

pub async fn get_favorite_worlds() -> Result<Vec<WorldWithInstance>> {
    get_recent_worlds(usize::MAX, enumset::enum_set!(DisplayStatus::Favorite))
        .await
}

pub async fn get_instance_worlds(instance_id: &str) -> Result<Vec<World>> {
    let state = State::get().await?;
    let instance = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or_else(|| ErrorKind::InputError("Unknown instance".to_string()))?;
    get_worlds_for_instance(&instance.instance, &state).await
}

async fn get_worlds_for_instance(
    instance: &crate::state::Instance,
    state: &State,
) -> Result<Vec<World>> {
    let adapter =
        InstanceRuntimeAdapter::for_instance(instance, &state.directories)?;
    let game_dir = io::canonicalize(adapter.game_dir())?;
    get_all_worlds_in_instance(&instance.id, &game_dir).await
}

async fn resolve_instance_id(instance: &str, state: &State) -> Result<String> {
    resolve_instance_identity(instance, state)
        .await
        .map(|(instance_id, _, _)| instance_id)
}

async fn resolve_instance_identity(
    instance: &str,
    state: &State,
) -> Result<(String, String, Option<String>)> {
    let row = sqlx::query!(
        "
		SELECT id, path, game_dir_override
		FROM instances
		WHERE id = ? OR path = ?
		ORDER BY CASE WHEN id = ? THEN 0 ELSE 1 END
		LIMIT 1
		",
        instance,
        instance,
        instance,
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError(format!(
            "Unknown instance id or path: {instance}"
        ))
        .as_error()
    })?;

    Ok((row.id, row.path, row.game_dir_override))
}

/// Resolves the directory holding an instance's worlds, `servers.dat` and
/// datapacks.
///
/// Directly associated instances keep that data inside the external
/// installation rather than a profile directory under `instances`, mirroring
/// `api::instance::paths::get_full_path`; a per-instance `game_dir_override`
/// resolves to the override root, and PCL version isolation resolves to the
/// version's own game directory. Unknown records fall back to the legacy
/// profile path so the error surface stays unchanged.
async fn resolve_instance_data_dir(
    instance_id: &str,
    instance_path: &str,
    game_dir_override: Option<&str>,
    state: &State,
) -> Result<PathBuf> {
    if let Some(metadata) =
        crate::state::get_instance(instance_id, &state.pool).await?
    {
        return crate::state::instances::instance_content_root(
            &state.directories,
            &metadata.instance,
        );
    }
    Ok(state
        .directories
        .resolve_game_dir(instance_path, game_dir_override))
}

async fn get_all_worlds_in_instance(
    instance_id: &str,
    instance_dir: &Path,
) -> Result<Vec<World>> {
    let mut worlds = vec![];
    get_singleplayer_worlds_in_instance(instance_dir, &mut worlds).await?;
    let state = State::get().await?;

    get_server_worlds_in_instance(instance_id, instance_dir, &mut worlds)
        .await?;

    let attached_data =
        AttachedWorldData::get_all_for_instance(instance_id, &state.pool)
            .await?;
    if !attached_data.is_empty() {
        for world in &mut worlds {
            if let Some(data) = attached_data
                .get(&(world.world_type(), world.world_id().to_owned()))
            {
                attach_world_data_to_world(world, data);
            }
        }
    }

    Ok(worlds)
}

async fn get_singleplayer_worlds_in_instance(
    instance_dir: &Path,
    worlds: &mut Vec<World>,
) -> Result<()> {
    let saves_dir = instance_dir.join("saves");
    if !saves_dir.exists() {
        return Ok(());
    }
    let mut entries = io::read_dir(&saves_dir).await?;
    let mut tasks = JoinSet::new();
    while let Some(world_dir) = entries.next_entry().await? {
        let world_path = world_dir.path();
        if !world_path.join("level.dat").exists() {
            continue;
        }
        tasks.spawn(read_singleplayer_world(world_path));
    }
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(world)) => worlds.push(world),
            Ok(Err(e)) => {
                tracing::warn!("Skipping unreadable world: {e}");
            }
            Err(e) => {
                tracing::warn!("World read task panicked: {e}");
            }
        }
    }

    Ok(())
}

pub async fn get_singleplayer_world(
    instance: &str,
    world: &str,
) -> Result<World> {
    let state = State::get().await?;
    let (instance_id, instance_path, game_dir_override) =
        resolve_instance_identity(instance, &state).await?;
    let instance_dir = resolve_instance_data_dir(
        &instance_id,
        &instance_path,
        game_dir_override.as_deref(),
        &state,
    )
    .await?;
    let mut world =
        read_singleplayer_world(get_world_dir(&instance_dir, world)?).await?;

    if let Some(data) = AttachedWorldData::get_for_world(
        &instance_id,
        world.world_type(),
        world.world_id(),
        &state.pool,
    )
    .await?
    {
        attach_world_data_to_world(&mut world, &data);
    }
    Ok(world)
}

async fn read_singleplayer_world(world_path: PathBuf) -> Result<World> {
    if let Some(_lock) = try_get_world_session_lock(&world_path).await? {
        read_singleplayer_world_maybe_locked(world_path, false).await
    } else {
        read_singleplayer_world_maybe_locked(world_path, true).await
    }
}

async fn read_singleplayer_world_maybe_locked(
    world_path: PathBuf,
    locked: bool,
) -> Result<World> {
    let raw = io::read(world_path.join("level.dat")).await?;
    let (root, _) = quartz_nbt::io::read_nbt(
        &mut Cursor::new(raw),
        quartz_nbt::io::Flavor::GzCompressed,
    )?;

    let data = root.get::<_, &NbtCompound>("Data").map_err(|_| {
        Error::from(ErrorKind::InputError(
            "Missing Data tag in level.dat".into(),
        ))
    })?;

    let level_name = data
        .get::<_, &str>("LevelName")
        .unwrap_or_default()
        .to_string();
    let last_played = data.get::<_, i64>("LastPlayed").unwrap_or(0);
    let game_type = data.get::<_, i32>("GameType").unwrap_or(0);
    let hardcore = read_hardcore(data);

    let icon = if tokio::fs::try_exists(world_path.join("icon.png"))
        .await
        .unwrap_or(false)
    {
        Some(Either::Left(world_path.join("icon.png")))
    } else {
        None
    };

    let game_mode = match game_type {
        0 => SingleplayerGameMode::Survival,
        1 => SingleplayerGameMode::Creative,
        2 => SingleplayerGameMode::Adventure,
        3 => SingleplayerGameMode::Spectator,
        _ => SingleplayerGameMode::Survival,
    };

    Ok(World {
        name: level_name,
        last_played: Utc.timestamp_millis_opt(last_played).single(),
        icon,
        display_status: DisplayStatus::Normal,
        details: WorldDetails::Singleplayer {
            path: world_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            game_mode,
            hardcore,
            locked,
        },
    })
}

fn read_hardcore(data: &NbtCompound) -> bool {
    data.get::<_, &NbtCompound>("difficulty_settings")
        .and_then(|settings| settings.get::<_, i8>("hardcore"))
        .or_else(|_| data.get::<_, i8>("hardcore"))
        .unwrap_or(0)
        != 0
}

/// Reads the `DataPacks` tag from a world's `level.dat` to determine which
/// datapacks the game has enabled or disabled. Entries look like
/// `file/<name>` (or `vanilla`); matching happens in `datapacks::match_datapack_state`.
async fn read_world_datapack_state(
    world_path: &Path,
) -> Result<(Vec<String>, Vec<String>)> {
    let world_path = world_path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let raw =
            std::fs::read(world_path.join("level.dat")).map_err(|error| {
                Error::from(ErrorKind::InputError(format!(
                    "Could not read level.dat: {error}"
                )))
            })?;
        let (root, _) = quartz_nbt::io::read_nbt(
            &mut Cursor::new(raw),
            quartz_nbt::io::Flavor::GzCompressed,
        )?;
        let data = root.get::<_, &NbtCompound>("Data").map_err(|_| {
            Error::from(ErrorKind::InputError(
                "Missing Data tag in level.dat".into(),
            ))
        })?;
        let data_packs = data.get::<_, &NbtCompound>("DataPacks").ok();

        let read_list = |key: &str| -> Vec<String> {
            data_packs
                .and_then(|packs| packs.get::<_, &NbtList>(key).ok())
                .map(|list| {
                    list.iter_map::<&str>()
                        .filter_map(|value| value.ok().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };

        Ok::<_, Error>((read_list("Enabled"), read_list("Disabled")))
    })
    .await
    .map_err(|error| {
        Error::from(ErrorKind::InputError(format!(
            "Datapack state read task failed: {error}"
        )))
    })?
}

async fn get_server_worlds_in_instance(
    instance_id: &str,
    instance_dir: &Path,
    worlds: &mut Vec<World>,
) -> Result<()> {
    let servers = servers_data::read(instance_dir).await?;
    if servers.is_empty() {
        return Ok(());
    }

    let state = State::get().await?;
    let join_log = server_join_log::get_joins(instance_id, &state.pool)
        .await
        .ok();

    for (index, server) in servers.into_iter().enumerate() {
        if server.hidden {
            // TODO: Figure out whether we want to hide or show direct connect servers
            continue;
        }
        let world = World {
            name: server.name,
            last_played: join_log
                .as_ref()
                .and_then(|log| {
                    let (host, port) = parse_server_address(&server.ip).ok()?;
                    log.get(&(host.to_owned(), port))
                })
                .copied(),
            icon: server
                .icon
                .and_then(|icon| {
                    Url::parse(&format!("data:image/png;base64,{icon}")).ok()
                })
                .map(Either::Right),
            display_status: DisplayStatus::Normal,
            details: WorldDetails::Server {
                index,
                address: server.ip,
                pack_status: server.accept_textures.into(),
                project_id: None,
                content_kind: None,
            },
        };
        worlds.push(world);
    }
    Ok(())
}

fn attach_world_data_to_world(world: &mut World, data: &AttachedWorldData) {
    world.display_status = data.display_status;
    if let WorldDetails::Server {
        project_id,
        content_kind,
        ..
    } = &mut world.details
    {
        *project_id = data.project_id.clone();
        *content_kind = data.content_kind.clone();
    }
}

pub async fn set_world_display_status(
    instance: &str,
    world_type: WorldType,
    world_id: &str,
    display_status: DisplayStatus,
) -> Result<()> {
    let state = State::get().await?;
    let instance_id = resolve_instance_id(instance, &state).await?;
    attached_world_data::set_display_status(
        &instance_id,
        world_type,
        world_id,
        display_status,
        &state.pool,
    )
    .await?;
    emit_instance(&instance_id, InstancePayloadType::Edited).await?;
    Ok(())
}

pub async fn rename_world(
    instance: &Path,
    world: &str,
    new_name: &str,
) -> Result<()> {
    let world = get_world_dir(instance, world)?;
    let level_dat_path = world.join("level.dat");
    if !level_dat_path.exists() {
        return Ok(());
    }
    let _lock = get_world_session_lock(&world).await?;

    let level_data = io::read(&level_dat_path).await?;
    let (mut root_data, _) = quartz_nbt::io::read_nbt(
        &mut Cursor::new(level_data),
        quartz_nbt::io::Flavor::GzCompressed,
    )?;
    let data = root_data.get_mut::<_, &mut NbtCompound>("Data")?;

    data.insert(
        "LevelName",
        NbtTag::String(new_name.trim_ascii().to_string()),
    );

    let mut level_data = vec![];
    quartz_nbt::io::write_nbt(
        &mut level_data,
        None,
        &root_data,
        quartz_nbt::io::Flavor::GzCompressed,
    )?;
    io::write(level_dat_path, level_data).await?;
    Ok(())
}

pub async fn reset_world_icon(instance: &Path, world: &str) -> Result<()> {
    let world = get_world_dir(instance, world)?;
    let icon = world.join("icon.png");
    if let Some(_lock) = try_get_world_session_lock(&world).await? {
        let _ = io::remove_file(icon).await;
    }
    Ok(())
}

pub async fn backup_world(instance: &Path, world: &str) -> Result<u64> {
    let world_dir = get_world_dir(instance, world)?;
    let _lock = get_world_session_lock(&world_dir).await?;
    let backups_dir = instance.join("backups");

    io::create_dir_all(&backups_dir).await?;

    let name_base = {
        let now = Local::now();
        let formatted_time = now.format("%Y-%m-%d_%H-%M-%S");
        format!("{formatted_time}_{world}")
    };
    let output_path =
        backups_dir.join(find_available_name(&backups_dir, &name_base, ".zip"));

    let writer = tokio::fs::File::create(&output_path).await?;
    let mut writer = async_zip::tokio::write::ZipFileWriter::with_tokio(writer);

    let mut walker = WalkDir::new(&world_dir);
    while let Some(entry) = walker.next().await {
        let entry = entry.map_err(|e| io::IOError::IOPathError {
            path: e.path().unwrap().to_string_lossy().to_string(),
            source: e.into_io().unwrap(),
        })?;
        if !entry.file_type().await?.is_file() {
            continue;
        }
        if entry.file_name() == "session.lock" {
            continue;
        }
        let zip_filename = format!(
            "{world}/{}",
            entry
                .path()
                .strip_prefix(&world_dir)?
                .display()
                .to_string()
                .replace('\\', "/")
        );
        let mut stream = writer
            .write_entry_stream(
                ZipEntryBuilder::new(zip_filename.into(), Compression::Deflate)
                    .build(),
            )
            .await?
            .compat_write();
        let mut source = tokio::fs::File::open(entry.path()).await?;
        tokio::io::copy(&mut source, &mut stream).await?;
        stream.into_inner().close().await?;
    }

    writer.close().await?;
    Ok(io::metadata(output_path).await?.len())
}

fn truncate_to_char_boundary(value: &mut String, max_bytes: usize) {
    if value.len() <= max_bytes {
        return;
    }
    let mut cut = max_bytes;
    while !value.is_char_boundary(cut) {
        cut -= 1;
    }
    value.truncate(cut);
}

fn find_available_name(dir: &Path, file_name: &str, extension: &str) -> String {
    static RESERVED_WINDOWS_FILENAMES: LazyLock<Regex> = LazyLock::new(|| {
        RegexBuilder::new(r#"^.*\.|(?:COM|CLOCK\$|CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\..*)?$"#)
            .case_insensitive(true)
            .build()
            .unwrap()
    });
    static COPY_COUNTER_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
        RegexBuilder::new(r#"^(?<name>.*) \((?<count>\d*)\)$"#)
            .case_insensitive(true)
            .unicode(true)
            .build()
            .unwrap()
    });

    let mut file_name = file_name.replace(
        [
            '/', '\n', '\r', '\t', '\0', '\x0c', '`', '?', '*', '\\', '<', '>',
            '|', '"', ':', '.', '/', '"',
        ],
        "_",
    );
    if RESERVED_WINDOWS_FILENAMES.is_match(&file_name) {
        file_name.insert(0, '_');
        file_name.push('_');
    }

    let mut count = 0;
    if let Some(find) = COPY_COUNTER_PATTERN.captures(&file_name) {
        count = find
            .name("count")
            .unwrap()
            .as_str()
            .parse::<i32>()
            .unwrap_or(0);
        let end = find.name("name").unwrap().end();
        drop(find);
        file_name.truncate(end);
    }

    if file_name.len() > 255 - extension.len() {
        truncate_to_char_boundary(&mut file_name, 255 - extension.len());
    }

    let mut current_attempt = file_name.clone();
    loop {
        if count != 0 {
            let with_count = format!(" ({count})");
            if file_name.len() > 255 - with_count.len() {
                truncate_to_char_boundary(
                    &mut current_attempt,
                    255 - with_count.len(),
                );
            }
            current_attempt.push_str(&with_count);
        }

        current_attempt.push_str(extension);

        let result = dir.join(&current_attempt);
        if !result.exists() {
            return current_attempt;
        }

        count += 1;
        current_attempt.replace_range(..current_attempt.len(), &file_name);
    }
}

pub async fn delete_world(instance: &Path, world: &str) -> Result<()> {
    let world = get_world_dir(instance, world)?;
    let lock = get_world_session_lock(&world).await?;
    let lock_path = world.join("session.lock");

    let mut dir = io::read_dir(&world).await?;
    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();
        if entry.file_type().await?.is_dir() {
            io::remove_dir_all(path).await?;
            continue;
        }
        if path != lock_path {
            io::remove_file(path).await?;
        }
    }

    drop(lock);
    io::remove_file(lock_path).await?;
    io::remove_dir(world).await?;

    Ok(())
}

fn get_world_dir(instance: &Path, world: &str) -> Result<PathBuf> {
    if !matches!(
        Path::new(world).components().collect::<Vec<_>>().as_slice(),
        [std::path::Component::Normal(_)]
    ) {
        return Err(ErrorKind::InputError("Invalid world name".into()).into());
    }
    Ok(instance.join("saves").join(world))
}

async fn get_world_session_lock(world: &Path) -> Result<tokio::fs::File> {
    let lock_path = world.join("session.lock");
    let mut file = tokio::fs::File::options()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .await?;
    file.write_all("☃".as_bytes()).await?;
    file.sync_all().await?;
    let locked = file.try_lock_exclusive()?;
    locked.then_some(file).ok_or_else(|| {
        io::IOError::IOPathError {
            source: std::io::Error::new(
                std::io::ErrorKind::ResourceBusy,
                "already locked by Minecraft",
            ),
            path: lock_path.to_string_lossy().into_owned(),
        }
        .into()
    })
}

async fn try_get_world_session_lock(
    world: &Path,
) -> Result<Option<tokio::fs::File>> {
    let file = tokio::fs::File::options()
        .create(true)
        .write(true)
        .truncate(false)
        .open(world.join("session.lock"))
        .await?;
    file.sync_all().await?;
    let locked = file.try_lock_exclusive()?;
    Ok(locked.then_some(file))
}

pub async fn add_server_to_instance(
    instance_id: &str,
    name: String,
    address: String,
    pack_status: ServerPackStatus,
    project_id: Option<String>,
    content_kind: Option<String>,
) -> Result<usize> {
    let state = State::get().await?;
    let (instance_id, instance_path, game_dir_override) =
        resolve_instance_identity(instance_id, &state).await?;
    let instance_dir = resolve_instance_data_dir(
        &instance_id,
        &instance_path,
        game_dir_override.as_deref(),
        &state,
    )
    .await?;
    let mut servers = servers_data::read(&instance_dir).await?;
    let insert_index = servers
        .iter()
        .position(|x| x.hidden)
        .unwrap_or(servers.len());
    servers.insert(
        insert_index,
        servers_data::ServerData {
            name,
            ip: address.clone(),
            accept_textures: pack_status.into(),
            hidden: false,
            icon: None,
        },
    );
    servers_data::write(&instance_dir, &servers).await?;

    if project_id.is_some() || content_kind.is_some() {
        if let Some(project_id) = &project_id {
            attached_world_data::set_project_id(
                &instance_id,
                WorldType::Server,
                &address,
                project_id,
                &state.pool,
            )
            .await?;
        }
        if let Some(content_kind) = &content_kind {
            attached_world_data::set_content_kind(
                &instance_id,
                WorldType::Server,
                &address,
                content_kind,
                &state.pool,
            )
            .await?;
        }
    }

    Ok(insert_index)
}

pub async fn edit_server_in_instance(
    instance_id: &str,
    index: usize,
    name: String,
    address: String,
    pack_status: ServerPackStatus,
) -> Result<()> {
    let state = State::get().await?;
    let (instance_id, instance_path, game_dir_override) =
        resolve_instance_identity(instance_id, &state).await?;
    let instance_dir = resolve_instance_data_dir(
        &instance_id,
        &instance_path,
        game_dir_override.as_deref(),
        &state,
    )
    .await?;
    let mut servers = servers_data::read(&instance_dir).await?;
    let server =
        servers
            .get_mut(index)
            .filter(|x| !x.hidden)
            .ok_or_else(|| {
                ErrorKind::InputError(format!(
                    "No editable server at index {index}"
                ))
                .as_error()
            })?;
    server.name = name;
    server.ip = address;
    server.accept_textures = pack_status.into();
    servers_data::write(&instance_dir, &servers).await?;
    Ok(())
}

pub async fn remove_server_from_instance(
    instance_id: &str,
    index: usize,
) -> Result<()> {
    let state = State::get().await?;
    let (_, instance_path, game_dir_override) =
        resolve_instance_identity(instance_id, &state).await?;
    let instance_dir = resolve_instance_data_dir(
        instance_id,
        &instance_path,
        game_dir_override.as_deref(),
        &state,
    )
    .await?;
    let mut servers = servers_data::read(&instance_dir).await?;
    if servers.get(index).as_ref().is_none_or(|x| x.hidden) {
        return Err(ErrorKind::InputError(format!(
            "No removable server at index {index}"
        ))
        .into());
    }
    servers.remove(index);
    servers_data::write(&instance_dir, &servers).await?;
    Ok(())
}

mod servers_data {
    use crate::Result;
    use crate::util::io;
    use serde::{Deserialize, Serialize};
    use std::path::Path;

    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(rename_all = "camelCase")]
    pub struct ServerData {
        #[serde(default)]
        pub hidden: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub icon: Option<String>,
        #[serde(default)]
        pub ip: String,
        #[serde(default)]
        pub name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub accept_textures: Option<bool>,
    }

    pub async fn read(instance_dir: &Path) -> Result<Vec<ServerData>> {
        #[derive(Deserialize, Debug)]
        struct ServersData {
            #[serde(default)]
            servers: Vec<ServerData>,
        }

        let servers_dat_path = instance_dir.join("servers.dat");
        if !servers_dat_path.exists() {
            return Ok(vec![]);
        }
        let servers_data = io::read(servers_dat_path).await?;
        let servers_data: ServersData = quartz_nbt::serde::deserialize(
            &servers_data,
            quartz_nbt::io::Flavor::Uncompressed,
        )?
        .0;
        Ok(servers_data.servers)
    }

    pub async fn write(
        instance_dir: &Path,
        servers: &[ServerData],
    ) -> Result<()> {
        #[derive(Serialize, Debug)]
        struct ServersData<'a> {
            servers: &'a [ServerData],
        }

        let servers_dat_path = instance_dir.join("servers.dat");
        let data = quartz_nbt::serde::serialize(
            &ServersData { servers },
            None,
            quartz_nbt::io::Flavor::Uncompressed,
        )?;
        io::write(servers_dat_path, data).await?;
        Ok(())
    }
}

pub async fn get_instance_protocol_version(
    instance_id: &str,
) -> Result<Option<ProtocolVersion>> {
    let metadata =
        crate::api::instance::get(instance_id)
            .await?
            .ok_or_else(|| {
                ErrorKind::InputError(format!(
                    "Could not find instance {instance_id}"
                ))
            })?;
    if metadata.instance.install_stage != InstanceInstallStage::Installed {
        return Ok(None);
    }

    if let Some(protocol_version) =
        metadata.applied_content_set.protocol_version
    {
        return Ok(Some(ProtocolVersion::modern(protocol_version)));
    }
    if let Some(protocol_version) =
        OLD_PROTOCOL_VERSIONS.get(&metadata.applied_content_set.game_version)
    {
        return Ok(Some(*protocol_version));
    }

    let state = State::get().await?;
    let (minecraft, version_index) =
        crate::launcher::resolve_minecraft_manifest(
            &metadata.applied_content_set.game_version,
            &state,
        )
        .await?;
    let version = &minecraft.versions[version_index];

    let loader_version = get_loader_version_from_profile(
        &metadata.applied_content_set.game_version,
        metadata.applied_content_set.loader,
        metadata.applied_content_set.loader_version.as_deref(),
    )
    .await?;
    if metadata.applied_content_set.loader != ModLoader::Vanilla
        && loader_version.is_none()
    {
        return Ok(None);
    }

    let version_jar =
        loader_version.as_ref().map_or(version.id.clone(), |it| {
            format!("{}-{}", version.id.clone(), it.id.clone())
        });

    let state = State::get().await?;
    let client_path = state
        .directories
        .version_dir(&version_jar)
        .join(format!("{version_jar}.jar"));

    if !client_path.exists() {
        return Ok(None);
    }

    let version = launcher::read_protocol_version_from_jar(client_path).await?;
    if version.is_some() {
        crate::state::instances::commands::set_applied_content_set_protocol_version(
            &metadata.instance.id,
            version,
            &state.pool,
        )
        .await?;
    }
    Ok(version.map(ProtocolVersion::modern))
}

pub async fn get_server_status(
    address: &str,
    protocol_version: Option<ProtocolVersion>,
) -> Result<ServerStatus> {
    tracing::debug!(
        "Pinging {address} with protocol version {protocol_version:?}"
    );

    get_server_status_old(address, protocol_version).await
    // get_server_status_new(address, protocol_version).await
}

async fn get_server_status_old(
    address: &str,
    protocol_version: Option<ProtocolVersion>,
) -> Result<ServerStatus> {
    let (original_host, original_port) = parse_server_address(address)?;
    let (host, port) =
        resolve_server_address(original_host, original_port).await?;
    tracing::debug!(
        "Pinging {address} with protocol version {protocol_version:?}"
    );
    server_ping::get_server_status(
        &(&host as &str, port),
        (original_host, original_port),
        protocol_version,
    )
    .await
}

async fn _get_server_status_new(
    address: &str,
    protocol_version: Option<ProtocolVersion>,
) -> Result<ServerStatus> {
    let (address, port) = match address.rsplit_once(':') {
        Some((addr, port)) => {
            let port = port.parse::<u16>().map_err(|_err| {
                Error::from(ErrorKind::InputError("invalid port number".into()))
            })?;
            (addr, port)
        }
        None => (address, 25565),
    };

    let mut builder = async_minecraft_ping::ConnectionConfig::build(address)
        .with_port(port)
        .with_srv_lookup();

    if let Some(version) = protocol_version {
        builder = builder.with_protocol_version(version.version as usize)
    }

    let conn = builder.connect().await.map_err(|_err| {
        Error::from(ErrorKind::InputError("failed to connect to server".into()))
    })?;

    let ping_conn = conn.status().await.map_err(|_err| {
        Error::from(ErrorKind::InputError("failed to get server status".into()))
    })?;
    let status = &ping_conn.status;
    let description = match &status.description {
        ServerDescription::Plain(text) => {
            serde_json::value::to_raw_value(&text).ok()
        }
        ServerDescription::Object { text } => {
            // TODO: `text` always seems to be empty?
            RawValue::from_string(text.clone()).ok()
        }
    };

    let players = ServerPlayers {
        max: status.players.max,
        online: status.players.online,
        sample: status
            .players
            .sample
            .as_ref()
            .map(|sample| {
                sample
                    .iter()
                    .map(|player| ServerGameProfile {
                        id: player.id.clone(),
                        name: player.name.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    };
    let version = ServerVersion {
        name: status.version.name.clone(),
        protocol: status.version.protocol,
        legacy: false,
    };
    let favicon = status.favicon.as_ref().and_then(|url| url.parse().ok());

    let latency = {
        let start = Instant::now();
        let ping_magic = Utc::now().timestamp_millis().cast_unsigned();
        ping_conn.ping(ping_magic).await.map_err(|_err| {
            Error::from(ErrorKind::InputError("failed to do ping".into()))
        })?;
        start.elapsed().as_millis() as i64
    };

    Ok(ServerStatus {
        description,
        players: Some(players),
        version: Some(version),
        favicon,
        enforces_secure_chat: false,
        ping: Some(latency),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::get_full_path;
    use crate::state::{
        CreateDirectLinkInstance, CreateInstance, InstanceLink,
    };
    use futures::FutureExt;
    use quartz_nbt::{NbtCompound, NbtTag};
    use tempfile::TempDir;

    fn write_world(game_dir: &Path, last_played: i64) {
        let world_dir = game_dir.join("saves").join("home-world");
        std::fs::create_dir_all(&world_dir).unwrap();
        let mut data = NbtCompound::new();
        data.insert("LevelName", "Home world");
        data.insert("LastPlayed", last_played);
        let mut root = NbtCompound::new();
        root.insert("Data", data);
        let mut bytes = Vec::new();
        quartz_nbt::io::write_nbt(
            &mut bytes,
            None,
            &root,
            quartz_nbt::io::Flavor::GzCompressed,
        )
        .unwrap();
        std::fs::write(world_dir.join("level.dat"), bytes).unwrap();
    }

    async fn check_home_worlds(
        state: &State,
        fixture: &TempDir,
        created_instances: &mut Vec<crate::state::Instance>,
    ) {
        let mut expected_ids = Vec::new();
        for (index, label) in
            ["pcl-isolated", "pcl-shared", "managed", "override"]
                .into_iter()
                .enumerate()
        {
            let instance = if label.starts_with("pcl-") {
                let launcher_root = fixture.path().join(label);
                let version_dir = launcher_root
                    .join(".minecraft")
                    .join("versions")
                    .join(label);
                std::fs::create_dir_all(&version_dir).unwrap();
                std::fs::write(
                    version_dir.join(format!("{label}.json")),
                    serde_json::to_vec(&serde_json::json!({
                        "id": label,
                        "inheritsFrom": "1.20.1",
                        "mainClass": "net.minecraft.client.main.Main"
                    }))
                    .unwrap(),
                )
                .unwrap();
                crate::state::create_direct_link_instance(
                    CreateDirectLinkInstance {
                        name: None,
                        launcher_type:
                            crate::api::pack::import::ImportLauncherType::PCL2,
                        base_path: launcher_root,
                        instance_folder: label.into(),
                        instance_path: Some(
                            version_dir.to_string_lossy().into_owned(),
                        ),
                        game_dir_mode: (label == "pcl-shared").then_some(
                            crate::launcher::ExternalGameDirMode::Shared,
                        ),
                    },
                    state,
                )
                .await
                .unwrap()
            } else {
                crate::state::instances::commands::create_instance(
                    CreateInstance {
                        name: format!("{label} {}", uuid::Uuid::new_v4()),
                        path: None,
                        game_version: "1.20.1".into(),
                        loader: ModLoader::Vanilla,
                        loader_version: None,
                        icon_path: None,
                        link: InstanceLink::Unmanaged,
                        symlink_target: None,
                        game_dir_override: (label == "override").then(|| {
                            fixture
                                .path()
                                .join(label)
                                .to_string_lossy()
                                .into_owned()
                        }),
                    },
                    state,
                )
                .await
                .unwrap()
            };
            created_instances.push(instance.clone());
            let expected_dir = if label.starts_with("pcl-") {
                let minecraft = fixture.path().join(label).join(".minecraft");
                if label == "pcl-shared" {
                    minecraft
                } else {
                    minecraft.join("versions").join(label)
                }
            } else if label == "override" {
                fixture.path().join(label)
            } else {
                state.directories.instances_dir().join(&instance.path)
            };
            let game_dir = io::canonicalize(expected_dir).unwrap();
            assert_eq!(get_full_path(&instance.id).await.unwrap(), game_dir);
            write_world(&game_dir, 1_900_000_000_000 - index as i64 * 100_000);
            if label == "managed" {
                // Axolotl's instance timestamp can be newer even when a
                // linked world has been played more recently through PCL.
                sqlx::query(
                    "UPDATE instances SET last_played = ? WHERE id = ?",
                )
                .bind(1_900_001_000_i64)
                .bind(&instance.id)
                .execute(&state.pool)
                .await
                .unwrap();
            }
            add_server_to_instance(
                &instance.id,
                "Home server".into(),
                "localhost:25565".into(),
                ServerPackStatus::Prompt,
                None,
                None,
            )
            .await
            .unwrap();
            attached_world_data::set_display_status(
                &instance.id,
                WorldType::Singleplayer,
                "home-world",
                DisplayStatus::Favorite,
                &state.pool,
            )
            .await
            .unwrap();
            assert_eq!(
                get_instance_worlds(&instance.id).await.unwrap().len(),
                2
            );
            expected_ids.push(instance.id);
        }

        let recent = get_recent_worlds(
            usize::MAX,
            enumset::enum_set!(DisplayStatus::Normal | DisplayStatus::Favorite),
        )
        .await
        .unwrap();
        let favorites = get_favorite_worlds().await.unwrap();
        for instance_id in &expected_ids {
            let worlds: Vec<_> = recent
                .iter()
                .filter(|world| &world.instance_id == instance_id)
                .collect();
            assert_eq!(worlds.len(), 2, "recent worlds for {instance_id}");
            assert!(
                worlds
                    .iter()
                    .any(|world| world.world.world_type() == WorldType::Server)
            );
            let pinned: Vec<_> = favorites
                .iter()
                .filter(|world| &world.instance_id == instance_id)
                .collect();
            assert_eq!(pinned.len(), 1, "favorite worlds for {instance_id}");
            assert_eq!(pinned[0].world.world_id(), "home-world");
            assert_eq!(pinned[0].world.name, "Home world");
        }

        assert!(recent.windows(2).all(|pair| {
            pair[0].world.last_played >= pair[1].world.last_played
        }));
        assert!(
            crate::state::get_instance(&expected_ids[0], &state.pool)
                .await
                .unwrap()
                .unwrap()
                .instance
                .last_played
                .is_none()
        );
        for limit in [0, 1, 2, 3, 5, 8] {
            let limited = get_recent_worlds(
                limit,
                enumset::enum_set!(
                    DisplayStatus::Normal | DisplayStatus::Favorite
                ),
            )
            .await
            .unwrap();
            assert_eq!(limited.len(), limit.min(recent.len()));
            assert_eq!(
                limited
                    .iter()
                    .map(|world| world.world.last_played)
                    .collect::<Vec<_>>(),
                recent
                    .iter()
                    .take(limit)
                    .map(|world| world.world.last_played)
                    .collect::<Vec<_>>()
            );
        }
        assert!(
            get_recent_worlds(8, EnumSet::empty())
                .await
                .unwrap()
                .is_empty()
        );

        // Test both a missing and a stale instance timestamp for the newest
        // linked world. Neither is an upper bound on its level.dat timestamp.
        sqlx::query("UPDATE instances SET last_played = ? WHERE id = ?")
            .bind(1_800_000_000_i64)
            .bind(&expected_ids[0])
            .execute(&state.pool)
            .await
            .unwrap();
        let newest = get_recent_worlds(1, EnumSet::all()).await.unwrap();
        assert_eq!(newest[0].instance_id, expected_ids[0]);
        let stale_instance =
            crate::state::get_instance(&expected_ids[0], &state.pool)
                .await
                .unwrap()
                .unwrap();
        assert!(
            stale_instance.instance.last_played < newest[0].world.last_played
        );

        let hidden_id = &expected_ids[1];
        set_world_display_status(
            hidden_id,
            WorldType::Server,
            "localhost:25565",
            DisplayStatus::Hidden,
        )
        .await
        .unwrap();
        let visible = get_recent_worlds(
            usize::MAX,
            enumset::enum_set!(DisplayStatus::Normal | DisplayStatus::Favorite),
        )
        .await
        .unwrap();
        assert!(!visible.iter().any(|world| {
            &world.instance_id == hidden_id
                && world.world.world_type() == WorldType::Server
        }));
        set_world_display_status(
            hidden_id,
            WorldType::Server,
            "localhost:25565",
            DisplayStatus::Favorite,
        )
        .await
        .unwrap();
        assert!(get_favorite_worlds().await.unwrap().iter().any(|world| {
            &world.instance_id == hidden_id
                && world.world.world_id() == "localhost:25565"
        }));

        // A disconnected installation must not hide worlds from valid ones.
        // Point the record at a missing root instead of moving watched folders.
        let missing_id = expected_ids.remove(0);
        sqlx::query(
            "UPDATE instances SET linked_dot_minecraft = ?, linked_version_json_path = NULL WHERE id = ?",
        )
        .bind(fixture.path().join("disconnected").to_string_lossy().as_ref())
        .bind(&missing_id)
        .execute(&state.pool)
        .await
            .unwrap();
        assert!(get_instance_worlds(&missing_id).await.is_err());
        let favorites = get_favorite_worlds().await.unwrap();
        assert!(
            !favorites
                .iter()
                .any(|world| world.instance_id == missing_id)
        );
        for instance_id in &expected_ids {
            assert!(
                favorites
                    .iter()
                    .any(|world| &world.instance_id == instance_id)
            );
        }
    }

    #[tokio::test]
    async fn home_worlds_use_instance_game_directories() {
        // Keep the singleton's data directory alive for other API tests.
        if !State::initialized() {
            let root = TempDir::new().unwrap().keep();
            State::init_for_test(root.to_string_lossy().into_owned())
                .await
                .unwrap();
        }
        let state = State::get().await.unwrap();
        let fixture = TempDir::new().unwrap();
        let mut created_instances = Vec::new();
        let outcome = std::panic::AssertUnwindSafe(check_home_worlds(
            &state,
            &fixture,
            &mut created_instances,
        ))
        .catch_unwind()
        .await;

        // Restore the deliberately disconnected record, then remove only our
        // fixtures while their directories still exist, including on failure.
        for instance in &created_instances {
            sqlx::query("UPDATE instances SET linked_dot_minecraft = ?, linked_version_json_path = ? WHERE id = ?")
                .bind(&instance.linked_dot_minecraft)
                .bind(&instance.linked_version_json_path)
                .bind(&instance.id)
                .execute(&state.pool)
                .await
                .unwrap();
            crate::state::remove_instance_preserving_external_files(
                &instance.id,
                &state,
            )
            .await
            .unwrap();
            assert!(
                crate::state::get_instance(&instance.id, &state.pool)
                    .await
                    .unwrap()
                    .is_none()
            );
            assert!(
                AttachedWorldData::get_all_for_instance(
                    &instance.id,
                    &state.pool
                )
                .await
                .unwrap()
                .is_empty()
            );
        }
        if let Err(panic) = outcome {
            std::panic::resume_unwind(panic);
        }
    }

    #[test]
    fn reads_hardcore_from_legacy_and_26_1_layouts() {
        let mut legacy = NbtCompound::new();
        legacy.insert("hardcore", NbtTag::Byte(1));
        assert!(read_hardcore(&legacy));

        let mut difficulty_settings = NbtCompound::new();
        difficulty_settings.insert("hardcore", NbtTag::Byte(1));
        let mut modern = NbtCompound::new();
        modern.insert(
            "difficulty_settings",
            NbtTag::Compound(difficulty_settings),
        );
        assert!(read_hardcore(&modern));

        assert!(!read_hardcore(&NbtCompound::new()));
    }
}
