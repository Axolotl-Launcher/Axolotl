//! `log_share.json`: the log-sharing preferences and the crash-analysis AI
//! choice. The two tables are one document because every reader reaches them
//! together and the AI source they share is one value: `log_share_settings`
//! and `crash_analysis_ai_settings` used to mirror it across a transaction.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::{
    Documents, Stored, WRITES, composed, default_true, is_false, is_true,
    without_shipped,
};
use crate::state::store;

pub(crate) const DOCUMENT: &str = "log_share";

/// Bumped whenever the key set of this document changes, including when a key
/// is dropped. The document records it; a newer document is still read.
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LogShareDocument {
    #[serde(default = "default_logshare", skip_serializing_if = "is_logshare")]
    pub(crate) share_provider: String,
    #[serde(default = "default_logshare", skip_serializing_if = "is_logshare")]
    pub(crate) ai_source: String,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub(crate) auto_upload: bool,
    /// No reader today; kept so the handover does not drop the column.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub(crate) metadata_enabled: bool,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub(crate) multi_file: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) no_storage: bool,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub(crate) show_progress: bool,
    #[serde(default, skip_serializing_if = "CrashAnalysis::is_default")]
    pub(crate) crash_analysis: CrashAnalysis,
}

impl Default for LogShareDocument {
    fn default() -> Self {
        Self {
            share_provider: default_logshare(),
            ai_source: default_logshare(),
            auto_upload: true,
            metadata_enabled: true,
            multi_file: true,
            no_storage: false,
            show_progress: true,
            crash_analysis: CrashAnalysis::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CrashAnalysis {
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) enabled: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) provider_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) model_id: String,
}

impl CrashAnalysis {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

fn default_logshare() -> String {
    "logshare".to_string()
}

fn is_logshare(value: &String) -> bool {
    value == "logshare"
}

/// The document, composed of the keys the deployment ships and the ones the
/// local document names, or the rows while neither of them answers. The two
/// rows are one document, so a layer answers both halves together.
pub(crate) async fn read(
    documents: &Documents<'_>,
) -> crate::Result<LogShareDocument> {
    composed(documents, DOCUMENT, SCHEMA_VERSION, async {
        from_database(documents.pool()).await
    })
    .await
}

async fn from_database(pool: &SqlitePool) -> crate::Result<LogShareDocument> {
    let Some(row) =
        store::settings::read_legacy_log_share_settings(pool).await?
    else {
        return Ok(LogShareDocument::default());
    };
    Ok(LogShareDocument {
        share_provider: row.share_provider,
        ai_source: row.ai_source,
        auto_upload: row.auto_upload,
        metadata_enabled: row.metadata_enabled,
        multi_file: row.multi_file,
        no_storage: row.no_storage,
        show_progress: row.show_progress,
        crash_analysis: CrashAnalysis {
            enabled: row.crash_enabled,
            provider_id: row.crash_provider_id,
            model_id: row.crash_model_id,
        },
    })
}

/// Writes the document the rows hand over to, unless one is already in place.
pub(crate) async fn hand_over_if_missing(documents: &Documents<'_>) -> bool {
    if documents
        .stored::<LogShareDocument>(DOCUMENT, SCHEMA_VERSION)
        .await
        .is_present()
    {
        return true;
    }
    let document = match from_database(documents.pool()).await {
        Ok(document) => document,
        Err(error) => {
            tracing::warn!(%error, "Could not read the log sharing rows");
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
                "Failed to save the log sharing document"
            );
            false
        }
    }
}

pub(crate) struct ShareSettings {
    pub(crate) share_provider: String,
    pub(crate) auto_upload: bool,
    pub(crate) multi_file: bool,
    pub(crate) no_storage: bool,
    pub(crate) show_progress: bool,
}

pub(crate) async fn update_share(
    documents: &Documents<'_>,
    share: ShareSettings,
) -> crate::Result<()> {
    update(documents, |document| {
        document.share_provider = share.share_provider;
        document.auto_upload = share.auto_upload;
        document.multi_file = share.multi_file;
        document.no_storage = share.no_storage;
        document.show_progress = share.show_progress;
    })
    .await
}

pub(crate) struct CrashAnalysisSettings {
    pub(crate) enabled: bool,
    pub(crate) provider_id: String,
    pub(crate) model_id: String,
    pub(crate) ai_source: String,
}

pub(crate) async fn update_crash_analysis(
    documents: &Documents<'_>,
    settings: CrashAnalysisSettings,
) -> crate::Result<()> {
    update(documents, |document| {
        document.ai_source = settings.ai_source;
        document.crash_analysis = CrashAnalysis {
            enabled: settings.enabled,
            provider_id: settings.provider_id,
            model_id: settings.model_id,
        };
    })
    .await
}

async fn update(
    documents: &Documents<'_>,
    apply: impl FnOnce(&mut LogShareDocument),
) -> crate::Result<()> {
    let _writing = WRITES.lock().await;
    let mut document = match documents
        .stored::<LogShareDocument>(DOCUMENT, SCHEMA_VERSION)
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
        create_log_share_tables, layered, pool, ship,
    };

    fn documents<'a>(
        root: &tempfile::TempDir,
        pool: &'a SqlitePool,
    ) -> Documents<'a> {
        Documents::at(root.path().to_path_buf(), pool)
    }

    async fn installed_pool() -> SqlitePool {
        let pool = pool().await;
        create_log_share_tables(&pool).await;
        pool
    }

    #[tokio::test]
    async fn a_missing_document_reads_both_rows() {
        let pool = installed_pool().await;
        sqlx::query(
            "UPDATE log_share_settings SET share_provider = 'mclogs', auto_upload = FALSE, metadata_enabled = FALSE, no_storage = TRUE",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE crash_analysis_ai_settings SET enabled = TRUE, provider_id = 'openai', model_id = 'gpt-x', ai_source = 'custom'",
        )
        .execute(&pool)
        .await
        .unwrap();

        let root = tempfile::tempdir().unwrap();
        let document = read(&documents(&root, &pool)).await.unwrap();

        assert_eq!(document.share_provider, "mclogs");
        assert_eq!(document.ai_source, "custom");
        assert!(!document.auto_upload);
        assert!(!document.metadata_enabled);
        assert!(document.no_storage);
        assert_eq!(
            document.crash_analysis,
            CrashAnalysis {
                enabled: true,
                provider_id: "openai".into(),
                model_id: "gpt-x".into(),
            }
        );
    }

    #[tokio::test]
    async fn a_document_that_leaves_a_key_out_gives_it_the_default() {
        let pool = installed_pool().await;
        sqlx::query("UPDATE log_share_settings SET share_provider = 'mclogs'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE crash_analysis_ai_settings SET enabled = TRUE, ai_source = 'custom'",
        )
        .execute(&pool)
        .await
        .unwrap();

        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("log_share.json"),
            r#"{"schema_version":1,"data":{}}"#,
        )
        .unwrap();

        let document = read(&documents(&root, &pool)).await.unwrap();

        assert_eq!(
            document.share_provider, "logshare",
            "a key the document leaves out is its default, never the row's value"
        );
        assert_eq!(document.ai_source, "logshare");
        assert!(!document.crash_analysis.enabled);
    }

    /// The two tables are one document, so a layer answers both of their keys
    /// together: the deployment's document reaches the crash-analysis half as
    /// well as the sharing one.
    #[tokio::test]
    async fn a_deployment_document_answers_both_halves() {
        let pool = installed_pool().await;
        sqlx::query(
            "UPDATE log_share_settings SET share_provider = 'mclogs', auto_upload = FALSE",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE crash_analysis_ai_settings SET enabled = TRUE, provider_id = 'openai', model_id = 'gpt-x', ai_source = 'custom'",
        )
        .execute(&pool)
        .await
        .unwrap();

        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        ship(
            &deployment,
            DOCUMENT,
            r#"{"share_provider":"logshare","ai_source":"logshare","crash_analysis":{"enabled":false,"provider_id":"deploy"}}"#,
        );

        let document =
            read(&layered(&local, &deployment, &pool)).await.unwrap();

        assert_eq!(
            document.share_provider, "logshare",
            "the deployment's answer for a key outranks the row's"
        );
        assert_eq!(document.ai_source, "logshare");
        assert_eq!(
            document.crash_analysis,
            CrashAnalysis {
                enabled: false,
                provider_id: "deploy".into(),
                model_id: String::new(),
            },
            "a layer answers the crash-analysis half too, and it answers the whole key rather than merging into it"
        );
        assert!(
            !document.auto_upload,
            "a key the deployment leaves out is still the row's"
        );
    }

    #[tokio::test]
    async fn the_two_settings_share_one_document() {
        let pool = installed_pool().await;
        let root = tempfile::tempdir().unwrap();
        let documents = documents(&root, &pool);

        update_crash_analysis(
            &documents,
            CrashAnalysisSettings {
                enabled: true,
                provider_id: "openai".into(),
                model_id: "gpt-x".into(),
                ai_source: "custom".into(),
            },
        )
        .await
        .unwrap();
        update_share(
            &documents,
            ShareSettings {
                share_provider: "mclogs".into(),
                auto_upload: false,
                multi_file: false,
                no_storage: true,
                show_progress: false,
            },
        )
        .await
        .unwrap();

        let document = read(&documents).await.unwrap();
        assert_eq!(document.share_provider, "mclogs");
        assert_eq!(document.ai_source, "custom");
        assert!(document.crash_analysis.enabled);
        assert_eq!(document.crash_analysis.model_id, "gpt-x");
        assert!(!document.auto_upload);
        assert!(document.no_storage);
    }

    /// A key the deployment answers is not copied out of the rows into the local
    /// document: the local copy is the last word for the keys it carries, so the
    /// deployment's answer would never arrive at this key again.
    #[tokio::test]
    async fn a_deployment_value_is_not_copied_into_the_local_document() {
        let pool = installed_pool().await;
        // A value the document carries without holding the default, so the copy
        // this test looks for is one it can see.
        sqlx::query(
            "UPDATE log_share_settings SET share_provider = 'paste-gg'",
        )
        .execute(&pool)
        .await
        .unwrap();
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        ship(&deployment, DOCUMENT, r#"{"share_provider":"mclogs"}"#);
        let documents = layered(&local, &deployment, &pool);

        assert!(hand_over_if_missing(&documents).await);

        let stored: serde_json::Value = serde_json::from_slice(
            &std::fs::read(documents.path(DOCUMENT)).unwrap(),
        )
        .unwrap();
        assert!(
            !stored["data"]
                .as_object()
                .unwrap()
                .contains_key("share_provider"),
            "the key the deployment answers is left out of the local document: {}",
            stored["data"]
        );
        assert_eq!(
            read(&documents).await.unwrap().share_provider,
            "mclogs",
            "and the answer the deployment gives is the one the domain reads"
        );
    }
}
