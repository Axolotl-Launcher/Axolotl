//! Managed dedicated Minecraft servers: manifests, downloads, and process control.
//! Each server lives in its own directory under the launcher's `servers` folder
//! and is described by an `axolotl-server.json` manifest.

mod detect;
mod files;
mod import;
pub use detect::{ServerImportPreview, inspect_server_import};
pub use import::{import_external_server, retry_external_server};
mod forge;
mod lifecycle;
mod link;
mod logs;
mod manage;
mod manifest;
mod modpack;
mod ports;

pub use self::files::{download_file, read_file, write_file};
pub use self::forge::install_forge;
pub use self::lifecycle::{
    kill, resize_console, send_command, send_console_input, start, stop,
};
pub use self::link::{
    DEFAULT_READY_TIMEOUT_MS, is_local_address, wait_until_ready,
};
pub use self::logs::{clear_log, get_log_buffer, get_log_delta};
pub use self::manage::{
    create, delete, get, list, set_icon, set_linked_world, update_settings,
};
pub use self::manifest::{
    LinkedWorld, ModpackInfo, ServerInfo, ServerManifest,
};
pub use self::modpack::install_modpack;
pub use self::ports::{PortProcessInfo, kill_port_process, port_process};
pub use crate::state::{LogCursor, LogDelta};
