//! Rebuildable public profile metadata; login tokens remain in the account DB.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::{Credentials, MinecraftProfile};

fn path(directory: &Path, credentials: &Credentials) -> PathBuf {
    // Account identity includes the provider, so equal UUIDs on different
    // Yggdrasil servers cannot reuse one another's skin metadata.
    let key = Sha256::digest(credentials.account_id().as_bytes());
    directory.join(format!("{key:x}.json"))
}

pub(in crate::api) async fn read(
    directory: &Path,
    credentials: &Credentials,
) -> Option<MinecraftProfile> {
    let bytes = tokio::fs::read(path(directory, credentials)).await.ok()?;
    let profile: MinecraftProfile = serde_json::from_slice(&bytes).ok()?;
    (profile.id == credentials.offline_profile.id).then_some(profile)
}

pub(in crate::api) async fn write(
    directory: &Path,
    credentials: &Credentials,
    profile: &MinecraftProfile,
) {
    let result = async {
        tokio::fs::create_dir_all(directory).await?;
        let bytes = serde_json::to_vec(profile)?;
        tokio::fs::write(path(directory, credentials), bytes).await?;
        Ok::<_, crate::Error>(())
    }
    .await;
    if let Err(error) = result {
        tracing::warn!(%error, "Could not cache Minecraft profile metadata");
    }
}

pub(super) async fn remove(directory: &Path, credentials: &Credentials) {
    if let Err(error) =
        tokio::fs::remove_file(path(directory, credentials)).await
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(%error, "Could not remove cached Minecraft profile metadata");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::minecraft_auth::MinecraftUser;
    use chrono::{Duration, Utc};

    #[tokio::test]
    async fn reads_cached_skin_with_expired_tokens() {
        let directory = tempfile::tempdir().unwrap();
        let mut credentials = Credentials::offline("SavedPlayer").unwrap();
        credentials.account_type =
            crate::state::MinecraftAccountType::Microsoft;
        credentials.access_token = "must-not-be-persisted".to_string();
        credentials.refresh_token = "must-not-be-refreshed".to_string();
        credentials.expires = Utc::now() - Duration::days(1);
        let mut profile = credentials.offline_profile.clone();
        profile.skins = serde_json::from_value(serde_json::json!([{
            "id": uuid::Uuid::new_v4(), "state": "ACTIVE",
            "url": "https://textures.minecraft.net/texture/saved-skin",
            "textureKey": "saved-skin", "variant": "CLASSIC"
        }]))
        .unwrap();
        write(directory.path(), &credentials, &profile).await;
        let bytes = tokio::fs::read(path(directory.path(), &credentials))
            .await
            .unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(!text.contains("must-not-be"));
        let user = MinecraftUser::from_credentials(
            credentials,
            true,
            directory.path(),
        )
        .await;
        assert_eq!(user.profile.name, "SavedPlayer");
        assert_eq!(user.profile.skins.len(), 1);
        assert_eq!(user.access_token, "must-not-be-persisted");
        assert!(user.expires < Utc::now());
    }

    #[tokio::test]
    async fn missing_corrupt_or_mismatched_cache_keeps_the_local_account() {
        let directory = tempfile::tempdir().unwrap();
        let credentials = Credentials::offline("LocalPlayer").unwrap();
        for bytes in [
            None,
            Some(b"invalid".to_vec()),
            Some(
                serde_json::to_vec(&MinecraftProfile {
                    id: uuid::Uuid::new_v4(),
                    ..Default::default()
                })
                .unwrap(),
            ),
        ] {
            if let Some(bytes) = bytes {
                tokio::fs::write(path(directory.path(), &credentials), bytes)
                    .await
                    .unwrap();
            }
            let user = MinecraftUser::from_credentials(
                Credentials::offline("LocalPlayer").unwrap(),
                true,
                directory.path(),
            )
            .await;
            assert_eq!(user.profile.name, "LocalPlayer");
            assert_eq!(user.profile.id, credentials.offline_profile.id);
        }
    }

    #[test]
    fn cache_keys_include_account_provider_identity() {
        let mut credentials = Credentials::offline("SamePlayer").unwrap();
        let offline = path(Path::new("cache"), &credentials);
        credentials.account_id = None;
        credentials.account_type =
            crate::state::MinecraftAccountType::Microsoft;
        assert_ne!(offline, path(Path::new("cache"), &credentials));
        credentials.account_type =
            crate::state::MinecraftAccountType::Yggdrasil;
        credentials.yggdrasil = Some(crate::state::YggdrasilAccount {
            api_root: "https://first.test/api/yggdrasil".to_string(),
            server_name: "First".to_string(),
            login: "player".to_string(),
            client_token: String::new(),
        });
        let first = path(Path::new("cache"), &credentials);
        credentials.yggdrasil.as_mut().unwrap().api_root =
            "https://second.test/api/yggdrasil".to_string();
        assert_ne!(first, path(Path::new("cache"), &credentials));
    }
}
