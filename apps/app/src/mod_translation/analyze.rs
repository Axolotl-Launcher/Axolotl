//! JAR 分析：解包、加载器探测、语言源发现、class 常量池提取、报价。

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::mod_translation::error::{
    Result, TranslateError, TranslateErrorCode,
};
use crate::mod_translation::jar::{self, ExtractionLimits};
use crate::mod_translation::quality::{
    extract_protected_tokens, has_chinese, requires_work,
};
use crate::mod_translation::writeback::FreeTextSnapshot;

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Loader {
    Fabric,
    NeoForge,
    Forge,
    #[default]
    Unknown,
}

impl Loader {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fabric => "fabric",
            Self::NeoForge => "neoforge",
            Self::Forge => "forge",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LanguageKind {
    Json,
    KeyValue,
    FreeText,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalizedLayout {
    pub bom: bool,
    pub eol: String,
    pub trailing_newline: bool,
    pub source_lines: Vec<String>,
    pub existing_target_lines: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageSource {
    pub kind: LanguageKind,
    pub namespace: String,
    pub source_path: String,
    pub target_path: String,
    pub entries: BTreeMap<String, String>,
    pub existing_target: BTreeMap<String, String>,
    #[serde(skip)]
    pub structured_template: Option<String>,
    pub localized_layout: Option<LocalizedLayout>,
}

impl LanguageSource {
    pub fn is_structured_json(&self) -> bool {
        self.kind == LanguageKind::Json
            && !self.entries.is_empty()
            && self.entries.keys().all(|key| key.starts_with('/'))
    }

    pub fn required_keys(&self) -> Vec<String> {
        self.entries
            .keys()
            .filter(|key| {
                let existing =
                    self.existing_target.get(*key).map(String::as_str);
                requires_work(
                    key,
                    self.entries.get(*key).map(String::as_str).unwrap_or(""),
                    existing,
                )
            })
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassCandidate {
    pub id: String,
    pub path: String,
    pub paths: Vec<String>,
    pub occurrences: usize,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceCoverage {
    pub path: String,
    pub media_type: String,
    pub disposition: String,
    pub target_path: Option<String>,
    pub text_candidates: usize,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainedMod {
    pub mod_id: String,
    pub display_name: Option<String>,
    pub mod_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JarInspection {
    pub input_path: PathBuf,
    pub original_filename: String,
    pub loader: Loader,
    pub mod_ids: Vec<String>,
    pub project_names: Vec<String>,
    pub mod_version: Option<String>,
    pub minecraft_version_range: Option<String>,
    pub contained_mods: Vec<ContainedMod>,
    pub signed: bool,
    pub total_entries: u64,
    pub uncompressed_bytes: u64,
    pub language_sources: Vec<LanguageSource>,
    pub class_candidates: Vec<ClassCandidate>,
    pub resource_coverage: Vec<ResourceCoverage>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    /// 预计输入 token（提示词 + 条目 JSON）
    pub estimated_input_tokens: u64,
    /// 预计输出 token（译文/判定）
    pub estimated_output_tokens: u64,
    /// 合计（含深通道/质量回修的余量）
    pub estimated_tokens: u64,
    /// 预计 AI 调用次数
    pub estimated_calls: u64,
    pub language_batches: u64,
    pub class_batches: u64,
    pub points: u64,
    pub characters: u64,
    pub entries: u64,
}

pub fn quote_translation_metrics(
    language_entries: usize,
    language_chars: usize,
    class_candidates: usize,
    class_chars: usize,
) -> Quote {
    let language_batches = language_entries.div_ceil(40);
    let class_batches = class_candidates.div_ceil(16);
    let points = 10
        + (language_batches + class_batches) as u64 * 2
        + ((language_chars + class_chars) as u64).div_ceil(1_000);

    // token 粗估：英文约 4 字符/token、中文约 1.5 字符/token，按 0.3-0.35 折算；
    // 每批还有固定开销（system prompt + JSON 包裹）。
    let language_input =
        (language_chars as f64 * 0.35 + language_batches as f64 * 600.0) as u64;
    let class_input =
        (class_chars as f64 * 0.3 + class_batches as f64 * 500.0) as u64;
    // 输出：英→中译文约为源字符的 0.6 倍，中文 1 字符约 1 token；class 判定每条约 40 token
    let language_output = (language_chars as f64 * 0.6) as u64;
    let class_output = class_candidates as u64 * 40;
    // 深通道/质量回修会把一部分条目重译，留 20% 余量
    let estimated_input_tokens = language_input + class_input;
    let estimated_output_tokens = language_output + class_output;
    let estimated_tokens = ((estimated_input_tokens + estimated_output_tokens)
        as f64
        * 1.2) as u64;
    let estimated_calls = language_batches as u64 + class_batches as u64;

    Quote {
        estimated_input_tokens,
        estimated_output_tokens,
        estimated_tokens,
        estimated_calls,
        language_batches: language_batches as u64,
        class_batches: class_batches as u64,
        points,
        characters: (language_chars + class_chars) as u64,
        entries: (language_entries + class_candidates) as u64,
    }
}

pub fn clean_metadata_text(value: &str, maximum: usize) -> Option<String> {
    let cleaned = value
        .chars()
        .filter(|character| {
            !(*character as u32 == 0
                || (*character as u32) < 0x20 && *character != '\t')
        })
        .collect::<String>();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.chars().take(maximum).collect())
    }
}

#[derive(Debug, Default)]
pub struct Metadata {
    pub loader: Loader,
    pub mod_ids: Vec<String>,
    pub project_names: Vec<String>,
    pub mod_version: Option<String>,
    pub minecraft_version_range: Option<String>,
    pub contained_mods: Vec<ContainedMod>,
}

pub fn detect_metadata(directory: &Path) -> Metadata {
    let fabric_path = directory.join("fabric.mod.json");
    if fabric_path.is_file() {
        if let Ok(content) = std::fs::read_to_string(&fabric_path)
            && let Ok(value) =
                serde_json::from_str::<serde_json::Value>(&content)
        {
            let mod_ids = value
                .get("id")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| clean_metadata_text(value, 100))
                .map(|value| vec![value])
                .unwrap_or_default();
            let project_names = value
                .get("name")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| clean_metadata_text(value, 160))
                .map(|value| vec![value])
                .unwrap_or_default();
            let mod_version = value
                .get("version")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| clean_metadata_text(value, 160));
            let minecraft_version_range = value
                .pointer("/depends/minecraft")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| clean_metadata_text(value, 160));
            let contained_mods = mod_ids
                .iter()
                .take(20)
                .map(|mod_id| ContainedMod {
                    mod_id: mod_id.clone(),
                    display_name: project_names.first().cloned(),
                    mod_version: mod_version.clone(),
                })
                .collect();
            return Metadata {
                loader: Loader::Fabric,
                mod_ids,
                project_names,
                mod_version,
                minecraft_version_range,
                contained_mods,
            };
        }
        return Metadata {
            loader: Loader::Fabric,
            ..Default::default()
        };
    }

    let neo_path = directory.join("META-INF/neoforge.mods.toml");
    let forge_path = directory.join("META-INF/mods.toml");
    let metadata_path = if neo_path.is_file() {
        Some((neo_path, Loader::NeoForge))
    } else if forge_path.is_file() {
        Some((forge_path, Loader::Forge))
    } else {
        None
    };
    if let Some((path, loader)) = metadata_path
        && let Ok(text) = std::fs::read_to_string(&path)
    {
        let mod_ids = extract_toml_values(&text, "modId", 100);
        let project_names = extract_toml_values(&text, "displayName", 160);
        let versions = extract_toml_values(&text, "version", 100);
        let minecraft_version_range = extract_minecraft_range(&text);
        let contained_mods = mod_ids
            .iter()
            .take(20)
            .enumerate()
            .map(|(index, mod_id)| ContainedMod {
                mod_id: mod_id.clone(),
                display_name: project_names.get(index).cloned(),
                mod_version: versions.get(index).cloned(),
            })
            .collect();
        return Metadata {
            loader,
            mod_ids,
            project_names,
            mod_version: versions.first().cloned(),
            minecraft_version_range,
            contained_mods,
        };
    }

    Metadata::default()
}

fn extract_toml_values(text: &str, key: &str, maximum: usize) -> Vec<String> {
    let pattern = format!(r#"\b{key}\s*=\s*(?:"([^"]+)"|'([^']+)')"#);
    let regex = regex::Regex::new(&pattern).unwrap();
    regex
        .captures_iter(text)
        .filter_map(|captures| {
            captures
                .get(1)
                .or_else(|| captures.get(2))
                .map(|value| value.as_str())
                .and_then(|value| clean_metadata_text(value, maximum))
        })
        .collect()
}

fn extract_minecraft_range(text: &str) -> Option<String> {
    let regex = regex::Regex::new(
        r#"\bmodId\s*=\s*["']minecraft["'][\s\S]{0,500}?\bversionRange\s*=\s*["']([^"']+)["']"#,
    )
    .unwrap();
    regex
        .captures(text)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str())
        .and_then(|value| clean_metadata_text(value, 160))
}

fn looks_like_player_text(value: &str) -> bool {
    let text = value.trim();
    if text.chars().count() < 2
        || text.chars().count() > 20_000
        || !text
            .chars()
            .any(|character| character.is_ascii_alphabetic())
    {
        return false;
    }
    if regex::Regex::new(r"^(?:https?:|mailto:|file:)")
        .unwrap()
        .is_match(text)
    {
        return false;
    }
    if regex::Regex::new(r"^(?:!?mod:|\|[a-z_]+,mod:|#[A-Za-z0-9_.-]+$)")
        .unwrap()
        .is_match(text)
    {
        return false;
    }
    if regex::Regex::new(r"^[a-z0-9_.-]+:[a-z0-9_./-]+$")
        .unwrap()
        .is_match(text)
    {
        return false;
    }
    if regex::Regex::new(r"^(?:[A-Za-z]:[\\/]|(?:\.\.?[\\/])|[\w.-]+\.(?:png|ogg|class|json|nbt|toml|cfg))$")
        .unwrap()
        .is_match(text)
    {
        return false;
    }
    if regex::Regex::new(r"^[a-z0-9_.:-]+$")
        .unwrap()
        .is_match(text)
        && !text.chars().any(|character| character.is_ascii_uppercase())
    {
        return false;
    }
    regex::Regex::new(r#"\s|[A-Z]|[.,!?;:'"()\[\]{}]|^/"#)
        .unwrap()
        .is_match(text)
}

fn flatten_structured_text(
    value: &serde_json::Value,
    pointer: &str,
    out: &mut BTreeMap<String, String>,
) {
    match value {
        serde_json::Value::String(text) if looks_like_player_text(text) => {
            out.insert(
                if pointer.is_empty() {
                    "/".to_string()
                } else {
                    pointer.to_string()
                },
                text.clone(),
            );
        }
        serde_json::Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                flatten_structured_text(
                    item,
                    &format!("{pointer}/{index}"),
                    out,
                );
            }
        }
        serde_json::Value::Object(map) => {
            for (key, item) in map {
                let escaped = key.replace('~', "~0").replace('/', "~1");
                flatten_structured_text(
                    item,
                    &format!("{pointer}/{escaped}"),
                    out,
                );
            }
        }
        _ => {}
    }
}

pub fn find_standard_language_sources(
    directory: &Path,
    cancel: &AtomicBool,
) -> Result<Vec<LanguageSource>> {
    let assets_root = directory.join("assets");
    if !assets_root.is_dir() {
        return Ok(Vec::new());
    }
    let mut sources = Vec::new();
    let entries = std::fs::read_dir(&assets_root).map_err(|error| {
        TranslateError::io("unable to read assets directory", error)
    })?;
    for entry in entries.flatten() {
        check_analysis_cancel(cancel)?;
        let namespace = entry.file_name().to_string_lossy().into_owned();
        let lang_dir = assets_root.join(&namespace).join("lang");
        if !lang_dir.is_dir() {
            continue;
        }
        let mut format = None;
        for candidate in ["json", "lang", "properties"] {
            if lang_dir.join(format!("en_us.{candidate}")).is_file() {
                format = Some(candidate);
                break;
            }
        }
        let Some(format) = format else { continue };
        let source_path = format!("assets/{namespace}/lang/en_us.{format}");
        let target_path = format!("assets/{namespace}/lang/zh_cn.{format}");
        let source_file = directory.join(&source_path);
        let content =
            std::fs::read_to_string(&source_file).map_err(|error| {
                TranslateError::io(
                    format!("unable to read {source_path}"),
                    error,
                )
            })?;
        let (entries, kind) = if format == "json" {
            (read_json_lang(&content)?, LanguageKind::Json)
        } else {
            (
                crate::mod_translation::writeback::read_key_value(&content),
                LanguageKind::KeyValue,
            )
        };
        let existing_target = if directory.join(&target_path).is_file() {
            let existing =
                std::fs::read_to_string(directory.join(&target_path))
                    .unwrap_or_default();
            if format == "json" {
                read_json_lang(&existing).unwrap_or_default()
            } else {
                crate::mod_translation::writeback::read_key_value(&existing)
            }
        } else {
            BTreeMap::new()
        };
        sources.push(LanguageSource {
            kind,
            namespace: namespace.clone(),
            source_path,
            target_path,
            entries,
            existing_target,
            structured_template: None,
            localized_layout: None,
        });
    }
    Ok(sources)
}

fn read_json_lang(content: &str) -> Result<BTreeMap<String, String>> {
    let value: serde_json::Value =
        serde_json::from_str(content).map_err(|error| {
            TranslateError::with_source(
                TranslateErrorCode::InvalidArchive,
                "unable to parse JSON language file",
                error,
            )
        })?;
    let mut result = BTreeMap::new();
    if let serde_json::Value::Object(map) = value {
        for (key, item) in map {
            if let serde_json::Value::String(text) = item {
                result.insert(key, text);
            }
        }
    }
    Ok(result)
}

pub fn discover_structured_sources(
    directory: &Path,
    standard: &HashSet<String>,
    cancel: &AtomicBool,
) -> Result<Vec<LanguageSource>> {
    let mut sources = Vec::new();
    for relative in jar::collect_files_cancellable(directory, cancel)? {
        check_analysis_cancel(cancel)?;
        let lower = relative.to_ascii_lowercase();
        if !lower.ends_with(".json") || standard.contains(&lower) {
            continue;
        }
        let segments: Vec<&str> = relative.split('/').collect();
        let Some(locale_index) = segments
            .iter()
            .position(|segment| segment.eq_ignore_ascii_case("en_us"))
        else {
            continue;
        };
        let raw = std::fs::read_to_string(directory.join(&relative))
            .unwrap_or_default();
        let Ok(template) = serde_json::from_str::<serde_json::Value>(&raw)
        else {
            continue;
        };
        let mut entries = BTreeMap::new();
        flatten_structured_text(&template, "", &mut entries);
        if entries.is_empty() {
            continue;
        }
        let mut target_segments = segments.clone();
        target_segments[locale_index] = "zh_cn";
        let target_path = target_segments.join("/");
        let mut existing_target = BTreeMap::new();
        if directory.join(&target_path).is_file()
            && let Ok(existing) = serde_json::from_str::<serde_json::Value>(
                &std::fs::read_to_string(directory.join(&target_path))
                    .unwrap_or_default(),
            )
        {
            flatten_structured_text(&existing, "", &mut existing_target);
        }
        let namespace = if segments
            .first()
            .is_some_and(|value| value.eq_ignore_ascii_case("assets"))
            && segments.len() >= 2
        {
            segments[1].to_string()
        } else {
            "unknown".to_string()
        };
        sources.push(LanguageSource {
            kind: LanguageKind::Json,
            namespace,
            source_path: relative,
            target_path,
            entries,
            existing_target,
            structured_template: Some(raw),
            localized_layout: None,
        });
    }
    Ok(sources)
}

pub fn discover_localized_text_sources(
    directory: &Path,
    known: &HashSet<String>,
    cancel: &AtomicBool,
) -> Result<Vec<LanguageSource>> {
    let mut sources = Vec::new();
    for relative in jar::collect_files_cancellable(directory, cancel)? {
        check_analysis_cancel(cancel)?;
        let lower = relative.to_ascii_lowercase();
        if known.contains(&lower)
            || !(lower.ends_with(".txt") || lower.ends_with(".md"))
        {
            continue;
        }
        let segments: Vec<&str> = relative.split('/').collect();
        let Some(locale_index) = segments
            .iter()
            .position(|segment| segment.eq_ignore_ascii_case("en_us"))
        else {
            continue;
        };
        let content = std::fs::read_to_string(directory.join(&relative))
            .unwrap_or_default();
        if !looks_like_player_text(&content) {
            continue;
        }
        let mut target_segments = segments.clone();
        target_segments[locale_index] = "zh_cn";
        let target_path = target_segments.join("/");
        let existing = std::fs::read_to_string(directory.join(&target_path))
            .unwrap_or_default();
        let source_layout = FreeTextSnapshot::parse(&content);
        let existing_layout = if directory.join(&target_path).is_file() {
            Some(FreeTextSnapshot::parse(&existing))
        } else {
            None
        };
        let namespace = if segments
            .first()
            .is_some_and(|value| value.eq_ignore_ascii_case("assets"))
            && segments.len() >= 2
        {
            segments[1].to_string()
        } else {
            "unknown".to_string()
        };
        let entries = localized_source_entries(&source_layout);
        let existing_target = localized_existing_entries(
            &source_layout,
            existing_layout.as_ref(),
        );
        sources.push(LanguageSource {
            kind: LanguageKind::FreeText,
            namespace,
            source_path: relative,
            target_path,
            entries,
            existing_target,
            structured_template: None,
            localized_layout: Some(LocalizedLayout {
                bom: source_layout.bom,
                eol: source_layout.eol,
                trailing_newline: source_layout.trailing_newline,
                source_lines: source_layout.lines,
                existing_target_lines: existing_layout
                    .map(|layout| layout.lines),
            }),
        });
    }
    Ok(sources)
}

fn localized_source_entries(
    snapshot: &FreeTextSnapshot,
) -> BTreeMap<String, String> {
    snapshot
        .lines
        .iter()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            (
                crate::mod_translation::writeback::localized_line_key(index),
                line.clone(),
            )
        })
        .collect()
}

fn localized_existing_entries(
    source: &FreeTextSnapshot,
    target: Option<&FreeTextSnapshot>,
) -> BTreeMap<String, String> {
    let Some(target) = target else {
        return BTreeMap::new();
    };
    source
        .lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            if line.trim().is_empty() {
                return None;
            }
            let translated =
                target.lines.get(index).map(String::as_str).unwrap_or("");
            if translated.trim().is_empty() {
                None
            } else {
                Some((
                    crate::mod_translation::writeback::localized_line_key(
                        index,
                    ),
                    translated.to_string(),
                ))
            }
        })
        .collect()
}

pub fn build_resource_coverage(
    directory: &Path,
    sources: &[LanguageSource],
    cancel: &AtomicBool,
) -> Result<Vec<ResourceCoverage>> {
    let source_by_path: HashMap<String, &LanguageSource> = sources
        .iter()
        .map(|source| (source.source_path.to_ascii_lowercase(), source))
        .collect();
    let target_paths: HashSet<String> = sources
        .iter()
        .map(|source| source.target_path.to_ascii_lowercase())
        .collect();
    let mut coverage = Vec::new();
    for relative in jar::collect_files_cancellable(directory, cancel)? {
        check_analysis_cancel(cancel)?;
        let lower = relative.to_ascii_lowercase();
        if let Some(source) = source_by_path.get(&lower) {
            coverage.push(ResourceCoverage {
                path: relative,
                media_type: if lower.ends_with(".json") {
                    "json"
                } else {
                    "text"
                }
                .to_string(),
                disposition: if source.kind == LanguageKind::Json {
                    if source.is_structured_json() {
                        "structured_source".to_string()
                    } else {
                        "standard_language".to_string()
                    }
                } else {
                    "standard_language".to_string()
                },
                target_path: Some(source.target_path.clone()),
                text_candidates: source.entries.len(),
                reason: "已进入翻译工作图".to_string(),
            });
            continue;
        }
        if target_paths.contains(&lower) {
            coverage.push(ResourceCoverage {
                path: relative,
                media_type: "json".to_string(),
                disposition: "generated_target".to_string(),
                target_path: None,
                text_candidates: 0,
                reason: "现有中文镜像".to_string(),
            });
            continue;
        }
        if lower.ends_with(".class") {
            coverage.push(ResourceCoverage {
                path: relative,
                media_type: "class".to_string(),
                disposition: "class_review".to_string(),
                target_path: None,
                text_candidates: 0,
                reason: "由 Class 常量扫描独立审计".to_string(),
            });
            continue;
        }
        let extension = lower.rsplit('.').next().unwrap_or("");
        if matches!(
            extension,
            "json"
                | "lang"
                | "properties"
                | "txt"
                | "md"
                | "toml"
                | "cfg"
                | "xml"
        ) {
            let sample = std::fs::read_to_string(directory.join(&relative))
                .map(|value| value.chars().take(200_000).collect::<String>())
                .unwrap_or_default();
            let localized_region = lower.split('/').any(|segment| {
                segment == "en_us" || segment.starts_with("en_us.")
            });
            let mut candidate =
                localized_region && looks_like_player_text(&sample);
            if extension == "json"
                && let Ok(value) =
                    serde_json::from_str::<serde_json::Value>(&sample)
            {
                let mut flat = BTreeMap::new();
                flatten_structured_text(&value, "", &mut flat);
                candidate = localized_region && !flat.is_empty();
            }
            coverage.push(ResourceCoverage {
                path: relative,
                media_type: if extension == "json" { "json" } else { "text" }
                    .to_string(),
                disposition: if candidate {
                    "unknown".to_string()
                } else {
                    "protected".to_string()
                },
                target_path: None,
                text_candidates: usize::from(candidate),
                reason: if candidate {
                    "存在自然语言迹象但尚无可验证的语言镜像".to_string()
                } else {
                    "未发现高置信玩家文本".to_string()
                },
            });
            continue;
        }
        coverage.push(ResourceCoverage {
            path: relative,
            media_type: "binary".to_string(),
            disposition: "protected".to_string(),
            target_path: None,
            text_candidates: 0,
            reason: "非文本资源保持原样".to_string(),
        });
    }
    Ok(coverage)
}

// ---- class 常量池 ----

#[derive(Debug, Clone)]
pub struct Utf8Entry {
    pub index: u16,
    pub start: usize,
    pub end: usize,
    pub text: String,
}

pub struct ConstantPool {
    pub utf8_entries: Vec<Utf8Entry>,
    pub string_utf8_indices: HashSet<u16>,
}

pub fn parse_class_constant_pool(buffer: &[u8]) -> Result<ConstantPool> {
    if buffer.len() < 10 || buffer[0..4] != [0xca, 0xfe, 0xba, 0xbe] {
        return Err(TranslateError::new(
            TranslateErrorCode::InvalidArchive,
            "not a valid Java class file",
        ));
    }
    let count = u16::from_be_bytes([buffer[8], buffer[9]]);
    let mut utf8_entries = Vec::new();
    let mut string_utf8_indices = HashSet::new();
    let mut offset = 10usize;
    let mut index = 1u16;
    while index < count {
        if offset >= buffer.len() {
            return Err(TranslateError::new(
                TranslateErrorCode::InvalidArchive,
                "class constant pool is truncated",
            ));
        }
        let tag = buffer[offset];
        offset += 1;
        match tag {
            1 => {
                if offset + 2 > buffer.len() {
                    return Err(TranslateError::new(
                        TranslateErrorCode::InvalidArchive,
                        "class UTF-8 constant is truncated",
                    ));
                }
                let length =
                    u16::from_be_bytes([buffer[offset], buffer[offset + 1]])
                        as usize;
                offset += 2;
                if offset + length > buffer.len() {
                    return Err(TranslateError::new(
                        TranslateErrorCode::InvalidArchive,
                        "class UTF-8 constant is truncated",
                    ));
                }
                let text =
                    String::from_utf8_lossy(&buffer[offset..offset + length])
                        .into_owned();
                utf8_entries.push(Utf8Entry {
                    index,
                    start: offset - 3,
                    end: offset + length,
                    text,
                });
                offset += length;
            }
            3 | 4 => offset += 4,
            5 | 6 => {
                offset += 8;
                index += 1;
            }
            8 => {
                if offset + 2 > buffer.len() {
                    return Err(TranslateError::new(
                        TranslateErrorCode::InvalidArchive,
                        "class String constant is truncated",
                    ));
                }
                string_utf8_indices.insert(u16::from_be_bytes([
                    buffer[offset],
                    buffer[offset + 1],
                ]));
                offset += 2;
            }
            7 | 16 | 19 | 20 => offset += 2,
            9 | 10 | 11 | 12 | 17 | 18 => offset += 4,
            15 => offset += 3,
            _ => {
                return Err(TranslateError::new(
                    TranslateErrorCode::InvalidArchive,
                    format!("unsupported class constant pool tag: {tag}"),
                ));
            }
        }
        if offset > buffer.len() {
            return Err(TranslateError::new(
                TranslateErrorCode::InvalidArchive,
                "class constant pool is truncated",
            ));
        }
        index += 1;
    }
    Ok(ConstantPool {
        utf8_entries,
        string_utf8_indices,
    })
}

pub fn class_utf8_entries(buffer: &[u8]) -> Result<Vec<Utf8Entry>> {
    Ok(parse_class_constant_pool(buffer)?.utf8_entries)
}

pub fn class_string_constants(buffer: &[u8]) -> Result<Vec<String>> {
    let parsed = parse_class_constant_pool(buffer)?;
    Ok(parsed
        .utf8_entries
        .iter()
        .filter(|entry| parsed.string_utf8_indices.contains(&entry.index))
        .map(|entry| entry.text.clone())
        .collect())
}

pub fn is_format_only_class_text(text: &str) -> bool {
    let mut remainder = text.to_string();
    let mut protected = extract_protected_tokens(text);
    protected.sort_by_key(|token| std::cmp::Reverse(token.len()));
    for token in protected {
        remainder = remainder.replace(&token, " ");
    }
    let words = regex::Regex::new(r"[A-Za-z]+")
        .unwrap()
        .find_iter(&remainder)
        .map(|value| value.as_str().to_ascii_lowercase())
        .collect::<Vec<_>>();
    words.is_empty()
        || words.iter().all(|word| {
            matches!(word.as_str(), "x" | "y" | "z" | "c" | "litematica")
        })
}

pub fn classify_class_text(text: &str) -> (bool, &'static str) {
    let value = text.trim();
    if has_chinese(value) {
        return (false, "already_localized");
    }
    if value.chars().count() < 3
        || value.chars().count() > 500
        || !value
            .chars()
            .any(|character| character.is_ascii_alphabetic())
        || value.contains('\0')
    {
        return (false, "invalid");
    }
    let plain = regex::Regex::new(r"§[0-9A-FK-OR]")
        .unwrap()
        .replace_all(value, "")
        .trim()
        .to_string();
    if is_format_only_class_text(&plain) {
        return (false, "format_only");
    }
    if regex::Regex::new(r"^[A-Z0-9_]+$").unwrap().is_match(&plain) {
        return (false, "constant");
    }
    let jvm_descriptor = regex::Regex::new(
        r"^(?:\[*[BCDFIJSVZ]|\[*L[\w/$]+;|\((?:\[*[BCDFIJSVZ]|\[*L[\w/$]+;)*\)(?:\[*[BCDFIJSVZ]|\[*L[\w/$]+;))$",
    )
    .unwrap();
    if jvm_descriptor.is_match(&plain) {
        return (false, "descriptor");
    }
    let pure_url = regex::Regex::new(r"^(?:https?|ftp)://\S+$")
        .unwrap()
        .is_match(&plain);
    let pure_windows_path =
        regex::Regex::new(r"^(?:[A-Za-z]:\\|\\\\)[^\r\n]+$")
            .unwrap()
            .is_match(&plain);
    let pure_unix_path = regex::Regex::new(r"^/(?:[^\s/]+/)+[^\s/]*$")
        .unwrap()
        .is_match(&plain);
    let pure_internal_path =
        regex::Regex::new(r"^(?:[A-Za-z_$][\w$.-]*[/:]){1,}[A-Za-z0-9_$.-]+$")
            .unwrap()
            .is_match(&plain);
    let pure_lower_identifier = regex::Regex::new(r"^[a-z0-9_$.:/-]+$")
        .unwrap()
        .is_match(&plain);
    if pure_url
        || pure_windows_path
        || pure_unix_path
        || pure_internal_path
        || pure_lower_identifier
    {
        return (false, "structural");
    }
    let pure_command = regex::Regex::new(
        r"^/[A-Za-z0-9_.:-]+(?:\s+(?:[A-Za-z0-9_.:-]+|\{[^{}\s]+\}|<[^<>\s]+>|\[[^\[\]\s]+\]))*$",
    )
    .unwrap();
    if !regex::Regex::new(r"\s(?:-|—|:)\s")
        .unwrap()
        .is_match(&plain)
        && pure_command.is_match(&plain)
    {
        return (false, "pure_command");
    }
    if regex::Regex::new(r"^[A-Z][A-Za-z'-]+$")
        .unwrap()
        .is_match(&plain)
    {
        return (true, "display_word");
    }
    if regex::Regex::new(r#"\s|[!?.,:'"-]"#)
        .unwrap()
        .is_match(&plain)
    {
        return (true, "natural_language");
    }
    (false, "structural")
}

pub fn likely_visible_class_text(text: &str) -> bool {
    classify_class_text(text).0
}

pub fn deterministic_class_exclusion_reason(
    candidate: &ClassCandidate,
) -> Option<&'static str> {
    let value = candidate.text.trim();
    if has_chinese(value) {
        return Some("already_localized");
    }
    if is_format_only_class_text(value) {
        return Some("format_only");
    }
    if regex::Regex::new(r"^(?:[a-z_][A-Za-z0-9_$]*\.)+[A-Z_$][A-Za-z0-9_$]*$")
        .unwrap()
        .is_match(value)
    {
        return Some("java_class_name");
    }
    let looks_like_regex = value.starts_with('^')
        && value.ends_with('$')
        && regex::Regex::new(
            r"(?:\\[.dDsSwW]|\[[^\]]+\]|\{\d+(?:,\d*)?\}|\(\?:?)",
        )
        .unwrap()
        .is_match(value);
    if looks_like_regex {
        return Some("regular_expression");
    }

    let paths = candidate
        .paths
        .iter()
        .chain(std::iter::once(&candidate.path))
        .map(|path| path.replace('\\', "/").to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let ui_host = paths.iter().any(|path| {
        regex::Regex::new(
            r"(?:^|/)(?:gui|screen|widget|tooltip|chat|menu|config)(?:/|$)",
        )
        .unwrap()
        .is_match(path)
    });
    if ui_host {
        return None;
    }
    let diagnostic_host = !paths.is_empty()
        && paths.iter().all(|path| {
            regex::Regex::new(
                r"(?:^|/)(?:file|graphics|server|palette|world|misc|region|pool|core)(?:/|$)|(?:worldmapsession|crashhandler)\.class$",
            )
            .unwrap()
            .is_match(path)
        });
    let diagnostic_text = regex::Regex::new(
        r"(?i)\b(?:ioexception|io exception|failed to|exception|retrying|requesting|cancelled|initialized|unknown status|cache file|save using an older version)\b",
    )
    .unwrap()
    .is_match(value);
    if diagnostic_host && diagnostic_text {
        return Some("internal_diagnostic");
    }
    None
}

fn is_host_structural_class_candidate(candidate: &ClassCandidate) -> bool {
    let value = candidate.text.trim();
    let paths = candidate
        .paths
        .iter()
        .map(|path| path.to_ascii_lowercase())
        .collect::<Vec<_>>();
    if value.chars().any(|character| ('\u{0000}'..='\u{001f}').contains(&character))
        || regex::Regex::new(
            r"^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$",
        )
        .unwrap()
        .is_match(value)
    {
        return true;
    }
    if regex::Regex::new(r#"(?:^|[\s"'])(?:[a-z0-9_.-]+:)?(?:textures?|models?|sounds?|recipes?|tags?)/"#)
        .unwrap()
        .is_match(value)
        || regex::Regex::new(r"[a-z0-9_.-]+:[a-z0-9_./-]+#").unwrap().is_match(value)
        || regex::Regex::new(r"\.(?:png|ogg|json|nbt|mcmeta)(?:$|[#?])").unwrap().is_match(value)
    {
        return true;
    }
    let schema_host = !paths.is_empty()
        && paths.iter().all(|path| {
            regex::Regex::new(r"(?:^|/)(?:entity|blockentity|item|level|world|render|renderer|codec|serializer|network|data)(?:/|$)")
                .unwrap()
                .is_match(path)
        });
    let identifier = regex::Regex::new(
        r"^[A-Z][A-Za-z0-9]*(?:ID|Id|UUID|Tag|Time|Timer|Count|State|Data|Owner|Variant|Color|Seed|Radius|Height|Direction|Level|Amount|Duration|Cooldown|Modifier|Loading|Exploding|Charged|Reeling|Filled|Waste)?$",
    )
    .unwrap()
    .is_match(value);
    if schema_host && identifier && !value.contains(' ') {
        return true;
    }
    let repeated_schema_label = candidate.occurrences >= 3
        && value.split_whitespace().count() <= 4
        && !regex::Regex::new(r"[.!?]").unwrap().is_match(value)
        && !paths.is_empty()
        && paths.iter().all(|path| {
            regex::Regex::new(
                r"(?:/widget/|/codec/|/serializer/|/render(?:er)?/)",
            )
            .unwrap()
            .is_match(path)
        });
    if repeated_schema_label {
        return true;
    }
    let developer_assertion = regex::Regex::new(
        r"\b(?:must not be null|must be|need at least|cannot be null|invalid (?:value|state)|failed to (?:load|parse|decode))\b",
    )
    .unwrap()
    .is_match(value);
    developer_assertion
        && !paths.is_empty()
        && paths.iter().all(|path| {
            regex::Regex::new(
                r"(?:/config/|/feature/|/codec/|/serializer/|/integration/)",
            )
            .unwrap()
            .is_match(path)
        })
}

pub fn discover_class_candidates(
    directory: &Path,
    cancel: &AtomicBool,
) -> Result<Vec<ClassCandidate>> {
    let mut grouped: HashMap<String, (String, BTreeSet<String>)> =
        HashMap::new();
    for relative in jar::collect_files_cancellable(directory, cancel)? {
        check_analysis_cancel(cancel)?;
        if !relative.to_ascii_lowercase().ends_with(".class") {
            continue;
        }
        let bytes =
            std::fs::read(directory.join(&relative)).map_err(|error| {
                TranslateError::io(
                    format!("unable to read class file {relative}"),
                    error,
                )
            })?;
        for text in class_string_constants(&bytes)? {
            if !likely_visible_class_text(&text) {
                continue;
            }
            let entry = grouped
                .entry(text.clone())
                .or_insert_with(|| (relative.clone(), BTreeSet::new()));
            entry.1.insert(relative.clone());
        }
    }
    let mut candidates = grouped
        .into_iter()
        .map(|(text, (path, paths))| {
            let paths = paths.into_iter().collect::<Vec<_>>();
            ClassCandidate {
                id: class_candidate_id(&path, &text),
                path: path.clone(),
                paths: paths.clone(),
                occurrences: paths.len(),
                text,
            }
        })
        .filter(|candidate| !is_host_structural_class_candidate(candidate))
        .filter(|candidate| {
            deterministic_class_exclusion_reason(candidate).is_none()
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(candidates)
}

fn is_camel_case_config_identifier(value: &str) -> bool {
    value.chars().count() >= 3
        && value.chars().count() <= 80
        && value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_lowercase())
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
        && value
            .chars()
            .any(|character| character.is_ascii_uppercase())
}

fn discover_external_ui_warnings(
    directory: &Path,
    cancel: &AtomicBool,
) -> Result<Vec<String>> {
    let mut references_malilib = false;
    let mut config_identifiers = BTreeSet::new();
    for relative in jar::collect_files_cancellable(directory, cancel)? {
        check_analysis_cancel(cancel)?;
        if !relative.to_ascii_lowercase().ends_with(".class") {
            continue;
        }
        let bytes =
            std::fs::read(directory.join(&relative)).map_err(|error| {
                TranslateError::io(
                    format!("unable to read class file {relative}"),
                    error,
                )
            })?;
        let parsed = parse_class_constant_pool(&bytes)?;
        let uses_malilib_config = parsed.utf8_entries.iter().any(|entry| {
            entry
                .text
                .starts_with("fi/dy/masa/malilib/config/options/Config")
        });
        references_malilib |= parsed
            .utf8_entries
            .iter()
            .any(|entry| entry.text.starts_with("fi/dy/masa/malilib/"));
        if !uses_malilib_config
            || !relative.to_ascii_lowercase().contains("/config/")
        {
            continue;
        }
        for entry in &parsed.utf8_entries {
            if parsed.string_utf8_indices.contains(&entry.index)
                && is_camel_case_config_identifier(&entry.text)
            {
                config_identifiers.insert(entry.text.clone());
            }
        }
    }

    let mut warnings = Vec::new();
    if !config_identifiers.is_empty() {
        let samples = config_identifiers
            .iter()
            .take(4)
            .cloned()
            .collect::<Vec<_>>()
            .join("、");
        warnings.push(format!(
            "检测到 {} 个 MaLiLib 配置内部 ID（如 {samples}）。这些字符串兼任配置持久化键，不能直接替换成中文；当前 Class 改写尚不能为其安全注入独立 prettyName，因此配置界面可能继续显示英文 ID",
            config_identifiers.len()
        ));
    }
    if references_malilib {
        warnings.push(
            "检测到 MaLiLib 提供的界面组件。RESET 等通用按钮以及 true/false 值不属于当前模组语言文件；当前任务只处理所选 JAR，需单独翻译或修补 MaLiLib 依赖"
                .to_string(),
        );
    }
    Ok(warnings)
}

pub fn class_candidate_id(path: &str, text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.as_bytes());
    hasher.update(b"\0");
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())[..24].to_string()
}

/// 常量池 UTF8 条目改写：条目数不变、索引不变，只重建被替换条目的字节区间。
pub fn replace_class_utf8(
    buffer: &[u8],
    replacements: &[(String, String)],
) -> Result<Vec<u8>> {
    let mapping: HashMap<&str, &str> = replacements
        .iter()
        .map(|(from, to)| (from.as_str(), to.as_str()))
        .collect();
    let entries = class_utf8_entries(buffer)?;
    let mut out = Vec::with_capacity(buffer.len());
    let mut cursor = 0usize;
    let mut changed = 0usize;
    for entry in &entries {
        let Some(replacement) = mapping.get(entry.text.as_str()) else {
            continue;
        };
        if *replacement == entry.text {
            continue;
        }
        let encoded = replacement.as_bytes();
        if encoded.len() > 65_535 {
            return Err(TranslateError::new(
                TranslateErrorCode::Config,
                "replacement class text is too long",
            ));
        }
        out.extend_from_slice(&buffer[cursor..entry.start]);
        out.push(1);
        out.extend_from_slice(&(encoded.len() as u16).to_be_bytes());
        out.extend_from_slice(encoded);
        cursor = entry.end;
        changed += 1;
    }
    if changed == 0 {
        return Ok(buffer.to_vec());
    }
    out.extend_from_slice(&buffer[cursor..]);
    Ok(out)
}

/// 计算输入 jar 的 sha256（断点匹配用）。
pub fn input_file_hash(path: &Path) -> Result<String> {
    input_file_hash_cancellable(path, &AtomicBool::new(false))
}

/// 同 [`input_file_hash`]，每读完一块数据检查一次 `cancel`。
pub fn input_file_hash_cancellable(
    path: &Path,
    cancel: &AtomicBool,
) -> Result<String> {
    let mut file = std::fs::File::open(path).map_err(|error| {
        TranslateError::io("unable to open input JAR for hashing", error)
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 1 << 20];
    loop {
        check_analysis_cancel(cancel)?;
        let read =
            std::io::Read::read(&mut file, &mut buffer).map_err(|error| {
                TranslateError::io("unable to hash input JAR", error)
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn check_analysis_cancel(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err(TranslateError::new(
            TranslateErrorCode::Cancelled,
            "analysis was canceled",
        ));
    }
    Ok(())
}

/// 完整解包 + 分析，返回 (工作区目录, 分析结果)。解包时、各阶段逐个
/// 文件检查 `cancel`；取消时删除已解包的工作区。
pub fn extract_and_inspect_cancellable(
    input_path: &Path,
    workspace_root: &Path,
    cancel: &AtomicBool,
) -> Result<(PathBuf, JarInspection)> {
    let mut directory = None;
    let result = extract_and_inspect_inner(
        input_path,
        workspace_root,
        cancel,
        &mut directory,
    );
    if result.is_err()
        && cancel.load(Ordering::Relaxed)
        && let Some(directory) = directory
    {
        let _ = std::fs::remove_dir_all(directory);
    }
    result
}

fn extract_and_inspect_inner(
    input_path: &Path,
    workspace_root: &Path,
    cancel: &AtomicBool,
    created: &mut Option<PathBuf>,
) -> Result<(PathBuf, JarInspection)> {
    check_analysis_cancel(cancel)?;
    let absolute_input = input_path.canonicalize().map_err(|error| {
        TranslateError::io("unable to resolve input JAR path", error)
    })?;
    std::fs::create_dir_all(workspace_root).map_err(|error| {
        TranslateError::io("unable to create mod translation workspace", error)
    })?;
    let directory = tempfile::Builder::new()
        .prefix("job-")
        .tempdir_in(workspace_root)
        .map_err(|error| {
            TranslateError::io("unable to create job workspace", error)
        })?;
    let directory_path = directory.keep();
    *created = Some(directory_path.clone());

    let extraction = jar::extract_archive(
        &absolute_input,
        &directory_path,
        &ExtractionLimits::default(),
        cancel,
    )?;
    check_analysis_cancel(cancel)?;

    let metadata = detect_metadata(&directory_path);
    let standard = find_standard_language_sources(&directory_path, cancel)?;
    check_analysis_cancel(cancel)?;
    let standard_set: HashSet<String> = standard
        .iter()
        .map(|source| source.source_path.to_ascii_lowercase())
        .collect();
    let structured =
        discover_structured_sources(&directory_path, &standard_set, cancel)?;
    let known_set: HashSet<String> = standard
        .iter()
        .chain(structured.iter())
        .map(|source| source.source_path.to_ascii_lowercase())
        .collect();
    check_analysis_cancel(cancel)?;
    let localized =
        discover_localized_text_sources(&directory_path, &known_set, cancel)?;
    check_analysis_cancel(cancel)?;
    let mut language_sources = standard;
    language_sources.extend(structured);
    language_sources.extend(localized);
    let resource_coverage =
        build_resource_coverage(&directory_path, &language_sources, cancel)?;
    check_analysis_cancel(cancel)?;
    let class_candidates = discover_class_candidates(&directory_path, cancel)?;
    check_analysis_cancel(cancel)?;
    let inferred_mod_ids = language_sources
        .iter()
        .map(|source| source.namespace.clone())
        .collect::<Vec<_>>();
    let mut mod_ids: Vec<String> = metadata.mod_ids.clone();
    for id in inferred_mod_ids {
        if !mod_ids.contains(&id) {
            mod_ids.push(id);
        }
    }
    let original_filename = absolute_input
        .file_name()
        .and_then(|value| value.to_str())
        .and_then(|value| clean_metadata_text(value, 240))
        .unwrap_or_else(|| "mod.jar".to_string());
    let mut warnings = Vec::new();
    if metadata.loader == Loader::Unknown {
        warnings.push("未识别模组加载器，将按通用资源结构处理".to_string());
    }
    if !language_sources.iter().any(|source| {
        source.kind == LanguageKind::Json && !source.is_structured_json()
    }) {
        warnings.push("没有找到标准 en_us.json 语言文件".to_string());
    }
    if resource_coverage
        .iter()
        .any(|item| item.disposition == "unknown")
    {
        warnings.push("发现尚未进入翻译工作图的自然语言资源，交付前必须完成诊断或明确排除".to_string());
    }
    if extraction.signed {
        warnings
            .push("检测到 JAR 数字签名，当前版本不会修改该文件".to_string());
    }
    warnings.extend(discover_external_ui_warnings(&directory_path, cancel)?);
    check_analysis_cancel(cancel)?;

    let inspection = JarInspection {
        input_path: absolute_input,
        original_filename,
        loader: metadata.loader,
        mod_ids,
        project_names: metadata.project_names,
        mod_version: metadata.mod_version,
        minecraft_version_range: metadata.minecraft_version_range,
        contained_mods: metadata.contained_mods,
        signed: extraction.signed,
        total_entries: extraction.total_entries,
        uncompressed_bytes: extraction.uncompressed_bytes,
        language_sources,
        class_candidates,
        resource_coverage,
        warnings,
    };
    Ok((directory_path, inspection))
}

/// 汇总给前端分析命令用的摘要。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageSourceSummary {
    pub kind: String,
    pub namespace: String,
    pub source_path: String,
    pub target_path: String,
    pub entries: usize,
    pub characters: usize,
    pub required: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassCandidateSummary {
    pub id: String,
    pub path: String,
    pub text: String,
    pub occurrences: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisSummary {
    pub loader: String,
    pub mod_ids: Vec<String>,
    pub project_names: Vec<String>,
    pub mod_version: Option<String>,
    pub minecraft_version_range: Option<String>,
    pub signed: bool,
    pub warnings: Vec<String>,
    pub language_sources: Vec<LanguageSourceSummary>,
    pub language_entries: usize,
    pub language_characters: usize,
    pub required_entries: usize,
    pub class_candidates: Vec<ClassCandidateSummary>,
    pub quote: Quote,
}

pub fn summarize_inspection(inspection: &JarInspection) -> AnalysisSummary {
    let mut language_sources = Vec::new();
    let mut language_entries = 0usize;
    let mut language_characters = 0usize;
    let mut required_entries = 0usize;
    for source in &inspection.language_sources {
        let required = source.required_keys();
        let characters = source
            .entries
            .values()
            .map(|value| value.chars().count())
            .sum::<usize>();
        language_entries += source.entries.len();
        language_characters += characters;
        required_entries += required.len();
        language_sources.push(LanguageSourceSummary {
            kind: match source.kind {
                LanguageKind::Json => {
                    if source.is_structured_json() {
                        "structured"
                    } else {
                        "json"
                    }
                }
                LanguageKind::KeyValue => "properties",
                LanguageKind::FreeText => "free_text",
            }
            .to_string(),
            namespace: source.namespace.clone(),
            source_path: source.source_path.clone(),
            target_path: source.target_path.clone(),
            entries: source.entries.len(),
            characters,
            required: required.len(),
        });
    }
    let class_characters = inspection
        .class_candidates
        .iter()
        .map(|candidate| candidate.text.chars().count())
        .sum::<usize>();
    let quote = quote_translation_metrics(
        required_entries,
        language_characters,
        inspection.class_candidates.len(),
        class_characters,
    );
    AnalysisSummary {
        loader: inspection.loader.as_str().to_string(),
        mod_ids: inspection.mod_ids.clone(),
        project_names: inspection.project_names.clone(),
        mod_version: inspection.mod_version.clone(),
        minecraft_version_range: inspection.minecraft_version_range.clone(),
        signed: inspection.signed,
        warnings: inspection.warnings.clone(),
        language_sources,
        language_entries,
        language_characters,
        required_entries,
        class_candidates: inspection
            .class_candidates
            .iter()
            .map(|candidate| ClassCandidateSummary {
                id: candidate.id.clone(),
                path: candidate.path.clone(),
                text: candidate.text.clone(),
                occurrences: candidate.occurrences,
            })
            .collect(),
        quote,
    }
}

/// 从持久化工作区重新载入结构化模板（检查点里不存模板本体）。
pub fn reload_structured_templates(
    workspace: &Path,
    sources: &mut [LanguageSource],
) {
    for source in sources {
        if source.is_structured_json()
            && source.structured_template.is_none()
            && let Ok(raw) =
                std::fs::read_to_string(workspace.join(&source.source_path))
        {
            source.structured_template = Some(raw);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_formula_matches_spec() {
        let quote = quote_translation_metrics(80, 2000, 20, 1000);
        assert_eq!(quote.points, 10 + (2 + 2) * 2 + 3);
        assert_eq!(quote.characters, 3000);
        assert_eq!(quote.entries, 100);
        // token 预估：80 条语言 2 批 + 20 个 class 候选 2 批
        assert_eq!(quote.language_batches, 2);
        assert_eq!(quote.class_batches, 2);
        assert!(quote.estimated_tokens > 0);
        assert!(quote.estimated_calls >= 4);
    }

    #[test]
    fn structured_flatten_uses_pointers() {
        let value: serde_json::Value = serde_json::json!({
            "title": "Hello World",
            "nested": { "array": ["First item", "minecraft:iron_ingot"] }
        });
        let mut flat = BTreeMap::new();
        flatten_structured_text(&value, "", &mut flat);
        assert_eq!(flat.get("/title").map(String::as_str), Some("Hello World"));
        assert_eq!(
            flat.get("/nested/array/0").map(String::as_str),
            Some("First item")
        );
        assert!(flat.get("/nested/array/1").is_none());
    }

    #[test]
    fn partial_free_text_only_requires_untranslated_lines() {
        let source = LanguageSource {
            kind: LanguageKind::FreeText,
            namespace: "demo".to_string(),
            source_path: "docs/readme.txt".to_string(),
            target_path: "docs/readme_zh_cn.txt".to_string(),
            entries: BTreeMap::from([
                ("line:0".to_string(), "First line".to_string()),
                ("line:1".to_string(), "Second line".to_string()),
            ]),
            existing_target: BTreeMap::from([(
                "line:0".to_string(),
                "第一行".to_string(),
            )]),
            structured_template: None,
            localized_layout: None,
        };
        assert_eq!(source.required_keys(), vec!["line:1"]);
    }

    #[test]
    fn class_candidate_filtering_skips_identifiers() {
        assert!(!likely_visible_class_text("minecraft:iron_ingot"));
        assert!(!likely_visible_class_text("Item_Registry"));
        assert!(!likely_visible_class_text("https://example.com"));
        assert!(!likely_visible_class_text("%s: %s%s%s - %s: %s"));
        assert!(!likely_visible_class_text("x: %d, y: %d, z: %d"));
        assert!(!likely_visible_class_text("%s[Litematica]%s %s C#: %d"));
        assert!(!likely_visible_class_text("信息 HUD 距屏幕边缘的 Y 偏移"));
        assert!(likely_visible_class_text("Iron Ingot"));
        assert!(likely_visible_class_text("Blocks: %d"));
        assert!(likely_visible_class_text("Welcome to the server"));
    }

    #[test]
    fn deterministic_class_exclusions_cover_internal_xaero_diagnostics() {
        let cases = [
            (
                "xaero/map/file/MapProcessor.class",
                "IOException trying to detect map files!",
            ),
            (
                "xaero/map/file/MapProcessor.class",
                "Failed to convert file to outdated! Retrying...",
            ),
            ("xaero/map/region/MapRegion.class", "Region failed to load:"),
            (
                "xaero/map/graphics/Framebuffer.class",
                "glCheckFramebufferStatus returned unknown status:",
            ),
            (
                "xaero/map/file/MapSaveLoad.class",
                r"^r\.(-{0,1}[0-9]+)\.(-{0,1}[0-9]+)\.mc[ar]$",
            ),
            ("xaero/map/file/MapSaveLoad.class", "cache file"),
            (
                "xaero/map/file/MapSaveLoad.class",
                " save using an older version of Xaero's World Map!",
            ),
            (
                "xaero/map/world/MapWorld.class",
                "IO exception while trying to load",
            ),
            ("xaero/map/world/MapWorld.class", "Requesting load for:"),
            (
                "xaero/map/world/MapWorld.class",
                "IO exception while loading world map dimension config. Retrying...",
            ),
            (
                "xaero/map/file/MapExporter.class",
                "IO exception while exporting PNG:",
            ),
            (
                "xaero/map/WorldMapSession.class",
                "New world map session initialized!",
            ),
            (
                "xaero/map/file/MapSaveLoad.class",
                "IOException trying to detect map layers!",
            ),
            (
                "xaero/map/world/MapWorld.class",
                "Cancelled loading an empty region:",
            ),
            (
                "xaero/map/server/ServerData.class",
                "dev.ftb.mods.ftbteams.FTBTeamsAPI",
            ),
        ];
        for (path, text) in cases {
            let candidate = ClassCandidate {
                id: text.to_string(),
                path: path.to_string(),
                paths: vec![path.to_string()],
                occurrences: 1,
                text: text.to_string(),
            };
            assert!(
                deterministic_class_exclusion_reason(&candidate).is_some(),
                "expected deterministic exclusion for {text}"
            );
        }
    }

    #[test]
    fn deterministic_class_exclusions_preserve_player_ui_text() {
        for (path, text) in [
            (
                "xaero/map/gui/GuiMap.class",
                "Failed to load your map. Retry?",
            ),
            ("xaero/map/tooltip/MapTooltip.class", "Cache file location"),
            (
                "xaero/map/config/ConfigScreen.class",
                "Save using an older version",
            ),
        ] {
            let candidate = ClassCandidate {
                id: text.to_string(),
                path: path.to_string(),
                paths: vec![path.to_string()],
                occurrences: 1,
                text: text.to_string(),
            };
            assert_eq!(deterministic_class_exclusion_reason(&candidate), None);
        }
    }

    #[test]
    fn camel_case_config_identifiers_are_distinguished_from_player_text() {
        assert!(is_camel_case_config_identifier("areaSelectionsPerWorld"));
        assert!(is_camel_case_config_identifier("easyPlaceMode"));
        assert!(!is_camel_case_config_identifier("placementrestriction"));
        assert!(!is_camel_case_config_identifier("RESET"));
        assert!(!is_camel_case_config_identifier("minecraft:stick"));
    }

    #[test]
    fn malilib_boundaries_are_reported_instead_of_counted_as_complete() {
        let dir = tempfile::tempdir().unwrap();
        let class_path = dir
            .path()
            .join("fi/dy/masa/litematica/config/Configs$Generic.class");
        std::fs::create_dir_all(class_path.parent().unwrap()).unwrap();
        std::fs::write(&class_path, malilib_config_class_fixture()).unwrap();

        let warnings =
            discover_external_ui_warnings(dir.path(), &AtomicBool::new(false))
                .unwrap();
        assert!(warnings.iter().any(|warning| {
            warning.contains("areaSelectionsPerWorld")
                && warning.contains("prettyName")
        }));
        assert!(warnings.iter().any(|warning| {
            warning.contains("RESET") && warning.contains("true/false")
        }));
    }

    #[test]
    fn metadata_detects_fabric_manifest() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("fabric.mod.json"),
            r#"{"id":"demo","name":"Demo Mod","version":"1.0.0","depends":{"minecraft":"~1.20"}}"#,
        )
        .unwrap();
        let metadata = detect_metadata(dir.path());
        assert_eq!(metadata.loader, Loader::Fabric);
        assert_eq!(metadata.mod_ids, vec!["demo"]);
        assert_eq!(metadata.project_names, vec!["Demo Mod"]);
        assert_eq!(metadata.minecraft_version_range.as_deref(), Some("~1.20"));
    }

    #[test]
    fn class_replacement_preserves_structure() {
        let bytes = tiny_class_fixture();
        let before = class_utf8_entries(&bytes).unwrap();
        let target = before
            .iter()
            .find(|entry| entry.text == "Hello")
            .expect("fixture should contain Hello");
        let replacement = vec![(target.text.clone(), "你好".to_string())];
        let rewritten = replace_class_utf8(&bytes, &replacement).unwrap();
        let after = class_utf8_entries(&rewritten).unwrap();
        assert_eq!(before.len(), after.len());
        assert!(after.iter().any(|entry| entry.text == "你好"));
        assert!(rewritten.starts_with(&[0xca, 0xfe, 0xba, 0xbe]));
        // 结构不变：类名/字段/方法计数在常量池后保持原样
        let before_tail = &bytes[before[0].start..];
        let after_tail = &rewritten[after[0].start..];
        assert!(
            before_tail.len() != after_tail.len() || before_tail == after_tail
        );
    }

    #[test]
    fn analysis_canceled_while_unpacking_removes_its_workspace() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("mod.jar");
        let mut writer =
            zip::ZipWriter::new(std::fs::File::create(&input).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        writer
            .start_file("assets/x/lang/en_us.json", options)
            .unwrap();
        writer.write_all(b"{\"a\":\"A\"}").unwrap();
        writer.start_file("data.bin", options).unwrap();
        writer.write_all(&jar::tests::noise(3 << 20)).unwrap();
        writer.finish().unwrap();
        let workspace_root = dir.path().join("workspaces");

        jar::CANCEL_AFTER_CHUNKS.with(|chunks| chunks.set(Some(2)));
        let cancel = AtomicBool::new(false);
        let result =
            extract_and_inspect_cancellable(&input, &workspace_root, &cancel);
        jar::CANCEL_AFTER_CHUNKS.with(|chunks| chunks.set(None));

        let error = result.expect_err("the analysis must stop while unpacking");
        assert_eq!(error.code, TranslateErrorCode::Cancelled);
        assert_eq!(std::fs::read_dir(&workspace_root).unwrap().count(), 0);
    }

    /// 手工构造一个最小合法 class 文件（只有常量池 + 空类体）。
    fn tiny_class_fixture() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 0x3d]);
        // constant_pool_count = 5（索引 1-4，索引 0 保留）
        bytes.extend_from_slice(&[0, 5]);
        // entry 1: Utf8 "Hello"
        let hello = b"Hello";
        bytes.extend_from_slice(&[1, 0, hello.len() as u8]);
        bytes.extend_from_slice(hello);
        // entry 2: Utf8 "World"
        let world = b"World";
        bytes.extend_from_slice(&[1, 0, world.len() as u8]);
        bytes.extend_from_slice(world);
        // entry 3: Class -> name_index 1
        bytes.extend_from_slice(&[7, 0, 1]);
        // entry 4: Class -> name_index 2
        bytes.extend_from_slice(&[7, 0, 2]);
        bytes.extend_from_slice(&[0, 1]); // access_flags ACC_PUBLIC
        bytes.extend_from_slice(&[0, 3]); // this_class
        bytes.extend_from_slice(&[0, 4]); // super_class
        bytes.extend_from_slice(&[0, 0]); // interfaces_count
        bytes.extend_from_slice(&[0, 0]); // fields_count
        bytes.extend_from_slice(&[0, 0]); // methods_count
        bytes.extend_from_slice(&[0, 0]); // attributes_count
        bytes
    }

    fn malilib_config_class_fixture() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 0x34]);
        bytes.extend_from_slice(&[0, 9]);
        for value in [
            "fi/dy/masa/litematica/config/Configs$Generic",
            "java/lang/Object",
            "fi/dy/masa/malilib/config/options/ConfigBoolean",
            "areaSelectionsPerWorld",
        ] {
            bytes.push(1);
            bytes.extend_from_slice(&(value.len() as u16).to_be_bytes());
            bytes.extend_from_slice(value.as_bytes());
        }
        bytes.extend_from_slice(&[7, 0, 1]);
        bytes.extend_from_slice(&[7, 0, 2]);
        bytes.extend_from_slice(&[7, 0, 3]);
        bytes.extend_from_slice(&[8, 0, 4]);
        bytes.extend_from_slice(&[0, 1]);
        bytes.extend_from_slice(&[0, 5]);
        bytes.extend_from_slice(&[0, 6]);
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&[0, 0]);
        bytes
    }
}
