//! Managed modpack updates, backups and staged upgrades.

mod managed;
mod staged;
mod switch;

pub use self::managed::update_managed_modpack;
pub(crate) use self::managed::*;
pub(crate) use self::staged::*;
pub use self::switch::switch_installed_file_version;
