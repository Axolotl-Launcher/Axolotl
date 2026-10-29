//! `translation.json`: what the translation page remembers — the provider, the
//! language and style it renders in, the AI provider and model it sends through
//! and the DeepL endpoint and key.
//!
//! The provider, mode and style are carried as the words the column held, so a
//! build that does not know one of them still reads the rest of the document.
//! The retired DeepL-X fields of the row are not carried over: nothing read them
//! any more.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::{Documents, Stored, WRITES, is_false};
use crate::state::store;

pub(crate) const DOCUMENT: &str = "translation";

/// Bumped whenever the key set of this document changes, including when a key
/// is dropped. The document records it; a newer document is still read.
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TranslationDocument {
    #[serde(default = "default_provider", skip_serializing_if = "is_provider")]
    pub(crate) provider: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) target_language: String,
    #[serde(default = "default_mode", skip_serializing_if = "is_mode")]
    pub(crate) mode: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) auto_translate: bool,
    #[serde(default = "default_style", skip_serializing_if = "is_style")]
    pub(crate) style: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) ai_provider_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) ai_model_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) ai_system_prompt: String,
    #[serde(
        default = "default_deepl_endpoint",
        skip_serializing_if = "is_deepl_endpoint"
    )]
    pub(crate) deepl_api_endpoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) deepl_api_key: Option<String>,
}

const PROVIDER: &str = "google";
const MODE: &str = "bilingual";
const STYLE: &str = "weakened";
const DEEPL_ENDPOINT: &str = "https://api-free.deepl.com/v2/translate";

impl Default for TranslationDocument {
    fn default() -> Self {
        Self {
            provider: PROVIDER.to_string(),
            target_language: String::new(),
            mode: MODE.to_string(),
            auto_translate: false,
            style: STYLE.to_string(),
            ai_provider_id: String::new(),
            ai_model_id: String::new(),
            ai_system_prompt: String::new(),
            deepl_api_endpoint: DEEPL_ENDPOINT.to_string(),
            deepl_api_key: None,
        }
    }
}

fn default_provider() -> String {
    PROVIDER.to_string()
}

fn is_provider(value: &String) -> bool {
    value == PROVIDER
}

fn default_mode() -> String {
    MODE.to_string()
}

fn is_mode(value: &String) -> bool {
    value == MODE
}

fn default_style() -> String {
    STYLE.to_string()
}

fn is_style(value: &String) -> bool {
    value == STYLE
}

fn default_deepl_endpoint() -> String {
    DEEPL_ENDPOINT.to_string()
}

fn is_deepl_endpoint(value: &String) -> bool {
    value == DEEPL_ENDPOINT
}

/// The document, or the row while it does not exist. A document that exists
/// answers on its own: every key it leaves out is a default, so an older build
/// falling back to the row cannot resurrect what a newer one wrote.
pub(crate) async fn read(
    documents: &Documents<'_>,
) -> crate::Result<TranslationDocument> {
    match documents.stored::<TranslationDocument>(DOCUMENT).await {
        Stored::Ready {
            schema_version,
            document,
        } => {
            if schema_version > SCHEMA_VERSION {
                tracing::debug!(
                    schema_version,
                    "Reading a translation settings document a newer build wrote"
                );
            }
            Ok(document)
        }
        Stored::Missing | Stored::Unreadable => {
            from_database(documents.pool()).await
        }
    }
}

async fn from_database(
    pool: &SqlitePool,
) -> crate::Result<TranslationDocument> {
    let Some(row) =
        store::settings::read_legacy_translation_settings(pool).await?
    else {
        return Ok(TranslationDocument::default());
    };
    Ok(TranslationDocument {
        provider: row.provider,
        target_language: row.target_language,
        mode: row.mode,
        auto_translate: row.auto_translate,
        style: row.style,
        ai_provider_id: row.ai_provider_id,
        ai_model_id: row.ai_model_id,
        ai_system_prompt: row.ai_system_prompt,
        deepl_api_endpoint: row.deepl_api_endpoint,
        deepl_api_key: row.deepl_api_key,
    })
}

/// Writes the document the row hands over to, unless one is already in place.
pub(crate) async fn hand_over_if_missing(documents: &Documents<'_>) -> bool {
    if matches!(
        documents.stored::<TranslationDocument>(DOCUMENT).await,
        Stored::Ready { .. }
    ) {
        return true;
    }
    let document = match from_database(documents.pool()).await {
        Ok(document) => document,
        Err(error) => {
            tracing::warn!(%error, "Could not read the translation settings row");
            return false;
        }
    };
    match documents.write(DOCUMENT, SCHEMA_VERSION, &document).await {
        Ok(()) => true,
        Err(error) => {
            tracing::warn!(
                path = %documents.path(DOCUMENT).display(),
                %error,
                "Failed to save the translation settings document"
            );
            false
        }
    }
}

pub(crate) async fn update(
    documents: &Documents<'_>,
    apply: impl FnOnce(&mut TranslationDocument),
) -> crate::Result<()> {
    let _writing = WRITES.lock().await;
    let mut document = match documents
        .stored::<TranslationDocument>(DOCUMENT)
        .await
    {
        Stored::Ready { schema_version, .. }
            if schema_version > SCHEMA_VERSION =>
        {
            tracing::debug!(
                schema_version,
                "Leaving a translation settings document a newer build wrote alone"
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
        create_translation_tables, pool,
    };

    fn documents<'a>(
        root: &tempfile::TempDir,
        pool: &'a SqlitePool,
    ) -> Documents<'a> {
        Documents::at(root.path().to_path_buf(), pool)
    }

    async fn installed_pool() -> SqlitePool {
        let pool = pool().await;
        create_translation_tables(&pool).await;
        pool
    }

    #[tokio::test]
    async fn a_missing_document_reads_the_row() {
        let pool = installed_pool().await;
        sqlx::query(
            "UPDATE translation_settings SET provider = 'deepl', target_language = 'zh-CN', mode = 'translation-only', auto_translate = TRUE, style = 'border', ai_provider_id = 'openai', ai_model_id = 'gpt-x', openai_system_prompt = 'Be terse', deepl_api_endpoint = 'https://example.test/translate', deepl_api_key = 'key'",
        )
        .execute(&pool)
        .await
        .unwrap();

        let root = tempfile::tempdir().unwrap();
        let document = read(&documents(&root, &pool)).await.unwrap();

        assert_eq!(document.provider, "deepl");
        assert_eq!(document.target_language, "zh-CN");
        assert_eq!(document.mode, "translation-only");
        assert!(document.auto_translate);
        assert_eq!(document.style, "border");
        assert_eq!(document.ai_provider_id, "openai");
        assert_eq!(document.ai_model_id, "gpt-x");
        assert_eq!(document.ai_system_prompt, "Be terse");
        assert_eq!(
            document.deepl_api_endpoint,
            "https://example.test/translate"
        );
        assert_eq!(document.deepl_api_key.as_deref(), Some("key"));
    }

    #[tokio::test]
    async fn a_document_that_leaves_a_key_out_gives_it_the_default() {
        let pool = installed_pool().await;
        sqlx::query(
            "UPDATE translation_settings SET provider = 'deepl', target_language = 'zh-CN', style = 'border', deepl_api_key = 'key'",
        )
        .execute(&pool)
        .await
        .unwrap();

        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("translation.json"),
            r#"{"schema_version":1,"data":{}}"#,
        )
        .unwrap();

        let document = read(&documents(&root, &pool)).await.unwrap();

        assert_eq!(
            document,
            TranslationDocument::default(),
            "a key the document leaves out is its default, never the row's value"
        );
        assert_eq!(document.provider, "google");
        assert_eq!(document.mode, "bilingual");
        assert_eq!(document.style, "weakened");
        assert_eq!(document.deepl_api_key, None);
    }

    #[tokio::test]
    async fn a_write_only_carries_the_keys_that_differ_from_the_default() {
        let pool = installed_pool().await;
        let root = tempfile::tempdir().unwrap();
        let documents = documents(&root, &pool);

        update(&documents, |document| {
            document.target_language = "zh-CN".into();
            document.deepl_api_key = Some("key".into());
        })
        .await
        .unwrap();

        let contents =
            std::fs::read_to_string(documents.path(DOCUMENT)).unwrap();
        let stored: serde_json::Value =
            serde_json::from_str(&contents).unwrap();
        assert_eq!(stored["schema_version"], serde_json::json!(1));
        let mut keys = stored["data"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        keys.sort_unstable();
        assert_eq!(keys, ["deepl_api_key", "target_language"]);

        let document = read(&documents).await.unwrap();
        assert_eq!(document.target_language, "zh-CN");
        assert_eq!(document.provider, "google");
    }
}
