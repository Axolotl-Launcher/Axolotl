use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use zhconv::{Variant, zhconv};

const WIKI_ENTRIES_DATA: &str = include_str!("content_search/WikiEntries.txt");
const SEARCHER_WORDS_DATA: &str =
    include_str!("content_search/searcher_words.txt");
const RADIX_DIGITS: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz/+=!?@#$%^&*()[]{}<>;:',";
const MAX_MATCHES: usize = 100;
const MIN_SIMILARITY: f64 = 0.25;
/// Compact queries shorter than this are never worth segmenting; they are
/// either a single known word or too ambiguous to split safely.
const MIN_COMPACT_QUERY_LENGTH: usize = 6;

static WIKI_ENTRIES: LazyLock<Vec<WikiEntry>> =
    LazyLock::new(|| parse_wiki_entries(WIKI_ENTRIES_DATA));

/// Segmentation dictionary, sorted longest-first so greedy prefix matching
/// prefers the longest known word at every step.
static SEARCHER_WORDS: LazyLock<Vec<String>> = LazyLock::new(|| {
    let mut words = SEARCHER_WORDS_DATA
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter(|line| {
            line.chars()
                .all(|character| character.is_ascii_alphabetic())
        })
        .map(str::to_lowercase)
        .collect::<Vec<_>>();
    words.sort();
    words.dedup();
    words.sort_by(|left, right| right.len().cmp(&left.len()));
    words
});

static CHINESE_NAME_INDEX: LazyLock<ChineseNameIndex> =
    LazyLock::new(|| build_chinese_name_index(&WIKI_ENTRIES));

static WIKI_ID_INDEX: LazyLock<WikiIdIndex> =
    LazyLock::new(|| build_wiki_id_index(&WIKI_ENTRIES));

#[derive(Clone, Debug)]
struct WikiEntry {
    /// MC 百科 (mcmod.cn) class ID, encoded as the entry's 1-based line
    /// number in `WikiEntries.txt`; empty lines reserve the IDs of entries
    /// without a platform slug.
    wiki_id: u32,
    variant_index: usize,
    chinese_name: Option<String>,
    curseforge_slug: Option<String>,
    modrinth_slug: Option<String>,
    popularity: u32,
}

#[derive(Clone, Copy)]
enum Platform {
    CurseForge,
    Modrinth,
}

#[derive(Clone, Debug)]
struct SearchMatch<'a> {
    entry: &'a WikiEntry,
    similarity: f64,
    exact: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChineseSearchTranslation {
    pub chinese_name: String,
    pub curseforge_slug: Option<String>,
    pub modrinth_slug: Option<String>,
    pub match_score: f64,
    pub exact: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChineseSearchResolution {
    pub is_chinese: bool,
    pub normalized_query: String,
    pub curseforge_query: Option<String>,
    pub modrinth_query: Option<String>,
    pub modrinth_slugs: Vec<String>,
    pub translations: Vec<ChineseSearchTranslation>,
}

#[derive(Debug, Default)]
struct ChineseNameIndex {
    modrinth: HashMap<String, String>,
    curseforge: HashMap<String, String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChineseNameLookup {
    pub modrinth: HashMap<String, String>,
    pub curseforge: HashMap<String, String>,
}

/// Batch-resolves Chinese display names for known platform slugs, keyed by
/// each input slug exactly as it was passed in.
pub fn lookup_chinese_content_names(
    modrinth_slugs: &[String],
    curseforge_slugs: &[String],
) -> ChineseNameLookup {
    ChineseNameLookup {
        modrinth: collect_chinese_names(
            modrinth_slugs,
            &CHINESE_NAME_INDEX.modrinth,
        ),
        curseforge: collect_chinese_names(
            curseforge_slugs,
            &CHINESE_NAME_INDEX.curseforge,
        ),
    }
}

fn collect_chinese_names(
    slugs: &[String],
    index: &HashMap<String, String>,
) -> HashMap<String, String> {
    slugs
        .iter()
        .filter_map(|slug| {
            index
                .get(&slug.to_lowercase())
                .map(|name| (slug.clone(), name.clone()))
        })
        .collect()
}

#[derive(Debug, Default)]
struct WikiIdIndex {
    modrinth: HashMap<String, u32>,
    curseforge: HashMap<String, u32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WikiIdLookup {
    pub modrinth: HashMap<String, u32>,
    pub curseforge: HashMap<String, u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentIdentityCounterpart {
    pub provider: String,
    pub project_id: String,
    pub slug: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentIdentityRecord {
    pub key: String,
    pub counterparts: Vec<ContentIdentityCounterpart>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentIdentityLookup {
    pub modrinth: HashMap<String, Vec<ContentIdentityRecord>>,
    pub curseforge: HashMap<String, Vec<ContentIdentityRecord>>,
}

/// Resolves curated cross-platform identities for project slugs.
///
/// Only entries that contain both a CurseForge and a Modrinth slug are
/// returned. A slug may have more than one candidate identity; callers must
/// treat that result as ambiguous instead of hard-blocking an installation.
pub fn lookup_content_identities(
    modrinth_slugs: &[String],
    curseforge_slugs: &[String],
) -> ContentIdentityLookup {
    let requested_modrinth = modrinth_slugs
        .iter()
        .map(|slug| (slug.to_lowercase(), slug.clone()))
        .collect::<HashMap<_, _>>();
    let requested_curseforge = curseforge_slugs
        .iter()
        .map(|slug| (slug.to_lowercase(), slug.clone()))
        .collect::<HashMap<_, _>>();
    let mut modrinth = HashMap::<String, Vec<ContentIdentityRecord>>::new();
    let mut curseforge = HashMap::<String, Vec<ContentIdentityRecord>>::new();

    for entry in WIKI_ENTRIES.iter() {
        let (Some(curseforge_slug), Some(modrinth_slug)) = (
            entry.curseforge_slug.as_deref(),
            entry.modrinth_slug.as_deref(),
        ) else {
            continue;
        };
        let requested_mr =
            requested_modrinth.get(&modrinth_slug.to_lowercase());
        let requested_cf =
            requested_curseforge.get(&curseforge_slug.to_lowercase());
        if requested_mr.is_none() && requested_cf.is_none() {
            continue;
        }
        let key = format!("mapping:{}:{}", entry.wiki_id, entry.variant_index);
        let counterparts = vec![
            ContentIdentityCounterpart {
                provider: "curseforge".to_string(),
                project_id: curseforge_slug.to_string(),
                slug: curseforge_slug.to_string(),
            },
            ContentIdentityCounterpart {
                provider: "modrinth".to_string(),
                project_id: modrinth_slug.to_string(),
                slug: modrinth_slug.to_string(),
            },
        ];
        let record = ContentIdentityRecord {
            key: key.clone(),
            counterparts,
        };
        if let Some(original) = requested_mr {
            push_identity_record(&mut modrinth, original, &record);
        }
        if let Some(original) = requested_cf {
            push_identity_record(&mut curseforge, original, &record);
        }
    }

    ContentIdentityLookup {
        modrinth,
        curseforge,
    }
}

fn push_identity_record(
    index: &mut HashMap<String, Vec<ContentIdentityRecord>>,
    slug: &str,
    record: &ContentIdentityRecord,
) {
    let records = index.entry(slug.to_string()).or_default();
    if records.iter().any(|candidate| candidate.key == record.key) {
        return;
    }
    records.push(record.clone());
    records.sort_by(|left, right| left.key.cmp(&right.key));
}

/// Batch-resolves MC 百科 (mcmod.cn) class IDs for known platform slugs,
/// keyed by each input slug exactly as it was passed in.
pub fn lookup_content_wiki_ids(
    modrinth_slugs: &[String],
    curseforge_slugs: &[String],
) -> WikiIdLookup {
    WikiIdLookup {
        modrinth: collect_wiki_ids(modrinth_slugs, &WIKI_ID_INDEX.modrinth),
        curseforge: collect_wiki_ids(
            curseforge_slugs,
            &WIKI_ID_INDEX.curseforge,
        ),
    }
}

fn collect_wiki_ids(
    slugs: &[String],
    index: &HashMap<String, u32>,
) -> HashMap<String, u32> {
    slugs
        .iter()
        .filter_map(|slug| {
            index
                .get(&slug.to_lowercase())
                .map(|wiki_id| (slug.clone(), *wiki_id))
        })
        .collect()
}

fn build_wiki_id_index(entries: &[WikiEntry]) -> WikiIdIndex {
    let mut modrinth = HashMap::<String, (u32, u32)>::new();
    let mut curseforge = HashMap::<String, (u32, u32)>::new();
    for entry in entries {
        for (slug, index) in [
            (entry.modrinth_slug.as_deref(), &mut modrinth),
            (entry.curseforge_slug.as_deref(), &mut curseforge),
        ] {
            let Some(slug) = slug else {
                continue;
            };
            let key = slug.to_lowercase();
            let is_better = index
                .get(&key)
                .is_none_or(|(_, popularity)| *popularity < entry.popularity);
            if is_better {
                index.insert(key, (entry.wiki_id, entry.popularity));
            }
        }
    }
    WikiIdIndex {
        modrinth: modrinth
            .into_iter()
            .map(|(key, (wiki_id, _))| (key, wiki_id))
            .collect(),
        curseforge: curseforge
            .into_iter()
            .map(|(key, (wiki_id, _))| (key, wiki_id))
            .collect(),
    }
}

fn build_chinese_name_index(entries: &[WikiEntry]) -> ChineseNameIndex {
    let mut modrinth = HashMap::<String, (String, u32)>::new();
    let mut curseforge = HashMap::<String, (String, u32)>::new();
    for entry in entries {
        let Some(chinese_name) = &entry.chinese_name else {
            continue;
        };
        if chinese_name.is_empty() {
            continue;
        }
        for (slug, index) in [
            (entry.modrinth_slug.as_deref(), &mut modrinth),
            (entry.curseforge_slug.as_deref(), &mut curseforge),
        ] {
            let Some(slug) = slug else {
                continue;
            };
            let key = slug.to_lowercase();
            let is_better = index
                .get(&key)
                .is_none_or(|(_, popularity)| *popularity < entry.popularity);
            if is_better {
                index.insert(key, (chinese_name.clone(), entry.popularity));
            }
        }
    }
    ChineseNameIndex {
        modrinth: modrinth
            .into_iter()
            .map(|(key, (name, _))| (key, name))
            .collect(),
        curseforge: curseforge
            .into_iter()
            .map(|(key, (name, _))| (key, name))
            .collect(),
    }
}

const MAX_LOCALIZED_FILE_NAME_BYTES: usize = 200;

/// Resolves the Chinese title used for `[中文名]` file-name prefixes from a
/// Modrinth project slug.
pub fn chinese_file_title_for_modrinth_slug(slug: &str) -> Option<String> {
    CHINESE_NAME_INDEX
        .modrinth
        .get(&slug.to_lowercase())
        .and_then(|name| chinese_file_title(name))
}

/// Resolves the Chinese title used for `[中文名]` file-name prefixes from a
/// CurseForge project slug.
pub fn chinese_file_title_for_curseforge_slug(slug: &str) -> Option<String> {
    CHINESE_NAME_INDEX
        .curseforge
        .get(&slug.to_lowercase())
        .and_then(|name| chinese_file_title(name))
}

/// Prefixes a downloaded content file name with `[chinese_title]`.
///
/// Returns `None` whenever renaming would be ambiguous or unsafe (the name
/// already carries a `[` prefix, the title has no Chinese characters, or the
/// result would exceed file-system name limits); callers must then keep the
/// original name.
pub fn localized_content_file_name(
    file_name: &str,
    chinese_title: &str,
) -> Option<String> {
    if file_name.is_empty() || file_name.starts_with('[') {
        return None;
    }
    if chinese_title.is_empty()
        || !contains_chinese(chinese_title)
        || chinese_title.contains(['[', ']', '/', '\\'])
    {
        return None;
    }
    let localized = format!("[{chinese_title}]{file_name}");
    (localized.len() <= MAX_LOCALIZED_FILE_NAME_BYTES).then_some(localized)
}

/// Strips the `[中文名]` marker this app prepends to downloaded content
/// files. Names without such a marker are returned unchanged.
pub fn original_content_file_name(file_name: &str) -> &str {
    let Some(rest) = file_name.strip_prefix('[') else {
        return file_name;
    };
    let Some((title, original)) = rest.split_once(']') else {
        return file_name;
    };
    if original.is_empty() || !contains_chinese(title) {
        return file_name;
    }
    original
}

/// Applies `[chinese_title]` to the file-name segment of an instance-relative
/// path, keeping the directory prefix untouched.
pub fn localized_content_relative_path(
    relative_path: &str,
    chinese_title: &str,
) -> Option<String> {
    let (directory, file_name) = match relative_path.rsplit_once('/') {
        Some((directory, file_name)) => (Some(directory), file_name),
        None => (None, relative_path),
    };
    let localized = localized_content_file_name(file_name, chinese_title)?;
    Some(match directory {
        Some(directory) => format!("{directory}/{localized}"),
        None => localized,
    })
}

/// Strips the `[中文名]` marker from the file-name segment of an
/// instance-relative path, e.g. when exporting a modpack.
pub fn original_content_relative_path(relative_path: &str) -> String {
    match relative_path.rsplit_once('/') {
        Some((directory, file_name)) => {
            format!("{directory}/{}", original_content_file_name(file_name))
        }
        None => original_content_file_name(relative_path).to_string(),
    }
}

fn chinese_file_title(chinese_name: &str) -> Option<String> {
    const ILLEGAL_CHARACTERS: &str = r#"/\:*?"<>|[]"#;
    let stripped = strip_english_alias(chinese_name);
    let sanitized = stripped
        .chars()
        .filter(|character| {
            !character.is_control() && !ILLEGAL_CHARACTERS.contains(*character)
        })
        .collect::<String>();
    let trimmed = sanitized.trim();
    (!trimmed.is_empty() && contains_chinese(trimmed))
        .then(|| trimmed.to_string())
}

/// Removes the trailing ` (English Name)` alias that wiki entries append to
/// their Chinese names, mirroring the frontend `bilingualTitle` helper.
fn strip_english_alias(name: &str) -> &str {
    let trimmed = name.trim_end();
    let Some(without_paren) = trimmed.strip_suffix(')') else {
        return trimmed;
    };
    let Some(open_index) = without_paren.rfind('(') else {
        return trimmed;
    };
    let inner = &without_paren[open_index + 1..];
    if inner.contains(['(', ')'])
        || !inner
            .chars()
            .any(|character| character.is_ascii_alphabetic())
    {
        return trimmed;
    }
    let prefix = &without_paren[..open_index];
    let stripped = prefix.trim_end();
    if stripped.is_empty() || stripped.len() == prefix.len() {
        return trimmed;
    }
    stripped
}

fn contains_chinese(value: &str) -> bool {
    value
        .chars()
        .any(|character| ('\u{4e00}'..='\u{9fbb}').contains(&character))
}

pub fn resolve_chinese_content_search(query: &str) -> ChineseSearchResolution {
    let lowercase_query = query.trim().to_lowercase();
    let normalized_query = zhconv(&lowercase_query, Variant::ZhCN);
    let is_chinese = contains_chinese(&normalized_query);

    if normalized_query.is_empty() || !is_chinese {
        return ChineseSearchResolution {
            is_chinese,
            normalized_query,
            ..ChineseSearchResolution::default()
        };
    }

    let curseforge_matches =
        search_entries(&WIKI_ENTRIES, &normalized_query, Platform::CurseForge);
    let modrinth_matches =
        search_entries(&WIKI_ENTRIES, &normalized_query, Platform::Modrinth);

    let curseforge_query = select_curseforge_query(&curseforge_matches);
    let modrinth_query =
        select_modrinth_query(&modrinth_matches, &normalized_query);
    let modrinth_slugs = modrinth_matches
        .iter()
        .filter_map(|result| result.entry.modrinth_slug.clone())
        .take(MAX_MATCHES)
        .collect();

    let mut translations = Vec::new();
    let mut seen = HashSet::new();
    for result in curseforge_matches.iter().chain(modrinth_matches.iter()) {
        let entry = result.entry;
        let key = (
            entry.curseforge_slug.as_deref(),
            entry.modrinth_slug.as_deref(),
        );
        if !seen.insert(key) {
            continue;
        }
        let Some(chinese_name) = &entry.chinese_name else {
            continue;
        };
        translations.push(ChineseSearchTranslation {
            chinese_name: chinese_name.clone(),
            curseforge_slug: entry.curseforge_slug.clone(),
            modrinth_slug: entry.modrinth_slug.clone(),
            match_score: result.similarity,
            exact: result.exact,
        });
    }

    ChineseSearchResolution {
        is_chinese,
        normalized_query,
        curseforge_query,
        modrinth_query,
        modrinth_slugs,
        translations,
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentSearchExpansion {
    /// Dictionary-based segmentation of a compact (separator-free) query,
    /// e.g. `sodiumextra` → `sodium extra`. `None` when the query is already
    /// a single known word, cannot be fully segmented, or is too short.
    pub suggested_split: Option<String>,
}

/// Expands a browse search query with the bundled segmentation dictionary.
///
/// Server-side search (Modrinth, CurseForge) tokenizes on whitespace and
/// separators, so a query typed without spaces (`sodiumextra`) cannot match a
/// project named "Sodium Extra". This resolves such queries to their
/// space-separated form whenever the whole query can be reconstructed from
/// known words; ambiguity (`xyzabc`) yields no suggestion.
pub fn expand_content_search_query(query: &str) -> ContentSearchExpansion {
    let compact = query
        .chars()
        .filter(|character| character.is_alphanumeric())
        .collect::<String>()
        .to_lowercase();
    ContentSearchExpansion {
        suggested_split: segment_compact_query(&compact),
    }
}

fn segment_compact_query(compact: &str) -> Option<String> {
    if compact.len() < MIN_COMPACT_QUERY_LENGTH
        || SEARCHER_WORDS.iter().any(|word| word == compact)
    {
        return None;
    }
    let mut parts = Vec::new();
    let mut rest = compact;
    while !rest.is_empty() {
        let Some((word, remainder)) = longest_prefix_word(rest) else {
            return None;
        };
        parts.push(word);
        rest = remainder;
    }
    if parts.len() < 2 || parts.iter().any(|part| part.len() < 2) {
        return None;
    }
    Some(parts.join(" "))
}

fn longest_prefix_word(value: &str) -> Option<(&str, &str)> {
    SEARCHER_WORDS.iter().find_map(|word| {
        value.strip_prefix(word).map(|rest| (word.as_str(), rest))
    })
}

fn parse_wiki_entries(source: &str) -> Vec<WikiEntry> {
    let mut lines = source.lines().collect::<Vec<_>>();
    let Some(popularity_data) = lines.pop() else {
        return Vec::new();
    };
    let popularities = popularity_data
        .as_bytes()
        .chunks_exact(3)
        .map(decode_popularity)
        .collect::<Vec<_>>();
    let mut popularity_iter = popularities.into_iter();
    let mut results = Vec::new();

    for (line_index, line) in lines.into_iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        let wiki_id = line_index as u32 + 1;
        let popularity = popularity_iter.next().unwrap_or_default();
        for (variant_index, raw_entry) in line.split('¨').enumerate() {
            let mut parts = raw_entry.split('|');
            let slugs = parts.next().unwrap_or_default();
            let final_part = parts.next_back();
            let (curseforge_slug, modrinth_slug) = parse_slugs(slugs);
            let chinese_name = final_part.map(|name| {
                if name.contains('*') {
                    let english_name = curseforge_slug
                        .as_deref()
                        .or(modrinth_slug.as_deref())
                        .unwrap_or_default()
                        .replace('-', " ")
                        .split_whitespace()
                        .map(capitalize)
                        .collect::<Vec<_>>()
                        .join(" ");
                    name.replace('*', &format!(" ({english_name})"))
                } else {
                    name.to_string()
                }
            });
            results.push(WikiEntry {
                wiki_id,
                variant_index,
                chinese_name,
                curseforge_slug,
                modrinth_slug,
                popularity,
            });
        }
    }

    results
}

fn parse_slugs(slugs: &str) -> (Option<String>, Option<String>) {
    if let Some(modrinth) = slugs.strip_prefix('@') {
        return (None, non_empty(modrinth));
    }
    if let Some(shared) = slugs.strip_suffix('@') {
        let shared = non_empty(shared);
        return (shared.clone(), shared);
    }
    if let Some((curseforge, modrinth)) = slugs.split_once('@') {
        return (non_empty(curseforge), non_empty(modrinth));
    }
    (non_empty(slugs), None)
}

fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

fn capitalize(value: &str) -> String {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => String::new(),
    }
}

fn decode_popularity(chunk: &[u8]) -> u32 {
    chunk.iter().fold(0_u32, |value, character| {
        let digit = RADIX_DIGITS
            .as_bytes()
            .iter()
            .position(|candidate| candidate == character)
            .unwrap_or_default() as u32;
        value.saturating_mul(86).saturating_add(digit)
    })
}

fn platform_slug(entry: &WikiEntry, platform: Platform) -> Option<&str> {
    match platform {
        Platform::CurseForge => entry.curseforge_slug.as_deref(),
        Platform::Modrinth => entry.modrinth_slug.as_deref(),
    }
}

fn search_sources(
    entry: &WikiEntry,
    platform: Platform,
) -> Vec<(Vec<String>, f64)> {
    let Some(slug) = platform_slug(entry, platform) else {
        return Vec::new();
    };
    match &entry.chinese_name {
        Some(chinese_name) => {
            let primary = chinese_name
                .split_once(" (")
                .map_or(chinese_name.as_str(), |(name, _)| name);
            vec![
                (
                    primary
                        .split('/')
                        .map(|alias| zhconv(alias, Variant::ZhCN))
                        .collect(),
                    1.0,
                ),
                (vec![format!("{chinese_name}{slug}")], 0.5),
            ]
        }
        None => vec![(vec![slug.to_string()], 0.5)],
    }
}

fn search_entries<'a>(
    entries: &'a [WikiEntry],
    query: &str,
    platform: Platform,
) -> Vec<SearchMatch<'a>> {
    let query_parts = query.split_whitespace().collect::<Vec<_>>();
    let mut candidates = entries
        .iter()
        .filter(|entry| platform_slug(entry, platform).is_some())
        .filter_map(|entry| {
            let sources = search_sources(entry, platform);
            let similarity = weighted_similarity(&sources, query);
            let exact = query_parts.iter().all(|query_part| {
                let query_part = query_part.to_lowercase();
                sources.iter().any(|(aliases, _)| {
                    aliases.iter().any(|alias| {
                        alias
                            .replace(' ', "")
                            .to_lowercase()
                            .contains(&query_part)
                    })
                })
            });
            (exact || similarity >= MIN_SIMILARITY).then_some(SearchMatch {
                entry,
                similarity,
                exact,
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .exact
            .cmp(&left.exact)
            .then_with(|| right.similarity.total_cmp(&left.similarity))
            .then_with(|| right.entry.popularity.cmp(&left.entry.popularity))
            .then_with(|| {
                platform_slug(left.entry, platform)
                    .cmp(&platform_slug(right.entry, platform))
            })
    });

    let mut fuzzy_count = 0;
    candidates
        .into_iter()
        .filter(|candidate| {
            if candidate.exact {
                true
            } else if fuzzy_count < MAX_MATCHES {
                fuzzy_count += 1;
                true
            } else {
                false
            }
        })
        .collect()
}

fn weighted_similarity(sources: &[(Vec<String>, f64)], query: &str) -> f64 {
    let total_weight = sources.iter().map(|(_, weight)| weight).sum::<f64>();
    if total_weight == 0.0 {
        return 0.0;
    }
    sources
        .iter()
        .map(|(aliases, weight)| {
            aliases
                .iter()
                .map(|alias| search_similarity(alias, query))
                .fold(0.0, f64::max)
                * weight
        })
        .sum::<f64>()
        / total_weight
}

fn search_similarity(source: &str, query: &str) -> f64 {
    if source.is_empty() || query.is_empty() {
        return 0.0;
    }
    let mut source_chars = source
        .to_lowercase()
        .chars()
        .filter(|character| *character != ' ')
        .collect::<Vec<_>>();
    let query_chars = query
        .to_lowercase()
        .chars()
        .filter(|character| *character != ' ')
        .collect::<Vec<_>>();
    let source_length = source_chars.len();
    let query_length = query_chars.len();
    if query_length == 0 {
        return 0.0;
    }

    let mut query_position = 0;
    let mut length_sum = 0.0;
    while query_position < query_length {
        let mut source_position = 0;
        let mut max_length = 0;
        let mut max_position = 0;
        while source_position < source_chars.len() {
            let mut length = 0;
            while query_position + length < query_length
                && source_position + length < source_chars.len()
                && source_chars[source_position + length]
                    == query_chars[query_position + length]
            {
                length += 1;
            }
            if length > max_length {
                max_length = length;
                max_position = source_position;
            }
            source_position += if length > 0 { length } else { 1 };
        }
        if max_length > 0 {
            source_chars.drain(max_position..max_position + max_length);
            let mut weight = 1.4_f64.powi(3 + max_length as i32) - 3.6;
            let distance = query_position.abs_diff(max_position) as f64;
            weight *= 1.0 + 0.3 * (3.0 - distance).max(0.0);
            length_sum += weight;
        }
        query_position += if max_length > 0 { max_length } else { 1 };
    }

    (length_sum / query_length as f64)
        * (3.0 / (source_length as f64 + 15.0).sqrt())
        * if query_length <= 2 {
            (3 - query_length) as f64
        } else {
            1.0
        }
}

fn select_curseforge_query(matches: &[SearchMatch<'_>]) -> Option<String> {
    let first = matches.first()?;
    let eligible = if first.exact {
        matches
            .iter()
            .filter(|result| result.exact)
            .collect::<Vec<_>>()
    } else {
        matches
            .iter()
            .filter(|result| result.similarity == first.similarity)
            .collect::<Vec<_>>()
    };
    let target = eligible.into_iter().max_by(|left, right| {
        left.entry
            .popularity
            .cmp(&right.entry.popularity)
            .then_with(|| {
                left.entry
                    .curseforge_slug
                    .cmp(&right.entry.curseforge_slug)
                    .reverse()
            })
    })?;
    let words = extract_words(target.entry, Platform::CurseForge);
    (!words.is_empty()).then(|| words.join(" "))
}

fn select_modrinth_query(
    matches: &[SearchMatch<'_>],
    query: &str,
) -> Option<String> {
    let mut word_weights = HashMap::<String, f64>::new();
    for result in matches {
        let sources = search_sources(result.entry, Platform::Modrinth);
        let exact_alias = sources.iter().any(|(aliases, _)| {
            aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(query))
        });
        let similarity = if exact_alias {
            1000.0
        } else {
            result.similarity
        };
        for word in extract_words(result.entry, Platform::Modrinth) {
            *word_weights.entry(word).or_default() +=
                similarity * result.entry.popularity as f64;
        }
    }
    word_weights
        .into_iter()
        .max_by(|(left_word, left_weight), (right_word, right_weight)| {
            left_weight
                .total_cmp(right_weight)
                .then_with(|| right_word.cmp(left_word))
        })
        .map(|(word, _)| word)
}

fn extract_words(entry: &WikiEntry, platform: Platform) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(slug) = platform_slug(entry, platform) {
        candidates.push(slug.replace(['-', '/'], " "));
    }
    if let Some(chinese_name) = &entry.chinese_name {
        let parenthetical = chinese_name
            .rsplit_once(" (")
            .map_or(chinese_name.as_str(), |(_, suffix)| suffix)
            .trim_end_matches([')', ' ']);
        candidates.push(
            parenthetical
                .split_once(" - ")
                .map_or(parenthetical, |(prefix, _)| prefix)
                .replace(['-', '/', ':', '(', ')'], " "),
        );
    }
    let stop_words = [
        "the", "of", "mod", "and", "forge", "fabric", "for", "quilt",
        "neoforge",
    ];
    let mut seen = HashSet::new();
    let mut words = candidates
        .iter()
        .flat_map(|candidate| candidate.split_whitespace())
        .map(|word| {
            word.trim_start_matches(['{', '[', '('])
                .trim_end_matches(['}', ']', ')'])
                .to_lowercase()
        })
        .filter(|word| {
            word.len() > 1
                && !stop_words.contains(&word.as_str())
                && !word.chars().all(|character| character.is_ascii_digit())
                && word.is_ascii()
        })
        .filter(|word| seen.insert(word.clone()))
        .collect::<Vec<_>>();
    let all_words = words.clone();
    words.retain(|word| {
        !all_words.iter().any(|prefix| {
            prefix.len() < word.len()
                && word.starts_with(prefix)
                && can_form_from_words(&word[prefix.len()..], &all_words)
        })
    });
    words
}

fn can_form_from_words(value: &str, words: &[String]) -> bool {
    value.is_empty()
        || words.iter().any(|word| {
            value == word
                || value.strip_prefix(word).is_some_and(|remainder| {
                    can_form_from_words(remainder, words)
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_platform_slugs_aliases_and_star_names() {
        let entries =
            parse_wiki_entries("curse@modrinth|中文*¨@only-mr|别名/又名\n001");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].curseforge_slug.as_deref(), Some("curse"));
        assert_eq!(entries[0].modrinth_slug.as_deref(), Some("modrinth"));
        assert_eq!(entries[0].chinese_name.as_deref(), Some("中文 (Curse)"));
        assert_eq!(entries[0].popularity, 1);
        assert_eq!(entries[0].wiki_id, 1);
        assert_eq!(entries[1].curseforge_slug, None);
        assert_eq!(entries[1].modrinth_slug.as_deref(), Some("only-mr"));
        assert_eq!(entries[1].wiki_id, 1);
    }

    #[test]
    fn empty_lines_reserve_wiki_ids_and_keep_popularity_aligned() {
        let entries = parse_wiki_entries("first@|甲\n\nthird@|乙\n001002");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].wiki_id, 1);
        assert_eq!(entries[0].popularity, 1);
        assert_eq!(entries[1].wiki_id, 3);
        assert_eq!(entries[1].popularity, 2);
    }

    #[test]
    fn decodes_base_86_popularity() {
        assert_eq!(decode_popularity(b"001"), 1);
        assert_eq!(decode_popularity(b"010"), 86);
    }

    #[test]
    fn resolves_simplified_and_traditional_queries() {
        let simplified = resolve_chinese_content_search("应用能源2");
        let traditional = resolve_chinese_content_search("應用能源2");
        let twilight_forest = resolve_chinese_content_search("暮色森林");
        assert!(simplified.is_chinese);
        assert_eq!(traditional.normalized_query, simplified.normalized_query);
        assert!(simplified.modrinth_slugs.iter().any(|slug| slug == "ae2"));
        assert!(
            simplified
                .translations
                .iter()
                .any(|translation| translation.curseforge_slug.as_deref()
                    == Some("applied-energistics-2"))
        );
        assert!(
            twilight_forest
                .translations
                .iter()
                .any(|translation| translation.curseforge_slug.as_deref()
                    == Some("the-twilight-forest"))
        );
        assert_eq!(
            twilight_forest.curseforge_query.as_deref(),
            Some("twilight forest")
        );
    }

    #[test]
    fn leaves_non_chinese_and_unknown_queries_without_alternatives() {
        let english = resolve_chinese_content_search("Applied Energistics");
        assert!(!english.is_chinese);
        assert!(english.modrinth_query.is_none());

        let unknown =
            resolve_chinese_content_search("龘靐齉齾爩鱻麤龗灪爨癵籱");
        assert!(unknown.is_chinese);
        assert!(unknown.curseforge_query.is_none());
        assert!(unknown.modrinth_query.is_none());
    }

    #[test]
    fn looks_up_chinese_names_for_known_slugs() {
        let lookup = lookup_chinese_content_names(
            &["ae2".to_string(), "totally-unknown-project".to_string()],
            &["the-twilight-forest".to_string()],
        );
        assert_eq!(
            lookup.modrinth.get("ae2").map(String::as_str),
            Some("应用能源2 (Applied Energistics 2)")
        );
        assert!(!lookup.modrinth.contains_key("totally-unknown-project"));
        assert_eq!(
            lookup
                .curseforge
                .get("the-twilight-forest")
                .map(String::as_str),
            Some("暮色森林 (The Twilight Forest)")
        );
    }

    #[test]
    fn looks_up_chinese_names_case_insensitively_keyed_by_input() {
        let lookup = lookup_chinese_content_names(&["AE2".to_string()], &[]);
        assert_eq!(
            lookup.modrinth.get("AE2").map(String::as_str),
            Some("应用能源2 (Applied Energistics 2)")
        );
    }

    #[test]
    fn looks_up_wiki_ids_for_known_slugs() {
        let lookup = lookup_content_wiki_ids(
            &[
                "industrial-craft".to_string(),
                "totally-unknown-project".to_string(),
            ],
            &["Industrial-Craft".to_string()],
        );
        assert_eq!(lookup.modrinth.get("industrial-craft"), Some(&2));
        assert!(!lookup.modrinth.contains_key("totally-unknown-project"));
        assert_eq!(lookup.curseforge.get("Industrial-Craft"), Some(&2));
    }

    #[test]
    fn looks_up_cross_platform_content_identities() {
        let lookup = lookup_content_identities(
            &["ae2".to_string(), "unknown".to_string()],
            &["applied-energistics-2".to_string()],
        );
        let modrinth = lookup.modrinth.get("ae2").expect("Modrinth identity");
        let curseforge = lookup
            .curseforge
            .get("applied-energistics-2")
            .expect("CurseForge identity");
        assert_eq!(modrinth.len(), 1);
        assert_eq!(modrinth[0].key, curseforge[0].key);
        assert!(
            modrinth[0]
                .counterparts
                .iter()
                .any(|counterpart| counterpart.provider == "curseforge")
        );
        assert!(!lookup.modrinth.contains_key("unknown"));
    }

    #[test]
    fn cross_platform_identity_lookup_requires_both_slugs() {
        let lookup = lookup_content_identities(
            &["only-mr".to_string()],
            &["not-present".to_string()],
        );
        assert!(lookup.modrinth.is_empty());
        assert!(lookup.curseforge.is_empty());
    }

    #[test]
    fn cross_platform_identities_keep_mcmod_subvariants_separate() {
        let lookup = lookup_content_identities(
            &["itemphysic".to_string(), "itemphysic-lite".to_string()],
            &["itemphysic".to_string(), "itemphysic-lite".to_string()],
        );
        for slug in ["itemphysic", "itemphysic-lite"] {
            assert_eq!(lookup.modrinth[slug].len(), 1);
            assert_eq!(
                lookup.modrinth[slug][0].key,
                lookup.curseforge[slug][0].key
            );
        }
        assert_ne!(
            lookup.modrinth["itemphysic"][0].key,
            lookup.curseforge["itemphysic-lite"][0].key
        );
    }

    #[test]
    fn wiki_id_index_prefers_more_popular_entries_for_duplicate_slugs() {
        let entries = parse_wiki_entries("dup@|旧名\ndup@|新名\n001002");
        let index = build_wiki_id_index(&entries);
        assert_eq!(index.modrinth.get("dup"), Some(&2));
        assert_eq!(index.curseforge.get("dup"), Some(&2));
    }

    #[test]
    fn prefers_more_popular_entries_for_duplicate_slugs() {
        let entries = parse_wiki_entries("dup@|旧名\ndup@|新名\n001002");
        let index = build_chinese_name_index(&entries);
        assert_eq!(index.modrinth.get("dup").map(String::as_str), Some("新名"));
        assert_eq!(
            index.curseforge.get("dup").map(String::as_str),
            Some("新名")
        );
    }

    #[test]
    fn skips_entries_without_chinese_names_in_index() {
        let entries = parse_wiki_entries("no-name@\n001");
        let index = build_chinese_name_index(&entries);
        assert!(index.modrinth.is_empty());
        assert!(index.curseforge.is_empty());
    }

    #[test]
    fn file_titles_strip_english_aliases() {
        assert_eq!(
            chinese_file_title_for_modrinth_slug("ae2").as_deref(),
            Some("应用能源2")
        );
        assert_eq!(
            chinese_file_title_for_curseforge_slug("the-twilight-forest")
                .as_deref(),
            Some("暮色森林")
        );
        assert_eq!(chinese_file_title_for_modrinth_slug("unknown-slug"), None);
    }

    #[test]
    fn localizes_content_file_names() {
        assert_eq!(
            localized_content_file_name("sodium-0.5.8.jar", "钠").as_deref(),
            Some("[钠]sodium-0.5.8.jar")
        );
        assert_eq!(localized_content_file_name("[1.19]mod.jar", "钠"), None);
        assert_eq!(localized_content_file_name("mod.jar", "Sodium"), None);
        assert_eq!(localized_content_file_name("mod.jar", "钠]x"), None);
        assert_eq!(localized_content_file_name("", "钠"), None);
        let oversized_title = "钠".repeat(80);
        assert_eq!(
            localized_content_file_name("mod.jar", &oversized_title),
            None
        );
    }

    #[test]
    fn recovers_original_content_file_names() {
        assert_eq!(
            original_content_file_name("[钠]sodium-0.5.8.jar"),
            "sodium-0.5.8.jar"
        );
        assert_eq!(
            original_content_file_name("[1.19]mod.jar"),
            "[1.19]mod.jar"
        );
        assert_eq!(original_content_file_name("[钠]"), "[钠]");
        assert_eq!(original_content_file_name("plain.jar"), "plain.jar");
    }

    #[test]
    fn localizes_and_recovers_relative_paths() {
        assert_eq!(
            localized_content_relative_path("mods/sodium-0.5.8.jar", "钠")
                .as_deref(),
            Some("mods/[钠]sodium-0.5.8.jar")
        );
        assert_eq!(
            original_content_relative_path("mods/[钠]sodium-0.5.8.jar"),
            "mods/sodium-0.5.8.jar"
        );
        assert_eq!(
            original_content_relative_path(
                "saves/world/datapacks/[钠]pack.zip"
            ),
            "saves/world/datapacks/pack.zip"
        );
        assert_eq!(
            original_content_relative_path("mods/plain.jar"),
            "mods/plain.jar"
        );
    }

    #[test]
    fn removes_words_that_can_be_composed_from_shorter_words() {
        let entry = WikiEntry {
            wiki_id: 1,
            variant_index: 0,
            chinese_name: Some("末影接口 (Ender IO EnderIO)".to_string()),
            curseforge_slug: Some("ender-io-enderio".to_string()),
            modrinth_slug: None,
            popularity: 1,
        };
        let words = extract_words(&entry, Platform::CurseForge);
        assert!(words.contains(&"ender".to_string()));
        assert!(words.contains(&"io".to_string()));
        assert!(!words.contains(&"enderio".to_string()));
    }

    #[test]
    fn segments_compact_queries_into_known_words() {
        assert_eq!(
            expand_content_search_query("sodiumextra")
                .suggested_split
                .as_deref(),
            Some("sodium extra")
        );
        assert_eq!(
            expand_content_search_query("irisshaders")
                .suggested_split
                .as_deref(),
            Some("iris shaders")
        );
        assert_eq!(
            expand_content_search_query("examplemod")
                .suggested_split
                .as_deref(),
            Some("example mod")
        );
        assert_eq!(
            expand_content_search_query("thetwilightforest")
                .suggested_split
                .as_deref(),
            Some("the twilight forest")
        );
        assert_eq!(
            expand_content_search_query("shulkerbox")
                .suggested_split
                .as_deref(),
            Some("shulker box")
        );
    }

    #[test]
    fn leaves_known_and_unsegmentable_queries_without_a_split() {
        assert_eq!(expand_content_search_query("sodium").suggested_split, None);
        assert_eq!(expand_content_search_query("ae2").suggested_split, None);
        assert_eq!(expand_content_search_query("xyzabc").suggested_split, None);
        assert_eq!(expand_content_search_query("").suggested_split, None);
        assert_eq!(expand_content_search_query("a b").suggested_split, None);
    }

    #[test]
    fn segmentation_ignores_separators_and_case() {
        assert_eq!(
            expand_content_search_query("Sodium-Extra")
                .suggested_split
                .as_deref(),
            Some("sodium extra")
        );
        assert_eq!(
            expand_content_search_query("sodium extra")
                .suggested_split
                .as_deref(),
            Some("sodium extra")
        );
    }
}
