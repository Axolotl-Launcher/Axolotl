//! Runtime verification and safe fallback for JVM (GC) arguments.
//!
//! The frontend picks the *preferred* GC strategy from machine heuristics and
//! sends an ordered list of candidate argument blocks. This module verifies
//! each candidate against the *actual* JVM binary that will launch Minecraft,
//! prunes only the unsupported *tuning* flags (never the GC selector, which
//! defines the strategy), and falls back down the candidate chain — ending at
//! launching with no GC arguments at all rather than refusing to start.
use crate::util::java_environment;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::time::timeout;
use tracing::warn;

/// Marker token the frontend persists to mean "auto-select GC".
pub const AUTO_GC_PRESET_ARG: &str = "@axolotl:gc:auto";

/// An `-XX:` flag whose key starts with `+Use` is a collector selector
/// (`UseG1GC`, `UseShenandoahGC`, `UseZGC`). Dropping it silently changes the
/// collector, so we never prune it — an unsupported selector means the whole
/// strategy is unusable on this JVM.
fn is_gc_selector(arg: &str) -> bool {
    arg.starts_with("-XX:+Use")
}

/// Frontend → backend intent describing which GC preset is active and the
/// ordered candidates to try.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GcLaunchIntent {
    /// Frontend preset id (`gc-auto`, `gc-g1gc-mojang`, `gc-g1gc-pcl`,
    /// `gc-shenandoah` or `gc-zgc`).
    pub active_preset_id: String,
    /// Exact tokens currently present in the effective java args that belong
    /// to this preset — or just the `@axolotl:gc:auto` marker for auto.
    pub block_tokens: Vec<String>,
    /// Strategy id for each candidate (`zgc`, `shenandoah`, `g1gc-mojang`,
    /// `g1gc-pcl`, `minimal-g1`). Parallel to `candidates`, `[0]` preferred.
    pub candidate_ids: Vec<String>,
    /// Parallel to `candidate_ids`: the ordered candidate argument blocks.
    pub candidates: Vec<Vec<String>>,
}

impl GcLaunchIntent {
    pub fn preferred_id(&self) -> String {
        self.candidate_ids
            .first()
            .cloned()
            .unwrap_or_else(|| self.active_preset_id.clone())
    }
}

/// Result of verification/fallback, returned to the frontend so it can tell
/// the user what the JVM actually accepted.
#[derive(Clone, Debug, Serialize)]
pub struct GcLaunchReport {
    pub preferred_strategy: String,
    pub chosen_strategy: String,
    pub chosen_args: Vec<String>,
    pub pruned_args: Vec<String>,
    pub reason_chain: Vec<String>,
}

impl GcLaunchReport {
    /// Whether the launch deviated from the preferred resolution (strategy
    /// fallback or flag-level pruning).
    pub fn fell_back(&self) -> bool {
        self.chosen_strategy != self.preferred_strategy
            || !self.pruned_args.is_empty()
    }
}

struct ProbeOutcome {
    supported: bool,
    stderr_text: String,
}

/// Whether a particular (java, args) set already accepted/succeeded.
fn probe_cache() -> &'static Mutex<HashMap<(String, Vec<String>), bool>> {
    static CACHE: OnceLock<Mutex<HashMap<(String, Vec<String>), bool>>> =
        OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Run `java <args> -version` and report whether the JVM accepted `args`.
/// A supported flag set exits 0; an unrecognized option exits non-zero with
/// an explanatory message on stderr.
async fn probe_jvm_arguments(java: &Path, args: &[String]) -> ProbeOutcome {
    let key = (java.to_string_lossy().into_owned(), args.to_vec());
    if let Some(supported) = probe_cache().lock().unwrap().get(&key).copied() {
        return ProbeOutcome {
            supported,
            stderr_text: String::new(),
        };
    }

    let timed = timeout(
        Duration::from_secs(5),
        java_environment::tokio_command(java)
            .args(args)
            .arg("-version")
            .kill_on_drop(true)
            .output(),
    )
    .await;

    let outcome = match timed {
        Ok(Ok(output)) => {
            let mut text = String::from_utf8_lossy(&output.stderr).into_owned();
            if !output.stdout.is_empty() {
                text.push_str(&String::from_utf8_lossy(&output.stdout));
            }
            ProbeOutcome {
                supported: output.status.success(),
                stderr_text: text,
            }
        }
        // Spawn failure or timeout: treat as unsupported and move on.
        Ok(Err(_)) | Err(_) => ProbeOutcome {
            supported: false,
            stderr_text: String::new(),
        },
    };

    probe_cache().lock().unwrap().insert(key, outcome.supported);
    outcome
}

/// Normalize an option token to a comparable key, e.g.
/// `-XX:+UseZGC` → `UseZGC`, `G1UncommitBias=1` → `G1UncommitBias`,
/// `-XX:G1HeapRegionSize=32M` → `G1HeapRegionSize`.
fn normalize_option_key(arg: &str) -> String {
    let trimmed = arg.trim_start_matches('-');
    let trimmed = trimmed.strip_prefix("XX:").unwrap_or(trimmed);
    let trimmed = trimmed.trim_start_matches(['+', '-']);
    trimmed.split('=').next().unwrap_or(trimmed).to_string()
}

/// Scan JVM stderr for the quoted option that was rejected, and return its
/// index inside `args` when recognizable. Heuristic across HotSpot and OpenJ9
/// message formats; `None` means we can't tell, so the caller treats the whole
/// candidate as unsupported.
fn find_offending_arg(text: &str, args: &[String]) -> Option<usize> {
    let quote_re = Regex::new(r"'([^']+)'").ok()?;
    for captures in quote_re.captures_iter(text) {
        let quoted = captures[1].trim();
        if quoted.len() < 2 {
            continue;
        }
        let key = normalize_option_key(quoted);
        if key.is_empty() {
            continue;
        }
        // Only consider quoted text that actually resembles a VM option.
        let looks_like_option = quoted.contains("XX:")
            || quoted.starts_with("-XX:")
            || args.iter().any(|arg| normalize_option_key(arg) == key);
        if !looks_like_option {
            continue;
        }
        if let Some(idx) =
            args.iter().position(|arg| normalize_option_key(arg) == key)
        {
            return Some(idx);
        }
    }
    None
}

const MAX_PRUNE_ROUNDS_PER_CANDIDATE: usize = 8;
const MAX_PROBES_PER_LAUNCH: usize = 14;

/// A probe that checks whether a JVM accepts a set of arguments. Abstracted so
/// the selection logic is unit-testable without spawning a real JVM.
trait JvmProbe {
    fn probe<'a>(
        &'a mut self,
        args: &'a [String],
    ) -> Pin<Box<dyn Future<Output = ProbeOutcome> + Send + 'a>>;
}

/// Probe against a concrete Java binary.
struct RealJvmProbe<'a> {
    java: &'a Path,
}

impl JvmProbe for RealJvmProbe<'_> {
    fn probe<'a>(
        &'a mut self,
        args: &'a [String],
    ) -> Pin<Box<dyn Future<Output = ProbeOutcome> + Send + 'a>> {
        Box::pin(probe_jvm_arguments(self.java, args))
    }
}

/// Try each candidate in order against a probe. Pure logic so it is
/// unit-testable without spawning a real JVM.
async fn select_best_candidate_with_probe<P>(
    intent: &GcLaunchIntent,
    probe: &mut P,
) -> (Vec<String>, GcLaunchReport)
where
    P: JvmProbe + ?Sized,
{
    let mut report = GcLaunchReport {
        preferred_strategy: intent.preferred_id(),
        chosen_strategy: String::new(),
        chosen_args: Vec::new(),
        pruned_args: Vec::new(),
        reason_chain: Vec::new(),
    };
    let mut probes = 0usize;
    let candidate_count =
        intent.candidates.len().min(intent.candidate_ids.len());

    for index in 0..candidate_count {
        let candidate_id = &intent.candidate_ids[index];
        let candidate = &intent.candidates[index];
        if index > 0 {
            report
                .reason_chain
                .push(format!("{candidate_id} is the fallback candidate"));
        }

        let mut current = candidate.clone();
        let mut pruned: Vec<String> = Vec::new();
        let mut rounds = 0usize;

        loop {
            if probes >= MAX_PROBES_PER_LAUNCH {
                report.reason_chain.push(
                    "JVM probe budget exhausted; using JVM default GC"
                        .to_string(),
                );
                return (Vec::new(), report);
            }
            probes += 1;

            let outcome = probe.probe(&current).await;
            if outcome.supported {
                report.chosen_strategy = candidate_id.clone();
                report.chosen_args = current;
                report.pruned_args = pruned;
                if !report.pruned_args.is_empty() {
                    report.reason_chain.push(format!(
                        "pruned {} unsupported argument(s)",
                        report.pruned_args.len()
                    ));
                }
                return (report.chosen_args.clone(), report);
            }

            if rounds >= MAX_PRUNE_ROUNDS_PER_CANDIDATE {
                break;
            }
            rounds += 1;

            match find_offending_arg(&outcome.stderr_text, &current) {
                Some(idx) if !is_gc_selector(&current[idx]) => {
                    pruned.push(current.remove(idx));
                }
                // Unsupported selector (or unrecognizable error): the whole
                // strategy is unusable on this JVM.
                _ => break,
            }
        }

        report
            .reason_chain
            .push(format!("{candidate_id} is not supported by this JVM"));
    }

    // Every candidate failed. Launch with no GC arguments and let the JVM
    // choose its default collector — never block the game over GC tuning.
    report.reason_chain.push(
        "no GC strategy is supported; falling back to JVM default GC"
            .to_string(),
    );
    (Vec::new(), report)
}

/// Verify `intent.candidates` against the real JVM and select the first one
/// the JVM accepts (with surgical pruning of unsupported tuning flags).
pub async fn select_best_candidate(
    java: &Path,
    intent: &GcLaunchIntent,
) -> (Vec<String>, GcLaunchReport) {
    select_best_candidate_with_probe(intent, &mut RealJvmProbe { java }).await
}

/// Replace the preset block (or auto marker) in `args` with the verified
/// `chosen` tokens. If the block cannot be located, leaves `args` untouched.
pub fn replace_gc_block(
    args: &mut Vec<String>,
    intent: &GcLaunchIntent,
    chosen: &[String],
) {
    let block_set: HashSet<&str> =
        intent.block_tokens.iter().map(String::as_str).collect();
    let is_auto = block_set.contains(AUTO_GC_PRESET_ARG)
        || intent.block_tokens.iter().any(|t| t == AUTO_GC_PRESET_ARG);

    let mut out: Vec<String> = Vec::with_capacity(args.len() + chosen.len());
    let mut inserted = false;
    for arg in args.iter() {
        let belongs_to_block = if is_auto {
            arg == AUTO_GC_PRESET_ARG
        } else {
            block_set.contains(arg.as_str())
        };
        if belongs_to_block {
            if !inserted {
                out.extend(chosen.iter().cloned());
                inserted = true;
            }
        } else {
            out.push(arg.clone());
        }
    }

    if !inserted {
        warn!(
            "GC intent block not found in effective java args; leaving them unchanged"
        );
        return;
    }
    *args = out;
}

/// Convenience: select and splice in one call, returning the report.
pub async fn resolve_gc_block(
    java: &Path,
    java_args: &mut Vec<String>,
    intent: &crate::launcher::jvm_args::GcLaunchIntent,
) -> GcLaunchReport {
    let (chosen, report) = select_best_candidate(java, intent).await;
    replace_gc_block(java_args, intent, &chosen);
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_supported(_args: &[String]) -> ProbeOutcome {
        ProbeOutcome {
            supported: true,
            stderr_text: String::new(),
        }
    }

    /// Wraps a synchronous fake probe so it can drive the async selection
    /// logic without spawning a real JVM.
    struct StubProbe<F>(F);
    impl<F> JvmProbe for StubProbe<F>
    where
        F: FnMut(&[String]) -> ProbeOutcome,
    {
        fn probe<'a>(
            &'a mut self,
            args: &'a [String],
        ) -> Pin<Box<dyn Future<Output = ProbeOutcome> + Send + 'a>> {
            let outcome = (self.0)(args);
            Box::pin(async move { outcome })
        }
    }

    #[test]
    fn normalizes_option_keys() {
        assert_eq!(normalize_option_key("-XX:+UseZGC"), "UseZGC");
        assert_eq!(
            normalize_option_key("-XX:G1UncommitBias=1"),
            "G1UncommitBias"
        );
        assert_eq!(normalize_option_key("UseZGC"), "UseZGC");
        assert_eq!(
            normalize_option_key("-XX:ShenandoahHeapRegionSize=256M"),
            "ShenandoahHeapRegionSize"
        );
        assert_eq!(
            normalize_option_key("-XX:+UnlockExperimentalVMOptions"),
            "UnlockExperimentalVMOptions"
        );
    }

    #[test]
    fn detects_only_collector_selectors_as_gc_selectors() {
        assert!(is_gc_selector("-XX:+UseZGC"));
        assert!(is_gc_selector("-XX:+UseShenandoahGC"));
        assert!(is_gc_selector("-XX:+UseG1GC"));
        assert!(!is_gc_selector("-XX:+ZGenerational"));
        assert!(!is_gc_selector("-XX:G1UncommitBias=1"));
        assert!(!is_gc_selector("-XX:+UnlockExperimentalVMOptions"));
    }

    #[test]
    fn finds_offending_tuning_flag_and_selector() {
        let args = vec![
            "-XX:+UseZGC".to_string(),
            "-XX:+UnlockExperimentalVMOptions".to_string(),
            "-XX:+ZGenerational".to_string(),
        ];
        let text = "Unrecognized VM option 'ZGenerational'";
        assert_eq!(find_offending_arg(text, &args), Some(2));

        let text = "Unrecognized VM option 'UseZGC'";
        assert_eq!(find_offending_arg(text, &args), Some(0));
    }

    #[tokio::test]
    async fn preferred_candidate_accepted_untouched() {
        let intent = GcLaunchIntent {
            active_preset_id: "gc-auto".to_string(),
            block_tokens: vec!["@axolotl:gc:auto".to_string()],
            candidate_ids: vec!["zgc".to_string(), "g1gc-mojang".to_string()],
            candidates: vec![
                vec!["-XX:+UseZGC".to_string()],
                vec!["-XX:+UseG1GC".to_string()],
            ],
        };
        let (chosen, report) = select_best_candidate_with_probe(
            &intent,
            &mut StubProbe(fake_supported),
        )
        .await;
        assert_eq!(report.chosen_strategy, "zgc");
        assert_eq!(chosen, vec!["-XX:+UseZGC"]);
        assert!(!report.fell_back());
    }

    #[tokio::test]
    async fn prunes_unsupported_tuning_flag_and_keeps_strategy() {
        let intent = GcLaunchIntent {
            active_preset_id: "gc-auto".to_string(),
            block_tokens: vec!["@axolotl:gc:auto".to_string()],
            candidate_ids: vec!["zgc".to_string()],
            candidates: vec![vec![
                "-XX:+UseZGC".to_string(),
                "-XX:+ZGenerational".to_string(),
            ]],
        };
        // A real JVM only rejects `-XX:+ZGenerational` while it is present;
        // once pruned the remaining set must be accepted.
        let probe = |args: &[String]| {
            if args
                .iter()
                .any(|a| normalize_option_key(a) == "ZGenerational")
            {
                ProbeOutcome {
                    supported: false,
                    stderr_text: "Unrecognized VM option 'ZGenerational'"
                        .to_string(),
                }
            } else {
                fake_supported(args)
            }
        };
        let (chosen, report) =
            select_best_candidate_with_probe(&intent, &mut StubProbe(probe))
                .await;
        assert_eq!(report.chosen_strategy, "zgc");
        assert_eq!(chosen, vec!["-XX:+UseZGC"]);
        assert_eq!(report.pruned_args, vec!["-XX:+ZGenerational"]);
        assert!(report.fell_back());
    }

    #[tokio::test]
    async fn selector_unsupported_falls_back_to_next_candidate() {
        let intent = GcLaunchIntent {
            active_preset_id: "gc-auto".to_string(),
            block_tokens: vec!["@axolotl:gc:auto".to_string()],
            candidate_ids: vec!["zgc".to_string(), "g1gc-mojang".to_string()],
            candidates: vec![
                vec!["-XX:+UseZGC".to_string()],
                vec!["-XX:+UseG1GC".to_string()],
            ],
        };
        let probe = |args: &[String]| {
            if args.iter().any(|a| a == "-XX:+UseZGC") {
                ProbeOutcome {
                    supported: false,
                    stderr_text: "Unrecognized VM option 'UseZGC'".to_string(),
                }
            } else {
                fake_supported(args)
            }
        };
        let (chosen, report) =
            select_best_candidate_with_probe(&intent, &mut StubProbe(probe))
                .await;
        assert_eq!(report.chosen_strategy, "g1gc-mojang");
        assert_eq!(chosen, vec!["-XX:+UseG1GC"]);
        assert!(report.fell_back());
        assert!(
            report
                .reason_chain
                .iter()
                .any(|r| r.contains("not supported by this JVM"))
        );
    }

    #[tokio::test]
    async fn all_candidates_fail_falls_back_to_empty_block() {
        let intent = GcLaunchIntent {
            active_preset_id: "gc-auto".to_string(),
            block_tokens: vec!["@axolotl:gc:auto".to_string()],
            candidate_ids: vec![
                "g1gc-mojang".to_string(),
                "minimal-g1".to_string(),
            ],
            candidates: vec![
                vec!["-XX:+UseG1GC".to_string()],
                vec!["-XX:+UseG1GC".to_string()],
            ],
        };
        let probe = |_args: &[String]| ProbeOutcome {
            supported: false,
            stderr_text: "Unrecognized VM option 'UseG1GC'".to_string(),
        };
        let (chosen, report) =
            select_best_candidate_with_probe(&intent, &mut StubProbe(probe))
                .await;
        assert!(chosen.is_empty());
        assert!(report.chosen_strategy.is_empty());
        assert!(
            report
                .reason_chain
                .iter()
                .any(|r| r.contains("JVM default"))
        );
    }

    #[test]
    fn replaces_auto_marker_block() {
        let intent = GcLaunchIntent {
            active_preset_id: "gc-auto".to_string(),
            block_tokens: vec!["@axolotl:gc:auto".to_string()],
            candidate_ids: vec!["g1gc-mojang".to_string()],
            candidates: vec![vec!["-XX:+UseG1GC".to_string()]],
        };
        let mut args =
            vec!["-Xmx2G".to_string(), "@axolotl:gc:auto".to_string()];
        replace_gc_block(&mut args, &intent, &["-XX:+UseG1GC".to_string()]);
        assert_eq!(args, vec!["-Xmx2G", "-XX:+UseG1GC"]);
    }

    #[test]
    fn replaces_manual_preset_block_tokens() {
        let intent = GcLaunchIntent {
            active_preset_id: "gc-shenandoah".to_string(),
            block_tokens: vec![
                "-XX:+UseShenandoahGC".to_string(),
                "-XX:ShenandoahHeapRegionSize=256M".to_string(),
            ],
            candidate_ids: vec!["shenandoah".to_string()],
            candidates: vec![vec![
                "-XX:+UseShenandoahGC".to_string(),
                "-XX:ShenandoahHeapRegionSize=256M".to_string(),
            ]],
        };
        let mut args = vec![
            "-XX:+UseShenandoahGC".to_string(),
            "-XX:ShenandoahHeapRegionSize=256M".to_string(),
            "-Dfoo=bar".to_string(),
        ];
        let chosen = vec!["-XX:+UseG1GC".to_string()];
        replace_gc_block(&mut args, &intent, &chosen);
        assert_eq!(args, vec!["-XX:+UseG1GC", "-Dfoo=bar"]);
    }

    #[test]
    fn leaves_args_unchanged_when_block_missing() {
        let intent = GcLaunchIntent {
            active_preset_id: "gc-shenandoah".to_string(),
            block_tokens: vec!["-XX:+UseShenandoahGC".to_string()],
            candidate_ids: vec!["shenandoah".to_string()],
            candidates: vec![vec!["-XX:+UseShenandoahGC".to_string()]],
        };
        let mut args = vec!["-Dfoo=bar".to_string()];
        replace_gc_block(&mut args, &intent, &["-XX:+UseG1GC".to_string()]);
        assert_eq!(args, vec!["-Dfoo=bar"]);
    }
}
