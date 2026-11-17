//! Stable first-party event names. Extensions should use names of the form
//! `ext.<extension-id>.<event>`.

/// An extension or instrumented subsystem started.
pub const EXTENSION_STARTED: &str = "ext.extension.started";
/// An extension or instrumented subsystem stopped.
pub const EXTENSION_STOPPED: &str = "ext.extension.stopped";
/// An extension operation failed.
pub const EXTENSION_ERROR: &str = "ext.extension.error";

/// The launcher reached a startup phase.
pub const APP_STARTUP_PHASE: &str = "app.startup.phase";
/// The launcher finished startup.
pub const APP_STARTED: &str = "app.started";
/// The launcher began shutting down.
pub const APP_SHUTDOWN: &str = "app.shutdown";
/// The main window became visible.
pub const APP_WINDOW_VISIBLE: &str = "app.window.visible";

/// An instance launch completed or failed.
pub const INSTANCE_LAUNCH: &str = "instance.launch";
/// An instance stopped.
pub const INSTANCE_STOP: &str = "instance.stop";

/// An install job completed or failed.
pub const INSTALL_JOB: &str = "install.job";
/// An install phase completed or failed.
pub const INSTALL_PHASE: &str = "install.phase";

/// A frontend route navigation completed.
pub const UI_NAVIGATION: &str = "ui.navigation";
/// A frontend action completed.
pub const UI_ACTION: &str = "ui.action";
/// A frontend error was observed.
pub const UI_ERROR: &str = "ui.error";
