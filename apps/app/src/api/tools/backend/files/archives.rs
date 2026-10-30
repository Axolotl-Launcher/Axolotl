use super::paths::*;
use cap_std::fs::Dir;
use launcher_tools::{Command, ToolError};
use serde_json::{Value, json};
use std::io::{Cursor, Read, Write};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

pub(super) fn execute(
    root: &Dir,
    command: Command,
) -> Result<Value, ToolError> {
    match command {
        Command::ArchiveCreate {
            paths,
            destination_path,
            ..
        } => {
            let mut seen = std::collections::HashSet::new();
            let mut total = 0;
            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
            for path in &paths {
                if !seen.insert(path) {
                    return Err(error("duplicate"));
                }
                let bytes = read(root, path, 32 * 1024 * 1024)?;
                total += bytes.len();
                if total > 32 * 1024 * 1024 {
                    return Err(error("size"));
                }
                writer
                    .start_file(
                        path,
                        SimpleFileOptions::default().compression_method(
                            zip::CompressionMethod::Deflated,
                        ),
                    )
                    .map_err(error)?;
                writer.write_all(&bytes).map_err(error)?;
            }
            let bytes = writer.finish().map_err(error)?.into_inner();
            write(root, &destination_path, &bytes, None, true)?;
            Ok(
                json!({"path":destination_path,"file_count":paths.len(),"source_bytes":total,"sha256":hash(&bytes)}),
            )
        }
        Command::ArchiveExtract {
            path,
            destination_path,
            expected_sha256,
            ..
        } => {
            let bytes = read(root, &path, 16 * 1024 * 1024)?;
            check(&bytes, &expected_sha256)?;
            let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(error)?;
            if zip.len() > 1000 {
                return Err(error("file count"));
            }
            let mut seen = std::collections::HashSet::new();
            let mut files = Vec::new();
            let mut total = 0;
            for i in 0..zip.len() {
                let mut entry = zip.by_index(i).map_err(error)?;
                let path = entry
                    .name()
                    .map_err(error)?
                    .trim_end_matches('/')
                    .to_owned();
                valid(&path, false)?;
                if !seen.insert(path.clone()) || entry.is_symlink() {
                    return Err(error("unsafe archive"));
                }
                if entry.is_dir() {
                    continue;
                }
                if entry.size() > 32 * 1024 * 1024 {
                    return Err(error("size"));
                }
                let mut data = Vec::new();
                entry
                    .by_ref()
                    .take((32 * 1024 * 1024 - total + 1) as u64)
                    .read_to_end(&mut data)
                    .map_err(error)?;
                total += data.len();
                if total > 32 * 1024 * 1024 {
                    return Err(error("size"));
                }
                files.push((path, data));
            }
            let (parent, name) = parent(root, &destination_path)?;
            parent.create_dir(&name).map_err(error)?;
            let target = directory(&parent, &name)?;
            let result = (|| {
                for (path, data) in &files {
                    if let Some((dir, _)) = path.rsplit_once('/') {
                        let mut current = target.try_clone().map_err(error)?;
                        for segment in dir.split('/') {
                            match current.create_dir(segment){Ok(())=>{},Err(e)if e.kind()==std::io::ErrorKind::AlreadyExists=>{},Err(e)=>return Err(error(e))};
                            current = directory(&current, segment)?;
                        }
                    }
                    write(&target, path, data, None, true)?;
                }
                Ok(
                    json!({"directory":destination_path,"file_count":files.len(),"expanded_bytes":total}),
                )
            })();
            if result.is_err() {
                let _ = target.remove_open_dir_all();
            }
            result
        }
        _ => Err(error("command")),
    }
}
