use crate::{ErrorKind, Result};
use std::path::Path;

pub(super) fn validate_file_name(file_name: &str) -> Result<()> {
    crate::util::io::sanitize_download_file_name(file_name).map(|_| ())
}

pub(super) fn validate_world_archive_name(file_name: &str) -> Result<()> {
    validate_file_name(file_name)?;
    if Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
    {
        return Ok(());
    }
    Err(ErrorKind::InputError(
        "CurseForge world downloads must be ZIP archives".to_string(),
    )
    .into())
}
