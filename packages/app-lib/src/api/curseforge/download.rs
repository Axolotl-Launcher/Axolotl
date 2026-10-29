use super::{
    CurseForgeFile, CurseForgeProject, derived_curseforge_download_url,
};
use crate::State;

pub(super) fn normalized_download_url(url: Option<String>) -> Option<String> {
    url.and_then(|url| {
        let url = url.trim();
        (!url.is_empty()).then(|| url.to_string())
    })
}

pub(super) async fn resolve_download_url(
    project_id: u32,
    file_id: u32,
    project: &CurseForgeProject,
    file: &CurseForgeFile,
) -> crate::Result<Option<String>> {
    let state = State::get().await?;
    let bypass_restrictions = state.bypass_curseforge_download_restrictions();
    if !bypass_restrictions && project.allow_mod_distribution == Some(false) {
        return Ok(None);
    }
    if let Some(url) = normalized_download_url(file.download_url.clone()) {
        return Ok(Some(url));
    }
    if bypass_restrictions {
        return Ok(Some(derived_curseforge_download_url(
            file_id,
            &file.file_name,
        )?));
    }
    super::get_download_url(project_id, file_id).await
}
