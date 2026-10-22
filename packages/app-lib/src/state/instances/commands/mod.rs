mod create_instance;
pub use self::create_instance::CreateInstance;
pub(crate) use self::create_instance::create_instance;

mod create_direct_link_instance;
pub use self::create_direct_link_instance::CreateDirectLinkInstance;
pub(crate) use self::create_direct_link_instance::create_direct_link_instance;
mod sync_direct_link_instances;
pub(crate) use self::sync_direct_link_instances::sync_direct_link_instances;
pub use self::sync_direct_link_instances::{
    DirectLinkSyncReport, ExternalMinecraftRoot,
};

mod edit_instance;
pub use self::edit_instance::{
    AppliedContentSetPatch, EditInstance, InstanceLaunchOverridesPatch,
};
pub(crate) use self::edit_instance::{
    edit_instance, repair_recorded_pack_unlink, restore_instance_metadata,
};

mod get_instance;
pub use self::get_instance::InstanceMetadata;
pub(crate) use self::get_instance::{
    get_instance, get_instance_metadata, get_instances_metadata, list_instances,
};

mod list_content;
pub(crate) use self::list_content::{
    dependencies_to_content_items, get_content_projects,
    get_installed_project_ids_for_instance, get_instance_install_candidates,
    get_linked_modpack_info, list_content, list_content_by_paths,
    list_content_sets, list_linked_modpack_content,
};

mod content_snapshot;
pub(crate) use self::content_snapshot::{
    get_cached_content_snapshot, get_content_snapshot,
    get_local_content_snapshot, reconcile_curseforge_members,
};

mod remove_instance;
pub(crate) use self::remove_instance::*;

mod refresh_instances;
pub(crate) use self::refresh_instances::*;

mod sync_content_files;
pub(crate) use self::sync_content_files::{
    instance_content_root, sync_content_files, sync_content_files_for_operation,
};

mod launch_context;
pub(crate) use self::launch_context::*;

mod apply_content_install;
pub(crate) use self::apply_content_install::*;
pub(crate) use apply_content_install::ProjectFileRecord;

mod instance_upgrade;
pub(crate) use self::instance_upgrade::{
    ReadOnlyUpgradeSource, UpgradePlanRuntimeValidation,
    create_instance_upgrade_plan_with_source,
    recompute_instance_upgrade_plan_from_source,
    scan_instance_upgrade_source_files, validate_instance_upgrade_plan_source,
};

mod post_upgrade_notice;
pub(crate) use self::post_upgrade_notice::*;

mod import_world_save;
pub(crate) use self::import_world_save::import_world_save;
