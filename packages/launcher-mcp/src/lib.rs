//! Local MCP transport and launcher session lifecycle.
mod control;
mod http;
mod info;
mod protocol;
mod server;
mod session;
mod tools;

pub use info::LauncherInfo;
pub use server::Server;
pub use session::{Connection, Snapshot};

#[cfg(test)]
mod tests;
