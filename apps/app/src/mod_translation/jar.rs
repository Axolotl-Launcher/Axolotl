use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};
use zip::ZipArchive;
use zip::write::SimpleFileOptions;

use crate::mod_translation::analyze::check_analysis_cancel;
use crate::mod_translation::error::{
    Result, TranslateError, TranslateErrorCode,
};

pub const ARCHIVE_MANIFEST: &str = ".mod-translator-archive-manifest.json";
pub const ARCHIVE_FILES_DIRECTORY: &str = ".mod-translator-archive-files";

const WINDOWS_RESERVED_SEGMENT_RE: &str =
    r"^(?:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\..*)?$";

/// 解包时的资源上限，防止恶意压缩包把磁盘/内存打爆。
#[derive(Debug, Clone)]
pub struct ExtractionLimits {
    pub max_archive_bytes: u64,
    pub max_entries: usize,
    pub max_entry_bytes: u64,
    pub max_uncompressed_bytes: u64,
    pub max_compression_ratio: f64,
}

impl Default for ExtractionLimits {
    fn default() -> Self {
        Self {
            max_archive_bytes: 1_200 * 1024 * 1024,
            max_entries: 200_000,
            max_entry_bytes: 256 * 1024 * 1024,
            max_uncompressed_bytes: 2_000 * 1024 * 1024,
            max_compression_ratio: 200.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveManifest {
    pub version: u32,
    pub entries: Vec<ArchiveEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveEntry {
    pub archive_path: String,
    pub workspace_path: String,
}

impl ArchiveManifest {
    pub fn read(directory: &Path) -> Option<Self> {
        let path = directory.join(ARCHIVE_MANIFEST);
        let content = std::fs::read_to_string(path).ok()?;
        let parsed: ArchiveManifest = serde_json::from_str(&content).ok()?;
        if parsed.version != 1 {
            return None;
        }
        Some(parsed)
    }

    pub fn write(&self, directory: &Path) -> Result<()> {
        let content = format!(
            "{}\n",
            serde_json::to_string_pretty(self).map_err(|error| {
                TranslateError::config(format!(
                    "manifest serialization: {error}"
                ))
            })?
        );
        std::fs::write(directory.join(ARCHIVE_MANIFEST), content).map_err(
            |error| {
                TranslateError::io("unable to write archive manifest", error)
            },
        )
    }
}

/// 归档路径的安全策略，集中一处方便单测。
pub struct PathPolicy;

impl PathPolicy {
    /// 规范化条目名并拒绝一切可能逃出工作区的写法。
    pub fn safe_entry_name(raw: &str) -> Result<String> {
        let name = raw.replace('\\', "/");
        if name.contains('\0')
            || name.starts_with('/')
            || name.starts_with("//")
            || looks_like_drive_prefix(&name)
            || name.split('/').any(|segment| segment == "..")
        {
            return Err(TranslateError::new(
                TranslateErrorCode::UnsafeArchivePath,
                format!("archive contains an unsafe path: {raw}"),
            ));
        }
        Ok(name)
    }

    #[cfg(test)]
    pub fn is_symlink(unix_mode: u32) -> bool {
        (unix_mode & 0o170000) == 0o120000
    }

    pub fn is_signature_file(name: &str) -> bool {
        let lower = name.to_ascii_lowercase();
        lower.starts_with("meta-inf/")
            && lower
                .rsplit_once('/')
                .map(|(_, tail)| {
                    tail.ends_with(".sf")
                        || tail.ends_with(".rsa")
                        || tail.ends_with(".dsa")
                        || tail.ends_with(".ec")
                })
                .unwrap_or(false)
    }

    /// Windows 上不可移植的段：非法字符、结尾点/空格、保留设备名。
    pub fn requires_portable_mapping(name: &str) -> bool {
        let reserved = regex::Regex::new(WINDOWS_RESERVED_SEGMENT_RE).unwrap();
        name.split('/').any(|segment| {
            segment.is_empty()
                || segment.chars().any(|character| {
                    matches!(
                        character,
                        '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\0'
                            ..='\u{001f}'
                    )
                })
                || segment.ends_with('.')
                || segment.ends_with(' ')
                || reserved.is_match(segment)
        })
    }

    pub fn portable_path_key(name: &str) -> String {
        name.split('/')
            .map(|segment| {
                segment.trim_end_matches(['.', ' ']).to_ascii_uppercase()
            })
            .collect::<Vec<_>>()
            .join("/")
    }

    pub fn mapped_workspace_path(index: usize, archive_path: &str) -> String {
        let extension = Path::new(archive_path)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .take(16)
            .collect::<String>();
        format!("{ARCHIVE_FILES_DIRECTORY}/{index:06}{extension}")
    }

    /// 解析工作区相对路径并保证不越界。
    pub fn workspace_path(
        workspace: &Path,
        requested: &str,
    ) -> Result<PathBuf> {
        let normalized = requested.replace('\\', "/");
        let mut result = workspace.to_path_buf();
        for segment in normalized.split('/') {
            if segment.is_empty() || segment == "." {
                continue;
            }
            if segment == ".." {
                return Err(TranslateError::new(
                    TranslateErrorCode::UnsafeArchivePath,
                    "workspace path escapes the task workspace",
                ));
            }
            result.push(segment);
        }
        if result != workspace && !result.starts_with(workspace) {
            return Err(TranslateError::new(
                TranslateErrorCode::UnsafeArchivePath,
                "workspace path escapes the task workspace",
            ));
        }
        Ok(result)
    }
}

fn looks_like_drive_prefix(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[derive(Debug)]
pub struct ExtractionResult {
    pub signed: bool,
    pub total_entries: u64,
    pub uncompressed_bytes: u64,
}

/// 把 input_path 里每个条目解到 workspace（目录需已存在）。
/// 每个条目开始前、每写入一块数据前检查 `cancel`，单个大条目也能中途停止；
/// 取消时不写清单。
pub fn extract_archive(
    input_path: &Path,
    workspace: &Path,
    limits: &ExtractionLimits,
    cancel: &AtomicBool,
) -> Result<ExtractionResult> {
    let metadata = std::fs::metadata(input_path).map_err(|error| {
        TranslateError::io("unable to stat input JAR", error)
    })?;
    if !metadata.is_file() {
        return Err(TranslateError::new(
            TranslateErrorCode::InvalidArchive,
            "input path is not a file",
        ));
    }
    if metadata.len() > limits.max_archive_bytes {
        return Err(TranslateError::new(
            TranslateErrorCode::InvalidArchive,
            "JAR file exceeds the safe size limit",
        ));
    }

    let file = File::open(input_path).map_err(|error| {
        TranslateError::io("unable to open input JAR", error)
    })?;
    let mut archive = ZipArchive::new(file).map_err(|error| {
        TranslateError::with_source(
            TranslateErrorCode::InvalidArchive,
            "the file is not a valid JAR/ZIP archive",
            error,
        )
    })?;

    let mut signed = false;
    let mut total_entries = 0u64;
    let mut uncompressed_bytes = 0u64;
    let mut manifest = ArchiveManifest {
        version: 1,
        entries: Vec::with_capacity(archive.len().min(100_000)),
    };
    let mut seen: HashSet<String> = HashSet::new();
    let mut portable_seen: HashSet<String> = HashSet::new();
    let mut buffer = vec![0; 1 << 20];
    #[cfg(test)]
    let mut written_chunks = 0usize;

    for index in 0..archive.len() {
        check_analysis_cancel(cancel)?;
        let mut entry = archive.by_index(index).map_err(|error| {
            TranslateError::with_source(
                TranslateErrorCode::InvalidArchive,
                format!("unable to read archive entry {index}"),
                error,
            )
        })?;
        let raw_name = entry
            .name()
            .map_err(|error| {
                TranslateError::with_source(
                    TranslateErrorCode::InvalidArchive,
                    format!("unable to read archive entry name {index}"),
                    error,
                )
            })?
            .to_string();
        let name = PathPolicy::safe_entry_name(&raw_name)?;
        if !seen.insert(name.clone()) {
            return Err(TranslateError::new(
                TranslateErrorCode::UnsafeArchivePath,
                format!("archive contains a duplicate entry: {name}"),
            ));
        }
        total_entries += 1;
        if total_entries > limits.max_entries as u64 {
            return Err(TranslateError::new(
                TranslateErrorCode::InvalidArchive,
                "JAR entry count exceeds the safety limit",
            ));
        }
        let entry_size = entry.size();
        uncompressed_bytes = uncompressed_bytes.saturating_add(entry_size);
        if entry_size > limits.max_entry_bytes {
            return Err(TranslateError::new(
                TranslateErrorCode::InvalidArchive,
                format!("JAR entry is too large: {name}"),
            ));
        }
        if uncompressed_bytes > limits.max_uncompressed_bytes {
            return Err(TranslateError::new(
                TranslateErrorCode::InvalidArchive,
                "JAR expands beyond the safety limit",
            ));
        }
        if entry.compressed_size() > 0
            && entry_size as f64 / entry.compressed_size() as f64
                > limits.max_compression_ratio
        {
            return Err(TranslateError::new(
                TranslateErrorCode::InvalidArchive,
                format!("JAR entry has an abnormal compression ratio: {name}"),
            ));
        }
        if entry.is_symlink() {
            return Err(TranslateError::new(
                TranslateErrorCode::UnsafeArchivePath,
                format!("JAR contains a symbolic link: {name}"),
            ));
        }
        if PathPolicy::is_signature_file(&name) {
            signed = true;
        }

        if name.ends_with('/') {
            continue;
        }

        let portable_key = PathPolicy::portable_path_key(&name);
        let must_map = PathPolicy::requires_portable_mapping(&name)
            || portable_seen.contains(&portable_key);
        portable_seen.insert(portable_key);
        let workspace_name = if must_map {
            PathPolicy::mapped_workspace_path(manifest.entries.len(), &name)
        } else {
            name.clone()
        };

        let output_path =
            PathPolicy::workspace_path(workspace, &workspace_name)?;
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                TranslateError::io(
                    format!("unable to create workspace directory for {name}"),
                    error,
                )
            })?;
        }
        let mut output = File::create(&output_path).map_err(|error| {
            TranslateError::io(
                format!("unable to create workspace file for {name}"),
                error,
            )
        })?;
        loop {
            check_analysis_cancel(cancel)?;
            let read = match entry.read(&mut buffer) {
                Ok(read) => read,
                Err(error)
                    if error.kind() == std::io::ErrorKind::Interrupted =>
                {
                    continue;
                }
                Err(error) => {
                    return Err(TranslateError::io(
                        format!("unable to extract {name}"),
                        error,
                    ));
                }
            };
            if read == 0 {
                break;
            }
            output.write_all(&buffer[..read]).map_err(|error| {
                TranslateError::io(format!("unable to extract {name}"), error)
            })?;
            #[cfg(test)]
            {
                written_chunks += 1;
                cancel_after_chunks(written_chunks, cancel);
            }
        }
        manifest.entries.push(ArchiveEntry {
            archive_path: name,
            workspace_path: workspace_name,
        });
    }

    manifest.write(workspace)?;
    Ok(ExtractionResult {
        signed,
        total_entries,
        uncompressed_bytes,
    })
}

#[cfg(test)]
thread_local! {
    /// 测试用：写入这么多块数据后置位取消标志，用来取消正在进行的解包。
    pub(crate) static CANCEL_AFTER_CHUNKS: std::cell::Cell<Option<usize>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(test)]
fn cancel_after_chunks(written: usize, cancel: &AtomicBool) {
    if CANCEL_AFTER_CHUNKS.with(std::cell::Cell::get) == Some(written) {
        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// 递归收集工作区相对路径。
pub fn collect_files(root: &Path) -> Result<Vec<String>> {
    collect_files_cancellable(root, &AtomicBool::new(false))
}

/// 同 [`collect_files`]，每个条目检查一次 `cancel`。
pub fn collect_files_cancellable(
    root: &Path,
    cancel: &AtomicBool,
) -> Result<Vec<String>> {
    let mut result = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory).map_err(|error| {
            TranslateError::io(
                format!("unable to read workspace directory {directory:?}"),
                error,
            )
        })?;
        for entry in entries {
            check_analysis_cancel(cancel)?;
            let entry = entry.map_err(|error| {
                TranslateError::io(
                    format!("unable to read workspace entry in {directory:?}"),
                    error,
                )
            })?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| {
                TranslateError::io(
                    format!("unable to stat {}", path.display()),
                    error,
                )
            })?;
            if file_type.is_dir() {
                pending.push(path);
                continue;
            }
            if file_type.is_file() {
                let relative = path.strip_prefix(root).map_err(|_| {
                    TranslateError::config("workspace path prefix error")
                })?;
                result.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    result.sort();
    Ok(result)
}

const INTERNAL_FILES: &[&str] = &[
    ".mod-translator-archive-manifest.json",
    ".mod-translator-checkpoint.json",
    ".mod-translator-resume.json",
    ".mod-translator-performance.json",
    ".mod-translator-events.ndjson",
    ".mod-translator-resource-coverage.json",
    ".mod-translator-agent-findings.json",
    ".mod-translator-agent-report.json",
];

fn is_internal_file(name: &str) -> bool {
    INTERNAL_FILES.iter().any(|internal| {
        name == *internal || name.starts_with(".mod-translator-")
    })
}

/// 把工作区重新打成 JAR，用 manifest 还原原始条目名；新生成的文件按相对路径加进去，
/// `.mod-translator-*` 内部文件一律不打进去。
pub fn package_archive(
    workspace: &Path,
    output_path: &Path,
    manifest: &ArchiveManifest,
) -> Result<()> {
    if let Some(parent) = output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|error| {
            TranslateError::io("unable to create output directory", error)
        })?;
    }
    if output_path.exists() {
        return Err(TranslateError::new(
            TranslateErrorCode::Config,
            format!("output file already exists: {}", output_path.display()),
        ));
    }

    let temporary = output_path.with_extension("jar.partial");
    let _ = std::fs::remove_file(&temporary);
    let file = File::create(&temporary).map_err(|error| {
        TranslateError::io("unable to create temporary output JAR", error)
    })?;
    let mut writer = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for entry in &manifest.entries {
        let source =
            PathPolicy::workspace_path(workspace, &entry.workspace_path)?;
        if !source.is_file() {
            continue;
        }
        let bytes = std::fs::read(&source).map_err(|error| {
            TranslateError::io(
                format!("unable to read workspace file {}", source.display()),
                error,
            )
        })?;
        writer
            .start_file(&entry.archive_path, options)
            .map_err(|error| {
                TranslateError::with_source(
                    TranslateErrorCode::Io,
                    format!(
                        "unable to write archive entry {}",
                        entry.archive_path
                    ),
                    error,
                )
            })?;
        writer.write_all(&bytes).map_err(|error| {
            TranslateError::io(
                format!("unable to write archive entry {}", entry.archive_path),
                error,
            )
        })?;
    }

    let mapped_files: HashSet<String> = manifest
        .entries
        .iter()
        .map(|entry| entry.workspace_path.to_ascii_lowercase())
        .collect();
    for relative in collect_files(workspace)? {
        if relative == ARCHIVE_MANIFEST
            || mapped_files.contains(&relative.to_ascii_lowercase())
            || is_internal_file(&relative)
        {
            continue;
        }
        let bytes =
            std::fs::read(workspace.join(&relative)).map_err(|error| {
                TranslateError::io(
                    format!("unable to read workspace file {relative}"),
                    error,
                )
            })?;
        writer.start_file(&relative, options).map_err(|error| {
            TranslateError::with_source(
                TranslateErrorCode::Io,
                format!("unable to write archive entry {relative}"),
                error,
            )
        })?;
        writer.write_all(&bytes).map_err(|error| {
            TranslateError::io(
                format!("unable to write archive entry {relative}"),
                error,
            )
        })?;
    }

    let mut writer = writer.finish().map_err(|error| {
        TranslateError::with_source(
            TranslateErrorCode::Io,
            "unable to finalize output JAR",
            error,
        )
    })?;
    writer.flush().map_err(|error| {
        TranslateError::io("unable to flush output JAR", error)
    })?;
    drop(writer);

    std::fs::rename(&temporary, output_path).map_err(|error| {
        TranslateError::io("unable to move temporary JAR into place", error)
    })?;
    Ok(())
}

/// 校验路径只有普通段（前端给的输出路径先过这一关）。
pub fn is_clean_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && path.components().all(|component| {
            matches!(
                component,
                Component::Normal(_)
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn build_zip(
        dir: &Path,
        name: &str,
        entries: &[(&str, &[u8], Option<u32>)],
    ) -> PathBuf {
        let path = dir.join(name);
        let file = File::create(&path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        for (name, data, mode) in entries {
            let mut options = SimpleFileOptions::default();
            if let Some(mode) = mode {
                options = options.unix_permissions(*mode);
            }
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
        path
    }

    #[test]
    fn unsafe_paths_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let input = build_zip(
            dir.path(),
            "malicious.jar",
            &[
                ("../evil.txt", b"evil", None),
                ("C:/evil.txt", b"evil", None),
                ("/absolute.txt", b"evil", None),
            ],
        );
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let error = extract_archive(
            &input,
            &workspace,
            &ExtractionLimits::default(),
            &AtomicBool::new(false),
        )
        .expect_err("malicious archive must be rejected");
        assert_eq!(error.code, TranslateErrorCode::UnsafeArchivePath);
    }

    #[test]
    fn nul_bytes_are_rejected_by_path_policy() {
        assert!(PathPolicy::safe_entry_name("a\0b").is_err());
        assert!(PathPolicy::safe_entry_name("..\\..\\escape").is_err());
        assert_eq!(PathPolicy::safe_entry_name("a\\b.txt").unwrap(), "a/b.txt");
        assert!(PathPolicy::safe_entry_name("normal.txt").is_ok());
    }

    #[test]
    fn symlink_mode_bits_are_detected() {
        assert!(PathPolicy::is_symlink(0o120777));
        assert!(!PathPolicy::is_symlink(0o100644));
        assert!(!PathPolicy::is_symlink(0o040755));
    }

    #[test]
    fn signature_files_flag_signed_archives() {
        let dir = tempfile::tempdir().unwrap();
        let input = build_zip(
            dir.path(),
            "signed.jar",
            &[
                ("assets/x/lang/en_us.json", b"{}", None),
                ("META-INF/MOD.SF", b"META-INF/MANIFEST.MF", None),
                ("META-INF/MOD.RSA", b"sig", None),
            ],
        );
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let result = extract_archive(
            &input,
            &workspace,
            &ExtractionLimits::default(),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(result.signed);
        // 打包本身不拒绝签名模组，拒绝发生在编排层（SignedModRefused）。
        let manifest = ArchiveManifest::read(&workspace).unwrap();
        assert_eq!(manifest.entries.len(), 3);
    }

    /// 不可压缩的数据，免得触发压缩比限制。
    pub(crate) fn noise(length: usize) -> Vec<u8> {
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        (0..length)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u8
            })
            .collect()
    }

    #[test]
    fn canceled_extraction_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let input = build_zip(
            dir.path(),
            "mod.jar",
            &[("a.txt", b"a", None), ("b.txt", b"b", None)],
        );
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let error = extract_archive(
            &input,
            &workspace,
            &ExtractionLimits::default(),
            &AtomicBool::new(true),
        )
        .expect_err("a canceled extraction must stop");
        assert_eq!(error.code, TranslateErrorCode::Cancelled);
        assert!(ArchiveManifest::read(&workspace).is_none());
        assert_eq!(std::fs::read_dir(&workspace).unwrap().count(), 0);
    }

    #[test]
    fn extraction_stops_inside_a_large_entry_once_canceled() {
        let dir = tempfile::tempdir().unwrap();
        let large = noise(3 << 20);
        let input = build_zip(
            dir.path(),
            "mod.jar",
            &[("large.bin", &large, None), ("after.txt", b"after", None)],
        );
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        CANCEL_AFTER_CHUNKS.with(|chunks| chunks.set(Some(1)));
        let cancel = AtomicBool::new(false);
        let result = extract_archive(
            &input,
            &workspace,
            &ExtractionLimits::default(),
            &cancel,
        );
        CANCEL_AFTER_CHUNKS.with(|chunks| chunks.set(None));
        let error = result.expect_err("the extraction must stop mid-entry");
        assert_eq!(error.code, TranslateErrorCode::Cancelled);
        let written = std::fs::metadata(workspace.join("large.bin"))
            .unwrap()
            .len();
        assert!(written < large.len() as u64, "wrote {written} bytes");
        assert!(!workspace.join("after.txt").exists());
        assert!(ArchiveManifest::read(&workspace).is_none());
    }

    #[test]
    fn canceled_workspace_walk_stops() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        let error =
            collect_files_cancellable(dir.path(), &AtomicBool::new(true))
                .expect_err("a canceled walk must stop");
        assert_eq!(error.code, TranslateErrorCode::Cancelled);
        assert_eq!(collect_files(dir.path()).unwrap(), vec!["a.txt"]);
    }

    #[test]
    fn package_restores_original_names_and_skips_internal_files() {
        let dir = tempfile::tempdir().unwrap();
        let input = build_zip(
            dir.path(),
            "plain.jar",
            &[
                ("assets/x/lang/en_us.json", b"{\"a\":\"A\"}", None),
                ("META-INF/mods.toml", b"modId = \"x\"", None),
            ],
        );
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let result = extract_archive(
            &input,
            &workspace,
            &ExtractionLimits::default(),
            &AtomicBool::new(false),
        )
        .unwrap();
        let _ = result;
        // 模拟翻译产物 + 内部文件
        std::fs::write(
            workspace.join("assets/x/lang/zh_cn.json"),
            "{\"a\":\"甲\"}\n",
        )
        .unwrap();
        std::fs::write(workspace.join(".mod-translator-checkpoint.json"), "{}")
            .unwrap();
        let output = dir.path().join("out-zh_cn.jar");
        let manifest = ArchiveManifest::read(&workspace).unwrap();
        package_archive(&workspace, &output, &manifest).unwrap();

        let file = File::open(&output).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        assert!(archive.by_name("assets/x/lang/en_us.json").is_ok());
        assert!(archive.by_name("assets/x/lang/zh_cn.json").is_ok());
        assert!(archive.by_name("META-INF/mods.toml").is_ok());
        assert!(archive.by_name(".mod-translator-checkpoint.json").is_err());
    }
}
