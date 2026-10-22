//! Theseus instance management interface

mod backup;
mod content;
mod core_components;
mod get;
mod groups;
mod home;
mod install;
mod lifecycle;
mod paths;
pub(crate) mod projects;
mod run;
mod screenshot_groups;
mod screenshots;
pub(crate) mod synced_options;
pub(crate) mod synced_packs {
    pub(crate) use super::synced_packs_axolotl::{
        capture_resource_pack_selection_change, detach,
        prepare_instance_update, reconcile, seed_from_instance,
    };
}
mod synced_packs_axolotl;
pub(crate) mod synced_servers;
mod upgrade;

pub use self::backup::{
    BackupDeleteSummary, BackupExclusion, BackupExclusionKind, BackupOperation,
    BackupOperationState, BackupOperationType, BackupRepositoryStatus,
    BackupRestorePreview, BackupSnapshot, InstanceBackupConfig,
    InstanceBackupEligibility, cancel_backup,
    delete_snapshot as delete_backup_snapshot, disable as disable_backups,
    enable as enable_backups,
    exclusion_from_absolute_path as get_backup_exclusion_from_path,
    has_active_operations as has_active_backup_operations,
    has_active_repository_move as has_active_backup_repository_move,
    instance_config as get_backup_config,
    instance_delete_summary as get_backup_delete_summary,
    interrupt_active_operations as interrupt_active_backup_operations,
    list_operations as list_backup_operations,
    list_snapshots as list_backup_snapshots,
    maintain_repository as maintain_backup_repository,
    recover_repository as recover_backup_repository,
    repository_status as get_backup_repository_status, restore_snapshot,
    start_repository_move as move_backup_repository,
    start_restore_preview as get_backup_restore_preview,
    start_snapshot as start_backup_snapshot,
    update_exclusions as update_backup_exclusions,
};
pub(crate) use self::backup::{
    begin_instance_deletion, cancel_instance_deletion, delete_instance_backups,
    ensure_backup_eligible_edit, has_active_instance_operation,
    lock_instance_maintenance, move_default_repository_for_launcher_directory,
};
pub(crate) use self::content::resolve_content_change_actions;
pub use self::content::{
    apply_content_update_plan, get_content_items, get_content_items_by_paths,
    get_content_snapshot, get_content_snapshot_for_display,
    get_dependencies_as_content_items, get_install_candidates,
    get_installed_project_ids, get_linked_modpack_content,
    get_linked_modpack_info, get_projects, list_content_sets,
    plan_content_updates, refresh_content, sync_content_files,
};
pub(crate) use self::core_components::assemble_for_launch;
pub use self::core_components::{
    McArchiveCoreInstallResult, add_core_jar_mod, import_mcarchive_modloader,
    install_mcarchive_modloader, list_core_components, move_core_component,
    preview_core_jar, remove_core_component, replace_core_jar,
    restore_core_component, set_core_component_enabled,
};
pub use self::get::{get, get_many, list};
pub use self::home::{
    get_daily_playtime, get_daily_playtime_details, set_pinned,
};
pub use self::install::get_optimal_jre_key;
pub(crate) use self::lifecycle::create;
pub use self::lifecycle::{
    cache_icon, create_with_direct_link, edit, edit_icon, remove,
    sync_direct_links,
};
pub use self::paths::{get_full_path, get_mod_full_path};
pub(crate) use self::projects::emit_content_changed;
pub use self::projects::{
    ContentToggleResult, InstallContentBatchRequest, add_project_from_path,
    import_world_save, install_datapack_bytes_to_world,
    install_datapack_to_world, queue_content_batch, queue_curseforge_world,
    remove_content_entry, remove_project, repair_managed_modrinth,
    restore_pack_member_default, rollback_project, toggle_content_entries,
    toggle_content_entry, toggle_disable_project,
    update_managed_modrinth_version,
};
pub use self::run::{
    GcLaunchIntent, GcLaunchReport, QuickPlayType, kill, run,
    run_with_extra_launch_args, run_with_extra_launch_args_with_gc,
    try_update_playtime_by_instance_id,
};
pub use self::screenshot_groups::{
    ScreenshotGroup, ScreenshotGroupImport, ScreenshotGroupMembershipUpdate,
    create_screenshot_group, delete_screenshot_group, import_screenshot_groups,
    list_screenshot_groups, rename_screenshot_group,
    set_screenshot_group_memberships,
};
pub use self::screenshots::{
    InstanceScreenshot, ScreenshotEditSaveMode, ScreenshotKey,
    delete_screenshots, export_screenshots, get_screenshot_path,
    list_all_screenshots, list_all_screenshots_with_warnings, list_screenshots,
    list_synced_screenshots, move_screenshots, save_edited_screenshot,
};
pub(crate) use self::screenshots::{
    reconcile_screenshots, reconcile_screenshots_with_state,
};
pub use self::synced_options::game_options::{
    GameOptionsSourceCandidate, GameSettingLocaleLabels,
    GameSettingsEditorState, SaveGameSettingsResult, UpdateGameSettingsRequest,
    get_config as get_synced_game_options_config,
    get_game_setting_locale_labels,
    get_local_config as get_local_game_options_config,
    list_sync_sources as list_game_options_sync_sources,
    preview_changes as preview_synced_game_option_changes,
    preview_local_changes as preview_local_game_option_changes,
    save_changes as save_synced_game_option_changes,
    save_local_changes as save_local_game_option_changes,
};
pub(crate) use self::synced_options::game_options::{
    apply_launcher_overrides as apply_game_options_launcher_overrides,
    sync_before_launch as sync_game_options_before_launch,
};
pub use self::synced_options::{
    GlobalSyncedOptions, SyncedOptionCapability, SyncedOptionJoinAction,
    SyncedOptionJoinPreview, SyncedOptionJoinResolution, SyncedOptionsOverview,
    get_capabilities as get_synced_option_capabilities,
    get_command_history as get_synced_command_history,
    get_global_options as get_global_synced_options,
    get_initialized_options as get_initialized_synced_options,
    get_instance_option_join_preview as get_synced_option_join_preview,
    get_overview as get_synced_options_overview,
    set_command_history as set_synced_command_history,
    set_global_option as set_global_synced_option,
    set_instance_option as set_instance_synced_option,
};
pub(crate) use self::synced_options::{
    reconcile_changed_file as reconcile_synced_option_file,
    remove_generated_instance_files,
};
pub use self::synced_packs_axolotl::{
    PackSyncPreview, PackSyncTarget, desync_pack, get_pack_sync_preview,
    list_synced_packs, remove_synced_pack, set_synced_pack_enabled, sync_pack,
    upload_synced_pack,
};
pub use self::synced_servers::{
    DesyncServerMode, ServerSource, SyncedServer, desync_server,
    list_synced_servers, remove_synced_server, update_synced_server,
};
pub use self::upgrade::{
    discard_instance_upgrade_plan, dismiss_instance_post_upgrade_notice,
    execute_instance_upgrade, get_instance_post_upgrade_notice,
    get_instance_upgrade_plan, plan_instance_upgrade,
    reset_instance_upgrade_resolution,
    resolve_custom_instance_upgrade_solution, select_instance_upgrade_solution,
    update_instance_upgrade_resolution, update_instance_upgrade_resolutions,
};
pub use crate::state::{DailyPlaytime, DailyPlaytimeEntry};
pub use crate::state::{InstanceSyncedOptions, SyncedOption};

pub use self::groups::FAVORITES_GROUP_ID;
pub use self::groups::{
    InstanceGroup, InstanceGroupMembershipChange,
    InstanceGroupMembershipUpdate, create_group, delete_group, list_groups,
    rename_group, set_group_memberships, set_group_order,
    update_group_memberships,
};

mod upgrade_plan_registry;
