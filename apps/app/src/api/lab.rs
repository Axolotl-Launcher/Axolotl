//! Lab tools backed by a program the launcher does not bundle.
//!
//! The swamp-hut search replays Minecraft's terrain generation to find the hut
//! that sits lowest for one seed. That is a large C program (`lysh`), not
//! something to reimplement here, so the launcher runs it and reads its output.
//! When it is missing the UI says so rather than pretending to have searched.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Serialize;

/// Writes everything about a failed lab scan to a `.log` file on the user's
/// desktop (falling back to the home directory) and returns the message shown
/// to the UI: the original failure next to the saved file's path. A scan takes
/// minutes and its stderr is the only way to diagnose it afterwards, so a
/// failure the user cannot read is a failure nobody can fix.
///
/// The log is best-effort: when the file cannot be written the original error
/// still returns unchanged, because a missing log must not mask the failure
/// that produced it.
fn scan_log_path<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Option<PathBuf> {
    use tauri::Manager;

    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let name = format!("axolotl-lab-scan-{stamp}.log");
    let fallback = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from));
    let base = app.path().desktop_dir().ok().or(fallback);
    base.map(|base| base.join(name))
}

fn write_scan_log<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    label: &str,
    program: &Path,
    args: &[OsString],
    status: Option<String>,
    stdout: &str,
    stderr: &str,
) -> String {
    let reason = match &status {
        Some(code) => format!("the search program exited with {code}"),
        None => "the search program could not be started".to_string(),
    };
    let quoted = |value: &OsString| {
        let text = value.to_string_lossy().into_owned();
        if text.contains(' ') {
            format!("\"{text}\"")
        } else {
            text
        }
    };
    let body = format!(
        "axolotl lab scan failure log\n\
         time: {}\n\
         tool: {label}\n\
         program: {}\n\
         arguments: {}\n\
         status: {}\n\n\
         --- stderr ---\n{stderr}\n\n\
         --- stdout ---\n{stdout}\n",
        chrono::Local::now().to_rfc3339(),
        program.display(),
        args.iter().map(quoted).collect::<Vec<_>>().join(" "),
        status.as_deref().unwrap_or("(not started)"),
    );
    if let Some(path) = scan_log_path(app) {
        let outcome =
            std::fs::create_dir_all(path.parent().unwrap_or(path.as_path()))
                .and_then(|_| std::fs::write(&path, body.as_bytes()))
                .and_then(|_| path.canonicalize());
        if let Ok(resolved) = outcome {
            return format!(
                "{reason}: the full log was saved to {}",
                resolved.display()
            );
        }
    }
    let detail = stderr.trim();
    if detail.is_empty() {
        reason
    } else {
        format!("{reason}: {detail}")
    }
}

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
/// The numeric seed a world is generated from.
///
/// Both programs take a signed 64-bit seed, while vanilla also accepts text and
/// runs it through Java's `String.hashCode` first. Doing the same here means a
/// word typed into the seed box searches the world the game would have built,
/// instead of failing to parse.
fn world_seed(seed: &str) -> i64 {
    match seed.parse::<i64>() {
        Ok(parsed) => parsed,
        Err(_) => {
            let mut hash: i32 = 0;
            for unit in seed.encode_utf16() {
                hash = hash.wrapping_mul(31).wrapping_add(unit as i32);
            }
            hash as i64
        }
    }
}

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
    let numeric_seed = world_seed(&seed);

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

    let args: Vec<OsString> = command
        .as_std()
        .get_args()
        .map(OsStr::to_os_string)
        .collect();
    // A scan runs for minutes; blocking the runtime thread for that long would
    // stall every other command in the launcher.
    // Windows attaches a console window to console programs; a launcher must not
    // flash one at the user, so every helper here is spawned without it.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.as_std_mut().creation_flags(0x0800_0000);
    }
    let output = command.output().await.map_err(|error| {
        write_scan_log(
            &app,
            "low_y_swamp_hut_scan",
            &program,
            &args,
            None,
            "",
            &error.to_string(),
        )
    })?;
    let elapsed_ms = started.elapsed().as_millis() as u64;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(write_scan_log(
            &app,
            "low_y_swamp_hut_scan",
            &program,
            &args,
            Some(output.status.to_string()),
            &stdout,
            &stderr,
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

/// Locate `java` and, when the launcher already knows which runtime it is, its
/// major version.
///
/// The launcher keeps its own list of runtimes, and someone who installed JDK 25
/// through it never set `JAVA_HOME`; checking that list first is what makes this
/// tool agree with the Java page instead of claiming no runtime is installed.
/// It also means the version is already known, so the caller does not have to
/// start the runtime just to parse `-version` again.
async fn find_java() -> Option<(PathBuf, Option<u32>)> {
    let binary = if cfg!(windows) { "java.exe" } else { "java" };
    if let Ok(versions) = crate::api::jre::get_java_versions().await {
        let mut best: Option<(u32, PathBuf)> = None;
        for version in versions {
            if version.parsed_version < JAVA_MAJOR {
                continue;
            }
            let candidate = PathBuf::from(&version.path);
            if !candidate.is_file() {
                continue;
            }
            if best
                .as_ref()
                .is_none_or(|(major, _)| version.parsed_version > *major)
            {
                best = Some((version.parsed_version, candidate));
            }
        }
        if let Some((major, path)) = best {
            return Some((path, Some(major)));
        }
    }
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        let candidate = PathBuf::from(home).join("bin").join(binary);
        if candidate.is_file() {
            return Some((candidate, None));
        }
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
        .map(|candidate| (candidate, None))
}

/// Whether `java -version` reports [`JAVA_MAJOR`] or newer.
///
/// Probing starts a process and the page asks on every mount, so the answer is
/// remembered per executable: the same JDK keeps the same path, and a stale
/// entry only costs a restart rather than a wrong answer.
async fn java_is_new_enough(java: &Path) -> bool {
    type Probes = std::sync::Mutex<std::collections::HashMap<PathBuf, bool>>;
    static PROBES: std::sync::OnceLock<Probes> = std::sync::OnceLock::new();

    let probes = PROBES.get_or_init(Default::default);
    let known = probes.lock().ok().and_then(|map| map.get(java).copied());
    if let Some(answer) = known {
        return answer;
    }

    let mut command = tokio::process::Command::new(java);
    command.arg("-version");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.as_std_mut().creation_flags(0x0800_0000);
    }
    // `-version` writes to stderr.
    let answer = match command.output().await {
        Ok(output) => {
            let text = String::from_utf8_lossy(&output.stderr);
            text.split('"')
                .nth(1)
                .and_then(|version| version.split('.').next())
                .and_then(|first| first.parse::<u32>().ok())
                .is_some_and(|major| major >= JAVA_MAJOR)
        }
        Err(_) => false,
    };
    if let Ok(mut map) = probes.lock() {
        map.insert(java.to_path_buf(), answer);
    }
    answer
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
pub async fn trial_chamber_available<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> bool {
    // The program directory is a cheap check and its absence decides the answer,
    // so it runs before the runtime probe instead of after it.
    if trial_program_dir(&app).is_none() {
        return false;
    }
    match find_java().await {
        Some((java, known_major)) => match known_major {
            Some(major) => major >= JAVA_MAJOR,
            None => java_is_new_enough(&java).await,
        },
        None => false,
    }
}

#[tauri::command]
pub async fn trial_chamber_find<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    seed: String,
    radius: u32,
    threads: u32,
) -> Result<TrialChamberScan, String> {
    // The program directory decides whether this tool can run here at all and
    // costs one directory lookup, so it is settled before resolving a runtime.
    // Reporting a missing JDK for a build that ships no program sends the user
    // off installing something that was never the problem.
    let program = trial_program_dir(&app).ok_or_else(|| {
        "the trial-chamber program is missing from this build".to_string()
    })?;
    let (java, known_major) = find_java().await.ok_or_else(|| {
        "no Java runtime found; install JDK 25 or let the launcher download one".to_string()
    })?;
    let java_is_usable = match known_major {
        Some(major) => major >= JAVA_MAJOR,
        None => java_is_new_enough(&java).await,
    };
    if !java_is_usable {
        return Err(format!(
            "the installed Java is older than {JAVA_MAJOR}; install JDK {JAVA_MAJOR} or newer"
        ));
    }
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
    let mut java_command = tokio::process::Command::new(&java);
    java_command
        .arg("-cp")
        .arg(&classpath)
        .arg("cn.minecraftfinder.runtime.Main")
        .arg("--seed")
        .arg(world_seed(&seed).to_string())
        .arg("--search-radius-blocks")
        .arg(radius.clamp(128, 1_000_000).to_string())
        .arg("--scan-threads")
        .arg(threads.clamp(1, 64).to_string())
        .arg("--no-progress")
        .current_dir(&program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        java_command.as_std_mut().creation_flags(0x0800_0000);
    }
    let args: Vec<OsString> = java_command
        .as_std()
        .get_args()
        .map(OsStr::to_os_string)
        .collect();
    // The program names its CSV after the start time and leaves older runs in
    // place, so "the newest file" would report a previous search's coordinates
    // when this run writes nothing at all. Remember what already exists and only
    // read a file this run created.
    let existing_results: Vec<PathBuf> = std::fs::read_dir(&program)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name().is_some_and(|name| {
                name.to_string_lossy().starts_with("results-trial-spawner-")
            })
        })
        .collect();
    let output = java_command.output().await.map_err(|error| {
        write_scan_log(
            &app,
            "trial_chamber_find",
            &java,
            &args,
            None,
            "",
            &error.to_string(),
        )
    })?;
    let elapsed_ms = started.elapsed().as_millis() as u64;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(write_scan_log(
            &app,
            "trial_chamber_find",
            &java,
            &args,
            Some(output.status.to_string()),
            &stdout,
            &stderr,
        ));
    }

    // The program writes its CSV next to itself; read this run's file rather than
    // parsing stdout, which also carries progress output.
    let newest = std::fs::read_dir(&program)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name().is_some_and(|name| {
                name.to_string_lossy().starts_with("results-trial-spawner-")
            })
        })
        .filter(|path| !existing_results.contains(path))
        .filter_map(|path| {
            let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
            Some((modified, path))
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

    // The program writes one file per run and never removes the earlier ones, so
    // the directory would grow with every scan and make each later lookup walk a
    // longer list. The run has already been read, so the older copies are spent.
    for path in &existing_results {
        let _ = std::fs::remove_file(path);
    }

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
