use std::process::Command;
use tauri_build::{DefaultPermissionRule, InlinedPlugin};

#[cfg(windows)]
const NPM_COMMAND: &str = "npm.cmd";
#[cfg(not(windows))]
const NPM_COMMAND: &str = "npm";

fn run_command(command: &mut Command, description: &str) {
    let status = command.status().unwrap_or_else(|error| {
        panic!("Failed to start {description}: {error}")
    });
    assert!(
        status.success(),
        "{description} failed with status {status}"
    );
}

fn copy_if_changed(source: &std::path::Path, destination: &std::path::Path) {
    let contents = std::fs::read(source).expect("Read generated resource");
    if std::fs::read(destination).ok().as_deref() != Some(contents.as_slice()) {
        std::fs::copy(source, destination).expect("Copy generated resource");
    }
}

fn newest_mtime(path: &std::path::Path) -> Option<std::time::SystemTime> {
    let mut newest: Option<std::time::SystemTime> = None;
    if path.is_file() {
        return path.metadata().and_then(|m| m.modified()).ok();
    }
    if path.is_dir()
        && let Ok(entries) = std::fs::read_dir(path)
    {
        for entry in entries.flatten() {
            if let Some(time) = newest_mtime(&entry.path()) {
                newest = Some(newest.map_or(time, |current| current.max(time)));
            }
        }
    }
    newest
}

fn blockbench_skin_is_fresh(
    blockbench_dir: &std::path::Path,
    synced_bundle: &std::path::Path,
) -> bool {
    let bundle = blockbench_dir.join("dist/skin.bundle.js");
    let Ok(bundle_time) = bundle.metadata().and_then(|m| m.modified()) else {
        return false;
    };
    let Ok(synced_time) = synced_bundle.metadata().and_then(|m| m.modified())
    else {
        return false;
    };
    if synced_time < bundle_time {
        return false;
    }

    // Inputs that feed `npm run build-skin` or the resource sync. node_modules
    // is intentionally excluded: it is huge and its mtimes churn independently
    // of Blockbench sources.
    let input_paths = [
        blockbench_dir.join("package.json"),
        blockbench_dir.join("package-lock.json"),
        blockbench_dir.join("build.js"),
        blockbench_dir.join("js"),
        blockbench_dir.join("css"),
        blockbench_dir.join("assets"),
        blockbench_dir.join("font"),
        blockbench_dir.join("index.html"),
    ];
    input_paths.iter().all(|input| {
        newest_mtime(input)
            .map(|input_time| input_time <= bundle_time)
            .unwrap_or(true)
    })
}

fn build_blockbench_skin_editor() {
    println!("cargo:rerun-if-env-changed=AXOLOTL_BUILD_RESOURCES");
    if let Some(resources) = std::env::var_os("AXOLOTL_BUILD_RESOURCES") {
        run_command(
            Command::new("node")
                .arg("scripts/axolotl/prepare-build-resources.mjs")
                .arg("--consume")
                .arg("--output")
                .arg(&resources)
                .current_dir("../.."),
            "Prepared Blockbench resource verification",
        );
        println!(
            "cargo:rerun-if-changed={}",
            std::path::Path::new(&resources)
                .join("manifest.json")
                .display()
        );
        return;
    }

    if std::env::var_os("AXOLOTL_SKIP_BLOCKBENCH_BUILD").is_some_and(|value| {
        !value.is_empty() && value != "0" && value != "false"
    }) {
        println!(
            "cargo:warning=AXOLOTL_SKIP_BLOCKBENCH_BUILD set; skipping Blockbench skin editor build"
        );
        return;
    }

    let blockbench_dir = std::path::Path::new("../../third-party/blockbench");
    assert!(
        blockbench_dir.join("package.json").is_file(),
        "Blockbench skin editor submodule is missing. Run git submodule update --init --recursive."
    );

    let synced_bundle = std::path::Path::new(
        "resources/blockbench-skin/dist/skin.bundle.js.gz",
    );
    if blockbench_skin_is_fresh(blockbench_dir, synced_bundle) {
        return;
    }

    if !blockbench_dir.join("node_modules").is_dir() {
        run_command(
            Command::new(NPM_COMMAND)
                .arg("ci")
                .current_dir(blockbench_dir),
            "Blockbench skin editor dependency install",
        );
    }

    run_command(
        Command::new(NPM_COMMAND)
            .arg("run")
            .arg("build-skin")
            .current_dir(blockbench_dir),
        "Blockbench skin editor build",
    );
    run_command(
        Command::new("node")
            .arg("scripts/axolotl/sync-blockbench-skin-editor.mjs")
            .current_dir("../.."),
        "Blockbench skin editor resource sync",
    );
}

fn collect_engine_sources(
    path: &std::path::Path,
    sources: &mut Vec<std::path::PathBuf>,
) {
    for entry in std::fs::read_dir(path).expect("Read cubiomes sources") {
        let entry = entry.expect("Read cubiomes entry");
        let path = entry.path();
        if path.is_dir() {
            if matches!(
                entry.file_name().to_str(),
                Some(
                    "features"
                        | "loot"
                        | "loot_tables"
                        | "barrels"
                        | "chests"
                        | "trial_chambers"
                        | "cjson"
                )
            ) {
                collect_engine_sources(&path, sources);
            }
        } else if path.extension().is_some_and(|extension| extension == "c")
            && path.file_name().is_some_and(|name| name != "tests.c")
        {
            sources.push(path);
        }
    }
}

fn build_seed_map_engine() {
    let source = std::path::Path::new("vendor/cubiomes");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let compatibility_header = out.join("cubiomes-loot-functions.h");
    let header = std::fs::read_to_string(source.join("loot/loot_functions.h"))
        .expect("Read loot header");
    let rng =
        std::fs::canonicalize(source.join("rng.h")).expect("Locate RNG header");
    let header = header
        .replace(
            "RandomSource*, const int min",
            "RandomSource *unused_rnd, const int min",
        )
        .replace(
            "\"../rng.h\"",
            &format!("\"{}\"", rng.display().to_string().replace('\\', "/")),
        );
    std::fs::write(&compatibility_header, header)
        .expect("Write pre-C23 compatibility header");
    let mut sources = vec![
        std::path::PathBuf::from("src/seed_map/cubiomes_bridge.c"),
        std::path::PathBuf::from("src/seed_map/cubiomes_details.c"),
        std::path::PathBuf::from("src/seed_map/underground_volume.c"),
        std::path::PathBuf::from("src/seed_map/ancient_city_layout.c"),
    ];
    collect_engine_sources(source, &mut sources);
    sources.sort();
    println!("cargo:rerun-if-changed=src/seed_map/cubiomes_bridge.c");
    println!("cargo:rerun-if-changed=src/seed_map/cubiomes_details.c");
    println!("cargo:rerun-if-changed=src/seed_map/underground_volume.c");
    println!("cargo:rerun-if-changed=src/seed_map/ancient_city_layout.c");
    println!("cargo:rerun-if-changed=src/seed_map/ancient_city_layout.h");
    println!("cargo:rerun-if-changed=src/seed_map/cubiomes_bridge.h");
    println!("cargo:rerun-if-changed=vendor/cubiomes");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-env-changed=AXOLOTL_CUBIOMES_CC");
        let compiler = std::env::var_os("AXOLOTL_CUBIOMES_CC")
            .unwrap_or_else(|| "gcc".into());
        let out =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let dll = out.join("axolotl-cubiomes.dll");
        let architecture = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
        let target = Command::new(&compiler)
            .arg("-dumpmachine")
            .output()
            .expect("Inspect MinGW target");
        let machine = String::from_utf8_lossy(&target.stdout);
        let prefix = if architecture == "x86_64" {
            "x86_64"
        } else if architecture == "aarch64" {
            "aarch64"
        } else {
            panic!("Unsupported Windows seed map architecture: {architecture}")
        };
        assert!(
            target.status.success() && machine.starts_with(prefix),
            "Seed map compiler architecture does not match Rust target: {machine}"
        );
        run_command(
            Command::new(compiler)
                .args([
                    "-shared",
                    "-O3",
                    "-std=gnu11",
                    "-fwrapv",
                    "-static-libgcc",
                    "-Wl,--export-all-symbols",
                    // Tauri watches the bundled DLL. Keep identical builds
                    // byte-for-byte stable so syncing it preserves its mtime.
                    "-Wl,--no-insert-timestamp",
                    // MinGW's automatic base depends on the output path, which
                    // differs between Cargo configurations. ASLR stays enabled.
                    "-Wl,--image-base,0x180000000",
                    "-Ivendor/cubiomes",
                ])
                .arg("-include")
                .arg(&compatibility_header)
                .args(&sources)
                .arg("-o")
                .arg(&dll),
            "MinGW cubiomes engine build",
        );
        let resources = std::path::Path::new("resources/seed-map");
        std::fs::create_dir_all(resources).expect("Create seed map resources");
        copy_if_changed(&dll, &resources.join("axolotl-cubiomes.dll"));
        copy_if_changed(
            &source.join("LICENSE"),
            &resources.join("LICENSE.cubiomes.txt"),
        );
        println!(
            "cargo:rustc-env=AXOLOTL_CUBIOMES_BUILD_DLL={}",
            dll.display()
        );
    } else {
        cc::Build::new()
            .flag("-include")
            .flag(compatibility_header.to_str().unwrap())
            .include(source)
            .files(&sources)
            .opt_level(3)
            .flag_if_supported("-fwrapv")
            .flag_if_supported("-std=gnu11")
            .warnings(false)
            .compile("axolotl-cubiomes");
        println!("cargo:rustc-link-lib=m");
    }
}

fn main() {
    println!("cargo:rerun-if-env-changed=CI");
    println!("cargo:rerun-if-changed=tauri.conf.json");
    println!("cargo:rerun-if-changed=tauri.windows.conf.json");
    println!("cargo:rerun-if-changed=tauri.macos.conf.json");
    println!("cargo:rerun-if-changed=tauri.linux.conf.json");
    println!("cargo:rerun-if-changed=tauri-modern.conf.json");
    println!("cargo:rerun-if-changed=tauri-release.conf.json");
    // Watch Blockbench inputs only. Watching the whole submodule (including
    // node_modules) made any npm install churn re-run the entire build script.
    println!(
        "cargo:rerun-if-changed=../../third-party/blockbench/package.json"
    );
    println!(
        "cargo:rerun-if-changed=../../third-party/blockbench/package-lock.json"
    );
    println!("cargo:rerun-if-changed=../../third-party/blockbench/build.js");
    println!("cargo:rerun-if-changed=../../third-party/blockbench/js");
    println!("cargo:rerun-if-changed=../../third-party/blockbench/css");
    println!("cargo:rerun-if-changed=../../third-party/blockbench/assets");
    println!("cargo:rerun-if-changed=../../third-party/blockbench/font");
    println!("cargo:rerun-if-changed=../../third-party/blockbench/index.html");
    // Tauri validates frontendDist during Cargo metadata/check builds. The
    // frontend build runs in parallel in CI, so create the directory before
    // tauri-build reads the configuration. A real frontend build overwrites
    // this directory with the actual assets before packaging.
    std::fs::create_dir_all("../app-frontend/dist")
        .expect("Failed to create the frontend distribution directory");
    build_blockbench_skin_editor();

    build_seed_map_engine();

    // Sadly, there is no better way to do it right now
    // You could try parsing source code here and detecting #[tauri::command]
    // But I think it's not worth it
    // https://github.com/tauri-apps/tauri/issues/10075
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .codegen(tauri_build::CodegenContext::new())
            .plugin(
                "auth",
                InlinedPlugin::new()
                    .commands(&[
                        "check_reachable",
                        "check_mojang_services",
                        "network_scope",
                        "set_mojang_auth_use_mirror",
                        "mojang_auth_use_mirror",
                        "login",
                        "browser_login",
                        "begin_device_login",
                        "poll_device_login",
                        "begin_yggdrasil_login",
                        "finish_yggdrasil_login",
                        "list_yggdrasil_saved_logins",
                        "get_yggdrasil_password",
                        "set_yggdrasil_password",
                        "delete_yggdrasil_password",
                        "add_offline_user",
                        "remove_user",
                        "get_default_user",
                        "set_default_user",
                        "get_users",
                        "get_anti_piracy_status",
                        "clear_official_login_marker",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "cache",
                InlinedPlugin::new()
                    .commands(&[
                        "get_project",
                        "get_project_many",
                        "get_project_v3",
                        "get_project_v3_many",
                        "get_version",
                        "get_version_many",
                        "get_user",
                        "get_user_many",
                        "get_team",
                        "get_team_many",
                        "get_organization",
                        "get_organization_many",
                        "get_search_results",
                        "get_search_results_many",
                        "get_search_results_v3",
                        "get_search_results_v3_many",
                        "cancel_search_request",
                        "purge_cache_types",
                        "get_project_versions",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "content-search",
                InlinedPlugin::new()
                    .commands(&[
                        "resolve_chinese_content_search",
                        "expand_content_search_query",
                        "lookup_chinese_content_names",
                        "lookup_content_wiki_ids",
                        "lookup_content_identities",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "content-favorites",
                InlinedPlugin::new()
                    .commands(&[
                        "content_favorites_list",
                        "content_favorites_add",
                        "content_favorites_remove",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "axolotl-resources",
                InlinedPlugin::new()
                    .commands(&[
                        "resource_sources",
                        "resource_search",
                        "resource_get",
                        "resource_get_description",
                        "resource_list_releases",
                        "resource_get_release",
                        "resource_get_changelog",
                        "resource_list_changelogs",
                        "resource_cancel",
                        "resource_preferred_source",
                        "resource_set_preferred_source",
                        "resource_cache_info",
                        "resource_cache_clear",
                        "resource_locate",
                        "resource_project_document",
                        "resource_project_documents",
                        "resource_project_protocol",
                        "resource_release_documents",
                        "resource_favorites",
                        "resource_favorite_add",
                        "resource_favorite_remove",
                        "resource_prepare_operation",
                        "resource_analysis_open",
                        "resource_analysis_update",
                        "resource_analysis_close",
                        "resource_prepare_entry_operation",
                        "resource_list_installed",
                        "resource_check_updates",
                        "resource_start_content_refresh",
                        "resource_get_content_refresh_state",
                        "resource_revise_plan",
                        "resource_queue_plan",
                        "resource_list_history",
                        "resource_scan_backups",
                        "resource_prepare_history_cleanup",
                        "resource_execute_history_cleanup",
                        "resource_export_scan",
                        "resource_export_refresh",
                        "resource_export_refresh_retry",
                        "resource_export_close",
                        "resource_export_plan",
                        "resource_export_execute",
                        "resource_export_queue",
                        "resource_export_cancel",
                        "resource_rollback",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "datapacks",
                InlinedPlugin::new()
                    .commands(&[
                        "list_datapacks",
                        "delete_datapack",
                        "set_datapack_enabled",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "curseforge",
                InlinedPlugin::new()
                    .commands(&[
                        "curseforge_capability",
                        "curseforge_api_key_override",
                        "curseforge_set_api_key",
                        "curseforge_validate_credentials",
                        "curseforge_search_projects",
                        "curseforge_get_download_url",
                        "curseforge_get_categories",
                        "curseforge_recognize_instance_files",
                        "curseforge_import_manual_downloads",
                        "curseforge_list_pending_manual_downloads",
                        "curseforge_import_pending_manual_download_file",
                        "curseforge_configure_manual_download_watcher",
                        "curseforge_install_modpack",
                        "curseforge_update_managed_modpack",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "mcarchive",
                InlinedPlugin::new()
                    .commands(&[
                        "mcarchive_get_game_versions",
                        "mcarchive_search_mods",
                        "mcarchive_get_mod_by_slug",
                        "mcarchive_get_file_by_filename",
                        "mcarchive_get_file_by_sha256",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "import",
                InlinedPlugin::new()
                    .commands(&[
                        "get_importable_instances",
                        "import_instance",
                        "is_valid_importable_instance",
                        "get_default_launcher_path",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "jre",
                InlinedPlugin::new()
                    .commands(&[
                        "get_java_versions",
                        "get_java_default_versions",
                        "set_java_version",
                        "set_java_default_version",
                        "remove_java_default_version",
                        "remove_java_version",
                        "jre_find_filtered_jres",
                        "jre_get_jre",
                        "jre_test_jre",
                        "jre_auto_install_java",
                        "jre_respond_to_download_confirmation",
                        "jre_get_max_memory",
                        "jre_get_memory_status",
                        "jre_optimize_memory",
                        "list_java_distribution_versions",
                        "list_java_feed_vendors",
                        "list_java_feed_versions",
                        "download_java_from_feed",
                        "download_java",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "logs",
                InlinedPlugin::new()
                    .commands(&[
                        "logs_get_logs",
                        "logs_get_logs_by_filename",
                        "logs_get_output_by_filename",
                        "logs_delete_logs",
                        "logs_delete_logs_by_filename",
                        "logs_get_latest_log_cursor",
                        "logs_get_minecraft_latest_log_cursor",
                        "logs_get_live_log_buffer",
                        "logs_clear_live_log_buffer",
                        "logs_analyze_crash",
                        "logs_get_crash_analysis_ai_settings",
                        "logs_update_crash_analysis_ai_settings",
                        "logs_logagent_prepare",
                        "logs_logagent_model_turn",
                        "logs_logagent_tool",
                        "logs_logagent_cloud",
                        "logs_logagent_cancel",
                        "logs_undo_added_mod",
                        "logs_export_crash_context",
                        "logs_get_log_share_settings",
                        "logs_update_log_share_settings",
                        "logs_logshare_upload_crash",
                        "logs_logshare_get_insights",
                        "logs_logshare_analyse_crash_direct",
                        "logs_logshare_delete",
                        "logs_list_shared_logs",
                        "logs_record_shared_log",
                        "logs_delete_shared_log",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "metadata",
                InlinedPlugin::new()
                    .commands(&[
                        "metadata_get_game_versions",
                        "metadata_get_loader_versions",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "minecraft-skins",
                InlinedPlugin::new()
                    .commands(&[
                        "get_available_capes",
                        "get_available_skins",
                        "lookup_player_skin",
                        "add_and_equip_custom_skin",
                        "equip_skin",
                        "remove_custom_skin",
                        "save_custom_skin",
                        "set_custom_skin_order",
                        "unequip_skin",
                        "flush_pending_skin_change",
                        "flush_pending_skin_change_for_profile",
                        "normalize_skin_texture",
                        "get_dragged_skin_data",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "mr-auth",
                InlinedPlugin::new()
                    .commands(&[
                        "modrinth_login",
                        "logout",
                        "get",
                        "cancel_modrinth_login",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "install",
                InlinedPlugin::new()
                    .commands(&[
                        "install_get_modpack_preview",
                        "install_create_instance",
                        "install_create_modpack_instance",
                        "install_import_instance",
                        "install_start_import_plan",
                        "install_cancel_import_plan",
                        "install_duplicate_instance",
                        "install_existing_instance",
                        "install_pack_to_existing_instance",
                        "install_job_list",
                        "install_job_get",
                        "install_job_retry",
                        "install_job_repair_cache_and_retry",
                        "install_job_resume",
                        "install_job_skip_missing_content",
                        "install_job_missing_files",
                        "install_job_scan_missing_files",
                        "install_job_retry_missing_file",
                        "install_job_import_missing_file",
                        "install_job_cancel",
                        "install_job_dismiss",
                        "install_job_support_details",
                        "download_job_list",
                        "download_job_get",
                        "download_job_retry",
                        "download_job_resume",
                        "download_job_cancel",
                        "download_job_delete",
                        "download_history_clear",
                        "download_job_support_details",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "process",
                InlinedPlugin::new()
                    .commands(&[
                        "process_get_all",
                        "process_get_by_instance_id",
                        "process_kill",
                        "process_wait_for",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "planet-minecraft",
                InlinedPlugin::new()
                    .commands(&[
                        "planet_minecraft_connector_available",
                        "planet_minecraft_search_projects",
                        "planet_minecraft_get_project",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "instance",
                InlinedPlugin::new()
                    .commands(&[
                        "instance_remove",
                        "instance_get_backup_repository_status",
                        "instance_move_backup_repository",
                        "instance_get_backup_config",
                        "instance_normalize_backup_exclusion",
                        "instance_enable_backups",
                        "instance_update_backup_exclusions",
                        "instance_disable_backups",
                        "instance_start_backup",
                        "instance_cancel_backup",
                        "instance_list_backup_operations",
                        "instance_list_backups",
                        "instance_delete_backup",
                        "instance_get_backup_restore_preview",
                        "instance_restore_backup",
                        "instance_get_backup_delete_summary",
                        "instance_create_direct_link",
                        "instance_sync_direct_links",
                        "instance_get",
                        "instance_get_many",
                        "instance_get_projects",
                        "instance_get_installed_project_ids",
                        "instance_get_install_candidates",
                        "instance_content",
                        "instance_get_content_items_by_paths",
                        "instance_get_content_items",
                        "instance_get_content_snapshot",
                        "instance_refresh_content",
                        "instance_plan_content_updates",
                        "instance_apply_content_update_plan",
                        "instance_plan_upgrade",
                        "instance_cancel_upgrade_planning",
                        "instance_get_upgrade_plan",
                        "instance_discard_upgrade_plan",
                        "instance_update_upgrade_resolution",
                        "instance_update_upgrade_resolutions",
                        "instance_reset_upgrade_resolution",
                        "instance_select_upgrade_solution",
                        "instance_resolve_custom_upgrade_solution",
                        "instance_execute_upgrade",
                        "instance_get_post_upgrade_notice",
                        "instance_dismiss_post_upgrade_notice",
                        "instance_get_dependencies_as_content_items",
                        "instance_get_linked_modpack_info",
                        "instance_get_linked_modpack_content",
                        "instance_get_optimal_jre_key",
                        "instance_list_core_components",
                        "instance_add_core_jar_mod",
                        "instance_replace_core_jar",
                        "instance_move_core_component",
                        "instance_set_core_component_enabled",
                        "instance_remove_core_component",
                        "instance_restore_core_component",
                        "instance_preview_core_jar",
                        "instance_install_mcarchive_modloader",
                        "instance_import_mcarchive_modloader",
                        "instance_get_full_path",
                        "instance_get_mod_full_path",
                        "instance_list",
                        "instance_set_pinned",
                        "instance_get_daily_playtime",
                        "instance_get_daily_playtime_details",
                        "instance_check_installed",
                        "instance_queue_content_batch",
                        "instance_queue_curseforge_world",
                        "instance_add_project_from_path",
                        "instance_import_world_save",
                        "instance_install_datapack_to_world",
                        "instance_install_datapack_to_world_bytes",
                        "instance_toggle_disable_project",
                        "instance_toggle_content_entry",
                        "instance_toggle_content_entries",
                        "instance_rollback_project",
                        "instance_remove_project",
                        "instance_remove_content_entry",
                        "instance_queue_content_change",
                        "instance_restore_pack_member_default",
                        "instance_update_managed_modrinth_version",
                        "instance_repair_managed_modrinth",
                        "instance_run",
                        "instance_kill",
                        "instance_edit",
                        "instance_cache_icon",
                        "instance_edit_icon",
                        "instance_list_screenshots",
                        "instance_list_all_screenshots",
                        "instance_list_synced_screenshots",
                        "instance_save_edited_screenshot",
                        "instance_list_screenshot_groups",
                        "instance_create_screenshot_group",
                        "instance_rename_screenshot_group",
                        "instance_delete_screenshot_group",
                        "instance_set_screenshot_group_memberships",
                        "instance_import_screenshot_groups",
                        "instance_delete_screenshots",
                        "instance_export_screenshots",
                        "instance_move_screenshots",
                        "instance_open_screenshot",
                        "instance_get_synced_options",
                        "instance_get_initialized_synced_options",
                        "instance_get_synced_options_overview",
                        "instance_get_synced_option_capabilities",
                        "instance_get_synced_option_join_preview",
                        "instance_set_synced_option",
                        "instance_set_instance_synced_option",
                        "instance_get_synced_command_history",
                        "instance_set_synced_command_history",
                        "instance_list_synced_servers",
                        "instance_update_synced_server",
                        "instance_remove_synced_server",
                        "instance_get_synced_game_options_config",
                        "instance_preview_synced_game_option_changes",
                        "instance_save_synced_game_option_changes",
                        "instance_list_game_options_sync_sources",
                        "instance_get_game_setting_locale_labels",
                        "instance_get_local_game_options_config",
                        "instance_preview_local_game_option_changes",
                        "instance_save_local_game_option_changes",
                        "instance_get_pack_sync_preview",
                        "instance_sync_pack",
                        "instance_desync_pack",
                        "instance_list_synced_packs",
                        "instance_upload_synced_pack",
                        "instance_set_synced_pack_enabled",
                        "instance_remove_synced_pack",
                        "instance_list_groups",
                        "instance_create_group",
                        "instance_rename_group",
                        "instance_delete_group",
                        "instance_set_group_order",
                        "instance_set_group_memberships",
                        "instance_update_group_memberships",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "settings",
                InlinedPlugin::new()
                    .commands(&[
                        "settings_get",
                        "settings_patch",
                        "privacy_get",
                        "privacy_set",
                        "telemetry_set",
                        "discord_rpc_set",
                        "cancel_directory_change",
                        "proxy_get",
                        "proxy_set",
                        "proxy_test",
                        "network_test",
                        "network_test_cancel",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "seed-map",
                InlinedPlugin::new()
                    .commands(&[
                        "seed_map_profiles",
                        "seed_map_render_tile",
                        "seed_map_find_features",
                        "seed_map_spawn",
                        "seed_map_biome_at",
                        "seed_map_scan_ores",
                        "seed_map_cancel_ore_scan",
                        "seed_map_read_level_dat",
                        "seed_map_structure_details",
                        "seed_map_container_loot",
                        "seed_map_underground_tile",
                        "seed_map_underground_volume",
                        "seed_map_cancel_underground",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "schematic-preview",
                InlinedPlugin::new()
                    .commands(&[
                        "schematic_preview_open",
                        "schematic_preview_list_instance_files",
                        "schematic_preview_list_instance_resource_packs",
                        "schematic_preview_read_chunk",
                        "schematic_preview_prepare_resources",
                        "schematic_preview_block_info",
                        "schematic_preview_apply_edits",
                        "schematic_preview_transform",
                        "schematic_preview_export_sponge",
                        "schematic_preview_export_litematic",
                        "schematic_preview_cancel",
                        "schematic_preview_close",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "tags",
                InlinedPlugin::new()
                    .commands(&[
                        "tags_get_categories",
                        "tags_get_report_types",
                        "tags_get_loaders",
                        "tags_get_game_versions",
                        "tags_get_donation_platforms",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "mcp",
                InlinedPlugin::new()
                    .commands(&[
                        "mcp_get_state",
                        "mcp_configure",
                        "mcp_respond",
                        "mcp_disconnect",
                        "mcp_ui_claim",
                        "mcp_ui_respond",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "ai",
                InlinedPlugin::new()
                    .commands(&[
                        "ai_get_catalog",
                        "ai_get_state",
                        "ai_update_settings",
                        "ai_update_provider",
                        "ai_set_api_key",
                        "ai_set_credential",
                        "ai_update_model",
                        "ai_remove_model",
                        "ai_fetch_models",
                        "ai_test_provider",
                        "ai_begin_oauth",
                        "ai_poll_oauth",
                        "ai_disconnect_oauth",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "translation",
                InlinedPlugin::new()
                    .commands(&[
                        "translation_get_settings",
                        "translation_update_settings",
                        "translation_test_provider",
                        "translation_translate",
                        "translation_clear_cache",
                        "translation_google_ip_pool_size",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "shortcuts",
                InlinedPlugin::new()
                    .commands(&["create_instance_shortcut"])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "utils",
                InlinedPlugin::new()
                    .commands(&[
                        "get_os",
                        "is_network_metered",
                        "should_disable_mouseover",
                        "highlight_in_folder",
                        "open_path",
                        "show_launcher_logs_folder",
                        "export_error_logs",
                        "export_launcher_logs",
                        "show_app_db_backups_folder",
                        "progress_bars_list",
                        "get_opening_command",
                        "get_minecraft_news",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "storage",
                InlinedPlugin::new()
                    .commands(&[
                        "storage_scan",
                        "storage_cache_clear",
                        "storage_meta_analyze",
                        "storage_meta_clean",
                        "storage_open_paths",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "system-accent",
                InlinedPlugin::new()
                    .commands(&["system_accent_color"])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "files",
                InlinedPlugin::new()
                    .commands(&[
                        "file_extract_zip",
                        "file_save_as",
                        "file_read_dragged_file",
                        "screenshot_thumbnail",
                        "studio_read_text",
                        "studio_read_binary",
                        "studio_write_binary",
                        "studio_trash",
                        "studio_watch_register",
                        "studio_watch_unregister",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "fonts",
                InlinedPlugin::new()
                    .commands(&["fonts_get_system_fonts"])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "drop",
                InlinedPlugin::new()
                    .commands(&[
                        "drop_classify",
                        "drop_classify_extract",
                        "drop_extract_zip_to_temp",
                        "drop_scan_launcher_instances",
                        "drop_resolve_gamedirs",
                        "drop_remove_temp_dir",
                        "drop_detect_file_lock",
                        "drop_extract_mod_metadata",
                        "drop_lookup_mod_hash",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "worlds",
                InlinedPlugin::new()
                    .commands(&[
                        "get_recent_worlds",
                        "get_favorite_worlds",
                        "get_instance_worlds",
                        "get_singleplayer_world",
                        "set_world_display_status",
                        "get_world_level_data",
                        "update_world_settings",
                        "rename_world",
                        "reset_world_icon",
                        "backup_world",
                        "delete_world",
                        "add_server_to_instance",
                        "edit_server_in_instance",
                        "remove_server_from_instance",
                        "get_instance_protocol_version",
                        "get_server_status",
                        "start_join_singleplayer_world",
                        "start_join_server",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "terracotta",
                InlinedPlugin::new()
                    .commands(&[
                        "terracotta_get_state",
                        "terracotta_get_meta",
                        "terracotta_start",
                        "terracotta_stop",
                        "terracotta_host",
                        "terracotta_join",
                        "terracotta_reset",
                        "terracotta_get_platform_key",
                        "terracotta_check_for_update",
                        "terracotta_download",
                        "terracotta_update",
                        "terracotta_get_player_name",
                        "terracotta_get_diagnostic_report",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "multiplayer",
                InlinedPlugin::new()
                    .commands(&[
                        "multiplayer_get_state",
                        "multiplayer_get_nodes",
                        "multiplayer_get_detected_ports",
                        "multiplayer_download_hongshi",
                        "multiplayer_switch_provider",
                        "multiplayer_prepare_terracotta",
                        "multiplayer_host",
                        "multiplayer_join",
                        "multiplayer_stop",
                        "multiplayer_reset",
                        "multiplayer_get_player_name",
                        "multiplayer_open_hongshi_logs",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "servers",
                InlinedPlugin::new()
                    .commands(&[
                        "servers_list",
                        "servers_get",
                        "servers_create",
                        "servers_update_settings",
                        "servers_set_icon",
                        "servers_set_linked_world",
                        "servers_is_local_address",
                        "servers_wait_until_ready",
                        "servers_delete",
                        "servers_read_file",
                        "servers_write_file",
                        "servers_download_file",
                        "servers_install_forge",
                        "servers_install_neoforge",
                        "servers_install_modpack",
                        "servers_start",
                        "servers_send_console_input",
                        "servers_resize_console",
                        "servers_stop",
                        "servers_kill",
                        "servers_kill_port_process",
                        "servers_port_process",
                        "servers_get_log_buffer",
                        "servers_get_log_delta",
                        "servers_get_console_delta",
                        "servers_clear_log",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "friends",
                InlinedPlugin::new()
                    .commands(&[
                        "friends",
                        "friend_statuses",
                        "add_friend",
                        "remove_friend",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "telemetry",
                InlinedPlugin::new()
                    .commands(&["notify_online"])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            )
            .plugin(
                "mod-translation",
                InlinedPlugin::new()
                    .commands(&[
                        "mod_translation_analyze",
                        "mod_translation_translate",
                        "mod_translation_cancel",
                        "mod_translation_cancel_analysis",
                        "mod_translation_list_tasks",
                        "mod_translation_get_task",
                        "mod_translation_dismiss_task",
                    ])
                    .default_permission(
                        DefaultPermissionRule::AllowAllCommands,
                    ),
            ),
    )
    .expect("Failed to run tauri-build");
}
