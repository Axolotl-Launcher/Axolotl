use super::page;
use launcher_tools::{Command, ToolError};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use theseus::minecraft_auth::{self as auth, MinecraftUser};

fn public_id(native_id: &str) -> String {
    format!("account:{:x}", Sha256::digest(native_id.as_bytes()))
}

fn summary(user: &MinecraftUser) -> Value {
    json!({"account_id":public_id(&user.account_id),"account_type":user.account_type,
        "player_name":user.profile.name.chars().take(64).collect::<String>(),"player_uuid":user.profile.id,
        "active":user.active,"stored_expires_at":if user.account_type==theseus::state::MinecraftAccountType::Offline {None} else {Some(user.expires)},
        "stored_token_expired":if user.account_type==theseus::state::MinecraftAccountType::Offline {None} else {Some(user.expires<=chrono::Utc::now())}})
}

fn error(error: theseus::Error) -> ToolError {
    tracing::warn!(%error,"MCP account operation failed");
    ToolError::new(
        "account_operation_failed",
        "Account operation failed; inspect offline eligibility or launcher sign-in state. No credentials are included in this response",
    )
}

async fn users() -> Result<Vec<MinecraftUser>, ToolError> {
    auth::users_with_cache(false, true).await.map_err(error)
}

fn find<'a>(
    users: &'a [MinecraftUser],
    id: &str,
) -> Result<&'a MinecraftUser, ToolError> {
    users
        .iter()
        .find(|user| public_id(&user.account_id) == id)
        .ok_or_else(|| {
            ToolError::new(
                "account_not_found",
                "Saved account no longer exists; refresh account_list",
            )
        })
}

fn reject_duplicate(
    users: &[MinecraftUser],
    native: &str,
) -> Result<(), ToolError> {
    if users.iter().any(|user| user.account_id == native) {
        return Err(ToolError::new(
            "account_exists",
            "An account with this UUID already exists",
        ));
    }
    Ok(())
}

pub(super) async fn execute(command: Command) -> Result<Value, ToolError> {
    match command {
        Command::AccountList {
            offline_only,
            offset,
            limit,
        } => {
            let mut users = users().await?;
            users.retain(|user| {
                !offline_only
                    || user.account_type
                        == theseus::state::MinecraftAccountType::Offline
            });
            users.sort_by(|a, b| {
                public_id(&a.account_id).cmp(&public_id(&b.account_id))
            });
            page(users.iter().map(summary).collect(), offset, limit)
        }
        Command::AccountGet { account_id } => {
            Ok(summary(find(&users().await?, &account_id)?))
        }
        Command::AccountGetDefault { offline_mode } => {
            let id =
                auth::get_default_user(offline_mode).await.map_err(error)?;
            let users = users().await?;
            Ok(
                json!({"account":id.and_then(|id|users.iter().find(|user|user.account_id==id).map(summary))}),
            )
        }
        Command::AccountSetDefault { account_id } => {
            let users = users().await?;
            let native = find(&users, &account_id)?.account_id.clone();
            auth::set_default_user(&native).await.map_err(error)?;
            Ok(summary(find(&self::users().await?, &account_id)?))
        }
        Command::AccountRemove { account_id } => {
            let users = users().await?;
            let native = find(&users, &account_id)?.account_id.clone();
            auth::remove_user(&native).await.map_err(error)?;
            let default = auth::get_default_user(false).await.map_err(error)?;
            Ok(
                json!({"removed":true,"account_id":account_id,"default_account_id":default.map(|id|public_id(&id))}),
            )
        }
        Command::AccountAddOffline { username, uuid } => {
            let proposed = match uuid {
                Some(uuid) => theseus::data::Credentials::offline_with_uuid(
                    &username, uuid,
                ),
                None => theseus::data::Credentials::offline(&username),
            }
            .map_err(error)?;
            reject_duplicate(&users().await?, &proposed.account_id())?;
            let credential = auth::add_offline_user(&username, uuid)
                .await
                .map_err(error)?;
            let id = public_id(&credential.account_id());
            // Never serialize Credentials/MinecraftUser: both carry secrets.
            Ok(summary(find(&users().await?, &id)?))
        }
        Command::AccountGetEligibility {} => {
            let status = theseus::anti_piracy::status().await;
            Ok(
                json!({"region":status.region,"restricted":status.restricted,"revision":status.revision,"offline_accounts_allowed":!status.restricted}),
            )
        }
        _ => Err(ToolError::new(
            "unsupported_command",
            "Not an account operation",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn user() -> MinecraftUser {
        MinecraftUser {account_id:"yggdrasil:https://private-server.example/auth:player".into(),profile:serde_json::from_value(json!({"id":"00000000-0000-4000-8000-000000000001","name":"玩家","skins":[],"capes":[]})).unwrap(),
            account_type:theseus::state::MinecraftAccountType::Yggdrasil,access_token:"secret-access".into(),refresh_token:"secret-refresh".into(),expires:chrono::Utc::now()-chrono::Duration::hours(1),active:true,yggdrasil:None}
    }
    #[test]
    fn summaries_only_include_approved_fields_and_mask_native_ids() {
        let value = summary(&user());
        assert_eq!(value["player_name"], "玩家");
        assert_eq!(value["stored_token_expired"], true);
        assert_eq!(value["account_id"].as_str().unwrap().len(), 72);
        for secret in [
            "secret-access",
            "secret-refresh",
            "private-server",
            "https://",
            "access_token",
            "refresh_token",
            "skins",
            "capes",
            "yggdrasil:",
        ] {
            assert!(!value.to_string().contains(secret), "{secret}");
        }
    }
    #[test]
    fn account_lookup_uses_stable_digest_ids_not_player_names_or_native_ids() {
        let user = user();
        let id = public_id(&user.account_id);
        let native = user.account_id.clone();
        let users = vec![user];
        assert_eq!(find(&users, &id).unwrap().account_id, native);
        assert!(find(&users, &native).is_err());
        assert!(find(&users, "玩家").is_err());
        assert_eq!(public_id(&native), id);
        assert_ne!(public_id("different-account"), id);
        assert_eq!(
            reject_duplicate(&users, &native).unwrap_err().code,
            "account_exists"
        );
    }
    #[test]
    fn offline_accounts_have_no_token_expiry_claims() {
        let mut user = user();
        user.account_type = theseus::state::MinecraftAccountType::Offline;
        let value = summary(&user);
        assert!(value["stored_expires_at"].is_null());
        assert!(value["stored_token_expired"].is_null());
    }
}
