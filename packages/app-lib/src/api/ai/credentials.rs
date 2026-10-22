//! Size-bounded OAuth records stored entirely in the OS credential store.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{OAUTH_CREDENTIAL, input_error};

// keyring's Windows backend checks CRED_MAX_CREDENTIAL_BLOB_SIZE (2560 bytes),
// even though its TooLong message labels the limit as "chars". Passwords use
// UTF-16, so each entry can hold at most 1280 code units.
const MAX_PASSWORD_UNITS: usize = 1280;
const MAX_CHUNKS: usize = 1024;
const MANIFEST_PREFIX: &str = "axolotl-oauth-chunks-v1:";

pub(super) trait Store {
    fn read(&self, name: &str) -> crate::Result<Option<String>>;
    fn write(&self, name: &str, value: Option<&str>) -> crate::Result<()>;
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    generation: Uuid,
    chunks: usize,
}

impl Manifest {
    fn key(&self, index: usize) -> String {
        format!("{OAUTH_CREDENTIAL}:{}:{index}", self.generation)
    }
}

fn parse_manifest(value: &str) -> crate::Result<Option<Manifest>> {
    let Some(value) = value.strip_prefix(MANIFEST_PREFIX) else {
        return Ok(None);
    };
    let manifest: Manifest = serde_json::from_str(value).map_err(|_| {
        input_error("Stored OAuth credential manifest is invalid; reconnect the provider".to_string())
    })?;
    if manifest.chunks == 0 || manifest.chunks > MAX_CHUNKS {
        return Err(input_error(
            "Stored OAuth credential chunk count is invalid; reconnect the provider".to_string(),
        ));
    }
    Ok(Some(manifest))
}

pub(super) fn read(store: &impl Store) -> crate::Result<Option<String>> {
    let Some(value) = store.read(OAUTH_CREDENTIAL)? else {
        return Ok(None);
    };
    let Some(manifest) = parse_manifest(&value)? else {
        // Existing versions stored the entire JSON document in this entry.
        return Ok(Some(value));
    };
    let mut value = String::new();
    for index in 0..manifest.chunks {
        let chunk = store.read(&manifest.key(index))?.ok_or_else(|| {
            input_error("Stored OAuth credentials are incomplete; reconnect the provider".to_string())
        })?;
        value.push_str(&chunk);
    }
    Ok(Some(value))
}

fn split_password(value: &str) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut units = 0;
    for (index, character) in value.char_indices() {
        if units + character.len_utf16() > MAX_PASSWORD_UNITS {
            chunks.push(&value[start..index]);
            start = index;
            units = 0;
        }
        units += character.len_utf16();
    }
    chunks.push(&value[start..]);
    chunks
}

fn cleanup(store: &impl Store, manifest: &Manifest) {
    for index in 0..manifest.chunks {
        if store.write(&manifest.key(index), None).is_err() {
            // The committed root must not be rolled back after cleanup fails.
            tracing::warn!("Could not remove an unused OAuth credential chunk");
        }
    }
}

pub(super) fn write(
    store: &impl Store,
    value: Option<&str>,
) -> crate::Result<()> {
    let old = store.read(OAUTH_CREDENTIAL)?;
    // Reconnecting must also be able to replace a damaged manifest.
    let old_manifest = old
        .as_deref()
        .and_then(|value| parse_manifest(value).ok().flatten());
    let chunks = value.map(split_password);
    if let Some(chunks) = &chunks
        && chunks.len() > MAX_CHUNKS
    {
        return Err(input_error(
            "OAuth credentials exceed the storage limit".to_string(),
        ));
    }
    if let Some(chunks) = chunks.filter(|chunks| chunks.len() > 1) {
        let manifest = Manifest {
            generation: Uuid::new_v4(),
            chunks: chunks.len(),
        };
        // Stage under unique names, then publish the root in one write. A
        // failed refresh leaves the previous credential document readable.
        for (index, chunk) in chunks.iter().enumerate() {
            if let Err(error) = store.write(&manifest.key(index), Some(chunk)) {
                cleanup(store, &manifest);
                return Err(error);
            }
        }
        let root = format!(
            "{MANIFEST_PREFIX}{}",
            serde_json::to_string(&manifest)
                .expect("OAuth credential manifest serializes")
        );
        if let Err(error) = store.write(OAUTH_CREDENTIAL, Some(&root)) {
            cleanup(store, &manifest);
            return Err(error);
        }
    } else {
        store.write(OAUTH_CREDENTIAL, value)?;
    }
    if let Some(manifest) = &old_manifest {
        cleanup(store, manifest);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::*;

    #[derive(Default)]
    struct MemoryStore {
        entries: RefCell<HashMap<String, String>>,
        fail_at: RefCell<Option<usize>>,
    }

    impl Store for MemoryStore {
        fn read(&self, name: &str) -> crate::Result<Option<String>> {
            Ok(self.entries.borrow().get(name).cloned())
        }

        fn write(&self, name: &str, value: Option<&str>) -> crate::Result<()> {
            let mut fail_at = self.fail_at.borrow_mut();
            if let Some(remaining) = fail_at.as_mut() {
                if *remaining == 0 {
                    *fail_at = None;
                    return Err(input_error(
                        "Simulated credential store failure".to_string(),
                    ));
                }
                *remaining -= 1;
            }
            let mut entries = self.entries.borrow_mut();
            if let Some(value) = value {
                assert!(value.encode_utf16().count() <= MAX_PASSWORD_UNITS);
                entries.insert(name.to_string(), value.to_string());
            } else {
                entries.remove(name);
            }
            Ok(())
        }
    }

    fn long_oauth_json() -> String {
        serde_json::to_string(&super::super::OAuthCredentials {
            access_token: "a".repeat(7000),
            refresh_token: Some("r".repeat(3000)),
            account_id: Some("test-account".to_string()),
            expires_at: Some(1234567890),
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn round_trips_long_oauth_tokens_with_windows_limits() {
        let store = MemoryStore::default();
        let value = long_oauth_json();
        write(&store, Some(&value)).unwrap();
        assert_eq!(read(&store).unwrap().as_deref(), Some(value.as_str()));
        let credentials: super::super::OAuthCredentials =
            serde_json::from_str(&read(&store).unwrap().unwrap()).unwrap();
        assert_eq!(credentials.access_token.len(), 7000);
        assert_eq!(credentials.refresh_token.unwrap().len(), 3000);
    }

    #[test]
    fn reads_and_replaces_legacy_json() {
        let store = MemoryStore::default();
        let legacy = r#"{"access_token":"legacy","refresh_token":"refresh"}"#;
        store.write(OAUTH_CREDENTIAL, Some(legacy)).unwrap();
        assert_eq!(read(&store).unwrap().as_deref(), Some(legacy));
        let value = long_oauth_json();
        write(&store, Some(&value)).unwrap();
        assert_eq!(read(&store).unwrap().as_deref(), Some(value.as_str()));
    }

    #[test]
    fn respects_utf16_boundaries_without_splitting_characters() {
        for value in [
            "a".repeat(1280),
            "a".repeat(1281),
            "a".repeat(2560),
            "a".repeat(2561),
            format!("{}🦎中{}", "a".repeat(1279), "🦎".repeat(3000)),
            format!("{} {} end ", "a".repeat(1279), " ".repeat(1280)),
        ] {
            let store = MemoryStore::default();
            write(&store, Some(&value)).unwrap();
            assert_eq!(read(&store).unwrap().as_deref(), Some(value.as_str()));
            assert_eq!(split_password(&value).concat(), value);
        }
    }

    #[test]
    fn refresh_and_disconnect_remove_old_chunks() {
        let store = MemoryStore::default();
        write(&store, Some(&long_oauth_json())).unwrap();
        let old_keys: Vec<_> = store
            .entries
            .borrow()
            .keys()
            .filter(|key| key.as_str() != OAUTH_CREDENTIAL)
            .cloned()
            .collect();
        let refreshed = "new-token".repeat(1000);
        write(&store, Some(&refreshed)).unwrap();
        assert_eq!(read(&store).unwrap().as_deref(), Some(refreshed.as_str()));
        assert!(
            old_keys
                .iter()
                .all(|key| !store.entries.borrow().contains_key(key))
        );
        write(&store, Some("short-token")).unwrap();
        assert_eq!(store.entries.borrow().len(), 1);
        write(&store, Some(&refreshed)).unwrap();
        write(&store, None).unwrap();
        assert!(read(&store).unwrap().is_none());
        assert!(store.entries.borrow().is_empty());
        write(&store, None).unwrap();
    }

    #[test]
    fn failed_writes_preserve_previous_credentials_and_clean_staged_chunks() {
        let value = long_oauth_json();
        for legacy in [false, true] {
            // Every chunk write and the final manifest write can fail.
            for fail_at in 0..=split_password(&value).len() {
                let store = MemoryStore::default();
                let previous = if legacy {
                    "legacy".to_string()
                } else {
                    "old".repeat(2000)
                };
                write(&store, Some(&previous)).unwrap();
                let snapshot = store.entries.borrow().clone();
                *store.fail_at.borrow_mut() = Some(fail_at);
                assert!(write(&store, Some(&value)).is_err());
                assert_eq!(
                    read(&store).unwrap().as_deref(),
                    Some(previous.as_str())
                );
                assert_eq!(*store.entries.borrow(), snapshot);
            }
        }
    }

    #[test]
    fn rejects_incomplete_or_invalid_credentials() {
        let store = MemoryStore::default();
        write(&store, Some(&long_oauth_json())).unwrap();
        let manifest =
            parse_manifest(&store.read(OAUTH_CREDENTIAL).unwrap().unwrap())
                .unwrap()
                .unwrap();
        store.write(&manifest.key(1), None).unwrap();
        assert!(read(&store).is_err());
        for root in [
            format!("{MANIFEST_PREFIX}invalid"),
            format!(
                "{MANIFEST_PREFIX}{}",
                serde_json::json!({ "generation": Uuid::nil(), "chunks": 0 })
            ),
            format!(
                "{MANIFEST_PREFIX}{}",
                serde_json::json!({
                    "generation": Uuid::nil(), "chunks": MAX_CHUNKS + 1
                })
            ),
        ] {
            store.write(OAUTH_CREDENTIAL, Some(&root)).unwrap();
            assert!(read(&store).is_err());
            write(&store, Some("reconnected")).unwrap();
            assert_eq!(read(&store).unwrap().as_deref(), Some("reconnected"));
        }
    }

    #[cfg(windows)]
    #[test]
    fn round_trips_in_windows_credential_manager() {
        struct WindowsStore(String);

        impl Store for WindowsStore {
            fn read(&self, name: &str) -> crate::Result<Option<String>> {
                match keyring::Entry::new(&self.0, name).unwrap().get_password()
                {
                    Ok(value) => Ok(Some(value)),
                    Err(keyring::Error::NoEntry) => Ok(None),
                    Err(error) => Err(input_error(error.to_string())),
                }
            }

            fn write(
                &self,
                name: &str,
                value: Option<&str>,
            ) -> crate::Result<()> {
                let entry = keyring::Entry::new(&self.0, name).unwrap();
                let result = match value {
                    Some(value) => entry.set_password(value),
                    None => entry.delete_credential(),
                };
                match result {
                    Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                    Err(error) => Err(input_error(error.to_string())),
                }
            }
        }

        impl Drop for WindowsStore {
            fn drop(&mut self) {
                let _ = write(self, None);
            }
        }

        // This namespace is unique to the test and never uses app credentials.
        let store =
            WindowsStore(format!("axolotl-oauth-test-{}", Uuid::new_v4()));
        store
            .write(OAUTH_CREDENTIAL, Some(&"a".repeat(1280)))
            .unwrap();
        assert!(
            store
                .write(OAUTH_CREDENTIAL, Some(&"a".repeat(1281)))
                .is_err()
        );
        let value = long_oauth_json();
        assert!(store.write(OAUTH_CREDENTIAL, Some(&value)).is_err());
        write(&store, Some(&value)).unwrap();
        assert_eq!(read(&store).unwrap().as_deref(), Some(value.as_str()));
        let refreshed =
            format!("{}🦎 中 {}", "a".repeat(2559), "r".repeat(5000));
        write(&store, Some(&refreshed)).unwrap();
        assert_eq!(read(&store).unwrap().as_deref(), Some(refreshed.as_str()));
        write(&store, None).unwrap();
        assert!(read(&store).unwrap().is_none());
    }
}
