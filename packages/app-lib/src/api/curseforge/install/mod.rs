//! Content installation, dependency resolution and plan execution.

mod dependencies;
mod edges;
mod entry;
mod fallback;
mod plan;

pub(crate) use self::dependencies::*;
pub(crate) use self::edges::*;
pub(crate) use self::entry::*;
pub use self::entry::{
    CurseForgeModrinthFallbackPreview, install_file,
    install_file_with_reporter, install_world_with_reporter,
};
pub(crate) use self::fallback::*;
pub(crate) use self::plan::*;
