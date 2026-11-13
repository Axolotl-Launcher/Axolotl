//! `ai.json`: the AI feature switch, the per-provider configuration and the
//! model overrides. Keys are columns of `ai_settings`, `ai_provider_configs`
//! and `ai_provider_models`; credentials never appear here, they are keyring
//! entries (`api/ai/credentials.rs`).
//!
//! A provider entry left out means the provider was never configured, so it
//! reads as its catalog definition; a field left out of an entry that does
//! exist holds that column's default.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::{
    Documents, Stored, WRITES, composed, default_true, is_false, is_true,
    without_shipped,
};
use crate::state::store;

pub(crate) const DOCUMENT: &str = "ai";

/// Bumped whenever the key set of this document changes, including when a key
/// is dropped, so a build that still knows the removed key cannot write it
/// back. The document records it; a newer document is still read.
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct AiDocument {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub(crate) enabled: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) legacy_openai_credential_cleanup: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) providers: BTreeMap<String, ProviderConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) models: Vec<ProviderModel>,
}

impl Default for AiDocument {
    fn default() -> Self {
        Self {
            enabled: true,
            legacy_openai_credential_cleanup: false,
            providers: BTreeMap::new(),
            models: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ProviderConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) custom_name: Option<String>,
    #[serde(
        default = "default_protocol",
        skip_serializing_if = "is_default_protocol"
    )]
    pub(crate) protocol: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) enabled: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) endpoint: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) settings: BTreeMap<String, String>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            custom_name: None,
            protocol: default_protocol(),
            enabled: false,
            endpoint: String::new(),
            settings: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProviderModel {
    pub(crate) provider_id: String,
    pub(crate) model_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) display_name: String,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub(crate) enabled: bool,
    #[serde(
        default = "default_source",
        skip_serializing_if = "is_default_source"
    )]
    pub(crate) source: String,
}

fn default_protocol() -> String {
    "openai".to_string()
}

fn is_default_protocol(value: &String) -> bool {
    value == "openai"
}

fn default_source() -> String {
    "custom".to_string()
}

fn is_default_source(value: &String) -> bool {
    value == "custom"
}

/// The document, composed of the keys the deployment ships and the ones the
/// local document names, or the rows while neither of them answers.
pub(crate) async fn read(
    documents: &Documents<'_>,
) -> crate::Result<AiDocument> {
    composed(documents, DOCUMENT, SCHEMA_VERSION, async {
        from_database(documents.pool()).await
    })
    .await
}

async fn from_database(pool: &SqlitePool) -> crate::Result<AiDocument> {
    let settings = store::settings::read_legacy_ai_settings(pool).await?;
    let providers = store::settings::read_legacy_ai_provider_configs(pool)
        .await?
        .into_iter()
        .map(|config| {
            (
                config.provider_id,
                ProviderConfig {
                    custom_name: config.custom_name,
                    protocol: config.protocol,
                    enabled: config.enabled,
                    endpoint: config.endpoint,
                    settings: serde_json::from_str(&config.settings)
                        .unwrap_or_default(),
                },
            )
        })
        .collect();
    let models = store::settings::read_legacy_ai_provider_models(pool)
        .await?
        .into_iter()
        .map(|model| ProviderModel {
            provider_id: model.provider_id,
            model_id: model.model_id,
            display_name: model.display_name,
            enabled: model.enabled,
            source: model.source,
        })
        .collect();
    Ok(AiDocument {
        enabled: settings.as_ref().is_none_or(|settings| settings.enabled),
        legacy_openai_credential_cleanup: settings
            .is_some_and(|settings| settings.legacy_openai_credential_cleanup),
        providers,
        models,
    })
}

/// Writes the document the rows hand over to, unless one is already in place.
pub(crate) async fn hand_over_if_missing(documents: &Documents<'_>) -> bool {
    let path = documents.path(DOCUMENT);
    if documents
        .stored::<AiDocument>(DOCUMENT, SCHEMA_VERSION)
        .await
        .is_present()
    {
        return true;
    }
    let document = match from_database(documents.pool()).await {
        Ok(document) => document,
        Err(error) => {
            tracing::warn!(%error, "Could not read the AI settings rows");
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
                path = %path.display(),
                %error,
                "Failed to save the AI settings document"
            );
            false
        }
    }
}

pub(crate) async fn set_enabled(
    documents: &Documents<'_>,
    enabled: bool,
) -> crate::Result<()> {
    update(documents, |document| document.enabled = enabled).await
}

pub(crate) async fn set_legacy_openai_credential_cleanup(
    documents: &Documents<'_>,
    cleanup: bool,
) -> crate::Result<()> {
    update(documents, |document| {
        document.legacy_openai_credential_cleanup = cleanup;
    })
    .await
}

pub(crate) async fn upsert_provider(
    documents: &Documents<'_>,
    provider_id: String,
    config: ProviderConfig,
) -> crate::Result<()> {
    update(documents, |document| {
        document.providers.insert(provider_id, config);
    })
    .await
}

pub(crate) async fn upsert_model(
    documents: &Documents<'_>,
    provider_id: String,
    model_id: String,
    display_name: String,
    enabled: bool,
    source: String,
) -> crate::Result<()> {
    update(documents, |document| {
        if let Some(existing) = document.models.iter_mut().find(|model| {
            model.provider_id == provider_id && model.model_id == model_id
        }) {
            existing.display_name = display_name;
            existing.enabled = enabled;
            if source == "builtin" {
                existing.source = source;
            }
            return;
        }
        document.models.push(ProviderModel {
            provider_id,
            model_id,
            display_name,
            enabled,
            source,
        });
    })
    .await
}

/// A model the provider's catalog endpoint listed: it only replaces a name no
/// user chose, and leaves a model the user added alone.
pub(crate) async fn upsert_discovered_model(
    documents: &Documents<'_>,
    provider_id: String,
    model_id: String,
    display_name: String,
) -> crate::Result<()> {
    update(documents, |document| {
        if let Some(existing) = document.models.iter_mut().find(|model| {
            model.provider_id == provider_id && model.model_id == model_id
        }) {
            if existing.source != "custom" {
                existing.display_name = display_name;
            }
            return;
        }
        document.models.push(ProviderModel {
            provider_id,
            model_id,
            display_name,
            enabled: true,
            source: "remote".to_string(),
        });
    })
    .await
}

pub(crate) async fn remove_model(
    documents: &Documents<'_>,
    provider_id: &str,
    model_id: &str,
) -> crate::Result<()> {
    update(documents, |document| {
        document.models.retain(|model| {
            model.provider_id != provider_id || model.model_id != model_id
        });
    })
    .await
}

async fn update(
    documents: &Documents<'_>,
    apply: impl FnOnce(&mut AiDocument),
) -> crate::Result<()> {
    let _writing = WRITES.lock().await;
    let mut document = match documents
        .stored::<AiDocument>(DOCUMENT, SCHEMA_VERSION)
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
    use std::collections::BTreeMap;

    use serde_json::Value;

    use super::*;
    use crate::state::documents::test_support::{
        create_ai_tables, layered, pool, ship,
    };

    fn documents<'a>(
        root: &tempfile::TempDir,
        pool: &'a SqlitePool,
    ) -> Documents<'a> {
        Documents::at(root.path().to_path_buf(), pool)
    }

    async fn installed_pool() -> SqlitePool {
        let pool = pool().await;
        create_ai_tables(&pool).await;
        pool
    }

    #[tokio::test]
    async fn a_missing_document_reads_the_rows() {
        let pool = installed_pool().await;
        sqlx::query(
            "UPDATE ai_settings SET enabled = FALSE, legacy_openai_credential_cleanup = TRUE",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO ai_provider_configs (provider_id, custom_name, protocol, enabled, endpoint, settings) VALUES ('openai', 'Mine', 'openai', TRUE, 'https://example.test/v1', '{\"temperature\":\"0.2\"}')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO ai_provider_models (provider_id, model_id, display_name, enabled, source) VALUES ('openai', 'local', 'Local', FALSE, 'custom')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let root = tempfile::tempdir().unwrap();
        let document = read(&documents(&root, &pool)).await.unwrap();

        assert!(!document.enabled);
        assert!(document.legacy_openai_credential_cleanup);
        let provider = document.providers.get("openai").unwrap();
        assert_eq!(provider.custom_name.as_deref(), Some("Mine"));
        assert_eq!(provider.endpoint, "https://example.test/v1");
        assert_eq!(
            provider.settings.get("temperature").map(String::as_str),
            Some("0.2")
        );
        assert_eq!(
            document.models,
            vec![ProviderModel {
                provider_id: "openai".into(),
                model_id: "local".into(),
                display_name: "Local".into(),
                enabled: false,
                source: "custom".into(),
            }]
        );
    }

    #[tokio::test]
    async fn a_document_that_leaves_a_key_out_gives_it_the_default() {
        let pool = installed_pool().await;
        sqlx::query(
            "UPDATE ai_settings SET enabled = FALSE, legacy_openai_credential_cleanup = TRUE",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO ai_provider_configs (provider_id, custom_name, protocol, enabled, endpoint) VALUES ('openai', 'Mine', 'openai', TRUE, 'https://example.test/v1')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("ai.json"),
            r#"{"schema_version":1,"data":{}}"#,
        )
        .unwrap();

        let document = read(&documents(&root, &pool)).await.unwrap();

        assert!(
            document.enabled,
            "a key the document leaves out is its default, never the row's value"
        );
        assert!(!document.legacy_openai_credential_cleanup);
        assert!(document.providers.is_empty());
    }

    /// The layers are laid key by key, so a key the local document leaves out
    /// is answered by the deployment's document below it.
    #[tokio::test]
    async fn a_key_the_local_document_omits_comes_from_the_deployment() {
        let pool = installed_pool().await;
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        ship(&deployment, DOCUMENT, r#"{"enabled":false}"#);
        std::fs::write(
            local.path().join("ai.json"),
            r#"{"schema_version":1,"data":{"providers":{"openai":{"custom_name":"Mine","endpoint":"https://example.test/v1"}}}}"#,
        )
        .unwrap();

        let document =
            read(&layered(&local, &deployment, &pool)).await.unwrap();

        assert!(
            !document.enabled,
            "a key the local document leaves out is answered by the layer below it"
        );
        assert_eq!(
            document
                .providers
                .get("openai")
                .and_then(|provider| provider.custom_name.as_deref()),
            Some("Mine"),
            "the keys the local document names stay its own"
        );
    }

    /// The local document is the last word for the keys it carries, whatever
    /// the deployment ships for them.
    #[tokio::test]
    async fn a_local_document_wins_over_the_deployment_one() {
        let pool = installed_pool().await;
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        ship(&deployment, DOCUMENT, r#"{"enabled":false}"#);
        std::fs::write(
            local.path().join("ai.json"),
            r#"{"schema_version":1,"data":{"enabled":true}}"#,
        )
        .unwrap();

        let document =
            read(&layered(&local, &deployment, &pool)).await.unwrap();

        assert!(
            document.enabled,
            "the user's own document outranks the one the deployment ships"
        );
    }

    #[tokio::test]
    async fn a_write_reads_back_as_json_without_credentials() {
        let pool = installed_pool().await;
        let root = tempfile::tempdir().unwrap();
        let documents = documents(&root, &pool);
        let document = AiDocument {
            enabled: false,
            legacy_openai_credential_cleanup: true,
            providers: BTreeMap::from([(
                "openai".to_string(),
                ProviderConfig {
                    custom_name: Some("Mine".to_string()),
                    protocol: "openai".to_string(),
                    enabled: true,
                    endpoint: "https://example.test/v1".to_string(),
                    settings: BTreeMap::from([(
                        "temperature".to_string(),
                        "0.2".to_string(),
                    )]),
                },
            )]),
            models: vec![
                ProviderModel {
                    provider_id: "openai".into(),
                    model_id: "gpt-4o".into(),
                    display_name: String::new(),
                    enabled: true,
                    source: "builtin".into(),
                },
                ProviderModel {
                    provider_id: "openai".into(),
                    model_id: "local".into(),
                    display_name: "Local".into(),
                    enabled: false,
                    source: "custom".into(),
                },
            ],
        };

        documents
            .write(DOCUMENT, SCHEMA_VERSION, &document)
            .await
            .unwrap();

        assert_eq!(read(&documents).await.unwrap(), document);

        let contents =
            std::fs::read_to_string(documents.path(DOCUMENT)).unwrap();
        let stored: Value = serde_json::from_str(&contents).unwrap();
        assert_eq!(stored["schema_version"], Value::from(SCHEMA_VERSION));
        assert_eq!(
            keys(stored["data"].as_object().unwrap()),
            [
                "enabled",
                "legacy_openai_credential_cleanup",
                "models",
                "providers"
            ]
        );
        assert_eq!(
            keys(stored["data"]["providers"]["openai"].as_object().unwrap()),
            ["custom_name", "enabled", "endpoint", "settings"]
        );
        assert_eq!(
            keys(stored["data"]["models"][0].as_object().unwrap()),
            ["model_id", "provider_id", "source"]
        );
    }

    fn keys(map: &serde_json::Map<String, Value>) -> Vec<&str> {
        let mut keys = map.keys().map(String::as_str).collect::<Vec<_>>();
        keys.sort_unstable();
        keys
    }

    /// A key the deployment answers is not copied out of the rows into the local
    /// document: the local copy is the last word for the keys it carries, so the
    /// deployment's answer would never arrive at this key again.
    #[tokio::test]
    async fn a_deployment_value_is_not_copied_into_the_local_document() {
        let pool = installed_pool().await;
        // A key the document carries without holding the default, so the copy
        // this test looks for is one it can see.
        sqlx::query(
            "UPDATE ai_settings SET legacy_openai_credential_cleanup = TRUE",
        )
        .execute(&pool)
        .await
        .unwrap();
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        ship(
            &deployment,
            DOCUMENT,
            r#"{"legacy_openai_credential_cleanup":false}"#,
        );
        let documents = layered(&local, &deployment, &pool);

        assert!(hand_over_if_missing(&documents).await);

        let stored: Value = serde_json::from_slice(
            &std::fs::read(documents.path(DOCUMENT)).unwrap(),
        )
        .unwrap();
        assert!(
            !stored["data"]
                .as_object()
                .unwrap()
                .contains_key("legacy_openai_credential_cleanup"),
            "the key the deployment answers is left out of the local document: {}",
            stored["data"]
        );
        assert!(
            !read(&documents)
                .await
                .unwrap()
                .legacy_openai_credential_cleanup,
            "and the answer the deployment gives is the one the domain reads"
        );
    }
}
