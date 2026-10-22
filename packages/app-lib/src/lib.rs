/*!
# Theseus

Theseus is a library which provides utilities for launching minecraft, creating Modrinth mod packs,
and launching Modrinth mod packs
*/
#![warn(unused_import_braces)]
#![deny(unused_must_use)]
#![recursion_limit = "256"]

#[macro_use]
mod util;

mod api;
pub mod brand;
mod error;
mod event;
pub mod install;
mod launcher;
mod logger;
pub mod mod_metadata;
pub mod resources;
pub mod state;
pub mod storage;
pub mod telemetry;

pub use api::*;
pub use error::*;
pub use event::{
    EventState, LoadingBar, LoadingBarType, emit::emit_loading,
    emit::init_loading,
};
pub use logger::{
    DEFAULT_LOG_LEVEL, filter_log_contents, set_log_level, start_logger,
};
pub use state::db::{
    UpdateChannelState, backup_current_app_db_for_update, beta_database_exists,
    copy_database_between_channels, copy_release_database_to_beta,
    current_app_database_path, default_update_channel,
    read_update_channel_state, update_channel_state_file_path,
};
pub use state::{DirectoryInfo, State};
pub use storage::*;
pub use util::fetch::{DownloadReason, build_proxied_client};
pub use util::file_lock::{LockingProcess, get_locking_processes};
pub use util::network_diagnostics::{
    NetworkTestMode, NetworkTestResult, NetworkTestTarget,
    test_network_connection,
};
pub use util::platform::is_process_elevated;
pub use util::proxy::{
    ProxyConfig, ProxyMode, ProxyTestResult, test_proxy_connection,
};
pub use util::symlink::SymlinkCapability;

pub fn launcher_user_agent() -> String {
    brand::user_agent(env!("CARGO_PKG_VERSION"), std::env::consts::OS)
}
