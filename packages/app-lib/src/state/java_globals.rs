use serde::{Deserialize, Serialize};

use crate::state::documents::Documents;
use crate::state::documents::java::{self, JavaRuntime};

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Clone)]
pub struct JavaVersion {
    pub parsed_version: u32,
    pub version: String,
    pub architecture: String,
    pub path: String,
    pub distribution: Option<String>,
}

impl JavaVersion {
    /// The default runtime of `major_version`, or nothing when that version has
    /// no default or its default is not installed.
    pub(crate) async fn get(
        major_version: u32,
        documents: &Documents<'_>,
    ) -> crate::Result<Option<JavaVersion>> {
        Ok(java::read(documents)
            .await?
            .default_for(major_version)
            .map(Self::from_runtime))
    }

    /// Every default runtime the launcher holds, the highest version first.
    pub(crate) async fn get_all_defaults(
        documents: &Documents<'_>,
    ) -> crate::Result<Vec<Self>> {
        Ok(java::read(documents)
            .await?
            .installed_defaults()
            .into_iter()
            .map(Self::from_runtime)
            .collect())
    }

    pub(crate) async fn get_all(
        documents: &Documents<'_>,
    ) -> crate::Result<Vec<Self>> {
        Ok(java::read(documents)
            .await?
            .runtimes
            .iter()
            .map(Self::from_runtime)
            .collect())
    }

    pub(crate) async fn upsert(
        &self,
        documents: &Documents<'_>,
    ) -> crate::Result<()> {
        java::save(documents, self.to_runtime()).await
    }

    /// Records the runtime and makes it the default of its major version.
    pub(crate) async fn upsert_as_default(
        &self,
        documents: &Documents<'_>,
    ) -> crate::Result<()> {
        java::save_as_default(documents, self.to_runtime()).await
    }

    /// Records every runtime it is handed, in one write.
    pub(crate) async fn save_all(
        documents: &Documents<'_>,
        versions: &[Self],
    ) -> crate::Result<()> {
        java::save_all(documents, versions.iter().map(Self::to_runtime)).await
    }

    pub(crate) async fn remove_default(
        major_version: u32,
        documents: &Documents<'_>,
    ) -> crate::Result<()> {
        java::remove_default(documents, major_version).await
    }

    /// Forgets a runtime, and with it every default that named it.
    pub(crate) async fn delete(
        path: &str,
        documents: &Documents<'_>,
    ) -> crate::Result<()> {
        java::forget(documents, path).await
    }

    fn from_runtime(runtime: &JavaRuntime) -> Self {
        Self {
            parsed_version: runtime.major_version,
            version: runtime.full_version.clone(),
            architecture: runtime.architecture.clone(),
            path: runtime.path.clone(),
            distribution: runtime.distribution.clone(),
        }
    }

    fn to_runtime(&self) -> JavaRuntime {
        JavaRuntime {
            path: self.path.clone(),
            major_version: self.parsed_version,
            full_version: self.version.clone(),
            architecture: self.architecture.clone(),
            distribution: self.distribution.clone(),
        }
    }
}
