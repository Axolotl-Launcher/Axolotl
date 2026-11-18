use super::*;
use crate::state::store::secrets::testing;

fn record(kind: AccountType) -> MinecraftUserRecord {
    MinecraftUserRecord {
        uuid: uuid::Uuid::new_v4().to_string(),
        active: true,
        username: "Player".into(),
        account_type: kind,
        access_token: "access-secret-value".into(),
        refresh_token: "refresh-secret-value".into(),
        expires: 0,
        yggdrasil_api_root: String::new(),
        yggdrasil_server_name: String::new(),
        yggdrasil_login: String::new(),
        yggdrasil_client_token: String::new(),
    }
}

#[tokio::test]
async fn keyring_credentials_survive_reopen_without_plaintext_documents() {
    let host = testing::with_store();
    let root = tempfile::tempdir().unwrap();
    let persistence = Persistence::new(root.path());
    let accounts = Accounts::new(&persistence);
    let record = record(AccountType::Microsoft);
    assert_eq!(
        accounts.set("microsoft:test", &record).await.unwrap(),
        CredentialPersistence::Keyring
    );
    let reopened = Accounts::new(&persistence);
    let list = reopened.get_all().await.unwrap();
    assert_eq!(list[0].1.access_token, record.access_token);
    assert_eq!(list[0].2, CredentialPersistence::Keyring);
    assert_eq!(host.store().unwrap().keys().len(), 2);
    let contents = tokio::fs::read_to_string(
        root.path().join(crate::state::layout::ACCOUNTS),
    )
    .await
    .unwrap();
    assert!(!contents.contains(&record.access_token));
    assert!(!contents.contains(&record.refresh_token));
}

#[tokio::test]
async fn rejected_keyring_stays_in_session_and_requires_login_after_restart() {
    let _host = testing::without_store();
    let root = tempfile::tempdir().unwrap();
    let persistence = Persistence::new(root.path());
    let accounts = Accounts::new(&persistence);
    let record = record(AccountType::Microsoft);
    assert_eq!(
        accounts.set("microsoft:test", &record).await.unwrap(),
        CredentialPersistence::SessionOnly
    );
    assert_eq!(
        accounts.get_all().await.unwrap()[0].1.refresh_token,
        record.refresh_token
    );
    let reopened = Accounts::new(&persistence);
    let list = reopened.get_all().await.unwrap();
    assert!(list[0].1.access_token.is_empty());
    assert!(list[0].1.refresh_token.is_empty());
    assert_eq!(list[0].2, CredentialPersistence::RequiresLogin);
    let contents = tokio::fs::read_to_string(
        root.path().join(crate::state::layout::ACCOUNTS),
    )
    .await
    .unwrap();
    assert!(!contents.contains(&record.refresh_token));
}

#[tokio::test]
async fn oversized_credentials_also_use_memory_without_partial_keyring_reuse() {
    let _host = testing::with_store();
    let accounts = Accounts::temporary();
    let mut record = record(AccountType::Microsoft);
    record.refresh_token = "x".repeat(20000);
    assert_eq!(
        accounts.set("test", &record).await.unwrap(),
        CredentialPersistence::SessionOnly
    );
    assert_eq!(
        accounts.get_all().await.unwrap()[0].1.refresh_token,
        record.refresh_token
    );
    assert!(
        accounts
            .document
            .read()
            .await
            .unwrap()
            .snapshot
            .unwrap()
            .data()
            .unwrap()["accounts"]["test"]["credential_fields"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn offline_accounts_and_active_selection_do_not_depend_on_keyring() {
    let _host = testing::without_store();
    let accounts = Accounts::temporary();
    for id in ["first", "second"] {
        assert_eq!(
            accounts
                .set(id, &record(AccountType::Offline))
                .await
                .unwrap(),
            CredentialPersistence::NotRequired
        );
    }
    accounts.select("first").await.unwrap();
    let list = accounts.get_all().await.unwrap();
    assert!(
        list.iter()
            .find(|(id, _, _)| id == "first")
            .unwrap()
            .1
            .active
    );
    assert!(
        !list
            .iter()
            .find(|(id, _, _)| id == "second")
            .unwrap()
            .1
            .active
    );
    assert!(
        list.iter()
            .all(|(_, account, _)| account.access_token == "0")
    );
}

#[tokio::test]
async fn deletion_clears_session_and_keys_and_rejects_a_late_refresh() {
    let host = testing::with_store();
    let accounts = Accounts::temporary();
    let record = record(AccountType::Microsoft);
    accounts.set("test", &record).await.unwrap();
    accounts.remove("test").await.unwrap();
    assert!(accounts.get_all().await.unwrap().is_empty());
    assert!(host.store().unwrap().keys().is_empty());
    assert!(accounts.set_existing("test", &record).await.is_err());
    assert!(accounts.get_all().await.unwrap().is_empty());
}

#[tokio::test]
async fn supported_updates_preserve_future_fields_and_unknown_account_types() {
    let _host = testing::without_store();
    let accounts = Accounts::temporary();
    accounts
        .set("known", &record(AccountType::Offline))
        .await
        .unwrap();
    accounts
        .document
        .update(|data| {
            data["accounts"]["known"]["future"] = json!({"keep":true});
            let mut future = data["accounts"]["known"].clone();
            future["account_type"] = json!("future_auth");
            data["accounts"]["future"] = future;
            Ok(())
        })
        .await
        .unwrap();
    accounts
        .set("known", &record(AccountType::Offline))
        .await
        .unwrap();
    accounts.select("known").await.unwrap();
    assert_eq!(accounts.get_all().await.unwrap().len(), 1);
    assert!(accounts.remove("future").await.is_err());
    let snapshot = accounts.document.read().await.unwrap().snapshot.unwrap();
    let data = snapshot.data().unwrap();
    assert_eq!(data["accounts"]["known"]["future"], json!({"keep":true}));
    assert_eq!(data["accounts"]["future"]["account_type"], "future_auth");
}

#[tokio::test]
async fn unavailable_keyring_propagates_without_changing_account_metadata() {
    let host = testing::with_store();
    let accounts = Accounts::temporary();
    let expected = record(AccountType::Microsoft);
    accounts.set("test", &expected).await.unwrap();
    let before = tokio::fs::read(accounts.document.path()).await.unwrap();
    host.store().unwrap().refuse_reads(true);
    assert!(accounts.get_all().await.is_err());
    assert!(accounts.tokens_for_redaction().await.is_err());
    assert_eq!(
        tokio::fs::read(accounts.document.path()).await.unwrap(),
        before
    );
    host.store().unwrap().refuse_reads(false);
    let list = accounts.get_all().await.unwrap();
    assert_eq!(list[0].1.access_token, expected.access_token);
    assert_eq!(list[0].1.refresh_token, expected.refresh_token);
    assert_eq!(list[0].2, CredentialPersistence::Keyring);
}

#[tokio::test]
async fn background_refresh_does_not_change_the_selected_account() {
    let _host = testing::without_store();
    let accounts = Accounts::temporary();
    let first = record(AccountType::Microsoft);
    accounts.set("first", &first).await.unwrap();
    accounts
        .set("second", &record(AccountType::Offline))
        .await
        .unwrap();
    accounts.set_existing("first", &first).await.unwrap();
    let list = accounts.get_all().await.unwrap();
    assert!(
        list.iter()
            .find(|(id, _, _)| id == "second")
            .unwrap()
            .1
            .active
    );
    assert!(
        !list
            .iter()
            .find(|(id, _, _)| id == "first")
            .unwrap()
            .1
            .active
    );
}

#[tokio::test]
async fn device_credentials_follow_the_same_keyring_or_session_policy() {
    for persistent in [false, true] {
        let _host = if persistent {
            testing::with_store()
        } else {
            testing::without_store()
        };
        let root = tempfile::tempdir().unwrap();
        let persistence = Persistence::new(root.path());
        let accounts = Accounts::new(&persistence);
        let record = crate::state::store::minecraft_device_tokens::MinecraftDeviceTokenRecord {
            uuid: uuid::Uuid::new_v4().to_string(), private_key:"private-device-key".into(),
            x:"x".into(),y:"y".into(),issue_instant:0,not_after:1,token:"device-secret".into(),display_claims:"{}".into(),
        };
        accounts.set_device_token(&record).await.unwrap();
        assert_eq!(
            accounts.device_token().await.unwrap(),
            Some(record.clone())
        );
        let reopened = Accounts::new(&persistence);
        assert_eq!(
            reopened.device_token().await.unwrap().is_some(),
            persistent
        );
        let bytes = tokio::fs::read_to_string(
            root.path().join(crate::state::layout::ACCOUNTS),
        )
        .await
        .unwrap();
        assert!(!bytes.contains("private-device-key"));
        assert!(!bytes.contains("device-secret"));
    }
}
