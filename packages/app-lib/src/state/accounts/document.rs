//! Account metadata contains only identity, service information and key names.

use super::session::Tokens;
use crate::state::store::minecraft_users::{AccountType, MinecraftUserRecord};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Metadata {
    pub(crate) uuid: String,
    pub(crate) username: String,
    pub(crate) account_type: String,
    #[serde(default)]
    pub(crate) expires: i64,
    #[serde(default)]
    pub(crate) yggdrasil_api_root: String,
    #[serde(default)]
    pub(crate) yggdrasil_server_name: String,
    #[serde(default)]
    pub(crate) yggdrasil_login: String,
    #[serde(default)]
    pub(crate) credential_fields: Vec<String>,
}

impl Metadata {
    pub(crate) fn from_record(
        record: &MinecraftUserRecord,
        credential_fields: Vec<String>,
    ) -> Self {
        Self {
            uuid: record.uuid.clone(),
            username: record.username.clone(),
            account_type: record.account_type.as_str().into(),
            expires: record.expires,
            yggdrasil_api_root: record.yggdrasil_api_root.clone(),
            yggdrasil_server_name: record.yggdrasil_server_name.clone(),
            yggdrasil_login: record.yggdrasil_login.clone(),
            credential_fields,
        }
    }

    pub(super) fn record(
        &self,
        active: bool,
        tokens: Tokens,
    ) -> Option<MinecraftUserRecord> {
        if self.credential_fields.iter().any(|field| {
            !matches!(
                field.as_str(),
                "access_token" | "refresh_token" | "yggdrasil_client_token"
            )
        }) {
            return None;
        }
        let account_type = match self.account_type.as_str() {
            "microsoft" => AccountType::Microsoft,
            "offline" => AccountType::Offline,
            "yggdrasil" => AccountType::Yggdrasil,
            _ => return None,
        };
        uuid::Uuid::parse_str(&self.uuid).ok()?;
        Some(MinecraftUserRecord {
            uuid: self.uuid.clone(),
            username: self.username.clone(),
            account_type,
            active,
            expires: self.expires,
            access_token: if account_type == AccountType::Offline {
                "0".into()
            } else {
                tokens.access
            },
            refresh_token: tokens.refresh,
            yggdrasil_client_token: tokens.client,
            yggdrasil_api_root: self.yggdrasil_api_root.clone(),
            yggdrasil_server_name: self.yggdrasil_server_name.clone(),
            yggdrasil_login: self.yggdrasil_login.clone(),
        })
    }
}
