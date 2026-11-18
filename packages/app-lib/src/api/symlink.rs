use crate::util::symlink::SymlinkCapability;

#[tracing::instrument(skip_all, level = "info")]
pub async fn check_symlink_capability() -> crate::Result<SymlinkCapability> {
    Ok(crate::util::symlink::check_symlink_capability().await)
}

/// Entry point for the elevated link-creation helper process. Exits with 0 on
/// success and 1 on failure, after writing the outcome to the request's
/// result file.
#[cfg(target_os = "windows")]
pub fn create_link_elevated_helper(payload: &str) -> i32 {
    crate::util::symlink::create_link_elevated_helper(payload)
}
