//! `login_proof.json`: the record that an official Minecraft login succeeded
//! once on this installation. The key it is checked against is a keyring entry
//! (`api/anti_piracy.rs`), so the document only carries the MAC.
//!
//! The row's `verified` and `version` columns are the document's keys as well:
//! a proof only counts when it is verified and in the format this build knows.
//!
//! `cleared` is the one key no column answers: it is how a clear says that this
//! installation holds no proof, rather than leaving the question to the layers
//! below it.

use serde::{Deserialize, Serialize};

use super::{Documents, Stored, WRITES, composed, is_false, without_shipped};
use crate::state::store;

pub(crate) const DOCUMENT: &str = "login_proof";

/// Bumped whenever the key set of this document changes, including when a key
/// is dropped. The document records it; a newer document is still read.
const SCHEMA_VERSION: u32 = 2;

/// The proof format this build verifies.
const PROOF_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LoginProofDocument {
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) verified: bool,
    #[serde(default = "default_version", skip_serializing_if = "is_version")]
    pub(crate) version: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) mac: String,
    /// The installation holds no proof, said out loud.
    ///
    /// A document of the defaults alone says nothing, so a proof the layers
    /// below carry would answer for this installation — and that one is the
    /// user never cleared. The flag outranks those layers, because the keys a
    /// proof is made of are exactly the ones they answer.
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) cleared: bool,
}

impl Default for LoginProofDocument {
    fn default() -> Self {
        Self {
            verified: false,
            version: PROOF_VERSION,
            mac: String::new(),
            cleared: false,
        }
    }
}

fn default_version() -> u32 {
    PROOF_VERSION
}

fn is_version(value: &u32) -> bool {
    *value == PROOF_VERSION
}

impl LoginProofDocument {
    fn counts(&self) -> bool {
        self.verified && self.version == PROOF_VERSION && !self.mac.is_empty()
    }
}

/// The MAC of the proof this installation holds, or nothing when it holds none.
///
/// The proof is composed like the other domains, and the MAC has to verify
/// against this machine's keyring key wherever it came from (`api/anti_piracy`),
/// so a layer can answer with a proof but cannot mint one: the binding is the
/// key, not the file.
pub(crate) async fn read(
    documents: &Documents<'_>,
) -> crate::Result<Option<String>> {
    let document: LoginProofDocument =
        composed(documents, DOCUMENT, SCHEMA_VERSION, async {
            Ok(from_row(
                store::official_login_proof::read_legacy_official_login_proof(
                    documents.pool(),
                )
                .await?,
            ))
        })
        .await?;

    if document.cleared {
        return Ok(None);
    }
    Ok(document.counts().then_some(document.mac))
}

/// The proof a row holds as a document, and the empty proof of an installation
/// whose row holds none.
fn from_row(mac: Option<String>) -> LoginProofDocument {
    match mac {
        Some(mac) => LoginProofDocument {
            verified: true,
            version: PROOF_VERSION,
            mac,
            cleared: false,
        },
        None => LoginProofDocument::default(),
    }
}

/// Writes the document the row hands over to, unless one is already in place.
pub(crate) async fn hand_over_if_missing(documents: &Documents<'_>) -> bool {
    if documents
        .stored::<LoginProofDocument>(DOCUMENT, SCHEMA_VERSION)
        .await
        .is_present()
    {
        return true;
    }
    let mac =
        match store::official_login_proof::read_legacy_official_login_proof(
            documents.pool(),
        )
        .await
        {
            Ok(mac) => mac,
            Err(error) => {
                tracing::warn!(%error, "Could not read the official login proof row");
                return false;
            }
        };
    let document = from_row(mac);
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
                "Failed to save the login proof document"
            );
            false
        }
    }
}

pub(crate) async fn mark(
    documents: &Documents<'_>,
    mac: String,
) -> crate::Result<()> {
    update(documents, |document| {
        document.verified = true;
        document.version = PROOF_VERSION;
        document.mac = mac;
        document.cleared = false;
    })
    .await
}

pub(crate) async fn clear(documents: &Documents<'_>) -> crate::Result<()> {
    update(documents, |document| {
        *document = LoginProofDocument {
            cleared: true,
            ..LoginProofDocument::default()
        }
    })
    .await
}

async fn update(
    documents: &Documents<'_>,
    apply: impl FnOnce(&mut LoginProofDocument),
) -> crate::Result<()> {
    let _writing = WRITES.lock().await;
    let mut document = match documents
        .stored::<LoginProofDocument>(DOCUMENT, SCHEMA_VERSION)
        .await
    {
        // A newer build owns the shape of this document: writing it would
        // lower it to the keys this build knows.
        Stored::Newer { .. } => return Ok(()),
        Stored::Ready { document, .. } => document,
        Stored::Missing | Stored::Unreadable => LoginProofDocument::default(),
    };
    apply(&mut document);
    documents.write(DOCUMENT, SCHEMA_VERSION, &document).await
}

#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    use super::*;
    use crate::state::documents::test_support::{
        create_official_login_proof_table, layered, pool, ship,
    };
    use crate::state::store::Collection;
    use crate::state::store::Handle;
    use crate::state::store::official_login_proof::{
        Installation, OfficialLoginProof, OfficialLoginProofRecord,
    };

    fn documents<'a>(
        root: &tempfile::TempDir,
        pool: &'a SqlitePool,
    ) -> Documents<'a> {
        Documents::at(root.path().to_path_buf(), pool)
    }

    async fn installed_pool(mac: Option<&str>) -> SqlitePool {
        let pool = pool().await;
        create_official_login_proof_table(&pool).await;
        if let Some(mac) = mac {
            OfficialLoginProof::set(
                &Handle::new(pool.clone()),
                &Installation,
                Some(&OfficialLoginProofRecord {
                    mac: mac.to_string(),
                }),
            )
            .await
            .unwrap();
        }
        pool
    }

    #[tokio::test]
    async fn a_missing_document_reads_the_row() {
        let pool = installed_pool(Some("stored-mac")).await;
        let root = tempfile::tempdir().unwrap();

        assert_eq!(
            read(&documents(&root, &pool)).await.unwrap().as_deref(),
            Some("stored-mac")
        );
    }

    #[tokio::test]
    async fn a_document_that_leaves_a_key_out_gives_it_the_default() {
        let pool = installed_pool(Some("stored-mac")).await;
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("login_proof.json"),
            r#"{"schema_version":1,"data":{}}"#,
        )
        .unwrap();

        assert_eq!(
            read(&documents(&root, &pool)).await.unwrap(),
            None,
            "a key the document leaves out is its default, never the row's value"
        );
    }

    /// The proof is machine state, so the one this installation holds is the
    /// last word: a MAC a deployment ships cannot take its place. It could not
    /// count even if it did, because a proof is checked against this machine's
    /// keyring key (`api/anti_piracy`) rather than against the file.
    #[tokio::test]
    async fn a_local_proof_wins_over_the_deployment_one() {
        let pool = installed_pool(None).await;
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        ship(
            &deployment,
            DOCUMENT,
            r#"{"verified":true,"mac":"deployment-mac"}"#,
        );
        std::fs::write(
            local.path().join("login_proof.json"),
            r#"{"schema_version":1,"data":{"verified":true,"mac":"local-mac"}}"#,
        )
        .unwrap();

        assert_eq!(
            read(&layered(&local, &deployment, &pool))
                .await
                .unwrap()
                .as_deref(),
            Some("local-mac"),
            "the proof this installation holds outranks the one the deployment names"
        );
    }

    #[tokio::test]
    async fn a_cleared_proof_does_not_come_back_from_the_row() {
        let pool = installed_pool(Some("stored-mac")).await;
        let root = tempfile::tempdir().unwrap();
        let documents = documents(&root, &pool);

        mark(&documents, "fresh-mac".to_string()).await.unwrap();
        assert_eq!(
            read(&documents).await.unwrap().as_deref(),
            Some("fresh-mac")
        );

        clear(&documents).await.unwrap();
        assert_eq!(
            read(&documents).await.unwrap(),
            None,
            "the row still names the old proof; the document is what counts"
        );

        mark(&documents, "fresh-mac".to_string()).await.unwrap();
        let contents =
            std::fs::read_to_string(documents.path(DOCUMENT)).unwrap();
        let stored: serde_json::Value =
            serde_json::from_str(&contents).unwrap();
        let mut keys = stored["data"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        keys.sort_unstable();
        assert_eq!(keys, ["mac", "verified"]);
    }

    /// A clear says that this installation holds no proof rather than leaving
    /// the question to the layers below it: a document of the defaults alone
    /// says nothing about the keys, so the proof the deployment ships would go
    /// on answering for an installation the user has cleared.
    #[tokio::test]
    async fn a_cleared_proof_masks_the_deployment_one() {
        let pool = installed_pool(None).await;
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        ship(
            &deployment,
            DOCUMENT,
            r#"{"verified":true,"mac":"deployment-mac"}"#,
        );
        let documents = layered(&local, &deployment, &pool);
        assert_eq!(
            read(&documents).await.unwrap().as_deref(),
            Some("deployment-mac"),
            "the proof the deployment ships answers while this installation has none"
        );

        clear(&documents).await.unwrap();

        assert_eq!(
            read(&documents).await.unwrap(),
            None,
            "a cleared proof is the installation's answer, and it outranks the deployment's"
        );
        let stored: serde_json::Value = serde_json::from_slice(
            &std::fs::read(documents.path(DOCUMENT)).unwrap(),
        )
        .unwrap();
        assert_eq!(
            stored["data"]["cleared"],
            serde_json::Value::from(true),
            "the clear is what the local document records: {stored}"
        );
    }

    /// A key the deployment answers is not copied out of the row into the local
    /// document by the handover: the local copy is the last word for the keys it
    /// carries, so the deployment's answer would never arrive there again.
    #[tokio::test]
    async fn a_deployment_value_is_not_copied_into_the_local_document() {
        let pool = installed_pool(Some("stored-mac")).await;
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        ship(
            &deployment,
            DOCUMENT,
            r#"{"verified":true,"mac":"deployment-mac"}"#,
        );
        let documents = layered(&local, &deployment, &pool);

        assert!(hand_over_if_missing(&documents).await);

        let stored: serde_json::Value = serde_json::from_slice(
            &std::fs::read(documents.path(DOCUMENT)).unwrap(),
        )
        .unwrap();
        assert!(
            !stored["data"].as_object().unwrap().contains_key("mac"),
            "the key the deployment answers is left out of the local document"
        );
        assert_eq!(
            read(&documents).await.unwrap().as_deref(),
            Some("deployment-mac"),
            "and the answer the deployment gives is the one the domain reads"
        );
    }
}
