use super::CurseForgeCategory;

use crate::state::ProjectType;

pub(super) fn mod_loader_to_slug(mod_loader_type: u32) -> &'static str {
    match mod_loader_type {
        1 => "forge",
        4 => "fabric",
        5 => "quilt",
        6 => "neoforge",
        _ => "unknown",
    }
}

pub(super) fn project_type_for_class(class_id: Option<u32>) -> &'static str {
    match class_id {
        Some(5) => "plugin",
        Some(6) => "mod",
        Some(12) => "resourcepack",
        Some(17) => "world",
        Some(6945) => "datapack",
        Some(4471) => "modpack",
        Some(6552) => "shader",
        _ => "mod",
    }
}

pub(super) fn recognized_project_type(
    class_id: Option<u32>,
) -> Option<ProjectType> {
    match class_id {
        Some(6) => Some(ProjectType::Mod),
        Some(12) => Some(ProjectType::ResourcePack),
        Some(6552) => Some(ProjectType::ShaderPack),
        Some(6945) => Some(ProjectType::DataPack),
        Some(17) => Some(ProjectType::WorldSave),
        _ => None,
    }
}

pub(super) fn filter_categories(
    categories: Vec<CurseForgeCategory>,
    class_id: Option<u32>,
) -> Vec<CurseForgeCategory> {
    let Some(class_id) = class_id else {
        return categories;
    };

    categories
        .into_iter()
        .filter(|category| {
            category.id == class_id || category.class_id == Some(class_id)
        })
        .collect()
}

pub(super) fn push_query<T: ToString>(
    query: &mut Vec<(String, String)>,
    name: &str,
    value: Option<T>,
) {
    if let Some(value) = value {
        query.push((name.to_string(), value.to_string()));
    }
}
