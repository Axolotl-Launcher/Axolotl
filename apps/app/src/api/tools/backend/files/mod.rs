mod archives;
pub(super) mod nbt;
pub(super) mod paths;
#[cfg(test)]
mod tests;
use super::{LauncherBackend, page};
use base64::{Engine, engine::general_purpose::STANDARD};
use launcher_tools::{BackendContext, Command, ToolError, log_text};
use paths::*;
use serde_json::{Value, json};
const MAX: u64 = 2 * 1024 * 1024;

pub(super) async fn execute(
    _: &LauncherBackend,
    command: Command,
    _: BackendContext,
) -> Result<Value, ToolError> {
    let (kind, id, mutating) = match &command {
        Command::FileList {
            target_kind,
            target_id,
            ..
        }
        | Command::FileStat {
            target_kind,
            target_id,
            ..
        }
        | Command::FileSearch {
            target_kind,
            target_id,
            ..
        }
        | Command::FileReadText {
            target_kind,
            target_id,
            ..
        }
        | Command::FileReadBinary {
            target_kind,
            target_id,
            ..
        }
        | Command::NbtRead {
            target_kind,
            target_id,
            ..
        } => (target_kind, target_id, false),
        Command::FileWrite {
            target_kind,
            target_id,
            ..
        }
        | Command::FileApplyPatch {
            target_kind,
            target_id,
            ..
        }
        | Command::FileCreateDirectory {
            target_kind,
            target_id,
            ..
        }
        | Command::FileCopy {
            target_kind,
            target_id,
            ..
        }
        | Command::FileMove {
            target_kind,
            target_id,
            ..
        }
        | Command::FileTrash {
            target_kind,
            target_id,
            ..
        }
        | Command::ArchiveCreate {
            target_kind,
            target_id,
            ..
        }
        | Command::ArchiveExtract {
            target_kind,
            target_id,
            ..
        }
        | Command::NbtPatch {
            target_kind,
            target_id,
            ..
        } => (target_kind, target_id, true),
        _ => {
            return Err(ToolError::new(
                "unsupported_command",
                "Not a file operation",
            ));
        }
    };
    let root_path = root(kind, id, mutating).await?;
    let _content_guard = if mutating && kind == "instance" {
        Some(
            theseus::instance::lock_content_file_edit(id)
                .await
                .map_err(super::business_error)?,
        )
    } else {
        None
    };
    if mutating {
        root(kind, id, true).await?;
    }
    tokio::task::spawn_blocking(move || {
        run(open_root(&root_path)?, command, root_path)
    })
    .await
    .map_err(error)?
}
fn run(
    root: cap_std::fs::Dir,
    command: Command,
    _root_path: std::path::PathBuf,
) -> Result<Value, ToolError> {
    match command {
        Command::FileList {
            directory,
            offset,
            limit,
            ..
        } => scan(
            &root,
            directory.as_deref().unwrap_or(""),
            None,
            offset.unwrap_or(0),
            limit.unwrap_or(50),
        ),
        Command::FileSearch {
            directory,
            query,
            offset,
            limit,
            ..
        } => scan(
            &root,
            directory.as_deref().unwrap_or(""),
            Some(&query),
            offset.unwrap_or(0),
            limit.unwrap_or(50),
        ),
        Command::FileStat { path, .. } => {
            let (dir, name) = parent(&root, &path)?;
            let meta = dir.symlink_metadata(name).map_err(error)?;
            let sha = if meta.is_file()
                && !meta.file_type().is_symlink()
                && meta.len() <= 64 * 1024 * 1024
            {
                Some(hash(&read(&root, &path, 64 * 1024 * 1024)?))
            } else {
                None
            };
            Ok(
                json!({"path":path,"is_file":meta.is_file(),"is_directory":meta.is_dir(),"is_symlink":meta.file_type().is_symlink(),"bytes":meta.len(),"sha256":sha}),
            )
        }
        Command::FileReadText {
            path,
            offset,
            max_bytes,
            ..
        } => {
            let bytes = read(&root, &path, MAX)?;
            let text = std::str::from_utf8(&bytes).map_err(error)?;
            let mut result = log_text::read(
                text,
                offset.unwrap_or(0),
                max_bytes.unwrap_or(32768),
                false,
            );
            result["sha256"] = json!(hash(&bytes));
            Ok(result)
        }
        Command::FileReadBinary {
            path,
            offset,
            max_bytes,
            ..
        } => {
            let bytes = read(&root, &path, MAX)?;
            let start = offset.unwrap_or(0).min(bytes.len());
            let end = (start + max_bytes.unwrap_or(65536)).min(bytes.len());
            Ok(
                json!({"base64":STANDARD.encode(&bytes[start..end]),"sha256":hash(&bytes),"bytes":bytes.len(),"next_offset":(end<bytes.len()).then_some(end)}),
            )
        }
        Command::FileWrite {
            path,
            content,
            encoding,
            expected_sha256,
            create_new,
            ..
        } => {
            let bytes = decode(&content, &encoding)?;
            write(
                &root,
                &path,
                &bytes,
                expected_sha256.as_deref(),
                create_new,
            )?;
            Ok(json!({"path":path,"bytes":bytes.len(),"sha256":hash(&bytes)}))
        }
        Command::FileApplyPatch {
            path,
            find,
            replace,
            expected_sha256,
            ..
        } => {
            let bytes = read(&root, &path, MAX)?;
            check(&bytes, &expected_sha256)?;
            let text = std::str::from_utf8(&bytes).map_err(error)?;
            if find.is_empty() || text.matches(&find).count() != 1 {
                return Err(ToolError::new(
                    "ambiguous_patch",
                    "Find text must occur exactly once",
                ));
            }
            let changed = text.replacen(&find, &replace, 1);
            if changed.len() > MAX as usize {
                return Err(error("size"));
            }
            write(
                &root,
                &path,
                changed.as_bytes(),
                Some(&expected_sha256),
                false,
            )?;
            Ok(json!({"path":path,"sha256":hash(changed.as_bytes())}))
        }
        Command::FileCreateDirectory { path, .. } => {
            let (dir, name) = parent(&root, &path)?;
            dir.create_dir(name).map_err(error)?;
            Ok(json!({"created":true,"path":path}))
        }
        c @ (Command::FileCopy { .. } | Command::FileMove { .. }) => {
            let moving = matches!(c, Command::FileMove { .. });
            let (Command::FileCopy {
                source_path,
                destination_path,
                expected_sha256,
                ..
            }
            | Command::FileMove {
                source_path,
                destination_path,
                expected_sha256,
                ..
            }) = c
            else {
                unreachable!()
            };
            let bytes = read(&root, &source_path, MAX)?;
            check(&bytes, &expected_sha256)?;
            write(&root, &destination_path, &bytes, None, true)?;
            if moving {
                check(&read(&root, &source_path, MAX)?, &expected_sha256)?;
                let (dir, name) = parent(&root, &source_path)?;
                dir.remove_file(name).map_err(error)?;
            }
            Ok(
                json!({"source":source_path,"destination":destination_path,"moved":moving,"sha256":hash(&bytes)}),
            )
        }
        Command::FileTrash {
            path,
            expected_sha256,
            ..
        } => {
            check(&read(&root, &path, MAX)?, &expected_sha256)?;
            let (dir, name) = parent(&root, &path)?;
            let metadata = dir.symlink_metadata(&name).map_err(error)?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(error("file"));
            }
            // Move through directory capabilities before giving the OS recycle
            // API an absolute path; a swapped instance parent cannot redirect it.
            let staging = tempfile::tempdir().map_err(error)?;
            let staged_root = open_root(staging.path())?;
            dir.rename(&name, &staged_root, &name).map_err(error)?;
            if let Err(failure) = trash::delete(staging.path().join(&name)) {
                if dir.symlink_metadata(&name).is_ok()
                    || staged_root.rename(&name, &dir, &name).is_err()
                {
                    let recovery = staging.keep();
                    tracing::error!(path=%recovery.display(),"MCP recycle failed; staged source retained for recovery");
                    return Err(ToolError::new(
                        "trash_recovery_required",
                        "Recycle failed; source retained in a recovery directory recorded in launcher diagnostics",
                    ));
                }
                return Err(error(failure));
            }
            Ok(
                json!({"trashed":true,"path":path,"recycle_restore_uses_staging_directory":true}),
            )
        }
        c
        @ (Command::ArchiveCreate { .. } | Command::ArchiveExtract { .. }) => {
            archives::execute(&root, c)
        }
        c @ (Command::NbtRead { .. } | Command::NbtPatch { .. }) => {
            nbt::execute(&root, c)
        }
        _ => Err(ToolError::new(
            "unsupported_command",
            "Not a file operation",
        )),
    }
}
pub(super) fn decode(
    content: &str,
    encoding: &str,
) -> Result<Vec<u8>, ToolError> {
    let bytes = match encoding {
        "utf8" => content.as_bytes().to_vec(),
        "base64" => STANDARD.decode(content).map_err(error)?,
        _ => return Err(error("encoding")),
    };
    if bytes.len() > MAX as usize {
        return Err(error("size"));
    }
    Ok(bytes)
}
fn scan(
    root: &cap_std::fs::Dir,
    start: &str,
    query: Option<&str>,
    offset: usize,
    limit: usize,
) -> Result<Value, ToolError> {
    let mut pending = vec![(start.to_owned(), 0)];
    let mut scanned = 0;
    let mut items = Vec::new();
    let mut truncated = false;
    while let Some((relative, depth)) = pending.pop() {
        let dir = directory(root, &relative)?;
        let mut entries = dir
            .entries()
            .map_err(error)?
            .take(10001)
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?;
        entries.sort_by_key(|a| a.file_name());
        for entry in entries {
            scanned += 1;
            if scanned > 10000 {
                truncated = true;
                break;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned)
            else {
                continue;
            };
            let path = if relative.is_empty() {
                name.clone()
            } else {
                format!("{relative}/{name}")
            };
            let metadata = dir.symlink_metadata(&name).map_err(error)?;
            if query
                .is_none_or(|q| name.to_lowercase().contains(&q.to_lowercase()))
            {
                items.push(json!({"path":path,"is_file":metadata.is_file(),"is_directory":metadata.is_dir(),"is_symlink":metadata.file_type().is_symlink(),"bytes":metadata.len()}));
            }
            if query.is_some()
                && metadata.is_dir()
                && !metadata.file_type().is_symlink()
            {
                if depth < 32 {
                    pending.push((path, depth + 1));
                } else {
                    truncated = true;
                }
            }
        }
        if scanned > 10000 {
            break;
        }
    }
    items.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    let mut result = page(items, offset, limit)?;
    result["scan_truncated"] = json!(truncated);
    result["scanned_entries"] = json!(scanned.min(10000));
    Ok(result)
}
