//! What the Minecraft account collections promise, and the records the
//! launcher credentials leave in them.

use futures::StreamExt;
use uuid::Uuid;

use super::support;
use crate::state::Credentials;
use crate::state::MinecraftSkinVariant;
use crate::state::minecraft_skins::{
    CustomMinecraftSkin, CustomMinecraftSkinInsertPosition,
    OfflineMinecraftSkin,
};
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::custom_minecraft_skin_textures::{
    CustomMinecraftSkinTextureRecord, CustomMinecraftSkinTextures,
};
use crate::state::store::custom_minecraft_skins::{
    CustomMinecraftSkinKey, CustomMinecraftSkinRecord, CustomMinecraftSkins,
    SkinVariant,
};
use crate::state::store::minecraft_device_tokens::{
    MinecraftDeviceTokenRecord, MinecraftDeviceTokens, Singleton,
};
use crate::state::store::minecraft_users::{
    AccountType, MinecraftUserRecord, MinecraftUsers,
};
use crate::state::store::offline_minecraft_skins::{
    OfflineMinecraftSkinRecord, OfflineMinecraftSkins,
};

fn handle_of(pool: &sqlx::SqlitePool) -> Handle {
    Handle::new(pool.clone())
}

fn user_key(uuid: Uuid) -> String {
    uuid.as_hyphenated().to_string()
}

fn account_record(
    uuid: Uuid,
    account_type: AccountType,
) -> MinecraftUserRecord {
    MinecraftUserRecord {
        uuid: user_key(uuid),
        active: false,
        username: "Player".to_string(),
        account_type,
        access_token: "access".to_string(),
        refresh_token: "refresh".to_string(),
        expires: 0,
        yggdrasil_api_root: String::new(),
        yggdrasil_server_name: String::new(),
        yggdrasil_login: String::new(),
        yggdrasil_client_token: String::new(),
    }
}

async fn add_account(handle: &Handle, account_id: &str, uuid: Uuid) {
    MinecraftUsers::set(
        handle,
        &account_id.to_string(),
        Some(&account_record(uuid, AccountType::Microsoft)),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn an_account_round_trips_and_clears() {
    let handle = support::handle().await;
    let account_id = "microsoft:one".to_string();
    let record = account_record(Uuid::new_v4(), AccountType::Microsoft);

    MinecraftUsers::set(&handle, &account_id, Some(&record))
        .await
        .unwrap();
    assert_eq!(
        MinecraftUsers::get(&handle, &account_id).await.unwrap(),
        Some(record)
    );

    MinecraftUsers::set(&handle, &account_id, None)
        .await
        .unwrap();
    assert_eq!(
        MinecraftUsers::get(&handle, &account_id).await.unwrap(),
        None
    );
}

#[tokio::test]
async fn every_account_type_round_trips() {
    let handle = support::handle().await;

    for (index, account_type) in [
        AccountType::Microsoft,
        AccountType::Offline,
        AccountType::Yggdrasil,
    ]
    .into_iter()
    .enumerate()
    {
        let account_id = format!("account:{index}");
        let record = account_record(Uuid::new_v4(), account_type);

        MinecraftUsers::set(&handle, &account_id, Some(&record))
            .await
            .unwrap();

        assert_eq!(
            MinecraftUsers::get(&handle, &account_id).await.unwrap(),
            Some(record)
        );
    }
}

#[tokio::test]
async fn a_replacement_deletes_the_accounts_it_leaves_out() {
    let handle = support::handle().await;
    let kept_id = "microsoft:kept".to_string();
    let dropped_id = "microsoft:dropped".to_string();
    add_account(&handle, &kept_id, Uuid::new_v4()).await;
    add_account(&handle, &dropped_id, Uuid::new_v4()).await;
    let kept = MinecraftUsers::get(&handle, &kept_id)
        .await
        .unwrap()
        .unwrap();

    MinecraftUsers::set_all(&handle, &[(kept_id.clone(), kept)])
        .await
        .unwrap();

    assert!(
        MinecraftUsers::get(&handle, &kept_id)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        MinecraftUsers::get(&handle, &dropped_id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn a_scoped_read_names_the_account_that_holds_it() {
    let handle = support::handle().await;
    let account_id = "microsoft:one".to_string();
    let record = account_record(Uuid::new_v4(), AccountType::Microsoft);
    MinecraftUsers::set(&handle, &account_id, Some(&record))
        .await
        .unwrap();

    assert_eq!(
        MinecraftUsers::get_scoped(&handle, &account_id)
            .await
            .unwrap(),
        vec![(account_id.clone(), record)]
    );
    assert!(
        MinecraftUsers::get_scoped(&handle, &"microsoft:other".to_string())
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn the_device_token_round_trips_with_its_claims() {
    let handle = support::handle().await;
    let record = MinecraftDeviceTokenRecord {
        uuid: "device".to_string(),
        private_key: "private-key".to_string(),
        x: "1".to_string(),
        y: "2".to_string(),
        issue_instant: 10,
        not_after: 20,
        token: "token".to_string(),
        display_claims: r#"{"xui":[{"uhs":"1"}]}"#.to_string(),
    };

    MinecraftDeviceTokens::set(&handle, &Singleton, Some(&record))
        .await
        .unwrap();
    let stored = MinecraftDeviceTokens::get(&handle, &Singleton)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(stored.uuid, record.uuid);
    assert_eq!(stored.private_key, record.private_key);
    assert_eq!(stored.x, record.x);
    assert_eq!(stored.y, record.y);
    assert_eq!(stored.issue_instant, record.issue_instant);
    assert_eq!(stored.not_after, record.not_after);
    assert_eq!(stored.token, record.token);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&stored.display_claims)
            .unwrap(),
        serde_json::json!({"xui": [{"uhs": "1"}]})
    );

    MinecraftDeviceTokens::set(&handle, &Singleton, None)
        .await
        .unwrap();
    assert!(
        MinecraftDeviceTokens::get(&handle, &Singleton)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn a_texture_round_trips_and_clears() {
    let handle = support::handle().await;
    let texture_key = "texture".to_string();
    let record = CustomMinecraftSkinTextureRecord {
        texture: vec![1, 2, 3],
    };

    CustomMinecraftSkinTextures::set(&handle, &texture_key, Some(&record))
        .await
        .unwrap();
    assert_eq!(
        CustomMinecraftSkinTextures::get(&handle, &texture_key)
            .await
            .unwrap(),
        Some(record)
    );

    CustomMinecraftSkinTextures::set(&handle, &texture_key, None)
        .await
        .unwrap();
    assert!(
        CustomMinecraftSkinTextures::get(&handle, &texture_key)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn a_saved_skin_round_trips_under_its_account_and_texture() {
    let handle = support::handle().await;
    let uuid = Uuid::new_v4();
    add_account(&handle, &format!("microsoft:{}", user_key(uuid)), uuid).await;
    let texture_key = "texture".to_string();
    CustomMinecraftSkinTextures::set(
        &handle,
        &texture_key,
        Some(&CustomMinecraftSkinTextureRecord { texture: vec![1] }),
    )
    .await
    .unwrap();
    let key = CustomMinecraftSkinKey {
        minecraft_user_uuid: user_key(uuid),
        texture_key: texture_key.clone(),
    };
    let record = CustomMinecraftSkinRecord {
        variant: SkinVariant::Slim,
        cape_id: Some(user_key(Uuid::new_v4())),
        display_order: 2,
    };

    CustomMinecraftSkins::set(&handle, &key, Some(&record))
        .await
        .unwrap();
    assert_eq!(
        CustomMinecraftSkins::get(&handle, &key).await.unwrap(),
        Some(record)
    );

    CustomMinecraftSkins::set(&handle, &key, None)
        .await
        .unwrap();
    assert_eq!(
        CustomMinecraftSkins::get(&handle, &key).await.unwrap(),
        None
    );
}

#[tokio::test]
async fn a_scope_reads_the_saved_skins_in_their_displayed_order() {
    let handle = support::handle().await;
    let uuid = Uuid::new_v4();
    add_account(&handle, &format!("microsoft:{}", user_key(uuid)), uuid).await;

    for (texture_key, display_order) in
        [("second", 1_i64), ("first", 0), ("third", 2)]
    {
        let texture_key = texture_key.to_string();
        CustomMinecraftSkinTextures::set(
            &handle,
            &texture_key,
            Some(&CustomMinecraftSkinTextureRecord { texture: vec![0] }),
        )
        .await
        .unwrap();
        CustomMinecraftSkins::set(
            &handle,
            &CustomMinecraftSkinKey {
                minecraft_user_uuid: user_key(uuid),
                texture_key,
            },
            Some(&CustomMinecraftSkinRecord {
                variant: SkinVariant::Classic,
                cape_id: None,
                display_order,
            }),
        )
        .await
        .unwrap();
    }

    let saved = CustomMinecraftSkins::get_scoped(&handle, &user_key(uuid))
        .await
        .unwrap();

    assert_eq!(
        saved
            .iter()
            .map(|(key, _)| key.texture_key.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second", "third"]
    );
}

#[tokio::test]
async fn an_offline_skin_round_trips_and_clears() {
    let handle = support::handle().await;
    let uuid = Uuid::new_v4();
    add_account(&handle, &format!("offline:{}", user_key(uuid)), uuid).await;
    let key = user_key(uuid);
    let record = OfflineMinecraftSkinRecord {
        texture_key: "texture".to_string(),
        variant: SkinVariant::Classic,
    };

    OfflineMinecraftSkins::set(&handle, &key, Some(&record))
        .await
        .unwrap();
    assert_eq!(
        OfflineMinecraftSkins::get(&handle, &key).await.unwrap(),
        Some(record)
    );

    OfflineMinecraftSkins::set(&handle, &key, None)
        .await
        .unwrap();
    assert!(
        OfflineMinecraftSkins::get(&handle, &key)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn persists_offline_account_as_active() {
    let handle = support::handle().await;
    let credentials = Credentials::offline("OfflineUser").unwrap();

    credentials.upsert(&handle).await.unwrap();

    let stored = Credentials::get_active(&handle).await.unwrap().unwrap();
    assert_eq!(stored.offline_profile.name, "OfflineUser");
    assert_eq!(stored.offline_profile.id, credentials.offline_profile.id);
    assert!(stored.is_offline());
    assert!(stored.active);
}

#[tokio::test]
async fn persists_offline_account_with_custom_uuid() {
    let handle = support::handle().await;
    let uuid = Uuid::new_v4();
    let credentials =
        Credentials::offline_with_uuid("CustomUuid", uuid).unwrap();

    credentials.upsert(&handle).await.unwrap();

    let stored = Credentials::get_active(&handle).await.unwrap().unwrap();
    assert_eq!(stored.offline_profile.id, uuid);
    assert_eq!(stored.offline_profile.name, "CustomUuid");
    assert!(stored.is_offline());
}

#[tokio::test]
async fn preserves_accounts_with_the_same_uuid_across_account_types() {
    let handle = support::handle().await;
    let uuid = Uuid::new_v4();
    let third_party = MinecraftUserRecord {
        active: true,
        username: "ThirdPartyUser".to_string(),
        account_type: AccountType::Yggdrasil,
        access_token: "third-party-token".to_string(),
        refresh_token: String::new(),
        ..account_record(uuid, AccountType::Yggdrasil)
    };
    MinecraftUsers::set(
        &handle,
        &format!("yggdrasil:https://example.invalid:{}", user_key(uuid)),
        Some(&third_party),
    )
    .await
    .unwrap();

    let offline = Credentials::offline_with_uuid("OfflineUser", uuid).unwrap();
    offline.upsert(&handle).await.unwrap();

    let stored = Credentials::get_all_without_refresh(&handle).await.unwrap();
    assert_eq!(stored.len(), 2);
    assert!(stored.iter().any(Credentials::is_yggdrasil));
    assert!(stored.iter().any(Credentials::is_offline));
    assert_ne!(stored[0].account_id(), stored[1].account_id());
}

#[tokio::test]
async fn selects_offline_account_when_online_account_is_active() {
    let handle = support::handle().await;
    let mut offline = Credentials::offline("OfflineUser").unwrap();
    offline.active = false;
    offline.upsert(&handle).await.unwrap();

    let online = MinecraftUserRecord {
        active: true,
        username: "OnlineUser".to_string(),
        access_token: "expired-access-token".to_string(),
        refresh_token: "expired-refresh-token".to_string(),
        ..account_record(Uuid::new_v4(), AccountType::Microsoft)
    };
    MinecraftUsers::set(
        &handle,
        &"microsoft:test-account".to_string(),
        Some(&online),
    )
    .await
    .unwrap();

    let selected = Credentials::get_offline_credential(&handle)
        .await
        .unwrap()
        .unwrap();

    assert!(selected.is_offline());
    assert_eq!(selected.offline_profile.id, offline.offline_profile.id);
}

#[tokio::test]
async fn persists_offline_skin_selection_per_account() {
    let handle = support::handle().await;
    let credentials = Credentials::offline("SkinTester").unwrap();
    credentials.upsert(&handle).await.unwrap();
    let minecraft_user_id = credentials.offline_profile.id;

    OfflineMinecraftSkin::set(
        minecraft_user_id,
        "local-test-texture",
        MinecraftSkinVariant::Slim,
        &handle,
    )
    .await
    .unwrap();

    let selected = OfflineMinecraftSkin::get(minecraft_user_id, &handle)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected.texture_key, "local-test-texture");
    assert_eq!(selected.variant, MinecraftSkinVariant::Slim);

    OfflineMinecraftSkin::clear_if_texture(
        minecraft_user_id,
        "local-test-texture",
        &handle,
    )
    .await
    .unwrap();
    assert!(
        OfflineMinecraftSkin::get(minecraft_user_id, &handle)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn saved_skins_keep_the_order_they_are_added_and_reordered_in() {
    let handle = support::handle().await;
    let credentials = Credentials::offline("SkinTester").unwrap();
    credentials.upsert(&handle).await.unwrap();
    let minecraft_user_id = credentials.offline_profile.id;

    for (texture_key, position) in [
        ("first", CustomMinecraftSkinInsertPosition::Bottom),
        ("second", CustomMinecraftSkinInsertPosition::Bottom),
        ("top", CustomMinecraftSkinInsertPosition::Top),
    ] {
        CustomMinecraftSkin::add(
            minecraft_user_id,
            texture_key,
            b"png",
            MinecraftSkinVariant::Classic,
            None,
            position,
            &handle,
        )
        .await
        .unwrap();
    }
    assert_eq!(
        saved_skin_keys(minecraft_user_id, &handle).await,
        vec!["top".to_string(), "first".to_string(), "second".to_string()]
    );

    CustomMinecraftSkin::set_order(
        minecraft_user_id,
        &[
            "second".to_string(),
            "missing".to_string(),
            "first".to_string(),
        ],
        &handle,
    )
    .await
    .unwrap();
    assert_eq!(
        saved_skin_keys(minecraft_user_id, &handle).await,
        vec!["second".to_string(), "first".to_string(), "top".to_string()]
    );
}

async fn saved_skin_keys(
    minecraft_user_id: Uuid,
    handle: &Handle,
) -> Vec<String> {
    CustomMinecraftSkin::get_all(minecraft_user_id, handle)
        .await
        .unwrap()
        .map(|skin| skin.texture_key)
        .collect()
        .await
}

#[tokio::test]
async fn a_saved_skin_reports_a_texture_it_cannot_read() {
    let handle = support::handle().await;
    let credentials = Credentials::offline("SkinTester").unwrap();
    credentials.upsert(&handle).await.unwrap();
    let minecraft_user_id = credentials.offline_profile.id;
    CustomMinecraftSkin::add(
        minecraft_user_id,
        "texture",
        b"png",
        MinecraftSkinVariant::Classic,
        None,
        CustomMinecraftSkinInsertPosition::Bottom,
        &handle,
    )
    .await
    .unwrap();
    let saved = CustomMinecraftSkin::get_by_texture(
        minecraft_user_id,
        "texture",
        &handle,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(saved.texture_blob(&handle).await.unwrap(), b"png".to_vec());

    CustomMinecraftSkinTextures::set(&handle, &"texture".to_string(), None)
        .await
        .unwrap();

    assert!(saved.texture_blob(&handle).await.is_err());
}

#[tokio::test]
async fn two_handles_read_the_same_accounts() {
    let pool = support::pool().await;
    let writer = handle_of(&pool);
    let reader = handle_of(&pool);
    let account_id = "microsoft:one".to_string();
    add_account(&writer, &account_id, Uuid::new_v4()).await;

    assert!(
        MinecraftUsers::get(&reader, &account_id)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn signing_in_clears_the_account_that_was_active() {
    let pool = support::pool().await;
    let handle = handle_of(&pool);

    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let mut signed_in = account_record(first, AccountType::Microsoft);
    signed_in.active = true;
    MinecraftUsers::set(&handle, &user_key(first), Some(&signed_in))
        .await
        .unwrap();
    let mut next = account_record(second, AccountType::Microsoft);
    next.active = true;
    MinecraftUsers::set(&handle, &user_key(second), Some(&next))
        .await
        .unwrap();

    assert_eq!(
        MinecraftUsers::get(&handle, &user_key(first))
            .await
            .unwrap()
            .map(|record| record.active),
        Some(false)
    );
    assert_eq!(
        MinecraftUsers::get(&handle, &user_key(second))
            .await
            .unwrap()
            .map(|record| record.active),
        Some(true)
    );
}
