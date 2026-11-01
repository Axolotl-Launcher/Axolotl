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

use super::{Documents, Stored, WRITES, composed, is_false, without_shipped};
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

/// The document, composed of the keys the deployment ships and the ones the
/// local document names, or the row while neither of them answers.
pub(crate) async fn read(
    documents: &Documents<'_>,
) -> crate::Result<TranslationDocument> {
    composed(documents, DOCUMENT, SCHEMA_VERSION, async {
        from_database(documents.pool()).await
    })
    .await
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
    if documents
        .stored::<TranslationDocument>(DOCUMENT, SCHEMA_VERSION)
        .await
        .is_present()
    {
        return true;
    }
    let document = match from_database(documents.pool()).await {
        Ok(document) => document,
        Err(error) => {
            tracing::warn!(%error, "Could not read the translation settings row");
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
        .stored::<TranslationDocument>(DOCUMENT, SCHEMA_VERSION)
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
        create_translation_tables, layered, pool, ship,
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

    /// A deployment's document answers a domain no local document covers, and
    /// it answers above the row: a value the row froze cannot shadow it.
    #[tokio::test]
    async fn a_deployment_document_applies_without_a_local_one() {
        let pool = installed_pool().await;
        sqlx::query(
            "UPDATE translation_settings SET provider = 'deepl', target_language = 'zh-CN'",
        )
        .execute(&pool)
        .await
        .unwrap();

        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        ship(&deployment, DOCUMENT, r#"{"target_language":"ja"}"#);

        let document =
            read(&layered(&local, &deployment, &pool)).await.unwrap();

        assert_eq!(
            document.target_language, "ja",
            "the deployment's answer for a key outranks the row's"
        );
        assert_eq!(
            document.provider, "deepl",
            "a key the deployment leaves out is still the row's"
        );
        assert!(
            !local.path().join("translation.json").exists(),
            "a read must not write the local directory"
        );
    }

    /// A domain with no document on either layer is still answered by its row.
    #[tokio::test]
    async fn the_row_answers_when_no_document_exists() {
        let pool = installed_pool().await;
        sqlx::query(
            "UPDATE translation_settings SET provider = 'deepl', target_language = 'zh-CN'",
        )
        .execute(&pool)
        .await
        .unwrap();

        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        // The deployment ships documents, just not one for this domain.
        ship(&deployment, "ai", r#"{"enabled":false}"#);

        let document =
            read(&layered(&local, &deployment, &pool)).await.unwrap();

        assert_eq!(
            document.provider, "deepl",
            "a domain no layer holds a document for is answered by its row"
        );
        assert_eq!(document.target_language, "zh-CN");
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

    /// A key the deployment answers is not copied out of the row into the local
    /// document by the handover: the local copy would be the last word for it,
    /// and the deployment's answer would never arrive there again.
    #[tokio::test]
    async fn a_deployment_value_is_not_copied_into_the_local_document() {
        let pool = installed_pool().await;
        sqlx::query(
            "UPDATE translation_settings SET target_language = 'zh-CN'",
        )
        .execute(&pool)
        .await
        .unwrap();
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        ship(&deployment, DOCUMENT, r#"{"target_language":"ja"}"#);
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
                .contains_key("target_language"),
            "the key the deployment answers is left out of the local document"
        );
        assert_eq!(
            read(&documents).await.unwrap().target_language,
            "ja",
            "and the answer the deployment gives is the one the domain reads"
        );
    }

    /// The draft a write of a domain with no document starts from is the row,
    /// and the same keys are left out of it: the update that gives the document
    /// its first key does not copy the deployment's key beside it.
    #[tokio::test]
    async fn an_update_does_not_copy_a_deployment_value() {
        let pool = installed_pool().await;
        sqlx::query(
            "UPDATE translation_settings SET target_language = 'zh-CN'",
        )
        .execute(&pool)
        .await
        .unwrap();
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        ship(&deployment, DOCUMENT, r#"{"target_language":"ja"}"#);
        let documents = layered(&local, &deployment, &pool);

        update(&documents, |document| {
            document.deepl_api_key = Some("key".into());
        })
        .await
        .unwrap();

        let stored: serde_json::Value = serde_json::from_slice(
            &std::fs::read(documents.path(DOCUMENT)).unwrap(),
        )
        .unwrap();
        let data = stored["data"].as_object().unwrap();
        assert!(
            !data.contains_key("target_language"),
            "the key the deployment answers is not copied out of the row: {data:?}"
        );
        assert_eq!(
            data.get("deepl_api_key"),
            Some(&serde_json::Value::from("key")),
            "the key the update named is written"
        );
        assert_eq!(
            read(&documents).await.unwrap().target_language,
            "ja",
            "the domain reads the deployment's answer for the key it leaves out"
        );
    }
}
