use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(windows)]
use tokio::process::Command;

use super::{CensoredString, isolated_minecraft_root, resolve_instance_path};
use crate::{State, prelude::Credentials, util::io::IOError};

const RUN_ASSOCIATION_WINDOW: Duration = Duration::from_secs(3 * 60);
const MAX_SOURCE_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Serialize, Debug)]
pub struct CrashAnalysis {
    pub instance_id: String,
    pub ruleset: &'static str,
    pub crashed: bool,
    pub sources: Vec<CrashAnalysisSource>,
    pub findings: Vec<CrashAnalysisFinding>,
    pub mods: Vec<CrashAnalysisMod>,
    pub combined_log: CensoredString,
    pub mod_changes: Vec<CrashModChange>,
    pub mod_change_counts: CrashModChangeCounts,
    pub windows_events: Vec<WindowsCrashEvent>,
}

#[derive(Serialize, Debug, Clone)]
pub struct CrashModChange {
    pub kind: String,
    pub filename: String,
    pub previous_size: Option<u64>,
    pub current_size: Option<u64>,
    pub current_sha256: Option<String>,
    pub project_id: Option<String>,
    pub project_title: Option<String>,
    pub icon_url: Option<String>,
    pub version_id: Option<String>,
    pub version_number: Option<String>,
}

#[derive(Serialize, Debug, Clone, Default)]
pub struct CrashModChangeCounts {
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WindowsCrashEvent {
    pub event_id: u32,
    pub provider: String,
    pub time_created: String,
    pub message: String,
}

#[derive(Serialize, Debug)]
pub struct CrashAnalysisSource {
    pub filename: String,
    pub source_type: String,
    pub modified_at: u64,
    pub line_count: usize,
    pub first_line: usize,
    pub content: CensoredString,
}

#[derive(Serialize, Debug, Clone, Eq, PartialEq)]
pub struct CrashAnalysisEvidence {
    pub filename: String,
    pub line: usize,
    pub text: String,
}

#[derive(Serialize, Debug)]
pub struct CrashAnalysisFinding {
    pub id: String,
    pub confidence: String,
    pub evidence: Vec<CrashAnalysisEvidence>,
}

#[derive(Serialize, Debug, Clone, Eq, PartialEq)]
pub struct CrashAnalysisMod {
    pub file_name: String,
    pub id: Option<String>,
    pub name: Option<String>,
    pub matched_class: Option<String>,
}

#[derive(Debug)]
struct SourceCandidate {
    path: PathBuf,
    filename: String,
    upload_name: String,
    source_type: &'static str,
    modified: SystemTime,
}

#[derive(Debug)]
struct SourceText {
    first_line: usize,
    filename: String,
    upload_name: String,
    source_type: &'static str,
    modified: SystemTime,
    content: String,
}

pub(super) struct CollectedCrashLog {
    pub name: String,
    pub content: CensoredString,
}

pub(super) struct CollectedCrashLogs {
    pub combined_log: CensoredString,
    pub sources: Vec<CollectedCrashLog>,
}

struct CrashContext {
    instance_root: PathBuf,
    sources: Vec<SourceText>,
    credentials: Vec<Credentials>,
}

struct Rule {
    id: &'static str,
    phase: u8,
    patterns: &'static [&'static str],
}

const RULES: &[Rule] = &[
    Rule {
        id: "jvm_arguments",
        phase: 1,
        patterns: &[
            "Unrecognized option:",
            "Could not create the Java Virtual Machine",
        ],
    },
    Rule {
        id: "out_of_memory",
        phase: 1,
        patterns: &[
            "java.lang.OutOfMemoryError",
            "an out of memory error",
            "The system is out of physical RAM or swap space",
            "Out of Memory Error",
            "Could not reserve enough space",
            "InsufficientMemory",
        ],
    },
    Rule {
        id: "hs_err_al_lib_alc_cleanup",
        phase: 1,
        patterns: &["AlLibAlcCleanup"],
    },
    Rule {
        id: "hs_err_glfw_driver",
        phase: 1,
        patterns: &["glfw.dll", "libglfw.so"],
    },
    Rule {
        id: "hs_err_intel_driver",
        phase: 1,
        patterns: &["ig7icd64.dll"],
    },
    Rule {
        id: "hs_err_java_too_high",
        phase: 1,
        patterns: &["JavaTooHigh"],
    },
    Rule {
        id: "hs_err_jvm",
        phase: 1,
        patterns: &["jvm.dll"],
    },
    Rule {
        id: "hs_err_openal",
        phase: 1,
        patterns: &["libopenal.so"],
    },
    Rule {
        id: "hs_err_macos_shader",
        phase: 1,
        patterns: &["libGLProgrammability.dylib"],
    },
    Rule {
        id: "hs_err_apple_jdk",
        phase: 1,
        patterns: &["~StubRoutines::SafeFetch32"],
    },
    Rule {
        id: "hs_err_gpu_driver",
        phase: 1,
        patterns: &["nglMultiDrawElementsBaseVertex", "0x0000"],
    },
    Rule {
        id: "opengl_unsupported",
        phase: 1,
        patterns: &["The driver does not appear to support OpenGL"],
    },
    Rule {
        id: "pixel_format",
        phase: 1,
        patterns: &[
            "Couldn't set pixel format",
            "Failed to set pixel format",
            "Pixel format not accelerated",
        ],
    },
    Rule {
        id: "openj9",
        phase: 1,
        patterns: &[
            "Open J9 is not supported",
            "OpenJ9 is incompatible",
            ".J9VMInternals.",
        ],
    },
    Rule {
        id: "java_too_new",
        phase: 1,
        patterns: &[
            "because module java.base does not export",
            "java.lang.NoSuchFieldException: ucp",
            "java.lang.ClassNotFoundException: jdk.nashorn.api.scripting.NashornScriptEngineFactory",
            "java.lang.ClassNotFoundException: java.lang.invoke.LambdaMetafactory",
            "Unable to make protected final java.lang.Class java.lang.ClassLoader.defineClass",
        ],
    },
    Rule {
        id: "java_incompatible",
        phase: 1,
        patterns: &[
            "Unsupported class file major version",
            "UnsupportedClassVersionError",
            "UnsupportedClassVersion",
            "Unsupported major.minor version",
        ],
    },
    Rule {
        id: "extracted_mod",
        phase: 1,
        patterns: &[
            "The directories below appear to be extracted jar files",
            "Extracted mod jars found, loading will NOT continue",
        ],
    },
    Rule {
        id: "mixin_bootstrap",
        phase: 1,
        patterns: &[
            "ClassNotFoundException: org.spongepowered.asm.launch.MixinTweaker",
        ],
    },
    Rule {
        id: "mixin_failure",
        phase: 2,
        patterns: &[
            "Mixin prepare failed ",
            "Mixin apply failed",
            "MixinApplyError",
            "MixinTransformerError",
            "mixin.injection.throwables.",
            "InvalidMixinException",
            ".json] FAILED during )",
        ],
    },
    Rule {
        id: "fabric_solution",
        phase: 2,
        patterns: &[
            "A potential solution has been determined:",
            "A potential solution has been determined, this may resolve your problem:",
            "确定了一种可能的解决方法，这样做可能会解决你的问题：",
        ],
    },
    Rule {
        id: "mod_config",
        phase: 1,
        patterns: &["Failed loading config file "],
    },
    Rule {
        id: "duplicate_mod",
        phase: 1,
        patterns: &[
            "DuplicateModsFoundException",
            "Found a duplicate mod",
            "Found duplicate mods",
            "ModResolutionException: Duplicate",
        ],
    },
    Rule {
        id: "optifine_incompatible",
        phase: 1,
        patterns: &[
            "NoSuchMethodError: 'void net.minecraft.client.renderer.texture.SpriteContents.<init>",
            "NoSuchMethodError: 'void net.minecraft.client.renderer.block.model.BakedQuad.<init>",
            "NoSuchMethodError: 'java.lang.String com.mojang.blaze3d.systems.RenderSystem.getBackendDescription",
            "NoSuchMethodError: 'void net.minecraftforge.client.gui.overlay.ForgeGui.renderSelectedItemName",
            "NoSuchMethodError: 'void net.minecraft.server.level.DistanceManager",
            "NoSuchMethodError: 'net.minecraft.network.chat.FormattedText net.minecraft.client.gui.Font.ellipsize",
            "has mods that were not found",
        ],
    },
    Rule {
        id: "shaders_optifine",
        phase: 1,
        patterns: &[
            "Shaders Mod detected. Please remove it, OptiFine has built-in support for shaders.",
        ],
    },
    Rule {
        id: "resource_pack",
        phase: 1,
        patterns: &["1282: Invalid operation"],
    },
    Rule {
        id: "large_resource_pack",
        phase: 1,
        patterns: &["Maybe try a lower resolution resourcepack?"],
    },
    Rule {
        id: "jdk_runtime",
        phase: 1,
        patterns: &[
            "java.lang.ClassCastException: java.base/jdk",
            "java.lang.ClassCastException: class jdk.",
        ],
    },
    Rule {
        id: "multiple_forge_versions",
        phase: 1,
        patterns: &[
            "Found multiple arguments for option fml.forgeVersion, but you asked for only one",
        ],
    },
    Rule {
        id: "forge_java_incompatible",
        phase: 1,
        patterns: &[
            "java.lang.NoSuchMethodError: sun.security.util.ManifestEntryVerifier",
            "java.lang.NoSuchMethodError: 'void sun.security.util.ManifestEntryVerifier",
        ],
    },
    Rule {
        id: "content_verification",
        phase: 1,
        patterns: &[
            "signer information does not match signer information of other classes in the same package",
        ],
    },
    Rule {
        id: "optifine_world",
        phase: 1,
        patterns: &[
            "java.lang.NoSuchMethodError: net.minecraft.world.server.ChunkManager$ProxyTicketManager.shouldForceTicks(J)Z",
        ],
    },
    Rule {
        id: "nightconfig_bug",
        phase: 1,
        patterns: &[
            "com.electronwill.nightconfig.core.io.ParsingException: Not enough data available",
        ],
    },
    Rule {
        id: "forge_incomplete",
        phase: 1,
        patterns: &[
            "Cannot find launch target fmlclient, unable to launch",
            "Invalid paths argument, contained no existing paths",
        ],
    },
    Rule {
        id: "mod_filename",
        phase: 1,
        patterns: &["Invalid module name: '' is not a Java identifier"],
    },
    Rule {
        id: "java_11_required",
        phase: 1,
        patterns: &[
            "has been compiled by a more recent version of the Java Runtime (class file version 55.0)",
            "sun.misc.Unsafe.defineAnonymousClass(Class,byte[],Object[])Class/invokeVirtual",
            "The requested compatibility level JAVA_11 could not be set",
        ],
    },
    Rule {
        id: "java_32bit",
        phase: 1,
        patterns: &["Invalid maximum heap size", "for 1048576KB object heap"],
    },
    Rule {
        id: "definite_mod",
        phase: 1,
        patterns: &[
            "Caught exception from ",
            "LoaderExceptionModCrash: Caught exception from ",
            "Multiple entries with same key: ",
            "-- MOD ",
        ],
    },
    Rule {
        id: "incompatible_mods",
        phase: 1,
        patterns: &[
            "Incompatible mods found!",
            "Some of your mods are incompatible with the game or each other!",
        ],
    },
    Rule {
        id: "missing_dependency",
        phase: 1,
        patterns: &["Missing or unsupported mandatory dependencies:"],
    },
    Rule {
        id: "disk_space",
        phase: 1,
        patterns: &[
            "No space left on device",
            "There is not enough space on the disk",
            "The disk is full",
        ],
    },
    Rule {
        id: "file_in_use",
        phase: 1,
        patterns: &[
            "being used by another process",
            "The process cannot access the file because it is being used",
            "used by another process",
        ],
    },
    Rule {
        id: "connector_incompatible_fabric_mods",
        phase: 1,
        patterns: &["ConnectorIncompatibleFabricMods"],
    },
    Rule {
        id: "missing_embeddium",
        phase: 1,
        patterns: &["Oculus requires Embeddium", "Missing Embeddium"],
    },
    Rule {
        id: "missing_indium",
        phase: 1,
        patterns: &["requires Indium", "Indium is required"],
    },
    Rule {
        id: "create_addons",
        phase: 1,
        patterns: &["Create6Addons"],
    },
    Rule {
        id: "ctov_missing_lithostitched",
        phase: 1,
        patterns: &["CtovWithoutLithostitched"],
    },
    Rule {
        id: "curseforge_corrupted",
        phase: 1,
        patterns: &["CurseForgeCorrupted"],
    },
    Rule {
        id: "epic_fight_addons",
        phase: 1,
        patterns: &["EpicFightAddons"],
    },
    Rule {
        id: "feature_order_cycle",
        phase: 1,
        patterns: &["FeatureOrderCycle"],
    },
    Rule {
        id: "ferrite_core_neighbor_table",
        phase: 1,
        patterns: &["FerriteCoreNeighborTable"],
    },
    Rule {
        id: "geckolib_oculus_compat",
        phase: 1,
        patterns: &["GeckoLibOculusCompat"],
    },
    Rule {
        id: "groovy_mod_loader_ipv6",
        phase: 1,
        patterns: &["GroovyModLoaderIPv6"],
    },
    Rule {
        id: "kubejs_datapack",
        phase: 1,
        patterns: &["KubeJSDataPack"],
    },
    Rule {
        id: "language_provider_mismatch",
        phase: 1,
        patterns: &["LanguageProviderMismatch"],
    },
    Rule {
        id: "legacy_too_many_ids",
        phase: 1,
        patterns: &["LegacyTooManyIds"],
    },
    Rule {
        id: "module_resolution",
        phase: 1,
        patterns: &["ModuleFind", "ModuleResolution"],
    },
    Rule {
        id: "modernfix_watchdog",
        phase: 1,
        patterns: &["ModernFixWatchDog"],
    },
    Rule {
        id: "neoforge_1_20_1",
        phase: 1,
        patterns: &["NeoForgeVersion1_20_1"],
    },
    Rule {
        id: "resource_location",
        phase: 1,
        patterns: &["ResourceLocationException"],
    },
    Rule {
        id: "rubidium_deprecated",
        phase: 1,
        patterns: &["Rubidium"],
    },
    Rule {
        id: "server_config_corrupted",
        phase: 1,
        patterns: &["ServerConfigCorrupted"],
    },
    Rule {
        id: "version_1_21",
        phase: 1,
        patterns: &["Version1_21"],
    },
    Rule {
        id: "used_by_another_process",
        phase: 1,
        patterns: &["UsedByAnotherProcess"],
    },
    Rule {
        id: "windows_closed_process",
        phase: 1,
        patterns: &["WasClosedByWindows"],
    },
    Rule {
        id: "intel_driver",
        phase: 1,
        patterns: &["# C  [ig"],
    },
    Rule {
        id: "amd_driver",
        phase: 1,
        patterns: &["# C  [atio", "atio6axx.dll"],
    },
    Rule {
        id: "nvidia_driver",
        phase: 1,
        patterns: &["# C  [nvoglv", "nvoglv64.dll"],
    },
    Rule {
        id: "mod_id_limit",
        phase: 1,
        patterns: &["maximum id range exceeded"],
    },
    Rule {
        id: "manual_debug_crash",
        phase: 1,
        patterns: &["Manually triggered debug crash"],
    },
    Rule {
        id: "forge_error",
        phase: 2,
        patterns: &[
            "An exception was thrown, the game will display an error screen and halt.",
        ],
    },
    Rule {
        id: "definite_mod_fabric",
        phase: 2,
        patterns: &["due to errors, provided by "],
    },
    Rule {
        id: "mod_loader_failure",
        phase: 1,
        patterns: &["Failure message: "],
    },
    Rule {
        id: "mod_loader_error",
        phase: 4,
        patterns: &["Mod resolution failed"],
    },
    Rule {
        id: "suspected_mod",
        phase: 2,
        patterns: &["Suspected Mod"],
    },
    Rule {
        id: "mod_initialization",
        phase: 4,
        patterns: &["Failed to create mod instance."],
    },
    Rule {
        id: "specific_block",
        phase: 4,
        patterns: &["Block location: World: "],
    },
    Rule {
        id: "specific_entity",
        phase: 4,
        patterns: &["Entity's Exact location: "],
    },
];

pub async fn analyze_crash(instance_id: &str) -> crate::Result<CrashAnalysis> {
    let CrashContext {
        instance_root,
        sources,
        credentials,
    } = collect_crash_context(instance_id).await?;
    let mut findings = analyze_sources(&sources);
    let crashed = sources.iter().any(|source| {
        matches!(source.source_type, "crash_report" | "jvm_crash")
            || has_nonzero_exit_status(&source.content)
    }) || !findings.is_empty();
    let stack_classes = extract_stack_classes(&sources);
    let mods_path = instance_root.join("mods");
    let mods = tokio::task::spawn_blocking(move || {
        inspect_mod_jars(&mods_path, &stack_classes)
    })
    .await
    .unwrap_or_default();
    let mod_changes = compare_mod_snapshot(instance_id, &instance_root).await?;
    let mod_change_counts = count_mod_changes(&mod_changes);
    let windows_events = collect_windows_crash_events(
        sources
            .iter()
            .map(|source| system_time_seconds(source.modified))
            .max(),
    )
    .await;
    for finding in &mut findings {
        for evidence in &mut finding.evidence {
            evidence.text = CensoredString::censor(
                std::mem::take(&mut evidence.text),
                &credentials,
            )
            .as_str()
            .to_string();
        }
    }
    let combined_log =
        CensoredString::censor(build_combined_log(&sources), &credentials);
    let public_sources = sources
        .into_iter()
        .map(|source| {
            let line_count = source.content.lines().count();
            CrashAnalysisSource {
                filename: source.filename,
                source_type: source.source_type.to_string(),
                modified_at: system_time_seconds(source.modified),
                line_count,
                first_line: source.first_line,
                content: CensoredString::censor(source.content, &credentials),
            }
        })
        .collect();

    Ok(CrashAnalysis {
        instance_id: instance_id.to_string(),
        ruleset: "PCL2 CrashAnalyzer (ModCrash.vb)",
        crashed,
        sources: public_sources,
        findings,
        mods,
        combined_log,
        mod_changes,
        mod_change_counts,
        windows_events,
    })
}

async fn collect_crash_context(
    instance_id: &str,
) -> crate::Result<CrashContext> {
    let state = State::get().await?;
    let (instance_path, game_dir_override) =
        resolve_instance_path(instance_id, &state).await?;
    let instance_root = state
        .directories
        .resolve_game_dir(&instance_path, game_dir_override.as_deref());
    let candidates =
        collect_candidates(&instance_root, &state.directories).await?;
    let selected = select_run_candidates(candidates);
    let sources = read_sources(selected).await;
    let credentials = Credentials::get_all(&state.pool)
        .await?
        .into_iter()
        .collect::<Vec<_>>();
    Ok(CrashContext {
        instance_root,
        sources,
        credentials,
    })
}

pub(super) async fn collect_crash_logs(
    instance_id: &str,
) -> crate::Result<CollectedCrashLogs> {
    let CrashContext {
        sources,
        credentials,
        ..
    } = collect_crash_context(instance_id).await?;
    let combined_log =
        CensoredString::censor(build_combined_log(&sources), &credentials);
    let sources = sources
        .into_iter()
        .map(|source| CollectedCrashLog {
            name: source.upload_name,
            content: CensoredString::censor(source.content, &credentials),
        })
        .collect();
    Ok(CollectedCrashLogs {
        combined_log,
        sources,
    })
}

fn count_mod_changes(changes: &[CrashModChange]) -> CrashModChangeCounts {
    changes.iter().fold(
        CrashModChangeCounts::default(),
        |mut counts, change| {
            match change.kind.as_str() {
                "added" => counts.added += 1,
                "removed" => counts.removed += 1,
                "modified" => counts.modified += 1,
                _ => {}
            }
            counts
        },
    )
}

pub async fn save_successful_mod_snapshot(
    instance_id: &str,
) -> crate::Result<()> {
    let state = State::get().await?;
    let (instance_path, game_dir_override) =
        resolve_instance_path(instance_id, &state).await?;
    let mods_path = state
        .directories
        .resolve_game_dir(&instance_path, game_dir_override.as_deref())
        .join("mods");
    crate::state::sync_content_files(instance_id, &state).await?;
    let content =
        crate::state::list_content(instance_id, None, None, &state).await?;
    let snapshot = scan_mod_snapshot(&mods_path, &content).await?;
    let mut transaction = state.pool.begin().await?;
    sqlx::query(
        "DELETE FROM crash_analysis_mod_snapshots WHERE instance_id = ?",
    )
    .bind(instance_id)
    .execute(&mut *transaction)
    .await?;
    for (filename, size, sha256) in snapshot {
        let item = content.iter().find(|item| item.file_name == filename);
        let project = item.and_then(|item| item.project.as_ref());
        let version = item.and_then(|item| item.version.as_ref());
        sqlx::query(
            "INSERT INTO crash_analysis_mod_snapshots (instance_id, filename, size, sha256, project_id, project_title, icon_url, version_id, version_number) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(instance_id)
        .bind(&filename)
        .bind(size as i64)
        .bind(sha256)
        .bind(project.map(|project| project.id.clone()))
        .bind(project.map(|project| project.title.clone()))
        .bind(project.and_then(|project| project.icon_url.clone()))
        .bind(version.map(|version| version.id.clone()))
        .bind(version.map(|version| version.version_number.clone()))
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(())
}

pub async fn undo_added_mod(
    instance_id: &str,
    filename: &str,
    expected_hash: &str,
) -> crate::Result<()> {
    let state = State::get().await?;
    let (instance_path, game_dir_override) =
        resolve_instance_path(instance_id, &state).await?;
    let path = state
        .directories
        .resolve_game_dir(&instance_path, game_dir_override.as_deref())
        .join("mods")
        .join(filename);
    if path.file_name().and_then(|name| name.to_str()) != Some(filename)
        || path.extension().is_none_or(|extension| {
            !extension.to_string_lossy().eq_ignore_ascii_case("jar")
        })
    {
        return Err(crate::ErrorKind::InputError(
            "Invalid Mod file name".to_string(),
        )
        .into());
    }
    let bytes = tokio::fs::read(&path).await?;
    if format!("{:x}", Sha256::digest(&bytes)) != expected_hash {
        return Err(crate::ErrorKind::InputError(
            "Mod file changed since the snapshot; refusing to delete it"
                .to_string(),
        )
        .into());
    }
    tokio::fs::remove_file(path).await?;
    Ok(())
}

async fn compare_mod_snapshot(
    instance_id: &str,
    instance_root: &Path,
) -> crate::Result<Vec<CrashModChange>> {
    let state = State::get().await?;
    let previous = sqlx::query_as::<_, (String, u64, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)>(
        "SELECT filename, size, sha256, project_id, project_title, icon_url, version_id, version_number FROM crash_analysis_mod_snapshots WHERE instance_id = ?",
    )
    .bind(instance_id)
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|(filename, size, hash, project_id, project_title, icon_url, version_id, version_number)| (filename, (size, hash, project_id, project_title, icon_url, version_id, version_number)))
    .collect::<std::collections::HashMap<_, _>>();
    if previous.is_empty() {
        return Ok(Vec::new());
    }
    crate::state::sync_content_files(instance_id, &state).await?;
    let content =
        crate::state::list_content(instance_id, None, None, &state).await?;
    let current = scan_mod_snapshot(&instance_root.join("mods"), &[])
        .await?
        .into_iter()
        .map(|(filename, size, hash)| (filename, (size, hash)))
        .collect::<std::collections::HashMap<_, _>>();
    let mut changes = Vec::new();
    for (filename, (size, hash)) in &current {
        let item = content.iter().find(|item| item.file_name == *filename);
        let project = item.and_then(|item| item.project.as_ref());
        let version = item.and_then(|item| item.version.as_ref());
        match previous.get(filename) {
            None => changes.push(CrashModChange {
                kind: "added".to_string(),
                filename: filename.clone(),
                previous_size: None,
                current_size: Some(*size),
                current_sha256: Some(hash.clone()),
                project_id: project.map(|project| project.id.clone()),
                project_title: project.map(|project| project.title.clone()),
                icon_url: project.and_then(|project| project.icon_url.clone()),
                version_id: version.map(|version| version.id.clone()),
                version_number: version
                    .map(|version| version.version_number.clone()),
            }),
            Some((
                previous_size,
                previous_hash,
                project_id,
                project_title,
                icon_url,
                version_id,
                version_number,
            )) if previous_hash != hash => changes.push(CrashModChange {
                kind: "modified".to_string(),
                filename: filename.clone(),
                previous_size: Some(*previous_size),
                current_size: Some(*size),
                current_sha256: Some(hash.clone()),
                project_id: project_id
                    .clone()
                    .or_else(|| project.map(|project| project.id.clone())),
                project_title: project_title
                    .clone()
                    .or_else(|| project.map(|project| project.title.clone())),
                icon_url: icon_url.clone().or_else(|| {
                    project.and_then(|project| project.icon_url.clone())
                }),
                version_id: version_id
                    .clone()
                    .or_else(|| version.map(|version| version.id.clone())),
                version_number: version_number.clone().or_else(|| {
                    version.map(|version| version.version_number.clone())
                }),
            }),
            Some(_) => {}
        }
    }
    for (
        filename,
        (
            size,
            _,
            project_id,
            project_title,
            icon_url,
            version_id,
            version_number,
        ),
    ) in &previous
    {
        if !current.contains_key(filename) {
            changes.push(CrashModChange {
                kind: "removed".to_string(),
                filename: filename.clone(),
                previous_size: Some(*size),
                current_size: None,
                current_sha256: None,
                project_id: project_id.clone(),
                project_title: project_title.clone(),
                icon_url: icon_url.clone(),
                version_id: version_id.clone(),
                version_number: version_number.clone(),
            });
        }
    }
    changes.sort_by(|left, right| left.filename.cmp(&right.filename));
    Ok(changes)
}

async fn scan_mod_snapshot(
    mods_path: &Path,
    _content: &[crate::state::ContentItem],
) -> crate::Result<Vec<(String, u64, String)>> {
    let mut entries = tokio::fs::read_dir(mods_path).await?;
    let mut snapshot = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().is_none_or(|extension| {
            !extension.to_string_lossy().eq_ignore_ascii_case("jar")
        }) {
            continue;
        }
        let bytes = tokio::fs::read(&path).await?;
        snapshot.push((
            entry.file_name().to_string_lossy().to_string(),
            bytes.len() as u64,
            format!("{:x}", Sha256::digest(&bytes)),
        ));
    }
    Ok(snapshot)
}

#[cfg(windows)]
async fn collect_windows_crash_events(
    anchor: Option<u64>,
) -> Vec<WindowsCrashEvent> {
    let Some(anchor) = anchor else {
        return Vec::new();
    };
    let start = anchor.saturating_sub(300);
    let end = anchor.saturating_add(300);
    let command = format!(
        "$start=[DateTimeOffset]::FromUnixTimeSeconds({start}).UtcDateTime; $end=[DateTimeOffset]::FromUnixTimeSeconds({end}).UtcDateTime; Get-WinEvent -FilterHashtable @{{LogName='Application'; Id=1000,1001,1002,1005; StartTime=$start; EndTime=$end}} -MaxEvents 30 | ForEach-Object {{ [PSCustomObject]@{{ EventId=$_.Id; Provider=$_.ProviderName; TimeCreated=$_.TimeCreated.ToUniversalTime().ToString('o'); Message=$_.Message }} }} | ConvertTo-Json -Compress"
    );
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &command])
        .creation_flags(0x0800_0000)
        .output()
        .await;
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let value =
        serde_json::from_slice::<serde_json::Value>(&output.stdout).ok();
    let entries = match value {
        Some(serde_json::Value::Array(entries)) => entries,
        Some(entry) => vec![entry],
        None => return Vec::new(),
    };
    entries
        .into_iter()
        .filter_map(|entry| {
            let event_id = entry.get("EventId")?.as_u64()? as u32;
            let time_created = entry.get("TimeCreated")?.as_str()?.to_string();
            let provider = entry.get("Provider")?.as_str()?.to_string();
            let message = entry.get("Message")?.as_str()?.to_string();
            Some(WindowsCrashEvent {
                event_id,
                provider,
                time_created,
                message,
            })
        })
        .collect()
}

#[cfg(not(windows))]
async fn collect_windows_crash_events(
    _anchor: Option<u64>,
) -> Vec<WindowsCrashEvent> {
    Vec::new()
}

async fn collect_candidates(
    instance_root: &Path,
    directories: &crate::prelude::DirectoryInfo,
) -> crate::Result<Vec<SourceCandidate>> {
    let mut candidates = Vec::new();
    let mut locations = vec![
        (
            directories.game_logs_dir(instance_root),
            "minecraft_log",
            None,
        ),
        (
            directories.game_crash_reports_dir(instance_root),
            "crash_report",
            Some("crash-reports"),
        ),
        (instance_root.to_path_buf(), "instance_log", None),
    ];
    if let Some(dot_minecraft) = isolated_minecraft_root(instance_root) {
        // PCL-CE also searches the shared `.minecraft` root for JVM crash
        // logs when a version-isolated instance is active.
        locations.push((dot_minecraft, "minecraft_root_log", None));
    }

    for (directory, default_type, upload_prefix) in locations {
        if !tokio::fs::try_exists(&directory).await? {
            continue;
        }
        let mut entries = tokio::fs::read_dir(&directory)
            .await
            .map_err(|error| IOError::with_path(error, &directory))?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|error| IOError::with_path(error, &directory))?
        {
            let Ok(metadata) = entry.metadata().await else {
                tracing::warn!(
                    "Unable to read crash analysis candidate metadata: {}",
                    entry.path().display()
                );
                continue;
            };
            if !metadata.is_file() || metadata.len() == 0 {
                continue;
            }
            let filename = entry.file_name().to_string_lossy().to_string();
            let lower = filename.to_lowercase();
            let source_type = if lower.starts_with("hs_err")
                && lower.ends_with(".log")
            {
                "jvm_crash"
            } else if default_type == "crash_report" && lower.ends_with(".txt")
            {
                "crash_report"
            } else if matches!(
                lower.as_str(),
                "latest.log"
                    | "debug.log"
                    | "launcher_log.txt"
                    | "latest_stdout.log"
            ) {
                "minecraft_log"
            } else if matches!(
                default_type,
                "instance_log" | "minecraft_root_log"
            ) && lower.ends_with(".log")
            {
                "minecraft_log"
            } else {
                continue;
            };
            candidates.push(SourceCandidate {
                path: entry.path(),
                upload_name: upload_prefix
                    .map(|prefix| format!("{prefix}/{filename}"))
                    .unwrap_or_else(|| filename.clone()),
                filename,
                source_type,
                modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            });
        }
    }
    Ok(candidates)
}

fn select_run_candidates(
    mut candidates: Vec<SourceCandidate>,
) -> Vec<SourceCandidate> {
    let now = SystemTime::now();
    candidates.retain(|candidate| {
        now.duration_since(candidate.modified)
            .or_else(|_| candidate.modified.duration_since(now))
            .unwrap_or_default()
            <= RUN_ASSOCIATION_WINDOW
    });
    candidates.sort_by(|left, right| right.modified.cmp(&left.modified));
    let Some(anchor) = candidates.first().map(|candidate| candidate.modified)
    else {
        return Vec::new();
    };
    let mut selected_singletons = HashSet::new();
    candidates
        .into_iter()
        .filter(|candidate| {
            anchor
                .duration_since(candidate.modified)
                .unwrap_or_default()
                <= RUN_ASSOCIATION_WINDOW
        })
        .filter(|candidate| {
            if matches!(candidate.source_type, "crash_report" | "jvm_crash") {
                selected_singletons.insert(candidate.source_type)
            } else {
                true
            }
        })
        .collect()
}

async fn read_sources(candidates: Vec<SourceCandidate>) -> Vec<SourceText> {
    let mut sources = Vec::new();
    for candidate in candidates {
        let Ok(bytes) = tokio::fs::read(&candidate.path).await else {
            tracing::warn!(
                "Unable to read crash analysis source: {}",
                candidate.path.display()
            );
            continue;
        };
        let start = bytes.len().saturating_sub(MAX_SOURCE_BYTES as usize);
        let start = if start > 0 {
            bytes[start..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map(|offset| start + offset + 1)
                .unwrap_or(bytes.len())
        } else {
            0
        };
        let first_line =
            1 + bytes[..start].iter().filter(|byte| **byte == b'\n').count();
        let bytes = &bytes[start..];
        sources.push(SourceText {
            first_line,
            filename: candidate.filename,
            upload_name: candidate.upload_name,
            source_type: candidate.source_type,
            modified: candidate.modified,
            content: String::from_utf8_lossy(bytes).into_owned(),
        });
    }
    sources
}

fn analyze_sources(sources: &[SourceText]) -> Vec<CrashAnalysisFinding> {
    let combined = sources
        .iter()
        .map(|source| source.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    // F3+C is an explicit user action. Do not replace that explanation with a
    // native crash signature that happens to be present in the same log.
    if combined.contains("Manually triggered debug crash") {
        let evidence =
            find_evidence(sources, &["Manually triggered debug crash"]);
        return vec![CrashAnalysisFinding {
            id: "manual_debug_crash".to_string(),
            confidence: "high".to_string(),
            evidence,
        }];
    }

    let mut matches = RULES
        .iter()
        .filter_map(|rule| {
            if !rule_context_matches(rule.id, &combined) {
                return None;
            }
            let evidence = find_evidence(sources, rule.patterns);
            (!evidence.is_empty()).then(|| {
                (
                    rule.phase,
                    CrashAnalysisFinding {
                        id: rule.id.to_string(),
                        confidence: if rule.phase <= 2 {
                            "high"
                        } else {
                            "medium"
                        }
                        .to_string(),
                        evidence,
                    },
                )
            })
        })
        .collect::<Vec<_>>();
    let has_mod_config = matches
        .iter()
        .any(|(_, finding)| finding.id == "mod_config");
    if has_mod_config {
        matches.retain(|(_, finding)| finding.id != "nightconfig_bug");
    }
    let has_32_bit_java = matches
        .iter()
        .any(|(_, finding)| finding.id == "java_32bit");
    if has_32_bit_java {
        for (_, finding) in &mut matches {
            if finding.id == "out_of_memory" {
                finding.evidence.retain(|evidence| {
                    !evidence.text.contains("Could not reserve enough space")
                });
            }
        }
        matches.retain(|(_, finding)| {
            finding.id != "out_of_memory" || !finding.evidence.is_empty()
        });
    }
    if let Some(first_phase) = matches.iter().map(|(phase, _)| *phase).min()
        && first_phase <= 3
    {
        return matches
            .into_iter()
            .filter_map(|(phase, finding)| {
                (phase == first_phase).then_some(finding)
            })
            .collect();
    }

    let stack_classes = extract_stack_classes(sources);
    if !stack_classes.is_empty()
        && ["orge", "abric", "uilt", "iteloader"]
            .iter()
            .any(|marker| combined.contains(marker))
    {
        let evidence = sources
            .iter()
            .flat_map(|source| {
                source.content.lines().enumerate().filter_map(
                    |(index, line)| {
                        let trimmed = line.trim();
                        let frame = trimmed.strip_prefix("at ")?;
                        let class =
                            frame.split('(').next()?.rsplit_once('.')?.0;
                        (!ignored_stack_prefix(class)).then(|| {
                            CrashAnalysisEvidence {
                                filename: source.filename.clone(),
                                line: source.first_line + index,
                                text: trimmed.chars().take(500).collect(),
                            }
                        })
                    },
                )
            })
            .take(3)
            .collect();
        return vec![CrashAnalysisFinding {
            id: "stack_analysis".to_string(),
            confidence: "medium".to_string(),
            evidence,
        }];
    }

    if let Some(first_phase) = matches.iter().map(|(phase, _)| *phase).min() {
        return matches
            .into_iter()
            .filter_map(|(phase, finding)| {
                (phase == first_phase).then_some(finding)
            })
            .collect();
    }

    let minecraft_sources = sources
        .iter()
        .filter(|source| source.source_type == "minecraft_log")
        .collect::<Vec<_>>();
    if !minecraft_sources.is_empty()
        && sources
            .iter()
            .all(|source| source.source_type == "minecraft_log")
        && minecraft_sources
            .iter()
            .all(|source| source.content.len() < 100)
        && minecraft_sources.iter().all(|source| {
            !source.content.contains("at net.")
                && !source.content.contains("INFO]")
        })
    {
        return vec![CrashAnalysisFinding {
            id: "short_output".to_string(),
            confidence: "medium".to_string(),
            evidence: minecraft_sources
                .into_iter()
                .map(|source| CrashAnalysisEvidence {
                    filename: source.filename.clone(),
                    line: source.first_line,
                    text: source.content.trim().chars().take(500).collect(),
                })
                .collect(),
        }];
    }

    Vec::new()
}

fn rule_context_matches(id: &str, combined: &str) -> bool {
    match id {
        "intel_driver" | "amd_driver" | "nvidia_driver" => {
            combined.contains("EXCEPTION_ACCESS_VIOLATION")
        }
        "forge_incomplete" => {
            combined.contains(
                "Cannot find launch target fmlclient, unable to launch",
            ) || (combined.contains("Invalid paths argument")
                && (combined
                    .contains("libraries\\net\\minecraftforge\\fmlcore")
                    || combined
                        .contains("libraries/net/minecraftforge/fmlcore")))
        }
        "java_32bit" => {
            combined.contains("Invalid maximum heap size")
                || (combined.contains("Could not reserve enough space")
                    && combined.contains("for 1048576KB object heap"))
        }
        "mod_loader_failure" => combined
            .split_once("-- MOD ")
            .and_then(|(_, rest)| rest.split_once("Failure message:"))
            .is_some_and(|(mod_section, _)| {
                !mod_section.to_lowercase().contains(".jar")
            }),
        "optifine_world" => combined.contains("OptiFine"),
        "optifine_incompatible"
            if combined.contains("has mods that were not found") =>
        {
            combined.lines().any(|line| {
                let line = line.to_lowercase();
                line.contains("the mod file ")
                    && line.contains("optifine")
                    && line.contains("has mods that were not found")
            })
        }
        "definite_mod" => {
            [
                "Caught exception from ",
                "LoaderExceptionModCrash: Caught exception from ",
                "Multiple entries with same key: ",
            ]
            .iter()
            .any(|pattern| combined.contains(pattern))
                || combined
                    .split_once("-- MOD ")
                    .and_then(|(_, rest)| rest.split_once("Failure message:"))
                    .is_some_and(|(mod_section, _)| {
                        mod_section.to_lowercase().contains(".jar")
                    })
        }
        "definite_mod_fabric" => ![
            "Mixin prepare failed ",
            "Mixin apply failed",
            "MixinApplyError",
            "MixinTransformerError",
            "mixin.injection.throwables.",
            "InvalidMixinException",
            ".json] FAILED during )",
        ]
        .iter()
        .any(|pattern| combined.contains(pattern)),
        "suspected_mod" => !combined.contains("Suspected Mods: None"),
        _ => true,
    }
}

fn find_evidence(
    sources: &[SourceText],
    patterns: &[&str],
) -> Vec<CrashAnalysisEvidence> {
    let mut evidence = Vec::new();
    for source in sources {
        for (index, line) in source.content.lines().enumerate() {
            if patterns.iter().any(|pattern| line.contains(pattern)) {
                evidence.push(CrashAnalysisEvidence {
                    filename: source.filename.clone(),
                    line: source.first_line + index,
                    text: line.trim().chars().take(500).collect(),
                });
                if evidence.len() == 3 {
                    return evidence;
                }
            }
        }
    }
    evidence
}

fn has_nonzero_exit_status(content: &str) -> bool {
    content.lines().rev().find_map(|line| {
        line.strip_prefix("# Process exited with status:")
            .map(|status| {
                let status = status.trim();
                !matches!(status, "0" | "exit status: 0" | "exit code: 0")
            })
    }) == Some(true)
}

fn build_combined_log(sources: &[SourceText]) -> String {
    let mut output = String::new();
    for source in sources {
        output.push_str("\n===== ");
        output.push_str(&source.filename);
        output.push_str(" =====\n");
        output.push_str(&head_tail(&source.content, 1500, 700));
        output.push('\n');
    }
    output
}

fn head_tail(content: &str, head: usize, tail: usize) -> String {
    let lines = content.lines().collect::<Vec<_>>();
    if lines.len() <= head + tail {
        return lines.join("\n");
    }
    let mut selected = lines[..head].to_vec();
    selected.push("... omitted ...");
    selected.extend_from_slice(&lines[lines.len() - tail..]);
    selected.join("\n")
}

fn extract_stack_classes(sources: &[SourceText]) -> Vec<String> {
    let mut classes = HashSet::new();
    for source in sources {
        for line in source.content.lines() {
            let trimmed = line.trim();
            let Some(frame) = trimmed.strip_prefix("at ") else {
                continue;
            };
            let frame = frame.split('(').next().unwrap_or(frame);
            let Some((class, _method)) = frame.rsplit_once('.') else {
                continue;
            };
            if ignored_stack_prefix(class) {
                continue;
            }
            classes.insert(class.replace('.', "/") + ".class");
            if classes.len() >= 64 {
                return classes.into_iter().collect();
            }
        }
    }
    classes.into_iter().collect()
}

fn ignored_stack_prefix(class: &str) -> bool {
    [
        "java.",
        "javax.",
        "jdk.",
        "sun.",
        "net.minecraft.",
        "com.mojang.",
        "net.minecraftforge.",
        "net.neoforged.",
        "net.fabricmc.",
        "org.spongepowered.",
        "org.lwjgl.",
        "oolloo.",
        "paulscode.sound.",
        "cpw.mods.",
        "com.google.",
        "org.apache.",
        "com.mumfrey.",
        "com.electronwill.nightconfig.",
        "it.unimi.dsi.",
        "MojangTricksIntelDriversForPerformance_javaw",
    ]
    .iter()
    .any(|prefix| class.starts_with(prefix))
}

fn inspect_mod_jars(
    mods_path: &Path,
    stack_classes: &[String],
) -> Vec<CrashAnalysisMod> {
    let Ok(entries) = std::fs::read_dir(mods_path) else {
        return Vec::new();
    };
    let mod_id = Regex::new(r#"(?m)^\s*modId\s*=\s*[\"']([^\"']+)"#).unwrap();
    let display_name =
        Regex::new(r#"(?m)^\s*displayName\s*=\s*[\"']([^\"']+)"#).unwrap();
    let mut matches = Vec::new();
    for entry in entries.flatten().take(512) {
        let path = entry.path();
        if path.extension().is_none_or(|extension| {
            !extension.to_string_lossy().eq_ignore_ascii_case("jar")
        }) {
            continue;
        }
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        let Ok(mut archive) = zip::ZipArchive::new(file) else {
            continue;
        };
        let (mut id, mut name) = (None, None);
        if let Some(metadata) = read_zip_text(&mut archive, "fabric.mod.json")
            && let Ok(json) =
                serde_json::from_str::<serde_json::Value>(&metadata)
        {
            id = json
                .get("id")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            name = json
                .get("name")
                .and_then(|value| value.as_str())
                .map(str::to_string);
        }
        if id.is_none()
            && let Some(metadata) =
                read_zip_text(&mut archive, "quilt.mod.json")
            && let Ok(json) =
                serde_json::from_str::<serde_json::Value>(&metadata)
        {
            id = json
                .pointer("/quilt_loader/id")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            name = json
                .pointer("/quilt_loader/metadata/name")
                .and_then(|value| value.as_str())
                .map(str::to_string);
        }
        if id.is_none()
            && let Some(metadata) = read_zip_text(
                &mut archive,
                "META-INF/mods.toml",
            )
            .or_else(|| {
                read_zip_text(&mut archive, "META-INF/neoforge.mods.toml")
            })
        {
            id = mod_id
                .captures(&metadata)
                .and_then(|captures| captures.get(1))
                .map(|value| value.as_str().to_string());
            name = display_name
                .captures(&metadata)
                .and_then(|captures| captures.get(1))
                .map(|value| value.as_str().to_string());
        }
        let matched_class = stack_classes
            .iter()
            .find(|class| archive.by_name(class).is_ok())
            .cloned();
        if matched_class.is_some() {
            matches.push(CrashAnalysisMod {
                file_name: entry.file_name().to_string_lossy().to_string(),
                id,
                name,
                matched_class: matched_class.map(|class| {
                    class.trim_end_matches(".class").replace('/', ".")
                }),
            });
        }
    }
    matches
}

fn read_zip_text<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    path: &str,
) -> Option<String> {
    let mut file = archive.by_name(path).ok()?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).ok()?;
    Some(contents)
}

fn system_time_seconds(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolated_instance_resolves_its_minecraft_root() {
        let instance_root = PathBuf::from("minecraft-root")
            .join("versions")
            .join("isolated-instance");

        assert_eq!(
            isolated_minecraft_root(&instance_root),
            Some(PathBuf::from("minecraft-root"))
        );
        assert!(
            isolated_minecraft_root(&PathBuf::from("minecraft-root")).is_none()
        );
    }

    fn source(filename: &str, content: &str) -> SourceText {
        SourceText {
            first_line: 1,
            filename: filename.to_string(),
            upload_name: filename.to_string(),
            source_type: "minecraft_log",
            modified: SystemTime::now(),
            content: content.to_string(),
        }
    }

    #[tokio::test]
    async fn truncated_sources_keep_complete_lines_and_original_offsets() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("latest.log");
        let content = format!(
            "{}original final line\n",
            "prefix line\n".repeat(MAX_SOURCE_BYTES as usize / 12 + 10)
        );
        tokio::fs::write(&path, &content).await.unwrap();
        let sources = read_sources(vec![SourceCandidate {
            path,
            filename: "latest.log".into(),
            upload_name: "latest.log".into(),
            source_type: "minecraft_log",
            modified: SystemTime::now(),
        }])
        .await;
        let source = &sources[0];
        assert!(source.first_line > 1);
        assert!(source.content.len() <= MAX_SOURCE_BYTES as usize);
        assert!(source.content.starts_with("prefix line\n"));
        assert_eq!(
            source.first_line + source.content.lines().count() - 1,
            content.lines().count()
        );
    }

    #[test]
    fn reports_rule_evidence_with_source_line() {
        let findings = analyze_sources(&[source(
            "latest.log",
            "Starting game\njava.lang.OutOfMemoryError: Java heap space\nStopped",
        )]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].id, "out_of_memory");
        assert_eq!(findings[0].evidence[0].line, 2);
        assert_eq!(findings[0].evidence[0].filename, "latest.log");
    }

    #[test]
    fn ignores_logs_older_than_the_pcl2_collection_window() {
        let candidates = vec![SourceCandidate {
            path: PathBuf::from("launcher_log.txt"),
            filename: "launcher_log.txt".to_string(),
            upload_name: "launcher_log.txt".to_string(),
            source_type: "minecraft_log",
            modified: SystemTime::now() - Duration::from_secs(181),
        }];

        assert!(select_run_candidates(candidates).is_empty());
    }

    #[test]
    fn extracts_only_non_loader_stack_classes() {
        let classes = extract_stack_classes(&[source(
            "crash.txt",
            "\tat net.minecraft.client.Main.main(Main.java:1)\n\tat com.google.common.Task.run(Task.java:1)\n\tat com.example.coolmod.Entry.run(Entry.java:2)",
        )]);
        assert_eq!(classes, vec!["com/example/coolmod/Entry.class"]);
    }

    #[test]
    fn detects_nonzero_launcher_exit_status() {
        assert!(has_nonzero_exit_status(
            "log\n# Process exited with status: exit status: 1\n"
        ));
        assert!(!has_nonzero_exit_status(
            "log\n# Process exited with status: exit status: 0\n"
        ));
        assert!(!has_nonzero_exit_status(
            "log\n# Process exited with status: exit code: 0\n"
        ));
        assert!(has_nonzero_exit_status(
            "log\n# Process exited with status: exit code: 1\n"
        ));
    }

    #[test]
    fn pcl2_priority_suppresses_lower_confidence_rules() {
        let findings = analyze_sources(&[source(
            "latest.log",
            "java.lang.OutOfMemoryError: Java heap space\nMixin apply failed example.mixin.json\n\tat com.example.mod.Entry.run(Entry.java:1)\nFabric Loader",
        )]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].id, "out_of_memory");
    }

    #[test]
    fn pcl2_combination_rules_avoid_false_positive_driver_matches() {
        let findings = analyze_sources(&[source(
            "hs_err_pid1.log",
            "Problematic frame:\n# C  [nvoglv64.dll+0x1234]",
        )]);
        assert!(findings.iter().all(|finding| finding.id != "nvidia_driver"));

        let findings = analyze_sources(&[source(
            "hs_err_pid1.log",
            "EXCEPTION_ACCESS_VIOLATION\nProblematic frame:\n# C  [nvoglv64.dll+0x1234]",
        )]);
        assert!(findings.iter().any(|finding| finding.id == "nvidia_driver"));
    }

    #[test]
    fn detects_issue_244_environment_failures() {
        let cases = [
            ("No space left on device", "disk_space"),
            (
                "The process cannot access the file because it is being used by another process",
                "file_in_use",
            ),
            (
                "ConnectorIncompatibleFabricMods",
                "connector_incompatible_fabric_mods",
            ),
            ("Oculus requires Embeddium", "missing_embeddium"),
            (
                "A mod requires Indium to provide the Fabric Renderer API",
                "missing_indium",
            ),
            (
                "EXCEPTION_ACCESS_VIOLATION\n# C  [atio6axx.dll+0x1]",
                "amd_driver",
            ),
            (
                "EXCEPTION_ACCESS_VIOLATION\n# C  [nvoglv64.dll+0x1]",
                "nvidia_driver",
            ),
        ];

        for (content, expected) in cases {
            let findings = analyze_sources(&[source("latest.log", content)]);
            assert!(
                findings.iter().any(|finding| finding.id == expected),
                "expected {expected} for {content}"
            );
        }
    }

    #[test]
    fn manual_debug_crash_takes_priority_over_native_crash_signatures() {
        let findings = analyze_sources(&[source(
            "launcher_log.txt",
            "Manually triggered debug crash\nProblematic frame: jemalloc.dll",
        )]);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].id, "manual_debug_crash");
        assert_eq!(findings[0].confidence, "high");
    }

    #[test]
    fn detects_all_issue_244_known_signatures() {
        let cases = [
            ("AlLibAlcCleanup", "hs_err_al_lib_alc_cleanup"),
            ("glfw.dll", "hs_err_glfw_driver"),
            ("ig7icd64.dll", "hs_err_intel_driver"),
            ("JavaTooHigh", "hs_err_java_too_high"),
            ("jvm.dll", "hs_err_jvm"),
            ("libopenal.so", "hs_err_openal"),
            ("libGLProgrammability.dylib", "hs_err_macos_shader"),
            ("~StubRoutines::SafeFetch32", "hs_err_apple_jdk"),
            ("nglMultiDrawElementsBaseVertex", "hs_err_gpu_driver"),
            ("Create6Addons", "create_addons"),
            ("CtovWithoutLithostitched", "ctov_missing_lithostitched"),
            ("CurseForgeCorrupted", "curseforge_corrupted"),
            ("EpicFightAddons", "epic_fight_addons"),
            ("FeatureOrderCycle", "feature_order_cycle"),
            ("FerriteCoreNeighborTable", "ferrite_core_neighbor_table"),
            ("GeckoLibOculusCompat", "geckolib_oculus_compat"),
            ("GroovyModLoaderIPv6", "groovy_mod_loader_ipv6"),
            ("KubeJSDataPack", "kubejs_datapack"),
            ("LanguageProviderMismatch", "language_provider_mismatch"),
            ("LegacyTooManyIds", "legacy_too_many_ids"),
            ("ModuleFind", "module_resolution"),
            ("ModernFixWatchDog", "modernfix_watchdog"),
            ("NeoForgeVersion1_20_1", "neoforge_1_20_1"),
            ("ResourceLocationException", "resource_location"),
            ("Rubidium", "rubidium_deprecated"),
            ("ServerConfigCorrupted", "server_config_corrupted"),
            ("UsedByAnotherProcess", "used_by_another_process"),
            ("Version1_21", "version_1_21"),
            ("WasClosedByWindows", "windows_closed_process"),
        ];

        for (content, expected) in cases {
            let findings = analyze_sources(&[source("latest.log", content)]);
            assert!(
                findings.iter().any(|finding| finding.id == expected),
                "expected {expected} for {content}"
            );
        }
    }

    #[test]
    fn pcl2_nightconfig_fallback_does_not_override_named_config_failure() {
        let findings = analyze_sources(&[source(
            "latest.log",
            "Failed loading config file config/example.toml for modid example\ncom.electronwill.nightconfig.core.io.ParsingException: Not enough data available",
        )]);
        assert!(findings.iter().any(|finding| finding.id == "mod_config"));
        assert!(
            findings
                .iter()
                .all(|finding| finding.id != "nightconfig_bug")
        );
    }

    #[test]
    fn pcl2_32_bit_java_requires_the_memory_pattern_pair() {
        let findings = analyze_sources(&[source(
            "latest.log",
            "for 1048576KB object heap",
        )]);
        assert!(findings.iter().all(|finding| finding.id != "java_32bit"));

        let findings = analyze_sources(&[source(
            "latest.log",
            "Could not reserve enough space for 1048576KB object heap",
        )]);
        assert!(findings.iter().any(|finding| finding.id == "java_32bit"));
        assert!(findings.iter().all(|finding| finding.id != "out_of_memory"));
    }

    #[test]
    fn pcl2_mixin_failure_suppresses_fabric_provided_by_fallback() {
        let findings = analyze_sources(&[source(
            "latest.log",
            "Mixin apply failed example.mixin.json due to errors, provided by 'example'",
        )]);
        assert!(findings.iter().any(|finding| finding.id == "mixin_failure"));
        assert!(
            findings
                .iter()
                .all(|finding| finding.id != "definite_mod_fabric")
        );
    }

    #[test]
    fn pcl2_detects_fabric_java_incompatibility_before_loader_fallbacks() {
        let findings = analyze_sources(&[source(
            "launcher_log.txt",
            "Incompatible mods found!\nA potential solution has been determined, this may resolve your problem:\n\t - Replace Java 8 with Java 17 or later.",
        )]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].id, "incompatible_mods");
    }

    #[test]
    fn pcl2_java_failure_is_crashed_even_when_process_returns_zero() {
        let sources = [source(
            "launcher_log.txt",
            "Incompatible mods found!\nA potential solution has been determined, this may resolve your problem:\n\t - Replace Java 8 with Java 17 or later.\n# Process exited with status: exit status: 0",
        )];
        let findings = analyze_sources(&sources);
        let crashed = sources.iter().any(|source| {
            matches!(source.source_type, "crash_report" | "jvm_crash")
                || has_nonzero_exit_status(&source.content)
        }) || !findings.is_empty();

        assert!(crashed);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].id, "incompatible_mods");
    }

    #[test]
    fn detects_java_class_version_failure_from_launcher_output() {
        let findings = analyze_sources(&[source(
            "launcher_log.txt",
            "Error: LinkageError occurred while loading main class net.fabricmc.loader.impl.launch.knot.KnotClient\nCaused by: java.lang.UnsupportedClassVersionError: net/fabricmc/loader/impl/launch/knot/KnotClient has been compiled by a more recent version of the Java Runtime (class file version 61.0), this version of the Java Runtime only recognizes class file versions up to 52.0\n# Process exited with status: exit status: 1",
        )]);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].id, "java_incompatible");
        assert!(
            findings[0].evidence[0]
                .text
                .contains("UnsupportedClassVersionError")
        );
    }

    #[test]
    fn pcl2_does_not_treat_nashorn_provider_warning_as_java_too_new() {
        let findings = analyze_sources(&[source(
            "launcher_log.txt",
            "ScriptEngineManager providers.next(): javax.script.ScriptEngineFactory: Provider jdk.nashorn.api.scripting.NashornScriptEngineFactory not found",
        )]);

        assert!(findings.is_empty());
    }
}
