use std::io::Cursor;
use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeZone, Utc};
use either::Either;
use quartz_nbt::{NbtCompound, NbtTag};
use serde::{Deserialize, Serialize};
use url::Url;

use super::{
    SingleplayerGameMode, get_world_dir, get_world_session_lock,
    resolve_instance_data_dir, resolve_instance_identity,
    try_get_world_read_lock,
};
use crate::util::io;
use crate::{Error, ErrorKind, Result, State};

#[derive(Deserialize, Serialize, Debug, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorldDifficulty {
    Peaceful,
    Easy,
    Normal,
    Hard,
}

impl WorldDifficulty {
    fn from_byte(value: i8) -> Self {
        match value {
            0 => Self::Peaceful,
            1 => Self::Easy,
            3 => Self::Hard,
            _ => Self::Normal,
        }
    }

    fn as_byte(self) -> i8 {
        match self {
            Self::Peaceful => 0,
            Self::Easy => 1,
            Self::Normal => 2,
            Self::Hard => 3,
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "peaceful" => Some(Self::Peaceful),
            "easy" => Some(Self::Easy),
            "normal" => Some(Self::Normal),
            "hard" => Some(Self::Hard),
            _ => None,
        }
    }

    fn as_name(self) -> &'static str {
        match self {
            Self::Peaceful => "peaceful",
            Self::Easy => "easy",
            Self::Normal => "normal",
            Self::Hard => "hard",
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GameRuleEntry {
    pub key: String,
    pub value: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct WorldLevelData {
    pub name: String,
    #[serde(
        skip_serializing_if = "Option::is_none",
        with = "either::serde_untagged_optional"
    )]
    pub icon: Option<Either<PathBuf, Url>>,
    pub game_mode: SingleplayerGameMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<WorldDifficulty>,
    pub difficulty_locked: bool,
    pub hardcore: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_commands: Option<bool>,
    /// The world seed as a string, to avoid precision loss for the full
    /// i64 range when crossing the IPC boundary into JavaScript.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<String>,
    pub game_rules: Vec<GameRuleEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_played: Option<DateTime<Utc>>,
    pub modded: bool,
    pub locked: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
#[serde(default)]
pub struct WorldSettingsPatch {
    pub name: Option<String>,
    pub game_mode: Option<SingleplayerGameMode>,
    pub difficulty: Option<WorldDifficulty>,
    pub allow_commands: Option<bool>,
    /// The new world seed, as a base-10 string within the i64 range.
    pub seed: Option<String>,
    /// Values for existing game rules. Keys not present in the world's
    /// game rule storage are ignored rather than added.
    pub game_rules: Option<Vec<GameRuleEntry>>,
}

/// Where each editable aspect of a world is stored. Modern worlds
/// (roughly 26.x and later) split game rules, world generation settings
/// and player data out of level.dat into sibling files, while older
/// worlds keep everything inline. Detection is based purely on what is
/// present in the world's files, never on version numbers, so reads and
/// writes always target the location the data actually came from.
struct WorldDataSources {
    game_rules: Option<GameRulesSource>,
    seed: Option<SeedSource>,
    difficulty: Option<DifficultyFormat>,
    player_game_type: Option<PlayerGameTypeTarget>,
}

enum GameRulesSource {
    LevelDat,
    Savedata(PathBuf),
}

enum SeedSource {
    LevelDatWorldGenSettings,
    LevelDatRandomSeed,
    Savedata(PathBuf),
}

enum DifficultyFormat {
    LegacyByte,
    SettingsCompound,
}

enum PlayerGameTypeTarget {
    LevelDatPlayer,
    PlayerFile(PathBuf),
}

fn savedata_path(world_dir: &Path, file_name: &str) -> PathBuf {
    world_dir.join("data").join("minecraft").join(file_name)
}

async fn existing_savedata_path(
    world_dir: &Path,
    file_name: &str,
) -> Option<PathBuf> {
    let path = savedata_path(world_dir, file_name);
    tokio::fs::try_exists(&path)
        .await
        .unwrap_or(false)
        .then_some(path)
}

async fn probe_world_data_sources(
    world_dir: &Path,
    data: &NbtCompound,
) -> WorldDataSources {
    let game_rules = if data.get::<_, &NbtCompound>("GameRules").is_ok() {
        Some(GameRulesSource::LevelDat)
    } else {
        existing_savedata_path(world_dir, "game_rules.dat")
            .await
            .map(GameRulesSource::Savedata)
    };

    let has_world_gen_settings_seed = data
        .get::<_, &NbtCompound>("WorldGenSettings")
        .ok()
        .and_then(|settings| settings.get::<_, &NbtTag>("seed").ok())
        .and_then(numeric_nbt_value)
        .is_some();
    let seed = if has_world_gen_settings_seed {
        Some(SeedSource::LevelDatWorldGenSettings)
    } else if data
        .get::<_, &NbtTag>("RandomSeed")
        .ok()
        .and_then(numeric_nbt_value)
        .is_some()
    {
        Some(SeedSource::LevelDatRandomSeed)
    } else {
        existing_savedata_path(world_dir, "world_gen_settings.dat")
            .await
            .map(SeedSource::Savedata)
    };

    let difficulty = if data.get::<_, i8>("Difficulty").is_ok() {
        Some(DifficultyFormat::LegacyByte)
    } else if data.get::<_, &NbtCompound>("difficulty_settings").is_ok() {
        Some(DifficultyFormat::SettingsCompound)
    } else {
        None
    };

    let player_game_type = if data.get::<_, &NbtCompound>("Player").is_ok() {
        Some(PlayerGameTypeTarget::LevelDatPlayer)
    } else {
        match data.get::<_, &NbtTag>("singleplayer_uuid") {
            Ok(NbtTag::IntArray(parts)) => {
                uuid_string_from_parts(parts).map(|uuid| {
                    PlayerGameTypeTarget::PlayerFile(
                        world_dir
                            .join("players")
                            .join("data")
                            .join(format!("{uuid}.dat")),
                    )
                })
            }
            _ => None,
        }
    };

    WorldDataSources {
        game_rules,
        seed,
        difficulty,
        player_game_type,
    }
}

fn uuid_string_from_parts(parts: &[i32]) -> Option<String> {
    if parts.len() != 4 {
        return None;
    }
    let hex: String = parts
        .iter()
        .map(|part| format!("{:08x}", *part as u32))
        .collect();
    Some(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

fn game_type_of(mode: SingleplayerGameMode) -> i32 {
    match mode {
        SingleplayerGameMode::Survival => 0,
        SingleplayerGameMode::Creative => 1,
        SingleplayerGameMode::Adventure => 2,
        SingleplayerGameMode::Spectator => 3,
    }
}

fn numeric_nbt_value(tag: &NbtTag) -> Option<i64> {
    match tag {
        NbtTag::Byte(value) => Some(i64::from(*value)),
        NbtTag::Short(value) => Some(i64::from(*value)),
        NbtTag::Int(value) => Some(i64::from(*value)),
        NbtTag::Long(value) => Some(*value),
        _ => None,
    }
}

fn extract_level_seed(data: &NbtCompound) -> Option<i64> {
    if let Ok(settings) = data.get::<_, &NbtCompound>("WorldGenSettings")
        && let Ok(tag) = settings.get::<_, &NbtTag>("seed")
        && let Some(seed) = numeric_nbt_value(tag)
    {
        return Some(seed);
    }
    data.get::<_, &NbtTag>("RandomSeed")
        .ok()
        .and_then(numeric_nbt_value)
}

/// Rewrites every numeric tag named `seed` in the compound, covering the
/// modern `WorldGenSettings.seed` field, the per-dimension generator and
/// biome source seeds used by the 1.16 and 1.17 world formats, and the
/// `data.seed` field of split world_gen_settings.dat files.
fn set_seed_tags_recursively(compound: &mut NbtCompound, seed: i64) {
    for (name, tag) in compound.inner_mut() {
        if name.eq_ignore_ascii_case("seed") && numeric_nbt_value(tag).is_some()
        {
            *tag = NbtTag::Long(seed);
            continue;
        }
        set_seed_in_tag(tag, seed);
    }
}

fn set_seed_in_tag(tag: &mut NbtTag, seed: i64) {
    match tag {
        NbtTag::Compound(child) => set_seed_tags_recursively(child, seed),
        NbtTag::List(list) => {
            for item in list.iter_mut() {
                set_seed_in_tag(item, seed);
            }
        }
        _ => {}
    }
}

/// Renders a stored game rule value as the string form used across the
/// IPC boundary. Legacy worlds store every rule as a string; split
/// game_rules.dat files store native types, where a Byte of 0 or 1 is a
/// boolean and other numeric tags are plain numbers.
fn rule_tag_to_value(tag: &NbtTag) -> Option<String> {
    match tag {
        NbtTag::String(value) => Some(value.clone()),
        NbtTag::Byte(0) => Some("false".to_string()),
        NbtTag::Byte(1) => Some("true".to_string()),
        NbtTag::Byte(value) => Some(value.to_string()),
        NbtTag::Short(value) => Some(value.to_string()),
        NbtTag::Int(value) => Some(value.to_string()),
        NbtTag::Long(value) => Some(value.to_string()),
        _ => None,
    }
}

/// Converts a string value back into the tag type the rule already has
/// in the file, so the file's own schema decides the written type and a
/// value that cannot be represented is rejected instead of guessed at.
fn rule_value_to_tag(existing: &NbtTag, value: &str) -> Result<NbtTag> {
    let invalid = || {
        Error::from(ErrorKind::InputError(format!(
            "Invalid game rule value: {value}"
        )))
    };
    Ok(match existing {
        NbtTag::String(_) => NbtTag::String(value.to_string()),
        NbtTag::Byte(_) => match value {
            "true" => NbtTag::Byte(1),
            "false" => NbtTag::Byte(0),
            other => NbtTag::Byte(other.trim().parse().map_err(|_| invalid())?),
        },
        NbtTag::Short(_) => {
            NbtTag::Short(value.trim().parse().map_err(|_| invalid())?)
        }
        NbtTag::Int(_) => {
            NbtTag::Int(value.trim().parse().map_err(|_| invalid())?)
        }
        NbtTag::Long(_) => {
            NbtTag::Long(value.trim().parse().map_err(|_| invalid())?)
        }
        _ => return Err(invalid()),
    })
}

fn collect_game_rules(rules: &NbtCompound) -> Vec<GameRuleEntry> {
    let mut entries: Vec<GameRuleEntry> = rules
        .inner()
        .iter()
        .filter_map(|(key, tag)| {
            rule_tag_to_value(tag).map(|value| GameRuleEntry {
                key: key.clone(),
                value,
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        a.key.to_ascii_lowercase().cmp(&b.key.to_ascii_lowercase())
    });
    entries
}

fn apply_game_rule_patch(
    target: &mut NbtCompound,
    rules: &[GameRuleEntry],
) -> Result<()> {
    for rule in rules {
        let Ok(existing) = target.get::<_, &NbtTag>(&*rule.key) else {
            continue;
        };
        let new_tag = rule_value_to_tag(existing, &rule.value)?;
        target.insert(rule.key.clone(), new_tag);
    }
    Ok(())
}

fn read_nbt_root(raw: &[u8]) -> Result<(NbtCompound, String)> {
    Ok(quartz_nbt::io::read_nbt(
        &mut Cursor::new(raw),
        quartz_nbt::io::Flavor::GzCompressed,
    )?)
}

fn write_nbt_root(root: &NbtCompound, root_name: &str) -> Result<Vec<u8>> {
    let mut out = vec![];
    quartz_nbt::io::write_nbt(
        &mut out,
        Some(root_name),
        root,
        quartz_nbt::io::Flavor::GzCompressed,
    )?;
    Ok(out)
}

/// Replaces a world file safely: the previous bytes are kept as a
/// sibling `<name>_old` backup (matching the vanilla level.dat_old
/// convention) and the new bytes land via a temp file and atomic rename.
async fn backup_and_replace(
    path: &Path,
    original: &[u8],
    updated: &[u8],
) -> Result<()> {
    let mut backup = path.as_os_str().to_owned();
    backup.push("_old");
    io::write(PathBuf::from(backup), original).await?;

    let mut temp = path.as_os_str().to_owned();
    temp.push(".axolotl-tmp");
    let temp = PathBuf::from(temp);
    io::write(&temp, updated).await?;
    io::rename_or_move(&temp, path).await?;
    Ok(())
}

async fn read_savedata_data_compound(path: &Path) -> Result<NbtCompound> {
    let raw = io::read(path).await?;
    let (mut root, _) = read_nbt_root(&raw)?;
    match root.inner_mut().remove("data") {
        Some(NbtTag::Compound(data)) => Ok(data),
        _ => Err(ErrorKind::InputError(format!(
            "Missing data tag in {}",
            path.display()
        ))
        .into()),
    }
}

/// Applies an in-place mutation to the `data` compound of a savedata
/// file, leaving every other tag (including DataVersion) untouched.
async fn modify_savedata_file<F>(path: &Path, mutate: F) -> Result<()>
where
    F: FnOnce(&mut NbtCompound) -> Result<()>,
{
    let original = io::read(path).await?;
    let (mut root, root_name) = read_nbt_root(&original)?;
    let data = root.get_mut::<_, &mut NbtCompound>("data").map_err(|_| {
        Error::from(ErrorKind::InputError(format!(
            "Missing data tag in {}",
            path.display()
        )))
    })?;
    mutate(data)?;
    let updated = write_nbt_root(&root, &root_name)?;
    backup_and_replace(path, &original, &updated).await
}

async fn update_player_game_type(path: &Path, game_type: i32) -> Result<()> {
    if !tokio::fs::try_exists(path).await.unwrap_or(false) {
        return Ok(());
    }
    let original = io::read(path).await?;
    let (mut root, root_name) = read_nbt_root(&original)?;
    root.insert("playerGameType", NbtTag::Int(game_type));
    let updated = write_nbt_root(&root, &root_name)?;
    backup_and_replace(path, &original, &updated).await
}

pub async fn get_world_level_data(
    instance: &str,
    world: &str,
) -> Result<WorldLevelData> {
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
    let world_dir = get_world_dir(&instance_dir, world)?;

    let session_lock = try_get_world_read_lock(&world_dir).await?;
    let locked = session_lock.is_none();

    let raw = io::read(world_dir.join("level.dat")).await?;
    let (root, _) = read_nbt_root(&raw)?;
    let data = root.get::<_, &NbtCompound>("Data").map_err(|_| {
        Error::from(ErrorKind::InputError(
            "Missing Data tag in level.dat".into(),
        ))
    })?;

    let sources = probe_world_data_sources(&world_dir, data).await;

    let game_rules = match &sources.game_rules {
        Some(GameRulesSource::LevelDat) => data
            .get::<_, &NbtCompound>("GameRules")
            .map(collect_game_rules)
            .unwrap_or_default(),
        Some(GameRulesSource::Savedata(path)) => {
            match read_savedata_data_compound(path).await {
                Ok(rules) => collect_game_rules(&rules),
                Err(e) => {
                    tracing::warn!(
                        "Failed to read game rules from {}: {e}",
                        path.display()
                    );
                    vec![]
                }
            }
        }
        None => vec![],
    };

    let seed = match &sources.seed {
        Some(
            SeedSource::LevelDatWorldGenSettings
            | SeedSource::LevelDatRandomSeed,
        ) => extract_level_seed(data),
        Some(SeedSource::Savedata(path)) => {
            match read_savedata_data_compound(path).await {
                Ok(settings) => settings
                    .get::<_, &NbtTag>("seed")
                    .ok()
                    .and_then(numeric_nbt_value),
                Err(e) => {
                    tracing::warn!(
                        "Failed to read world gen settings from {}: {e}",
                        path.display()
                    );
                    None
                }
            }
        }
        None => None,
    };

    let (difficulty, difficulty_locked, hardcore) = match &sources.difficulty {
        Some(DifficultyFormat::LegacyByte) | None => (
            data.get::<_, i8>("Difficulty")
                .ok()
                .map(WorldDifficulty::from_byte),
            data.get::<_, i8>("DifficultyLocked").unwrap_or(0) != 0,
            data.get::<_, i8>("hardcore").unwrap_or(0) != 0,
        ),
        Some(DifficultyFormat::SettingsCompound) => {
            let settings =
                data.get::<_, &NbtCompound>("difficulty_settings").ok();
            (
                settings
                    .and_then(|s| s.get::<_, &str>("difficulty").ok())
                    .and_then(WorldDifficulty::from_name),
                settings
                    .map(|s| s.get::<_, i8>("locked").unwrap_or(0) != 0)
                    .unwrap_or(false),
                settings
                    .map(|s| s.get::<_, i8>("hardcore").unwrap_or(0) != 0)
                    .unwrap_or(false),
            )
        }
    };

    let game_mode = match data.get::<_, i32>("GameType").unwrap_or(0) {
        1 => SingleplayerGameMode::Creative,
        2 => SingleplayerGameMode::Adventure,
        3 => SingleplayerGameMode::Spectator,
        _ => SingleplayerGameMode::Survival,
    };

    let icon_path = world_dir.join("icon.png");
    let icon = tokio::fs::try_exists(&icon_path)
        .await
        .unwrap_or(false)
        .then_some(Either::Left(icon_path));

    Ok(WorldLevelData {
        name: data
            .get::<_, &str>("LevelName")
            .unwrap_or_default()
            .to_string(),
        icon,
        game_mode,
        difficulty,
        difficulty_locked,
        hardcore,
        allow_commands: data
            .get::<_, i8>("allowCommands")
            .ok()
            .map(|allow| allow != 0),
        seed: seed.map(|seed| seed.to_string()),
        game_rules,
        version_name: data
            .get::<_, &NbtCompound>("Version")
            .ok()
            .and_then(|version| version.get::<_, &str>("Name").ok())
            .map(str::to_string),
        last_played: data
            .get::<_, i64>("LastPlayed")
            .ok()
            .and_then(|millis| Utc.timestamp_millis_opt(millis).single()),
        modded: data.get::<_, i8>("WasModded").unwrap_or(0) != 0,
        locked,
    })
}

pub async fn update_world_settings(
    instance: &Path,
    world: &str,
    patch: WorldSettingsPatch,
) -> Result<()> {
    let world_dir = get_world_dir(instance, world)?;
    let level_dat_path = world_dir.join("level.dat");
    if !level_dat_path.exists() {
        return Err(ErrorKind::InputError(
            "The world does not contain a level.dat file".into(),
        )
        .into());
    }
    let _lock = get_world_session_lock(&world_dir).await?;

    let original = io::read(&level_dat_path).await?;
    let (mut root, root_name) = read_nbt_root(&original)?;
    let sources = {
        let data = root.get::<_, &NbtCompound>("Data").map_err(|_| {
            Error::from(ErrorKind::InputError(
                "Missing Data tag in level.dat".into(),
            ))
        })?;
        probe_world_data_sources(&world_dir, data).await
    };
    let data = root.get_mut::<_, &mut NbtCompound>("Data").map_err(|_| {
        Error::from(ErrorKind::InputError(
            "Missing Data tag in level.dat".into(),
        ))
    })?;

    let mut level_dirty = false;

    if let Some(name) = &patch.name {
        data.insert("LevelName", NbtTag::String(name.trim_ascii().to_string()));
        level_dirty = true;
    }

    if let Some(mode) = patch.game_mode {
        let game_type = game_type_of(mode);
        data.insert("GameType", NbtTag::Int(game_type));
        level_dirty = true;
        match &sources.player_game_type {
            Some(PlayerGameTypeTarget::LevelDatPlayer) => {
                if let Ok(player) =
                    data.get_mut::<_, &mut NbtCompound>("Player")
                {
                    player.insert("playerGameType", NbtTag::Int(game_type));
                }
            }
            Some(PlayerGameTypeTarget::PlayerFile(path)) => {
                update_player_game_type(path, game_type).await?;
            }
            None => {}
        }
    }

    if let Some(difficulty) = patch.difficulty {
        match &sources.difficulty {
            Some(DifficultyFormat::SettingsCompound) => {
                if let Ok(settings) =
                    data.get_mut::<_, &mut NbtCompound>("difficulty_settings")
                {
                    settings.insert(
                        "difficulty",
                        NbtTag::String(difficulty.as_name().to_string()),
                    );
                }
            }
            Some(DifficultyFormat::LegacyByte) | None => {
                data.insert("Difficulty", NbtTag::Byte(difficulty.as_byte()));
            }
        }
        level_dirty = true;
    }

    if let Some(allow_commands) = patch.allow_commands {
        data.insert("allowCommands", NbtTag::Byte(allow_commands as i8));
        level_dirty = true;
    }

    if let Some(seed) = &patch.seed {
        let seed = seed.trim().parse::<i64>().map_err(|_| {
            Error::from(ErrorKind::InputError(
                "The world seed must be a whole number".into(),
            ))
        })?;
        match &sources.seed {
            Some(SeedSource::LevelDatWorldGenSettings) => {
                if let Ok(settings) =
                    data.get_mut::<_, &mut NbtCompound>("WorldGenSettings")
                {
                    set_seed_tags_recursively(settings, seed);
                }
                level_dirty = true;
            }
            Some(SeedSource::LevelDatRandomSeed) => {
                data.insert("RandomSeed", NbtTag::Long(seed));
                level_dirty = true;
            }
            Some(SeedSource::Savedata(path)) => {
                modify_savedata_file(path, |settings| {
                    set_seed_tags_recursively(settings, seed);
                    Ok(())
                })
                .await?;
            }
            None => {
                return Err(ErrorKind::InputError(
                    "The world does not store an editable seed".into(),
                )
                .into());
            }
        }
    }

    if let Some(rules) = &patch.game_rules
        && !rules.is_empty()
    {
        match &sources.game_rules {
            Some(GameRulesSource::LevelDat) => {
                if let Ok(game_rules) =
                    data.get_mut::<_, &mut NbtCompound>("GameRules")
                {
                    apply_game_rule_patch(game_rules, rules)?;
                    level_dirty = true;
                }
            }
            Some(GameRulesSource::Savedata(path)) => {
                modify_savedata_file(path, |data| {
                    apply_game_rule_patch(data, rules)
                })
                .await?;
            }
            None => {
                return Err(ErrorKind::InputError(
                    "The world does not store editable game rules".into(),
                )
                .into());
            }
        }
    }

    if level_dirty {
        let updated = write_nbt_root(&root, &root_name)?;
        backup_and_replace(&level_dat_path, &original, &updated).await?;
    }
    Ok(())
}
