//! Shared launcher tools, independent of MCP transport and control sessions.
mod backend;
mod info;
pub mod log_text;
mod operations;
pub mod registry;
mod result;
mod runtime;
mod scoped;
pub mod world_input;

pub use backend::*;
pub use info::LauncherInfo;
pub use registry::{Access, ExecutionPolicy, ToolDefinition};
pub use result::{ContentBlock, ToolOutput};
pub use runtime::{ExecutionContext, ExecutionScope, InputError, ToolRuntime};
pub use scoped::ScopedRegistry;

#[cfg(test)]
mod tests;
