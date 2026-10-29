//! `log_share.json`: the log-sharing preferences and the crash-analysis AI
//! choice. The two tables are one document because every reader reaches them
//! together and the AI source they share is one value: `log_share_settings`
//! and `crash_analysis_ai_settings` used to mirror it across a transaction.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::{Documents, Stored, WRITES, default_true, is_false, is_true};
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

/// The document, or the rows while it does not exist. A document that exists
/// answers on its own: every key it leaves out is a default, so an older build
/// falling back to the rows cannot resurrect what a newer one wrote.
pub(crate) async fn read(
    documents: &Documents<'_>,
) -> crate::Result<LogShareDocument> {
    match documents.stored::<LogShareDocument>(DOCUMENT).await {
        Stored::Ready {
            schema_version,
            document,
        } => {
            if schema_version > SCHEMA_VERSION {
                tracing::debug!(
                    schema_version,
                    "Reading a log sharing document a newer build wrote"
                );
            }
            Ok(document)
        }
        Stored::Missing | Stored::Unreadable => {
            from_database(documents.pool()).await
        }
    }
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
    if matches!(
        documents.stored::<LogShareDocument>(DOCUMENT).await,
        Stored::Ready { .. }
    ) {
        return true;
    }
    let document = match from_database(documents.pool()).await {
        Ok(document) => document,
        Err(error) => {
            tracing::warn!(%error, "Could not read the log sharing rows");
            return false;
        }
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
    let mut document =
        match documents.stored::<LogShareDocument>(DOCUMENT).await {
            Stored::Ready { schema_version, .. }
                if schema_version > SCHEMA_VERSION =>
            {
                tracing::debug!(
                    schema_version,
                    "Leaving a log sharing document a newer build wrote alone"
                );
                return Ok(());
            }
            Stored::Ready { document, .. } => document,
            Stored::Missing | Stored::Unreadable => {
                from_database(documents.pool()).await?
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
        create_log_share_tables, pool,
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
}
