//! Extract icon artwork from inside mod JARs and resource pack ZIPs so
//! unmatched content files can display real icons.

use bytes::Bytes;
use std::io::{Read, Seek};
use std::path::Path;
use zip::ZipArchive;

/// Entries larger than this are almost certainly not pack icons.
const MAX_ICON_BYTES: u64 = 2 * 1024 * 1024;

/// Returns the matched entry name and validated image bytes for a mod JAR.
///
/// Prefers the icon declared by the embedded mod metadata, then falls back
/// to the in-game `pack.png` convention at the archive root.
pub fn extract_mod_icon(
    bytes: &Bytes,
    metadata: Option<&crate::mod_metadata::LocalModMetadata>,
) -> Option<(String, Vec<u8>)> {
    let cursor = std::io::Cursor::new(&**bytes);
    let mut archive = ZipArchive::new(cursor).ok()?;

    if let Some(icon_path) = metadata.and_then(|meta| meta.icon_path.as_deref())
        && let Some(icon) = read_entry(&mut archive, icon_path)
    {
        return Some(icon);
    }

    read_entry(&mut archive, "pack.png")
}

/// Returns the matched entry name and validated image bytes for a resource
/// pack ZIP. Resource packs declare their icon as `pack.png` at the root.
pub fn extract_resource_pack_icon(path: &Path) -> Option<(String, Vec<u8>)> {
    let file = std::fs::File::open(path).ok()?;
    let mut archive = ZipArchive::new(file).ok()?;
    read_entry(&mut archive, "pack.png")
}

fn read_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    entry_name: &str,
) -> Option<(String, Vec<u8>)> {
    let normalized = entry_name.trim_start_matches('/').replace('\\', "/");
    let entry = open_entry(archive, &normalized)?;
    if !entry.is_file() || entry.size() > MAX_ICON_BYTES {
        return None;
    }

    let entry_name = entry.name().ok()?.to_string();
    let mut data = Vec::with_capacity(entry.size() as usize);
    entry.take(MAX_ICON_BYTES + 1).read_to_end(&mut data).ok()?;

    is_supported_image(&data).then_some((entry_name, data))
}

fn open_entry<'a, R: Read + Seek>(
    archive: &'a mut ZipArchive<R>,
    normalized: &str,
) -> Option<zip::read::ZipFile<'a, R>> {
    if archive.by_name(normalized).is_ok() {
        archive.by_name(normalized).ok()
    } else {
        let names: Vec<String> = archive
            .file_names()
            .filter_map(|name| name.ok())
            .map(|name| name.into_owned())
            .collect();
        let matched = names
            .iter()
            .find(|name| name.eq_ignore_ascii_case(normalized))?;
        archive.by_name(matched).ok()
    }
}

fn is_supported_image(data: &[u8]) -> bool {
    data.starts_with(&[0x89, b'P', b'N', b'G'])
        || data.starts_with(&[0xFF, 0xD8, 0xFF])
}
