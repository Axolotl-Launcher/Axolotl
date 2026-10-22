use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error(
    "resource identifiers must be nonempty with no surrounding whitespace or control characters"
)]
pub struct InvalidResourceIdentifier;

macro_rules! identifier {
    ($name:ident) => {
        #[derive(
            Clone,
            Debug,
            Eq,
            Hash,
            Ord,
            PartialEq,
            PartialOrd,
            Serialize,
            Deserialize,
        )]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub fn new(
                value: impl Into<String>,
            ) -> Result<Self, InvalidResourceIdentifier> {
                let value = value.into();
                if value.is_empty()
                    || value.trim() != value
                    || value.chars().any(char::is_control)
                {
                    return Err(InvalidResourceIdentifier);
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = InvalidResourceIdentifier;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

identifier!(ResourceId);
identifier!(SourceId);
identifier!(SourceProjectId);
identifier!(SourceReleaseId);
identifier!(SourceArtifactId);
identifier!(InstalledEntryId);
identifier!(HistoryId);
identifier!(PlanId);
identifier!(SessionId);
identifier!(RequestId);
identifier!(JobId);
identifier!(SelectionId);

impl SourceId {
    pub fn modrinth() -> Self {
        Self("modrinth".into())
    }

    pub fn curseforge() -> Self {
        Self("curseforge".into())
    }

    pub fn mcarchive() -> Self {
        Self("mcarchive".into())
    }

    pub fn planet_minecraft() -> Self {
        Self("planet_minecraft".into())
    }
}

/// The source-qualified stable project ID. Slugs and display names are metadata.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceProjectRef {
    pub source: SourceId,
    pub project_id: SourceProjectId,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceReleaseRef {
    pub project: SourceProjectRef,
    pub release_id: SourceReleaseId,
}

/// Identifies one file inside a release, including multi-file Modrinth releases.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceArtifactRef {
    pub release: SourceReleaseRef,
    pub artifact_id: SourceArtifactId,
}

/// Unconfirmed search groups deliberately cannot be used as resource references.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ResourceRef {
    Resource { id: ResourceId },
    Source { project: SourceProjectRef },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IdentityStatus {
    Confirmed,
    Unconfirmed,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CandidateBasis {
    ConfirmedIdentity,
    WikiSourcePair,
    ExactTitle,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSearchEntry {
    pub query_entry_id: String,
    pub resource_id: Option<ResourceId>,
    pub default_project: SourceProjectRef,
    pub candidates: Vec<SourceProjectRef>,
    pub status: IdentityStatus,
    pub basis: Vec<CandidateBasis>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HashAlgorithm {
    Sha1,
    Sha256,
    Sha512,
    Md5,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentHash {
    pub algorithm: HashAlgorithm,
    pub value: String,
}

/// Fingerprints are lookup hints, never exact-byte evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactIntegrity {
    pub size: Option<u64>,
    pub hashes: Vec<ContentHash>,
}

/// A verified common primary artifact establishes project identity without
/// claiming that every release or every artifact of those projects is equal.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedArtifactMatch {
    pub left: SourceArtifactRef,
    pub right: SourceArtifactRef,
    pub integrity: ArtifactIntegrity,
    pub verified_at: chrono::DateTime<chrono::Utc>,
}
