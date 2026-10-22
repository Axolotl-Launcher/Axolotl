use serde::{Deserialize, Serialize};

use super::{
    ContentDependencyInfo, ContentOwnershipKind,
    PackMemberMaterializationState, PackMemberOverrideKind,
    PendingManualDownload,
};
use crate::state::{
    ContentItem, ContentProvider, LinkedModpackInfo, ProjectType,
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentItemCapabilities {
    pub can_toggle: bool,
    pub can_delete: bool,
    pub can_update: bool,
    pub can_change_version: bool,
    pub can_restore_pack_default: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceContentSnapshotItem {
    #[serde(default)]
    pub artifact_ref: Option<crate::resources::SourceArtifactRef>,
    #[serde(default)]
    pub resource_id: Option<crate::resources::ResourceId>,
    #[serde(default)]
    pub pack_resource_id: Option<crate::resources::ResourceId>,
    #[serde(default)]
    pub compatibility: Option<crate::resources::CompatibilityAssessment>,
    pub file_id: Option<String>,
    pub entry_id: Option<String>,
    pub member_id: Option<String>,
    pub ownership_kind: ContentOwnershipKind,
    pub materialization_state: PackMemberMaterializationState,
    pub override_kind: PackMemberOverrideKind,
    pub expected_relative_path: String,
    pub required: bool,
    pub project_type: ProjectType,
    pub provider: Option<ContentProvider>,
    pub provider_project_id: Option<String>,
    pub provider_release_id: Option<String>,
    pub content: Option<ContentItem>,
    pub capabilities: ContentItemCapabilities,
    pub dependency: Option<ContentDependencyInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceContentPack {
    #[serde(default)]
    pub resource_id: Option<crate::resources::ResourceId>,
    pub name: String,
    pub icon_path: Option<String>,
    pub provider: Option<ContentProvider>,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub reconciled: bool,
    pub can_update: bool,
    pub metadata: Option<LinkedModpackInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceContentWarning {
    pub code: String,
    pub message: String,
    pub provider: Option<ContentProvider>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceContentSnapshot {
    pub instance_id: String,
    pub revision: u64,
    pub pack: Option<InstanceContentPack>,
    pub items: Vec<InstanceContentSnapshotItem>,
    pub pending_manual_downloads: Vec<PendingManualDownload>,
    pub warnings: Vec<InstanceContentWarning>,
}
