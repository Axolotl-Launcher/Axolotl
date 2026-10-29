//! What the collections promise, exercised against a database of their own.
//!
//! Each collection answers for the verbs it inherits from `Collection`, so its
//! tests live here together instead of inside the module that writes the
//! statements: a change to a verb is answered once, in one place.

mod atom;
mod cache;
mod content_refresh;
mod content_snapshot;
mod crash_analysis_mod_snapshots;
mod credentials;
mod favorites;
mod game_option_locale_origins;
mod game_option_pack_bases;
mod game_option_update_state;
mod google_translate_ip_cache;
mod install_jobs;
mod install_store;
mod instance_sync_checkpoints;
mod instance_sync_preferences;
mod java_globals;
mod join_log;
mod modrinth_sessions;
mod official_login_proof;
mod resource_cache;
mod resource_favorites;
mod resource_plans;
mod resource_source_projects;
mod secrets;
mod settings;
mod settings_api;
mod settings_db;
mod settings_owned_columns;
mod shared_logs;
mod store_blobs;
mod store_retained_refs;
mod support;
mod sync_feature_settings;
mod synced_game_option_preferences;
mod synced_game_option_state;
mod synced_game_option_values;
mod telemetry;
mod translation_cache;
