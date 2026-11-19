//! JSON account identities and OS credentials; SQL is only a legacy importer.

pub(crate) mod document;
mod session;
#[cfg(test)]
mod tests;

use self::document::Metadata;
use self::session::{Session, Tokens};
use super::minecraft_auth::CredentialPersistence;
use super::persistence::{
    Persistence, document::Document, status::Availability,
};
use super::store::minecraft_users::{AccountType, MinecraftUserRecord};
use super::store::secrets;
use serde_json::{Map, Value, json};
use std::sync::Arc;

const TOKEN_FIELDS: [&str; 3] =
    ["access_token", "refresh_token", "yggdrasil_client_token"];

#[derive(Clone)]
pub(crate) struct Accounts {
    document: Document,
    session: Arc<Session>,
    _temporary: Option<Arc<tempfile::TempDir>>,
}

impl Accounts {
    pub(crate) fn new(persistence: &Persistence) -> Self {
        Self {
            document: persistence
                .document(crate::state::layout::ACCOUNTS, "axolotl-accounts", 1)
                .expect("constant account document path"),
            session: Arc::new(Session::default()),
            _temporary: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn temporary() -> Self {
        let temporary = Arc::new(
            tempfile::tempdir().expect("temporary account state directory"),
        );
        let mut accounts = Self::new(&Persistence::new(temporary.path()));
        accounts._temporary = Some(temporary);
        accounts
    }

    pub(crate) async fn get_all(
        &self,
    ) -> crate::Result<Vec<(String, MinecraftUserRecord, CredentialPersistence)>>
    {
        let service = self.clone();
        let credentials = secrets::access();
        self.document.inspect(move |read| {
            if read.availability == Availability::Missing && service._temporary.is_some() { return Ok(Vec::new()); }
            if !matches!(read.availability, Availability::Ready | Availability::Recovered) {
                return Err(crate::ErrorKind::OtherError("Account metadata is unavailable or uses a newer protocol".into()).into());
            }
            let snapshot = read.snapshot.expect("readable account document");
            let data = snapshot.data().expect("account document payload");
            let selected = data.get("default_account_id").and_then(Value::as_str);
            let entries = account_map(data)?;
            let mut accounts = Vec::new();
            for (id, raw) in entries {
                let metadata: Metadata = match serde_json::from_value(raw.clone()) {
                    Ok(metadata) => metadata,
                    Err(error) => { tracing::warn!(account = id, %error, "Skipping unreadable account metadata"); continue; }
                };
                if metadata.record(false, Tokens::default()).is_none() {
                    tracing::warn!(account = id, "Skipping unsupported account metadata");
                    continue;
                }
                let (tokens, status) = service.tokens(id, &metadata, credentials)?;
                if let Some(record) = metadata.record(selected == Some(id.as_str()), tokens) {
                    accounts.push((id.clone(), record, status));
                } else { tracing::warn!(account = id, "Skipping an unsupported account identity"); }
            }
            Ok(accounts)
        }).await
    }

    fn tokens(
        &self,
        id: &str,
        metadata: &Metadata,
        credentials: secrets::Access,
    ) -> crate::Result<(Tokens, CredentialPersistence)> {
        if metadata.account_type == "offline" {
            return Ok((Tokens::default(), CredentialPersistence::NotRequired));
        }
        if let Some(tokens) = self.session.get(id) {
            return Ok((tokens, CredentialPersistence::SessionOnly));
        }
        let mut values = [String::new(), String::new(), String::new()];
        for (index, field) in TOKEN_FIELDS.iter().enumerate() {
            if metadata.credential_fields.iter().any(|name| name == field) {
                if let Some(value) = credentials.read(&token_key(id, field))? {
                    values[index] = value;
                }
            }
        }
        let status = if values[0].is_empty()
            || (metadata.account_type == "microsoft" && values[1].is_empty())
        {
            CredentialPersistence::RequiresLogin
        } else {
            CredentialPersistence::Keyring
        };
        let [access, refresh, client] = values;
        Ok((
            Tokens {
                access,
                refresh,
                client,
            },
            status,
        ))
    }

    pub(crate) async fn set(
        &self,
        id: &str,
        record: &MinecraftUserRecord,
    ) -> crate::Result<CredentialPersistence> {
        self.set_inner(id, record, false).await
    }

    pub(crate) async fn set_existing(
        &self,
        id: &str,
        record: &MinecraftUserRecord,
    ) -> crate::Result<CredentialPersistence> {
        self.set_inner(id, record, true).await
    }

    async fn set_inner(
        &self,
        id: &str,
        record: &MinecraftUserRecord,
        existing_only: bool,
    ) -> crate::Result<CredentialPersistence> {
        let account_id = id.to_owned();
        let id = id.to_owned();
        let record = record.clone();
        let session = self.session.clone();
        let credentials = secrets::access();
        let cached_tokens = Tokens {
            access: record.access_token.clone(),
            refresh: record.refresh_token.clone(),
            client: record.yggdrasil_client_token.clone(),
        };
        let completion_id = id.clone();
        let snapshot = self.document.update_and_then(move |data| {
            let accounts = data.entry("accounts").or_insert_with(|| json!({})).as_object_mut()
                .ok_or_else(|| crate::ErrorKind::InputError("Account identities are not an object".into()))?;
            if existing_only && !accounts.contains_key(&id) {
                return Err(crate::ErrorKind::InputError("Account was removed during token refresh".into()).into());
            }
            if let Some(raw) = accounts.get(&id) {
                let existing: Metadata = serde_json::from_value(raw.clone())?;
                if existing.record(false, Tokens::default()).is_none() {
                    return Err(crate::ErrorKind::InputError("Cannot replace an unsupported account identity".into()).into());
                }
            }
            uuid::Uuid::parse_str(&record.uuid)?;
            let tokens = Tokens { access: record.access_token.clone(), refresh: record.refresh_token.clone(), client: record.yggdrasil_client_token.clone() };
            let mut fields = Vec::new();
            let mut persistent = true;
            if record.account_type != AccountType::Offline {
                for (field, value) in TOKEN_FIELDS.into_iter().zip([&tokens.access, &tokens.refresh, &tokens.client]) {
                    if value.is_empty() { continue; }
                    if credentials.write(&token_key(&id, field), Some(value)).is_ok() { fields.push(field.to_owned()); }
                    else { persistent = false; }
                }
            }
            if !persistent { fields.clear(); }
            let metadata = Metadata::from_record(&record, fields);
            let entry = accounts.entry(id.clone()).or_insert_with(|| json!({})).as_object_mut()
                .ok_or_else(|| crate::ErrorKind::InputError("Account identity is not an object".into()))?;
            for (key, value) in serde_json::to_value(metadata)?.as_object().expect("metadata object") { entry.insert(key.clone(), value.clone()); }
            if !existing_only {
                if record.active { data.insert("default_account_id".into(), json!(id)); }
                else if data.get("default_account_id").and_then(Value::as_str) == Some(&id) { data.insert("default_account_id".into(), Value::Null); }
            }
            Ok(())
        }, move |snapshot| {
            let entry = &snapshot.data().expect("account payload")["accounts"][&completion_id];
            let metadata: Metadata = serde_json::from_value(entry.clone())?;
            let session_only = metadata.account_type != "offline" && metadata.credential_fields.is_empty()
                && (!cached_tokens.access.is_empty() || !cached_tokens.refresh.is_empty() || !cached_tokens.client.is_empty());
            session.set(completion_id, session_only.then_some(cached_tokens));
            Ok(())
        }).await?;
        let raw = snapshot
            .data()
            .and_then(|data| data.get("accounts"))
            .and_then(|data| data.get(&account_id));
        let metadata: Metadata = serde_json::from_value(
            raw.cloned().expect("just published account"),
        )?;
        Ok(if metadata.account_type == "offline" {
            CredentialPersistence::NotRequired
        } else if self.session.get(&account_id).is_some() {
            CredentialPersistence::SessionOnly
        } else if metadata
            .credential_fields
            .iter()
            .any(|field| field == "access_token")
        {
            CredentialPersistence::Keyring
        } else {
            CredentialPersistence::RequiresLogin
        })
    }

    pub(crate) async fn remove(&self, id: &str) -> crate::Result<()> {
        let account_id = id.to_owned();
        let id = id.to_owned();
        let session = self.session.clone();
        let credentials = secrets::access();
        self.document.update_and_then(move |data| {
            let raw = data.get("accounts").and_then(Value::as_object).and_then(|entries| entries.get(&id));
            if let Some(raw) = raw {
                let metadata: Metadata = serde_json::from_value(raw.clone())?;
                if metadata.record(false, Tokens::default()).is_none() {
                    return Err(crate::ErrorKind::InputError("Cannot remove an unsupported account identity".into()).into());
                }
            }
            if let Some(entries) = data.get_mut("accounts").and_then(Value::as_object_mut) { entries.remove(&id); }
            if data.get("default_account_id").and_then(Value::as_str) == Some(&id) { data.insert("default_account_id".into(), Value::Null); }
            Ok(())
        }, move |_| {
            session.set(account_id.clone(), None);
            for field in TOKEN_FIELDS {
                if let Err(error) = credentials.write(&token_key(&account_id, field), None) {
                    tracing::debug!(%error, "Deleted account left an unavailable orphan keyring entry");
                }
            }
            Ok(())
        }).await?;
        Ok(())
    }

    pub(crate) async fn select(&self, id: &str) -> crate::Result<()> {
        let id = id.to_owned();
        self.document
            .update(move |data| {
                let raw = account_map(data)?.get(&id).ok_or_else(|| {
                    crate::ErrorKind::InputError(
                        "Account does not exist".into(),
                    )
                })?;
                let metadata: Metadata = serde_json::from_value(raw.clone())?;
                if metadata.record(false, Tokens::default()).is_none() {
                    return Err(crate::ErrorKind::InputError(
                        "Cannot select an unsupported account identity".into(),
                    )
                    .into());
                }
                data.insert("default_account_id".into(), json!(id));
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub(crate) async fn tokens_for_redaction(
        &self,
    ) -> crate::Result<Vec<String>> {
        let service = self.clone();
        let credentials = secrets::access();
        self.document.inspect(move |read| {
            let mut tokens = service.session.values();
            let snapshot = match read.snapshot {
                Some(snapshot) if matches!(read.availability, Availability::Ready | Availability::Recovered) => snapshot,
                _ if service._temporary.is_some() && read.availability == Availability::Missing => return Ok(tokens),
                _ => return Err(crate::ErrorKind::OtherError("Account metadata is unavailable for diagnostic redaction".into()).into()),
            };
            for (id, raw) in account_map(snapshot.data().expect("account payload"))? {
                let metadata: Metadata = serde_json::from_value(raw.clone())?;
                if metadata.account_type == "offline" { continue; }
                if service.session.get(id).is_some() { continue; }
                for field in TOKEN_FIELDS {
                    if metadata.credential_fields.iter().any(|name| name == field)
                        && let Some(value) = credentials.read(&token_key(id, field))? { tokens.push(value); }
                }
            }
            tokens.retain(|token| !token.is_empty() && token != "0");
            Ok(tokens)
        }).await
    }

    pub(crate) async fn set_device_token(
        &self,
        record: &crate::state::store::minecraft_device_tokens::MinecraftDeviceTokenRecord,
    ) -> crate::Result<()> {
        // Device material is unused by current login. Explicit callers share
        // the keyring/session policy without recreating the SQL fallback.
        let key = secrets::key(&["minecraft-device-token"]);
        let tokens = serde_json::to_string(record)?;
        let cached = tokens.clone();
        let credentials = secrets::access();
        let session = self.session.clone();
        self.document
            .update_and_then(
                move |data| {
                    data.entry("accounts").or_insert_with(|| json!({}));
                    let persistent =
                        credentials.write(&key, Some(&tokens)).is_ok();
                    data.insert(
                        "device_credential_stored".into(),
                        json!(persistent),
                    );
                    Ok(())
                },
                move |snapshot| {
                    let persistent = snapshot
                        .data()
                        .and_then(|data| data.get("device_credential_stored"))
                        .and_then(Value::as_bool)
                        == Some(true);
                    session.set_device((!persistent).then_some(cached));
                    Ok(())
                },
            )
            .await?;
        Ok(())
    }

    pub(crate) async fn device_token(&self) -> crate::Result<Option<crate::state::store::minecraft_device_tokens::MinecraftDeviceTokenRecord>>{
        let credentials = secrets::access();
        let session = self.session.clone();
        self.document
            .inspect(move |read| {
                let allowed = read
                    .snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.data())
                    .and_then(|data| data.get("device_credential_stored"))
                    .and_then(Value::as_bool)
                    != Some(false);
                let value = match session.device() {
                    Some(value) => Some(value),
                    None if allowed => credentials
                        .read(&secrets::key(&["minecraft-device-token"]))?,
                    None => None,
                };
                value
                    .map(|value| {
                        serde_json::from_str(&value).map_err(Into::into)
                    })
                    .transpose()
            })
            .await
    }
}

pub(crate) fn token_key(id: &str, field: &str) -> String {
    secrets::key(&["minecraft-user", id, field])
}

pub(crate) fn account_map(
    data: &Map<String, Value>,
) -> crate::Result<&Map<String, Value>> {
    data.get("accounts")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Account document has no identity map".into(),
            )
            .into()
        })
}
