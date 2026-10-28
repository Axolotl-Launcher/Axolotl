//! Lab tools backed by a program the launcher does not bundle.
//!
//! The swamp-hut search replays Minecraft's terrain generation to find the hut
//! that sits lowest for one seed. That is a large C program (`lysh`), not
//! something to reimplement here, so the launcher runs it and reads its output.
//! When it is missing the UI says so rather than pretending to have searched.

use std::path::PathBuf;
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

/// Name of the external program, without the platform extension.
const PROGRAM: &str = "lysh";

/// Locate the search program on `PATH`.
///
/// Windows also tries `.exe`; the other platforms use the bare name, which is
/// what `PATH` lookup already expects.
fn find_program() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let mut candidates: Vec<String> = vec![PROGRAM.to_string()];
    if cfg!(windows) {
        candidates.push(format!("{PROGRAM}.exe"));
    }
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
pub fn low_y_swamp_hut_available() -> bool {
    find_program().is_some()
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
pub async fn low_y_swamp_hut_scan(
    seed: String,
    regions: u32,
    threads: u32,
    version: Option<String>,
    preset: Option<String>,
    max_y: Option<i32>,
    phase2_max_y: Option<i32>,
) -> Result<SwampHutScan, String> {
    let program = find_program().ok_or_else(|| {
        format!("the `{PROGRAM}` search program is not on PATH")
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

    let mut command = Command::new(&program);
    command
        .arg("scan")
        .arg("--seed")
        .arg(numeric_seed.to_string())
        .arg("--regions")
        .arg(regions.max(1).to_string())
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

    let output = command.output().map_err(|error| {
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

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("lab")
        .invoke_handler(tauri::generate_handler![
            low_y_swamp_hut_available,
            low_y_swamp_hut_scan,
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
