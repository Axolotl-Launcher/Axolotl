//! Provider-neutral contracts for resource services and desktop IPC.
//!
//! Project identity, release identity, exact bytes, and installed entries are
//! separate concepts. Search candidates never authorize an installation,
//! dependency satisfaction, update, or export substitution.

mod catalog;
mod export;
mod identity;
pub mod ipc;
mod operation;
mod planning;

pub use catalog::*;
pub use export::*;
pub use identity::*;
pub use operation::*;
pub use planning::*;

pub const RESOURCE_CONTRACT_VERSION: u32 = 1;

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize,
)]
#[serde(try_from = "u32", into = "u32")]
pub struct ResourceContractVersion;

impl TryFrom<u32> for ResourceContractVersion {
    type Error = String;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value == RESOURCE_CONTRACT_VERSION {
            Ok(Self)
        } else {
            Err(format!("unsupported resource contract version {value}"))
        }
    }
}

impl From<ResourceContractVersion> for u32 {
    fn from(_: ResourceContractVersion) -> Self {
        RESOURCE_CONTRACT_VERSION
    }
}
