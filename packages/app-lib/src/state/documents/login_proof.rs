//! `login_proof.json`: the record that an official Minecraft login succeeded
//! once on this installation. The key it is checked against is a keyring entry
//! (`api/anti_piracy.rs`), so the document only carries the MAC.
//!
//! The row's `verified` and `version` columns are the document's keys as well:
//! a proof only counts when it is verified and in the format this build knows.

use serde::{Deserialize, Serialize};

use super::{Documents, Stored, WRITES, is_false};
use crate::state::store;

pub(crate) const DOCUMENT: &str = "login_proof";

/// Bumped whenever the key set of this document changes, including when a key
/// is dropped. The document records it; a newer document is still read.
const SCHEMA_VERSION: u32 = 1;

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
}

impl Default for LoginProofDocument {
    fn default() -> Self {
        Self {
            verified: false,
            version: PROOF_VERSION,
            mac: String::new(),
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
/// A document that exists answers on its own: every key it leaves out is a
/// default, so a proof an older build left in the row cannot come back.
pub(crate) async fn read(
    documents: &Documents<'_>,
) -> crate::Result<Option<String>> {
    let document = match documents.stored::<LoginProofDocument>(DOCUMENT).await
    {
        Stored::Ready {
            schema_version,
            document,
        } => {
            if schema_version > SCHEMA_VERSION {
                tracing::debug!(
                    schema_version,
                    "Reading a login proof document a newer build wrote"
                );
            }
            document
        }
        Stored::Missing | Stored::Unreadable => {
            let Some(mac) =
                store::official_login_proof::read_legacy_official_login_proof(
                    documents.pool(),
                )
                .await?
            else {
                return Ok(None);
            };
            LoginProofDocument {
                verified: true,
                version: PROOF_VERSION,
                mac,
            }
        }
    };

    Ok(document.counts().then_some(document.mac))
}

/// Writes the document the row hands over to, unless one is already in place.
pub(crate) async fn hand_over_if_missing(documents: &Documents<'_>) -> bool {
    if matches!(
        documents.stored::<LoginProofDocument>(DOCUMENT).await,
        Stored::Ready { .. }
    ) {
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
    let document = match mac {
        Some(mac) => LoginProofDocument {
            verified: true,
            version: PROOF_VERSION,
            mac,
        },
        None => LoginProofDocument::default(),
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
    })
    .await
}

pub(crate) async fn clear(documents: &Documents<'_>) -> crate::Result<()> {
    update(documents, |document| {
        *document = LoginProofDocument::default()
    })
    .await
}

async fn update(
    documents: &Documents<'_>,
    apply: impl FnOnce(&mut LoginProofDocument),
) -> crate::Result<()> {
    let _writing = WRITES.lock().await;
    let mut document = match documents
        .stored::<LoginProofDocument>(DOCUMENT)
        .await
    {
        Stored::Ready { schema_version, .. }
            if schema_version > SCHEMA_VERSION =>
        {
            tracing::debug!(
                schema_version,
                "Leaving a login proof document a newer build wrote alone"
            );
            return Ok(());
        }
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
        create_official_login_proof_table, pool,
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
            sqlx::query(
                "INSERT INTO official_login_proof (id, verified, version, mac) VALUES (0, 1, 1, ?)",
            )
            .bind(mac)
            .execute(&pool)
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
}
