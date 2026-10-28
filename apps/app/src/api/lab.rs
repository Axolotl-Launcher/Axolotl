//! Lab tools backed by a program the launcher does not bundle.
//!
//! The swamp-hut search replays Minecraft's terrain generation to find the hut
//! that sits lowest for one seed. That is a large C program (`lysh`), not
//! something to reimplement here, so the launcher runs it and reads its output.
//! When it is missing the UI says so rather than pretending to have searched.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use serde::Serialize;

/// One hut the search accepted.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwampHutHit {
    pub x: i32,
    pub z: i32,
    /// Footprint average height; this is the Y the tool is named for.
    pub y: f64,
    pub flooded: bool,
    /// Distance from the world origin, in blocks.
    pub distance: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwampHutScan {
    pub seed: String,
    pub regions: u32,
    pub hits: Vec<SwampHutHit>,
    pub elapsed_ms: u64,
    /// Lowest `y` among the hits, which is the answer the tool exists for.
    pub lowest_y: Option<f64>,
}

/// One trial chamber the search accepted, with what it contains.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrialChamberHit {
    pub rank: u32,
    pub center_x: i32,
    pub center_z: i32,
    pub chamber_count: u32,
    pub spawner_count: u32,
    /// Chamber block coordinates, `(x, z)`.
    pub positions: Vec<(i32, i32)>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrialChamberScan {
    pub seed: String,
    pub radius: u32,
    pub hits: Vec<TrialChamberHit>,
    pub elapsed_ms: u64,
    /// Highest spawner count among the hits.
    pub best_spawners: Option<u32>,
}

/// Name of the external program, without the platform extension.
const PROGRAM: &str = "lysh";

/// Locate the search program.
///
/// The launcher builds its own copy from the vendored submodule and ships it as
/// an application resource, so that is checked first; `PATH` remains a fallback
/// for someone who built the CLI themselves.
fn find_program<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Option<PathBuf> {
    use tauri::Manager;

    let mut candidates: Vec<String> = vec![PROGRAM.to_string()];
    if cfg!(windows) {
        candidates.push(format!("{PROGRAM}.exe"));
    }

    if let Ok(resources) = app.path().resource_dir() {
        for root in [
            resources.join("resources").join(PROGRAM),
            resources.join(PROGRAM),
        ] {
            for name in &candidates {
                let candidate = root.join(name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for name in &candidates {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[tauri::command]
pub fn low_y_swamp_hut_available<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> bool {
    find_program(&app).is_some()
}

/// Parse one `--list` line: `hit <rx> <rz> <x> <y> <z> <flooded>`.
///
/// The README calls this the only format the CLI emits (the older four-column
/// form was retired), so a line that does not match is skipped rather than
/// guessed at.
fn parse_hit_line(line: &str) -> Option<SwampHutHit> {
    let mut parts = line.split_whitespace();
    if parts.next()? != "hit" {
        return None;
    }
    let _rx: i32 = parts.next()?.parse().ok()?;
    let _rz: i32 = parts.next()?.parse().ok()?;
    let x: i32 = parts.next()?.parse().ok()?;
    let y: f64 = parts.next()?.parse().ok()?;
    let z: i32 = parts.next()?.parse().ok()?;
    let flooded = parts.next().map(|v| v == "1").unwrap_or(false);
    Some(SwampHutHit {
        x,
        z,
        y,
        flooded,
        distance: ((x as f64).powi(2) + (z as f64).powi(2)).sqrt(),
    })
}

#[tauri::command]
pub async fn low_y_swamp_hut_scan<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    seed: String,
    regions: u32,
    threads: u32,
    version: Option<String>,
    preset: Option<String>,
    max_y: Option<i32>,
    phase2_max_y: Option<i32>,
) -> Result<SwampHutScan, String> {
    let program = find_program(&app).ok_or_else(|| {
        format!("the `{PROGRAM}` search program is missing from this build")
    })?;

    let seed = seed.trim().to_string();
    if seed.is_empty() {
        return Err("a world seed is required".to_string());
    }

    let started = Instant::now();
    // The program takes a signed 64-bit seed. A textual seed is hashed the same
    // way vanilla does before it reaches the terrain generator, which the
    // program cannot do itself, so reject rather than silently scanning 0.
    let numeric_seed: i64 = seed
        .parse()
        .map_err(|_| "the seed must be a whole number; textual seeds are not supported yet".to_string())?;

    // The limits the UI offers are advisory: this command is reachable over IPC
    // directly, and the scan is CPU-bound for as long as it runs, so clamp here
    // rather than trusting the caller.
    let regions = regions.clamp(1, 4096);
    let threads = threads.min(64);

    let mut command = tokio::process::Command::new(&program);
    command
        .arg("scan")
        .arg("--seed")
        .arg(numeric_seed.to_string())
        .arg("--regions")
        .arg(regions.to_string())
        .arg("--list")
        .arg("--quiet");
    if threads > 0 {
        command.arg("--threads").arg(threads.to_string());
    }
    // The CLI validates these itself; passing an unknown value through keeps the
    // error message next to the argument the user chose.
    if let Some(version) =
        version.as_deref().map(str::trim).filter(|v| !v.is_empty())
    {
        command.arg("--version").arg(version);
    }
    if let Some(preset) =
        preset.as_deref().map(str::trim).filter(|v| !v.is_empty())
    {
        command.arg("--preset").arg(preset);
    }
    if let Some(max_y) = max_y {
        command.arg("--max-y").arg(max_y.to_string());
    }
    if let Some(phase2) = phase2_max_y {
        command.arg("--phase2-max-y").arg(phase2.to_string());
    }

    // A scan runs for minutes; blocking the runtime thread for that long would
    // stall every other command in the launcher.
    let output = command.output().await.map_err(|error| {
        format!("cannot run {}: {error}", program.display())
    })?;
    let elapsed_ms = started.elapsed().as_millis() as u64;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "the search program exited with {}: {}",
            output.status,
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut hits: Vec<SwampHutHit> =
        stdout.lines().filter_map(parse_hit_line).collect();
    hits.sort_by(|a, b| a.y.total_cmp(&b.y));
    let lowest_y = hits.first().map(|hit| hit.y);

    Ok(SwampHutScan {
        seed,
        regions,
        hits,
        elapsed_ms,
        lowest_y,
    })
}

/// Minimum Java major version the trial-chamber program needs.
const JAVA_MAJOR: u32 = 25;

/// Locate `java`: `JAVA_HOME` first, then `PATH`.
fn find_java() -> Option<PathBuf> {
    let binary = if cfg!(windows) { "java.exe" } else { "java" };
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        let candidate = PathBuf::from(home).join("bin").join(binary);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}

/// Whether `java -version` reports [`JAVA_MAJOR`] or newer.
fn java_is_new_enough(java: &Path) -> bool {
    let output = match Command::new(java).arg("-version").output() {
        Ok(output) => output,
        Err(_) => return false,
    };
    // `-version` writes to stderr.
    let text = String::from_utf8_lossy(&output.stderr);
    let major = text
        .split('"')
        .nth(1)
        .and_then(|version| version.split('.').next())
        .and_then(|first| first.parse::<u32>().ok());
    major.map(|value| value >= JAVA_MAJOR).unwrap_or(false)
}

/// Directory holding the trial-chamber program, if it is installed.
///
/// The jars are built from the `third-party/trial-spawner-finder` submodule at
/// build time and shipped as application resources, so they live beside the
/// executable rather than in the data directory.
fn trial_program_dir<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Option<PathBuf> {
    use tauri::Manager;
    let resources = app.path().resource_dir().ok()?;
    let candidates = [
        resources.join("resources").join("trial-spawner-finder"),
        resources.join("trial-spawner-finder"),
    ];
    candidates.into_iter().find(|dir| dir.is_dir())
}

/// Parse the program's CSV.
///
/// Semicolon-separated, `#` comment lines first, then a header row. Columns are:
/// rank, center X, center Z, chamber count, spawner count, positions
/// (`x,z|x,z`).
fn parse_trial_csv(text: &str) -> Vec<TrialChamberHit> {
    let mut hits = Vec::new();
    let mut seen_header = false;
    for raw in text.lines() {
        let line = raw.trim_start_matches('\u{feff}').trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if !seen_header {
            seen_header = true;
            continue;
        }
        let mut parts = line.split(';');
        let rank = parts.next().and_then(|v| v.trim().parse::<u32>().ok());
        let center_x = parts.next().and_then(|v| v.trim().parse::<i32>().ok());
        let center_z = parts.next().and_then(|v| v.trim().parse::<i32>().ok());
        let (Some(rank), Some(center_x), Some(center_z)) =
            (rank, center_x, center_z)
        else {
            continue;
        };
        let chamber_count = parts
            .next()
            .and_then(|v| v.trim().parse::<u32>().ok())
            .unwrap_or(0);
        let spawner_count = parts
            .next()
            .and_then(|v| v.trim().parse::<u32>().ok())
            .unwrap_or(0);
        let positions = parts
            .next()
            .map(|raw| {
                raw.split('|')
                    .filter_map(|pair| {
                        let mut coords = pair.split(',');
                        let x = coords.next()?.trim().parse::<i32>().ok()?;
                        let z = coords.next()?.trim().parse::<i32>().ok()?;
                        Some((x, z))
                    })
                    .collect()
            })
            .unwrap_or_default();
        hits.push(TrialChamberHit {
            rank,
            center_x,
            center_z,
            chamber_count,
            spawner_count,
            positions,
        });
    }
    hits
}

#[tauri::command]
pub fn trial_chamber_available<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> bool {
    let has_java = find_java()
        .map(|java| java_is_new_enough(&java))
        .unwrap_or(false);
    has_java && trial_program_dir(&app).is_some()
}

#[tauri::command]
pub async fn trial_chamber_find<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    seed: String,
    radius: u32,
    threads: u32,
) -> Result<TrialChamberScan, String> {
    let java = find_java().ok_or_else(|| {
        "no Java runtime found; install JDK 25 or let the launcher download one".to_string()
    })?;
    if !java_is_new_enough(&java) {
        return Err(format!(
            "the installed Java is older than {JAVA_MAJOR}; install JDK {JAVA_MAJOR} or newer"
        ));
    }
    let program = trial_program_dir(&app).ok_or_else(|| {
        "the trial-chamber program is missing from this build".to_string()
    })?;
    // The program ships as several jars (engine, runtime, CLI) and only the
    // runtime jar carries a main class, so it is launched through the classpath
    // rather than `-jar`.
    let mut jars: Vec<PathBuf> = std::fs::read_dir(&program)
        .map_err(|error| format!("cannot read {}: {error}", program.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().and_then(|value| value.to_str()) == Some("jar")
        })
        .collect();
    jars.sort();
    if jars.is_empty() {
        return Err(format!("no jars found in {}", program.display()));
    }
    let separator = if cfg!(windows) { ";" } else { ":" };
    let classpath = jars
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(separator);

    let seed = seed.trim().to_string();
    if seed.is_empty() {
        return Err("a world seed is required".to_string());
    }

    let started = Instant::now();
    let output = tokio::process::Command::new(&java)
        .arg("-cp")
        .arg(&classpath)
        .arg("cn.minecraftfinder.runtime.Main")
        .arg("--seed")
        .arg(&seed)
        .arg("--search-radius-blocks")
        .arg(radius.clamp(128, 1_000_000).to_string())
        .arg("--scan-threads")
        .arg(threads.clamp(1, 64).to_string())
        .arg("--no-progress")
        .current_dir(&program)
        .output()
        .await
        .map_err(|error| format!("cannot run the search program: {error}"))?;
    let elapsed_ms = started.elapsed().as_millis() as u64;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "the search program exited with {}: {}",
            output.status,
            stderr.trim()
        ));
    }

    // The program writes its CSV next to itself; read the newest one rather than
    // parsing stdout, which also carries progress output.
    let newest = std::fs::read_dir(&program)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("results-trial-spawner-")
        })
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, entry.path()))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path);

    let hits = match newest {
        Some(path) => {
            let text = std::fs::read_to_string(&path).map_err(|error| {
                format!("cannot read {}: {error}", path.display())
            })?;
            parse_trial_csv(&text)
        }
        None => Vec::new(),
    };
    let best_spawners = hits.iter().map(|hit| hit.spawner_count).max();

    Ok(TrialChamberScan {
        seed,
        radius,
        hits,
        elapsed_ms,
        best_spawners,
    })
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("lab")
        .invoke_handler(tauri::generate_handler![
            low_y_swamp_hut_available,
            low_y_swamp_hut_scan,
            trial_chamber_available,
            trial_chamber_find,
        ])
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_hit_line() {
        let hit = parse_hit_line("hit 3 -2 96 41 -64 0").expect("line parses");
        assert_eq!(hit.x, 96);
        assert_eq!(hit.z, -64);
        assert!((hit.y - 41.0).abs() < f64::EPSILON);
        assert!(!hit.flooded);
        assert!(
            (hit.distance - ((96f64).powi(2) + 64f64.powi(2)).sqrt()).abs()
                < 1e-9
        );
    }

    #[test]
    fn ignores_lines_that_are_not_hits() {
        assert!(parse_hit_line("scanning region 1/8").is_none());
        assert!(parse_hit_line("hit 1 2 3").is_none());
        assert!(parse_hit_line("").is_none());
    }

    #[test]
    fn marks_flooded_candidates() {
        let hit = parse_hit_line("hit 0 0 10 62 10 1").expect("line parses");
        assert!(hit.flooded);
    }
}
