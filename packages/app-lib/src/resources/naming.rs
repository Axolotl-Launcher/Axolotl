use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct NamingPreferences {
    pub chinese: bool,
    pub preserve: bool,
}

impl Default for NamingPreferences {
    fn default() -> Self {
        Self {
            chinese: true,
            preserve: true,
        }
    }
}

pub(crate) fn replacement_path(
    current: &InstalledResource,
    filename: &str,
    preserve: bool,
) -> ResourceResult<String> {
    let filename = crate::util::io::sanitize_download_file_name(filename)
        .map_err(|error| ResourceError::InvalidRequest(error.to_string()))?;
    let path = if preserve {
        crate::state::instances::commands::preserved_update_relative_path(
            &current.relative_path,
            current.original_file_name.as_deref(),
            &filename,
        )
        .map_err(|error| ResourceError::InvalidRequest(error.to_string()))?
    } else {
        let (parent, _) =
            current.relative_path.rsplit_once('/').ok_or_else(|| {
                ResourceError::InvalidRequest("invalid installed path".into())
            })?;
        let disabled = if current.relative_path.ends_with(".disabled") {
            ".disabled"
        } else {
            ""
        };
        format!("{parent}/{filename}{disabled}")
    };
    if !plans::safe_relative_path(&path) {
        return Err(ResourceError::InvalidRequest(
            "invalid named resource path".into(),
        ));
    }
    Ok(path)
}

impl ResourceService {
    pub(crate) async fn named_path(
        &self,
        path: String,
        artifact: &ResourceArtifact,
        preferences: &NamingPreferences,
    ) -> ResourceResult<String> {
        if !preferences.chinese {
            return Ok(path);
        }
        let project = &artifact.reference.release.project;
        let Some(store) = self.store() else {
            return Ok(path);
        };
        let Some(slug) = store.project_alias(project).await? else {
            return Ok(path);
        };
        let title = match project.source.as_str() {
            "modrinth" => crate::api::content_search::chinese_file_title_for_modrinth_slug(&slug),
            "curseforge" => crate::api::content_search::chinese_file_title_for_curseforge_slug(&slug),
            _ => None,
        };
        Ok(title
            .and_then(|title| {
                crate::api::content_search::localized_content_relative_path(
                    &path, &title,
                )
            })
            .unwrap_or(path))
    }
}
