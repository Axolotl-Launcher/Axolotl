//! Container traversal shared by inspection and import. Nested ZIP payloads are
//! streamed to anonymous temporary files, never buffered in memory or executed.
use crate::api::drop_classifier::{
    MAX_ZIP_EXTRACTION_ENTRIES as MAX_ENTRIES, ZipEntrySet,
    is_noise_entry as noise,
};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub(super) const MAX_ARCHIVE_LAYERS: usize = 3;
const MAX_NESTED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_NESTED_TOTAL: u64 = 4 * 1024 * 1024 * 1024;

pub(super) fn read_bounded(
    mut reader: impl Read,
    limit: u64,
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("metadata-too-large".into());
    }
    Ok(bytes)
}

pub(super) fn safe_source_path(
    root: &Path,
    relative: &str,
) -> Result<PathBuf, String> {
    let path = super::super::files::safe_relative_join(root, relative)
        .map_err(|e| e.to_string())?;
    let mut current = root.to_path_buf();
    for component in Path::new(relative).components() {
        current.push(component);
        let meta =
            std::fs::symlink_metadata(&current).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() {
            return Err("symbolic-links".into());
        }
    }
    Ok(path)
}

pub(super) enum Source {
    Folder(PathBuf),
    Zip(zip::ZipArchive<std::fs::File>, ZipEntrySet),
}
impl Source {
    pub(super) fn list(&mut self, base: &str) -> Result<Vec<String>, String> {
        match self {
            Self::Folder(root) => {
                let dir = if base.is_empty() {
                    root.clone()
                } else {
                    safe_source_path(root, base)?
                };
                let mut files = Vec::new();
                for entry in
                    std::fs::read_dir(dir).map_err(|e| e.to_string())?
                {
                    let entry = entry.map_err(|e| e.to_string())?;
                    let ty = entry.file_type().map_err(|e| e.to_string())?;
                    let mut name =
                        entry.file_name().to_string_lossy().into_owned();
                    if noise(&name) {
                        continue;
                    }
                    if ty.is_symlink() {
                        continue;
                    }
                    if ty.is_dir() {
                        name.push('/');
                    }
                    files.push(name);
                    if files.len() > MAX_ENTRIES {
                        return Err("too-many-files".into());
                    }
                }
                Ok(files)
            }
            Self::Zip(_, entries) => {
                let prefix = if base.is_empty() {
                    String::new()
                } else {
                    format!("{base}/")
                };
                let mut names: Vec<String> = entries
                    .child_folders(&prefix)
                    .into_iter()
                    .map(|folder| format!("{folder}/"))
                    .collect();
                names.extend(
                    entries
                        .files
                        .iter()
                        .filter_map(|path| path.strip_prefix(&prefix))
                        .filter(|name| !name.contains('/') && !noise(name))
                        .map(str::to_string),
                );
                Ok(names)
            }
        }
    }
    pub(super) fn read(
        &mut self,
        path: &str,
        limit: u64,
    ) -> Result<Vec<u8>, String> {
        match self {
            Self::Folder(root) => read_bounded(
                std::fs::File::open(safe_source_path(root, path)?)
                    .map_err(|e| e.to_string())?,
                limit,
            ),
            Self::Zip(archive, _) => {
                let index = archive_index(archive, path)?;
                read_bounded(
                    archive.by_index(index).map_err(|e| e.to_string())?,
                    limit,
                )
            }
        }
    }
}

impl Source {
    pub(super) fn open(path: &Path) -> Result<Option<Self>, String> {
        if std::fs::symlink_metadata(path)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("symbolic-links".into());
        }
        if path.is_dir() {
            return Ok(Some(Self::Folder(path.into())));
        }
        if !path
            .extension()
            .is_some_and(|v| v.eq_ignore_ascii_case("zip"))
        {
            return Ok(None);
        }
        Self::archive(std::fs::File::open(path).map_err(|e| e.to_string())?)
    }
    fn archive(file: std::fs::File) -> Result<Option<Self>, String> {
        let Ok(mut archive) = zip::ZipArchive::new(file) else {
            return Ok(None);
        };
        let entries =
            ZipEntrySet::from_archive_with_cancellation(&mut archive, None)?;
        Ok(Some(Self::Zip(archive, entries)))
    }
    pub(super) fn nested(
        &mut self,
        path: &str,
        total: &mut u64,
    ) -> Result<Option<Self>, String> {
        if let Self::Folder(root) = self {
            return Self::archive(
                std::fs::File::open(safe_source_path(root, path)?)
                    .map_err(|e| e.to_string())?,
            );
        }
        let Self::Zip(archive, _) = self else {
            unreachable!()
        };
        validate_archive(archive)?;
        let index = archive_index(archive, path)?;
        let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
        let size = entry.size();
        if size > MAX_NESTED_BYTES
            || total.saturating_add(size) > MAX_NESTED_TOTAL
        {
            return Err(
                "Archive payload exceeds the inspection byte limit".into()
            );
        }
        if !crate::api::drop_classifier::temp_dir_has_space(
            &std::env::temp_dir(),
            size,
        ) {
            return Err(
                "Not enough free disk space to inspect archive payload".into(),
            );
        }
        let mut file = tempfile::tempfile().map_err(|e| e.to_string())?;
        let actual = std::io::copy(
            &mut entry.by_ref().take(size.min(MAX_NESTED_BYTES) + 1),
            &mut file,
        )
        .map_err(|e| e.to_string())?;
        if actual != size {
            return Err(
                "Archive payload size does not match its metadata".into()
            );
        }
        *total += actual;
        file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        Self::archive(file)
    }
    pub(super) fn select(
        path: &Path,
        archives: &[String],
    ) -> Result<Self, String> {
        if archives.len() > MAX_ARCHIVE_LAYERS {
            return Err(
                "Archive wrapping is too deep (maximum 3 ZIP layers)".into()
            );
        }
        let mut source = Self::open(path)?.ok_or("Unsupported container")?;
        let mut total = 0;
        for archive in archives {
            source = source
                .nested(archive, &mut total)?
                .ok_or("Invalid archive container")?;
        }
        Ok(source)
    }
}
fn archive_index(
    archive: &mut zip::ZipArchive<std::fs::File>,
    path: &str,
) -> Result<usize, String> {
    (0..archive.len())
        .find(|&i| {
            archive.by_index_raw(i).ok().is_some_and(|e| {
                crate::api::pack::detect::decode_zip_entry_name(e.name_raw())
                    .replace('\\', "/")
                    == path
            })
        })
        .ok_or_else(|| "missing-file".into())
}

/// Import rejects unsafe entries rather than silently omitting source content.
pub(super) fn validate_archive(
    archive: &mut zip::ZipArchive<std::fs::File>,
) -> Result<(), String> {
    let mut paths = std::collections::HashSet::new();
    for index in 0..archive.len() {
        let entry = archive.by_index_raw(index).map_err(|e| e.to_string())?;
        let name =
            crate::api::pack::detect::decode_zip_entry_name(entry.name_raw())
                .replace('\\', "/");
        if name.is_empty() {
            continue;
        }
        super::super::files::safe_relative_join(
            Path::new("archive-root"),
            name.trim_end_matches('/'),
        )
        .map_err(|_| format!("Unsafe archive path: {name}"))?;
        if !entry.is_dir() {
            let key = if cfg!(any(windows, target_os = "macos")) {
                name.to_lowercase()
            } else {
                name.clone()
            };
            if !paths.insert(key) {
                return Err(format!("Duplicate archive file: {name}"));
            }
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err("Symbolic links cannot be imported".into());
        }
    }
    Ok(())
}
