//! `java.json`: the Java runtimes the launcher has recorded and which runtime
//! answers for each major version.
//!
//! The two were two tables with the defaults pointing at a runtime row, and the
//! pointer is the reason they move together: a default names a runtime, so the
//! reader honours a default only while the document still holds that runtime,
//! and every write here keeps both halves in step.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::{Documents, Stored, WRITES, composed, without_shipped};
use crate::state::store;

pub(crate) const DOCUMENT: &str = "java";

/// Bumped whenever the key set of this document changes, including when a key
/// is dropped. The document records it; a newer document is still read.
const SCHEMA_VERSION: u32 = 1;

/// An installed runtime, identified by the path of its executable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct JavaRuntime {
    pub(crate) path: String,
    pub(crate) major_version: u32,
    pub(crate) full_version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) architecture: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) distribution: Option<String>,
}

/// Which runtime answers for a major version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct JavaDefault {
    pub(crate) major_version: u32,
    pub(crate) path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct JavaDocument {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) runtimes: Vec<JavaRuntime>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) defaults: Vec<JavaDefault>,
}

impl JavaDocument {
    pub(crate) fn runtime(&self, path: &str) -> Option<&JavaRuntime> {
        self.runtimes.iter().find(|runtime| runtime.path == path)
    }

    /// The runtime a major version answers with, which is a default only while
    /// the document still holds the runtime it names.
    pub(crate) fn default_for(
        &self,
        major_version: u32,
    ) -> Option<&JavaRuntime> {
        let default = self
            .defaults
            .iter()
            .find(|default| default.major_version == major_version)?;
        self.runtime(&default.path)
    }

    /// Every default the document can honour, the highest major version first.
    pub(crate) fn installed_defaults(&self) -> Vec<&JavaRuntime> {
        let mut runtimes = self
            .defaults
            .iter()
            .filter_map(|default| self.runtime(&default.path))
            .collect::<Vec<_>>();
        runtimes.sort_by(|left, right| {
            right.major_version.cmp(&left.major_version)
        });
        runtimes
    }

    fn insert(&mut self, runtime: JavaRuntime) {
        match self
            .runtimes
            .iter_mut()
            .find(|existing| existing.path == runtime.path)
        {
            Some(existing) => *existing = runtime,
            None => self.runtimes.push(runtime),
        }
    }

    fn set_default(&mut self, major_version: u32, path: &str) {
        match self
            .defaults
            .iter_mut()
            .find(|default| default.major_version == major_version)
        {
            Some(default) => default.path = path.to_string(),
            None => self.defaults.push(JavaDefault {
                major_version,
                path: path.to_string(),
            }),
        }
    }

    fn remove_default(&mut self, major_version: u32) {
        self.defaults
            .retain(|default| default.major_version != major_version);
    }

    /// Forgets a runtime, and with it every default that named it: a default is
    /// not a preference of its own but a pointer at a runtime.
    fn forget(&mut self, path: &str) {
        self.runtimes.retain(|runtime| runtime.path != path);
        self.defaults.retain(|default| default.path != path);
    }

    /// Rewrites the recorded paths a directory move made stale, dropping the
    /// runtimes whose new path is an incomplete installation. Reports the paths
    /// of the runtimes it dropped.
    ///
    /// A runtime the move would write over is refused rather than let it take
    /// the place of the one already recorded there.
    fn move_installations(
        &mut self,
        from: &str,
        to: &str,
        is_incomplete: &impl Fn(&str) -> bool,
    ) -> crate::Result<Vec<String>> {
        let mut dropped = Vec::new();
        let mut kept = Vec::with_capacity(self.runtimes.len());
        for runtime in std::mem::take(&mut self.runtimes) {
            let Some(suffix) = runtime.path.strip_prefix(from) else {
                kept.push(runtime);
                continue;
            };
            let path = format!("{to}{suffix}");
            if is_incomplete(&path) {
                dropped.push(path);
                continue;
            }
            if kept.iter().any(|moved| moved.path == path) {
                return Err(crate::ErrorKind::InputError(format!(
                    "A Java runtime is already recorded at {path}"
                ))
                .into());
            }
            kept.push(JavaRuntime { path, ..runtime });
        }
        self.runtimes = kept;
        for default in &mut self.defaults {
            if let Some(suffix) = default.path.strip_prefix(from) {
                default.path = format!("{to}{suffix}");
            }
        }
        let runtimes = &self.runtimes;
        let mut kept_defaults = std::mem::take(&mut self.defaults);
        kept_defaults.retain(|default| {
            runtimes.iter().any(|runtime| runtime.path == default.path)
        });
        self.defaults = kept_defaults;

        Ok(dropped)
    }
}

/// The document, composed of the keys the deployment ships and the ones the
/// local document names, or the rows while neither of them answers.
///
/// A layer answers a whole key, and the defaults are pointers at runtimes, so a
/// composed document can name a runtime it does not hold. That is what
/// `default_for` already answers nothing for: the pairing is kept by what the
/// document holds, not by which layer wrote it.
pub(crate) async fn read(
    documents: &Documents<'_>,
) -> crate::Result<JavaDocument> {
    composed(documents, DOCUMENT, SCHEMA_VERSION, async {
        from_database(documents.pool()).await
    })
    .await
}

async fn from_database(pool: &SqlitePool) -> crate::Result<JavaDocument> {
    let runtimes = store::java_versions::read_legacy_java_versions(pool)
        .await?
        .into_iter()
        .map(|(path, record)| JavaRuntime {
            path,
            major_version: record.major_version,
            full_version: record.full_version,
            architecture: record.architecture,
            distribution: record.distribution,
        })
        .collect();
    let defaults =
        store::java_default_versions::read_legacy_java_default_versions(pool)
            .await?
            .into_iter()
            .map(|(major_version, record)| JavaDefault {
                major_version,
                path: record.path,
            })
            .collect();
    Ok(JavaDocument { runtimes, defaults })
}

/// Writes the document the rows hand over to, unless one is already in place.
pub(crate) async fn hand_over_if_missing(documents: &Documents<'_>) -> bool {
    if documents
        .stored::<JavaDocument>(DOCUMENT, SCHEMA_VERSION)
        .await
        .is_present()
    {
        return true;
    }
    let document = match from_database(documents.pool()).await {
        Ok(document) => document,
        Err(error) => {
            tracing::warn!(%error, "Could not read the Java runtime rows");
            return false;
        }
    };
    let Some(document) =
        without_shipped(documents, DOCUMENT, SCHEMA_VERSION, document).await
    else {
        return false;
    };
    match documents.write(DOCUMENT, SCHEMA_VERSION, &document).await {
        Ok(()) => true,
        Err(error) => {
            tracing::warn!(
                path = %documents.path(DOCUMENT).display(),
                %error,
                "Failed to save the Java document"
            );
            false
        }
    }
}

pub(crate) async fn save(
    documents: &Documents<'_>,
    runtime: JavaRuntime,
) -> crate::Result<()> {
    update(documents, |document| {
        document.insert(runtime);
    })
    .await
}

/// Records a runtime and makes it the default of its major version, which is
/// what installing or choosing a runtime amounts to.
pub(crate) async fn save_as_default(
    documents: &Documents<'_>,
    runtime: JavaRuntime,
) -> crate::Result<()> {
    update(documents, |document| {
        document.set_default(runtime.major_version, &runtime.path);
        document.insert(runtime);
    })
    .await
}

/// Records every runtime it is handed, in one write: a scan finds a batch of
/// them at once and none of the defaults change.
pub(crate) async fn save_all(
    documents: &Documents<'_>,
    runtimes: impl IntoIterator<Item = JavaRuntime>,
) -> crate::Result<()> {
    update(documents, |document| {
        for runtime in runtimes {
            document.insert(runtime);
        }
    })
    .await
}

pub(crate) async fn remove_default(
    documents: &Documents<'_>,
    major_version: u32,
) -> crate::Result<()> {
    update(documents, |document| {
        document.remove_default(major_version);
    })
    .await
}

pub(crate) async fn forget(
    documents: &Documents<'_>,
    path: &str,
) -> crate::Result<()> {
    update(documents, |document| {
        document.forget(path);
    })
    .await
}

/// Rewrites the recorded paths a launcher directory move made stale, dropping
/// the installations whose new path is incomplete. Reports the dropped paths.
///
/// A launcher that has no document yet is answered by its rows, and the move
/// leaves the document holding their rewritten paths.
///
/// The draft is a composed document, so it carries the keys the deployment
/// answers as well as the local and row ones: copied into the local document
/// they would shadow every later change of the deployment's own, so they are
/// dropped before the write whichever of the two the draft came from. A key the
/// user's own document carries is not one of them and stays as it stands, and a
/// deployment document this build cannot read leaves the local one alone: what
/// it answers is unknown, so the move is reported without writing.
pub(crate) async fn move_installations(
    documents: &Documents<'_>,
    from: &str,
    to: &str,
    is_incomplete: impl Fn(&str) -> bool,
) -> crate::Result<Vec<String>> {
    let _writing = WRITES.lock().await;
    let mut document = read(documents).await?;
    let dropped = document.move_installations(from, to, &is_incomplete)?;
    let Some(document) =
        without_shipped(documents, DOCUMENT, SCHEMA_VERSION, document).await
    else {
        return Ok(dropped);
    };
    documents.write(DOCUMENT, SCHEMA_VERSION, &document).await?;

    Ok(dropped)
}

async fn update(
    documents: &Documents<'_>,
    apply: impl FnOnce(&mut JavaDocument),
) -> crate::Result<()> {
    let _writing = WRITES.lock().await;
    let mut document = match documents
        .stored::<JavaDocument>(DOCUMENT, SCHEMA_VERSION)
        .await
    {
        // A newer build owns the shape of this document: writing it would
        // lower it to the keys this build knows.
        Stored::Newer { .. } => return Ok(()),
        Stored::Ready { document, .. } => document,
        Stored::Missing | Stored::Unreadable => {
            let draft = from_database(documents.pool()).await?;
            let Some(draft) =
                without_shipped(documents, DOCUMENT, SCHEMA_VERSION, draft)
                    .await
            else {
                return Ok(());
            };
            draft
        }
    };
    apply(&mut document);
    documents.write(DOCUMENT, SCHEMA_VERSION, &document).await
}

#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    use super::*;
    use crate::state::documents::test_support::{
        create_java_tables, layered, pool, ship,
    };
    use crate::state::store::Collection;
    use crate::state::store::Handle;
    use crate::state::store::java_default_versions::{
        JavaDefaultVersionRecord, JavaDefaultVersions,
    };
    use crate::state::store::java_versions::{JavaVersionRecord, JavaVersions};

    fn documents<'a>(
        root: &tempfile::TempDir,
        pool: &'a SqlitePool,
    ) -> Documents<'a> {
        Documents::at(root.path().to_path_buf(), pool)
    }

    async fn installed_pool() -> SqlitePool {
        let pool = pool().await;
        create_java_tables(&pool).await;
        let handle = Handle::new(pool.clone());
        let runtimes = [
            (
                "/java/temurin-21/bin/java",
                JavaVersionRecord {
                    major_version: 21,
                    full_version: "21.0.1".to_string(),
                    architecture: "x86_64".to_string(),
                    distribution: Some("temurin".to_string()),
                },
            ),
            (
                "/java/temurin-17/bin/java",
                JavaVersionRecord {
                    major_version: 17,
                    full_version: "17.0.9".to_string(),
                    architecture: "x86_64".to_string(),
                    distribution: None,
                },
            ),
        ];
        for (path, record) in runtimes {
            JavaVersions::set(&handle, &path.to_string(), Some(&record))
                .await
                .unwrap();
        }
        JavaDefaultVersions::set(
            &handle,
            &21,
            Some(&JavaDefaultVersionRecord {
                path: "/java/temurin-21/bin/java".to_string(),
            }),
        )
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn a_missing_document_reads_both_rows() {
        let pool = installed_pool().await;
        let root = tempfile::tempdir().unwrap();
        let document = read(&documents(&root, &pool)).await.unwrap();

        assert_eq!(document.runtimes.len(), 2);
        assert_eq!(
            document
                .default_for(21)
                .map(|runtime| runtime.path.as_str()),
            Some("/java/temurin-21/bin/java")
        );
        assert!(document.default_for(17).is_none());
        assert_eq!(
            document
                .installed_defaults()
                .iter()
                .map(|runtime| runtime.major_version)
                .collect::<Vec<_>>(),
            [21]
        );
    }

    #[tokio::test]
    async fn a_document_that_leaves_a_key_out_gives_it_the_default() {
        let pool = installed_pool().await;
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("java.json"),
            r#"{"schema_version":1,"data":{}}"#,
        )
        .unwrap();

        let document = read(&documents(&root, &pool)).await.unwrap();

        assert_eq!(
            document,
            JavaDocument::default(),
            "a key the document leaves out is its default, never the row's value"
        );
        assert!(document.runtimes.is_empty());
        assert!(document.default_for(21).is_none());
        assert!(document.installed_defaults().is_empty());
    }

    #[tokio::test]
    async fn a_default_without_its_runtime_is_not_a_default() {
        let pool = installed_pool().await;
        let root = tempfile::tempdir().unwrap();
        let documents = documents(&root, &pool);
        std::fs::write(
            documents.path(DOCUMENT),
            r#"{"schema_version":1,"data":{}}"#,
        )
        .unwrap();

        save_as_default(
            &documents,
            JavaRuntime {
                path: "/java/temurin-21/bin/java".to_string(),
                major_version: 21,
                full_version: "21.0.1".to_string(),
                architecture: "x86_64".to_string(),
                distribution: None,
            },
        )
        .await
        .unwrap();

        let document = read(&documents).await.unwrap();
        assert_eq!(
            document
                .default_for(21)
                .map(|runtime| runtime.major_version),
            Some(21),
            "the fixture pairs the runtime with its default"
        );

        forget(&documents, "/java/temurin-21/bin/java")
            .await
            .unwrap();

        let document = read(&documents).await.unwrap();
        assert!(document.runtime("/java/temurin-21/bin/java").is_none());
        assert!(
            document.defaults.is_empty(),
            "a default is a pointer at a runtime, so forgetting the runtime forgets it"
        );
        assert!(document.default_for(21).is_none());
        assert!(document.installed_defaults().is_empty());
    }

    #[tokio::test]
    async fn a_default_written_before_its_runtime_reads_as_none() {
        let pool = installed_pool().await;
        let root = tempfile::tempdir().unwrap();
        let documents = documents(&root, &pool);

        std::fs::write(
            documents.path(DOCUMENT),
            r#"{"schema_version":1,"data":{"defaults":[{"major_version":21,"path":"/gone/bin/java"}]}}"#,
        )
        .unwrap();

        let document = read(&documents).await.unwrap();
        assert!(
            document.default_for(21).is_none(),
            "a default naming a runtime the document does not hold answers nothing"
        );
        assert!(document.installed_defaults().is_empty());
    }

    /// A deployment's document replaces the list it names, and the default it
    /// ships keeps pointing at a runtime the composed document holds.
    #[tokio::test]
    async fn a_deployment_document_pairs_its_own_runtime_with_its_default() {
        let pool = installed_pool().await;
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        ship(
            &deployment,
            DOCUMENT,
            r#"{"runtimes":[{"path":"/deploy/jdk/bin/java","major_version":21,"full_version":"21.0.2"}],"defaults":[{"major_version":21,"path":"/deploy/jdk/bin/java"}]}"#,
        );

        let document =
            read(&layered(&local, &deployment, &pool)).await.unwrap();

        assert_eq!(
            document
                .runtimes
                .iter()
                .map(|runtime| runtime.path.as_str())
                .collect::<Vec<_>>(),
            ["/deploy/jdk/bin/java"],
            "the deployment answers the runtimes it names in place of the rows'"
        );
        assert_eq!(
            document
                .default_for(21)
                .map(|runtime| runtime.path.as_str()),
            Some("/deploy/jdk/bin/java"),
            "a default answers only while the composed document holds the runtime it names"
        );
        assert!(document.default_for(17).is_none());
    }

    /// A default is a pointer, and the layers are laid key by key: a deployment
    /// that answers the defaults alone leaves them pointing at a runtime the
    /// composed document does not hold, which reads as no default at all.
    #[tokio::test]
    async fn a_deployment_default_without_its_runtime_is_not_a_default() {
        let pool = installed_pool().await;
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        ship(
            &deployment,
            DOCUMENT,
            r#"{"defaults":[{"major_version":21,"path":"/deploy/jdk/bin/java"}]}"#,
        );

        let document =
            read(&layered(&local, &deployment, &pool)).await.unwrap();

        assert_eq!(
            document.runtimes.len(),
            2,
            "a key the deployment leaves out is still the rows'"
        );
        assert!(
            document.default_for(21).is_none(),
            "the pairing is kept by what the document holds, not by which layer wrote the pointer"
        );
        assert!(document.installed_defaults().is_empty());
    }

    #[tokio::test]
    async fn a_write_only_carries_the_keys_that_differ_from_the_default() {
        let pool = installed_pool().await;
        let root = tempfile::tempdir().unwrap();
        let documents = documents(&root, &pool);
        std::fs::write(
            documents.path(DOCUMENT),
            r#"{"schema_version":1,"data":{}}"#,
        )
        .unwrap();

        save_as_default(
            &documents,
            JavaRuntime {
                path: "/java/temurin-21/bin/java".to_string(),
                major_version: 21,
                full_version: "21.0.2".to_string(),
                architecture: String::new(),
                distribution: None,
            },
        )
        .await
        .unwrap();

        let contents =
            std::fs::read_to_string(documents.path(DOCUMENT)).unwrap();
        let stored: serde_json::Value =
            serde_json::from_str(&contents).unwrap();
        assert_eq!(
            stored["data"]["runtimes"],
            serde_json::json!([{
                "major_version": 21,
                "full_version": "21.0.2",
                "path": "/java/temurin-21/bin/java",
            }])
        );
        assert_eq!(
            stored["data"]["defaults"],
            serde_json::json!([{
                "major_version": 21,
                "path": "/java/temurin-21/bin/java",
            }])
        );
    }

    #[tokio::test]
    async fn a_move_rewrites_the_paths_and_drops_the_incomplete_installations()
    {
        let pool = installed_pool().await;
        let root = tempfile::tempdir().unwrap();
        let documents = documents(&root, &pool);

        let dropped =
            move_installations(&documents, "/java", "/moved/java", |path| {
                path.contains("temurin-17")
            })
            .await
            .unwrap();

        let document = read(&documents).await.unwrap();
        assert_eq!(dropped, ["/moved/java/temurin-17/bin/java"]);
        assert_eq!(
            document
                .runtimes
                .iter()
                .map(|runtime| runtime.path.as_str())
                .collect::<Vec<_>>(),
            ["/moved/java/temurin-21/bin/java"]
        );
        assert_eq!(
            document
                .default_for(21)
                .map(|runtime| runtime.path.as_str()),
            Some("/moved/java/temurin-21/bin/java"),
            "the default follows the runtime it names"
        );
        assert!(
            document
                .runtimes
                .iter()
                .all(|runtime| !runtime.path.starts_with("/java/")),
            "the fixtures live under /java, so a stale path means the move missed one"
        );
    }

    /// A key the deployment answers is not copied out of the rows into the local
    /// document: the local copy is the last word for the keys it carries, so the
    /// deployment's list of runtimes would never arrive again.
    #[tokio::test]
    async fn a_deployment_value_is_not_copied_into_the_local_document() {
        let pool = installed_pool().await;
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        ship(
            &deployment,
            DOCUMENT,
            r#"{"runtimes":[{"path":"/deploy/jdk/bin/java","major_version":21,"full_version":"21.0.2"}]}"#,
        );
        let documents = layered(&local, &deployment, &pool);

        assert!(hand_over_if_missing(&documents).await);

        let stored: serde_json::Value = serde_json::from_slice(
            &std::fs::read(documents.path(DOCUMENT)).unwrap(),
        )
        .unwrap();
        assert!(
            !stored["data"].as_object().unwrap().contains_key("runtimes"),
            "the key the deployment answers is left out of the local document"
        );
        assert_eq!(
            read(&documents)
                .await
                .unwrap()
                .runtimes
                .iter()
                .map(|runtime| runtime.path.as_str())
                .collect::<Vec<_>>(),
            ["/deploy/jdk/bin/java"],
            "and the answer the deployment gives is the one the domain reads"
        );
    }
}
