use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "Usage: sync-history-cleanup <launcher-data-root> [--copied-from <original-root>] [--apply] [--limit <generations>]";

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match run().await {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<bool, Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let root = args.next().ok_or(USAGE)?;
    if root == "--help" || root == "-h" {
        println!("{USAGE}");
        println!(
            "Close the launcher first. Preview is the default. Press Ctrl+C to stop, then rerun to continue."
        );
        return Ok(true);
    }
    let mut copied_from = None;
    let mut apply = false;
    let mut limit = None;
    while let Some(flag) = args.next() {
        match flag.to_str() {
            Some("--apply") if !apply => apply = true,
            Some("--copied-from") if copied_from.is_none() => {
                copied_from = Some(PathBuf::from(args.next().ok_or(USAGE)?));
            }
            Some("--limit") if limit.is_none() => {
                limit = Some(
                    args.next()
                        .and_then(|value| value.to_str()?.parse::<usize>().ok())
                        .filter(|value| *value > 0)
                        .ok_or(USAGE)?,
                );
            }
            _ => return Err(USAGE.into()),
        }
    }
    let report = theseus::maintenance::cleanup_sync_history(
        &PathBuf::from(root),
        copied_from.as_deref(),
        apply,
        limit,
        |report| {
            eprintln!(
                "{}",
                serde_json::to_string(report).expect("cleanup counters")
            )
        },
        |token, error| eprintln!("{token}: {error}"),
    )
    .await?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(report.failed == 0)
}
