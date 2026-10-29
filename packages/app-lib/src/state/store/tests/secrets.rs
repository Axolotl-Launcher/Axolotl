//! What the account secrets promise: which columns a row keeps once the system
//! credential store takes the secret, and what stays in the row when it does not.

use sqlx::SqlitePool;

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::minecraft_device_tokens::{
    MinecraftDeviceTokenRecord, MinecraftDeviceTokens, Singleton,
};
use crate::state::store::minecraft_users::{
    AccountType, MinecraftUserRecord, MinecraftUsers,
};
use crate::state::store::modrinth_users::{ModrinthUserRecord, ModrinthUsers};
use crate::state::store::secrets;

fn account(access_token: &str, refresh_token: &str) -> MinecraftUserRecord {
    MinecraftUserRecord {
        uuid: "uuid".to_string(),
        active: false,
        username: "Player".to_string(),
        account_type: AccountType::Microsoft,
        access_token: access_token.to_string(),
        refresh_token: refresh_token.to_string(),
        expires: 0,
        yggdrasil_api_root: String::new(),
        yggdrasil_server_name: String::new(),
        yggdrasil_login: String::new(),
        yggdrasil_client_token: String::new(),
    }
}

fn session() -> ModrinthUserRecord {
    ModrinthUserRecord {
        active: true,
        session_id: "session".to_string(),
        expires: 10,
    }
}

fn device_token() -> MinecraftDeviceTokenRecord {
    MinecraftDeviceTokenRecord {
        uuid: "device".to_string(),
        private_key: "private-key".to_string(),
        x: "1".to_string(),
        y: "2".to_string(),
        issue_instant: 10,
        not_after: 20,
        token: "token".to_string(),
        display_claims: r#"{"xui":[{"uhs":"1"}]}"#.to_string(),
    }
}

fn account_tokens_key(account_id: &str, column: &str) -> String {
    secrets::key(&["minecraft-user", account_id, column])
}

/// The account's own columns, as the database holds them.
async fn account_row(
    pool: &SqlitePool,
    account_id: &str,
) -> (String, String, i64) {
    sqlx::query_as(
        "SELECT access_token, refresh_token, secrets_externalized
         FROM minecraft_users
         WHERE account_id = ?",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn session_row(pool: &SqlitePool, id: &str) -> (String, i64) {
    sqlx::query_as(
        "SELECT session_id, secrets_externalized FROM modrinth_users WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn device_row(pool: &SqlitePool) -> (String, String, i64) {
    sqlx::query_as(
        "SELECT token, json(display_claims), secrets_externalized
         FROM minecraft_device_tokens
         WHERE id = 0",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn without_a_store_the_row_keeps_its_tokens() {
    let _host = secrets::testing::without_store();
    let pool = support::pool().await;
    let handle = Handle::new(pool.clone());
    let account_id = "microsoft:no-store".to_string();

    MinecraftUsers::set(
        &handle,
        &account_id,
        Some(&account("access", "refresh")),
    )
    .await
    .unwrap();

    let (access_token, refresh_token, marks) =
        account_row(&pool, &account_id).await;
    assert_eq!(
        (access_token.as_str(), refresh_token.as_str()),
        ("access", "refresh")
    );
    assert_eq!(marks, 0, "a row nothing moved out of carries no mark");

    let read = MinecraftUsers::get(&handle, &account_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(read.access_token, "access");
    assert_eq!(read.refresh_token, "refresh");
}

#[tokio::test]
async fn a_store_holds_the_tokens_and_the_row_is_cleared() {
    let host = secrets::testing::with_store();
    let store = host.store().unwrap();
    let pool = support::pool().await;
    let handle = Handle::new(pool.clone());
    let account_id = "microsoft:one".to_string();

    MinecraftUsers::set(
        &handle,
        &account_id,
        Some(&account("access", "refresh")),
    )
    .await
    .unwrap();
    ModrinthUsers::set(&handle, &"account".to_string(), Some(&session()))
        .await
        .unwrap();
    MinecraftDeviceTokens::set(&handle, &Singleton, Some(&device_token()))
        .await
        .unwrap();

    let (access_token, refresh_token, marks) =
        account_row(&pool, &account_id).await;
    assert_eq!(
        access_token, "",
        "the token column no longer holds the token"
    );
    assert_eq!(refresh_token, "");
    assert_eq!(marks, 0b011, "the two tokens the account has were moved");

    let (session_id, session_marks) = session_row(&pool, "account").await;
    assert_eq!(session_id, "");
    assert_eq!(session_marks, 1);

    let (token, claims, device_marks) = device_row(&pool).await;
    assert_eq!(token, "");
    assert_eq!(claims, "{}");
    assert_eq!(device_marks, 1);

    assert_eq!(
        store
            .value(&account_tokens_key(&account_id, "access_token"))
            .as_deref(),
        Some("access")
    );
    assert_eq!(
        store
            .value(&secrets::key(&["modrinth-user", "account", "session_id"]))
            .as_deref(),
        Some("session")
    );

    let stored = store
        .value(&secrets::key(&["minecraft-device-token"]))
        .expect("the device token is in the store");
    assert_eq!(
        serde_json::from_str::<MinecraftDeviceTokenRecord>(&stored).unwrap(),
        device_token(),
        "the entry holds the record the row no longer carries"
    );
}

#[tokio::test]
async fn a_read_comes_from_the_store_rather_than_the_row() {
    let host = secrets::testing::with_store();
    let store = host.store().unwrap();
    let pool = support::pool().await;
    let handle = Handle::new(pool.clone());
    let account_id = "microsoft:moved".to_string();

    MinecraftUsers::set(
        &handle,
        &account_id,
        Some(&account("access", "refresh")),
    )
    .await
    .unwrap();
    // The row is empty now, so a read that answers with this is reading the
    // store and nothing else.
    store.put(&account_tokens_key(&account_id, "access_token"), "renewed");

    let read = MinecraftUsers::get(&handle, &account_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(read.access_token, "renewed");
    assert_eq!(read.refresh_token, "refresh");
}

#[tokio::test]
async fn a_secret_longer_than_one_entry_stays_in_the_row() {
    let host = secrets::testing::with_store();
    let store = host.store().unwrap();
    let pool = support::pool().await;
    let handle = Handle::new(pool.clone());
    let account_id = "microsoft:long".to_string();
    // Longer than any credential store entry holds, which is what makes a store
    // shorten or refuse it.
    let access_token = "a".repeat(16384);

    MinecraftUsers::set(
        &handle,
        &account_id,
        Some(&account(&access_token, "refresh")),
    )
    .await
    .unwrap();

    let (stored_access, stored_refresh, marks) =
        account_row(&pool, &account_id).await;
    assert_eq!(stored_access, access_token, "the token was not lost");
    assert_eq!(stored_refresh, "refresh");
    assert_eq!(marks, 0, "a row the store refused is marked as unmoved");
    assert!(
        !store.keys().iter().any(|key| key.contains(&account_id)),
        "and the store keeps none of its tokens"
    );

    let read = MinecraftUsers::get(&handle, &account_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(read.access_token, access_token);
}

#[tokio::test]
async fn moving_the_columns_runs_once_and_leaves_the_rows_as_they_are() {
    let pool = support::pool().await;
    let handle = Handle::new(pool.clone());
    let account_id = "microsoft:secrets-legacy".to_string();
    let session_id = "secrets-session".to_string();

    // An older build wrote these rows with their secrets in place.
    {
        let _host = secrets::testing::without_store();
        MinecraftUsers::set(
            &handle,
            &account_id,
            Some(&account("access", "refresh")),
        )
        .await
        .unwrap();
        ModrinthUsers::set(&handle, &session_id, Some(&session()))
            .await
            .unwrap();
        MinecraftDeviceTokens::set(&handle, &Singleton, Some(&device_token()))
            .await
            .unwrap();
    }

    let host = secrets::testing::with_store();
    let store = host.store().unwrap();
    secrets::externalize(&handle).await.unwrap();

    let (access_token, _, marks) = account_row(&pool, &account_id).await;
    assert_eq!(access_token, "");
    assert_eq!(marks, 0b011);
    assert_eq!(session_row(&pool, &session_id).await, (String::new(), 1));
    let (_, _, device_marks) = device_row(&pool).await;
    assert_eq!(device_marks, 1);
    let written = store.keys().len();
    assert_eq!(
        written, 4,
        "one entry per secret: two tokens, one session and the device token"
    );

    // A second pass finds nothing to move, so it writes nothing at all.
    secrets::externalize(&handle).await.unwrap();

    assert_eq!(store.keys().len(), written, "no second entry was written");
    let (_, _, marks) = account_row(&pool, &account_id).await;
    assert_eq!(marks, 0b011);
    assert_eq!(session_row(&pool, &session_id).await, (String::new(), 1));
    let (_, _, device_marks) = device_row(&pool).await;
    assert_eq!(device_marks, 1);
}

/// A store that stops answering leaves a row that says the store holds its secret
/// unreadable rather than reading as an account that lost its tokens: an emptied
/// secret is a secret the crash report would export in the clear.
#[tokio::test]
async fn a_store_that_does_not_answer_fails_the_read() {
    let pool = support::pool().await;
    let handle = Handle::new(pool.clone());
    let account_id = "microsoft:locked".to_string();
    {
        let _host = secrets::testing::with_store();
        MinecraftUsers::set(
            &handle,
            &account_id,
            Some(&account("access", "refresh")),
        )
        .await
        .unwrap();
        ModrinthUsers::set(&handle, &"account".to_string(), Some(&session()))
            .await
            .unwrap();
        MinecraftDeviceTokens::set(&handle, &Singleton, Some(&device_token()))
            .await
            .unwrap();
    }
    // The rows still say the store holds their secrets, and it no longer answers.
    let _locked = secrets::testing::without_store();

    assert!(
        MinecraftUsers::get(&handle, &account_id).await.is_err(),
        "the account is not read as signed out"
    );
    assert!(MinecraftUsers::get_all(&handle).await.is_err());
    assert!(
        crate::state::store::minecraft_users::token_pairs(&handle)
            .await
            .is_err(),
        "the last read before a report is exported fails too"
    );
    assert!(
        ModrinthUsers::get(&handle, &"account".to_string())
            .await
            .is_err()
    );
    assert!(
        MinecraftDeviceTokens::get(&handle, &Singleton)
            .await
            .is_err()
    );
}

/// A write the row refuses leaves the store as it was: the row and the store
/// otherwise disagree about which secret is the account's.
#[tokio::test]
async fn a_row_that_cannot_be_written_puts_the_tokens_back() {
    let host = secrets::testing::with_store();
    let store = host.store().unwrap();
    let pool = support::pool().await;
    let handle = Handle::new(pool.clone());
    let account_id = "microsoft:restore".to_string();

    MinecraftUsers::set(
        &handle,
        &account_id,
        Some(&account("access", "refresh")),
    )
    .await
    .unwrap();
    assert_eq!(
        store
            .value(&account_tokens_key(&account_id, "access_token"))
            .as_deref(),
        Some("access")
    );

    // The row cannot be written any more, so nothing can hold the new tokens.
    sqlx::query("ALTER TABLE minecraft_users RENAME TO minecraft_users_stuck")
        .execute(&pool)
        .await
        .unwrap();

    let written = MinecraftUsers::set(
        &handle,
        &account_id,
        Some(&account("renewed", "renewed-refresh")),
    )
    .await;

    assert!(written.is_err(), "the write is not reported as done");
    assert_eq!(
        store
            .value(&account_tokens_key(&account_id, "access_token"))
            .as_deref(),
        Some("access"),
        "the token the row still names is the one the store holds"
    );
    assert_eq!(
        store
            .value(&account_tokens_key(&account_id, "refresh_token"))
            .as_deref(),
        Some("refresh")
    );
}

/// Deleting an account the store will not let go of leaves the row in place: the
/// secret would otherwise stay behind with nothing left to name it.
#[tokio::test]
async fn an_account_the_store_will_not_let_go_of_keeps_its_row() {
    let host = secrets::testing::with_store();
    let store = host.store().unwrap();
    let pool = support::pool().await;
    let handle = Handle::new(pool.clone());
    let account_id = "microsoft:stuck".to_string();

    MinecraftUsers::set(
        &handle,
        &account_id,
        Some(&account("access", "refresh")),
    )
    .await
    .unwrap();
    store.refuse_deletes();

    assert!(
        MinecraftUsers::set(&handle, &account_id, None)
            .await
            .is_err(),
        "the removal can be retried instead of half done"
    );
    // `account_row` reads the row itself: the account is still there.
    let (_, _, marks) = account_row(&pool, &account_id).await;
    assert_eq!(marks, 0b011, "and it still names both tokens");
}

/// The same promise holds for the session a signed-out account stops naming.
#[tokio::test]
async fn a_session_the_store_will_not_let_go_of_keeps_its_row() {
    let host = secrets::testing::with_store();
    let store = host.store().unwrap();
    let pool = support::pool().await;
    let handle = Handle::new(pool.clone());

    ModrinthUsers::set(&handle, &"account".to_string(), Some(&session()))
        .await
        .unwrap();
    store.refuse_deletes();

    assert!(
        ModrinthUsers::set(&handle, &"account".to_string(), None)
            .await
            .is_err()
    );
    assert_eq!(
        session_row(&pool, "account").await,
        (String::new(), 1),
        "the row is still there, still naming the entry"
    );
}

/// A token the account loses is a token the store no longer holds, and an entry
/// the store would not drop does not block the row that keeps the others.
#[tokio::test]
async fn a_token_the_account_no_longer_has_leaves_nothing_behind() {
    let host = secrets::testing::with_store();
    let store = host.store().unwrap();
    let pool = support::pool().await;
    let handle = Handle::new(pool.clone());
    let account_id = "microsoft:dropped".to_string();
    let mut first = account("access", "refresh");
    first.yggdrasil_client_token = "client".to_string();

    MinecraftUsers::set(&handle, &account_id, Some(&first))
        .await
        .unwrap();
    assert_eq!(
        store
            .value(&account_tokens_key(&account_id, "yggdrasil_client_token"))
            .as_deref(),
        Some("client")
    );

    let second = account("access", "refresh");
    MinecraftUsers::set(&handle, &account_id, Some(&second))
        .await
        .unwrap();

    assert_eq!(
        store.value(&account_tokens_key(&account_id, "yggdrasil_client_token")),
        None,
        "the entry of a token the account no longer has is gone"
    );
    assert_eq!(
        store
            .value(&account_tokens_key(&account_id, "access_token"))
            .as_deref(),
        Some("access")
    );
}

/// A token the store will not take leaves the row as an older build wrote it, so
/// the entries the store still holds for it are secrets nothing names.
#[tokio::test]
async fn a_refused_token_leaves_no_entries_behind() {
    let host = secrets::testing::with_store();
    let store = host.store().unwrap();
    let pool = support::pool().await;
    let handle = Handle::new(pool.clone());
    let account_id = "microsoft:refused".to_string();

    MinecraftUsers::set(
        &handle,
        &account_id,
        Some(&account("access", "refresh")),
    )
    .await
    .unwrap();

    let long = "a".repeat(16384);
    MinecraftUsers::set(&handle, &account_id, Some(&account(&long, "refresh")))
        .await
        .unwrap();

    for column in ["access_token", "refresh_token", "yggdrasil_client_token"] {
        assert_eq!(
            store.value(&account_tokens_key(&account_id, column)),
            None,
            "no entry stays for a token the row holds"
        );
    }
    let (access_token, refresh_token, marks) =
        account_row(&pool, &account_id).await;
    assert_eq!(access_token, long);
    assert_eq!(refresh_token, "refresh");
    assert_eq!(marks, 0);
}

/// A token the store would not drop does not keep the row from being written.
#[tokio::test]
async fn a_token_the_store_will_not_drop_does_not_block_the_write() {
    let host = secrets::testing::with_store();
    let store = host.store().unwrap();
    let pool = support::pool().await;
    let handle = Handle::new(pool.clone());
    let account_id = "microsoft:kept".to_string();
    let mut first = account("access", "refresh");
    first.yggdrasil_client_token = "client".to_string();
    MinecraftUsers::set(&handle, &account_id, Some(&first))
        .await
        .unwrap();
    store.refuse_deletes();

    let written = MinecraftUsers::set(
        &handle,
        &account_id,
        Some(&account("access", "refresh")),
    )
    .await;

    assert!(written.is_ok(), "the account is written anyway");
    let (_, _, marks) = account_row(&pool, &account_id).await;
    assert_eq!(marks, 0b011, "the two tokens it has are in the store");
}

/// A column an older build wrote after the row was marked is the newer value, so
/// it is what the account reads as; reading it moves it into the store.
#[tokio::test]
async fn a_column_an_older_build_wrote_wins_over_the_store() {
    let host = secrets::testing::with_store();
    let store = host.store().unwrap();
    let pool = support::pool().await;
    let handle = Handle::new(pool.clone());
    let account_id = "microsoft:mixed".to_string();

    MinecraftUsers::set(
        &handle,
        &account_id,
        Some(&account("access", "refresh")),
    )
    .await
    .unwrap();
    store.put(&account_tokens_key(&account_id, "access_token"), "stale");
    // A build that did not know about the store wrote this row last: its column
    // carries the token, and the mark still says the store holds one.
    sqlx::query(
        "UPDATE minecraft_users SET access_token = 'newest'
         WHERE account_id = ?",
    )
    .bind(&account_id)
    .execute(&pool)
    .await
    .unwrap();

    let read = MinecraftUsers::get(&handle, &account_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(read.access_token, "newest", "the column is the newer value");

    let (access_token, _, marks) = account_row(&pool, &account_id).await;
    assert_eq!(access_token, "", "the token was moved into the store");
    assert_eq!(marks, 0b011);
    assert_eq!(
        store
            .value(&account_tokens_key(&account_id, "access_token"))
            .as_deref(),
        Some("newest")
    );
}

/// The same column, with a token the store will not take: the row keeps it and
/// stops saying the store holds it.
#[tokio::test]
async fn a_column_the_store_will_not_take_is_kept_and_unmarked() {
    let _host = secrets::testing::with_store();
    let pool = support::pool().await;
    let handle = Handle::new(pool.clone());
    let account_id = "microsoft:mixed-long".to_string();

    MinecraftUsers::set(
        &handle,
        &account_id,
        Some(&account("access", "refresh")),
    )
    .await
    .unwrap();
    let long = "a".repeat(16384);
    sqlx::query(
        "UPDATE minecraft_users SET access_token = ? WHERE account_id = ?",
    )
    .bind(&long)
    .bind(&account_id)
    .execute(&pool)
    .await
    .unwrap();

    let read = MinecraftUsers::get(&handle, &account_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(read.access_token, long);

    let (access_token, _, marks) = account_row(&pool, &account_id).await;
    assert_eq!(access_token, long, "the column keeps the token");
    assert_eq!(marks, 0b010, "and the row no longer names it");
}
