//! Local MCP transport and launcher session lifecycle.
mod catalog;
mod control;
mod http;
mod protocol;
mod server;
mod session;
mod tools;

pub use launcher_tools::*;
pub use server::Server;
pub use session::{Connection, Snapshot};

#[cfg(test)]
mod tests;
