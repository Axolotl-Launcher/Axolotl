use std::cell::RefCell;
use std::collections::HashMap;

use super::*;

#[derive(Default)]
struct MemoryStore {
    values: RefCell<HashMap<String, String>>,
    fail_writes: bool,
}

impl Store for MemoryStore {
    fn read(&self, name: &str) -> crate::Result<Option<String>> {
        Ok(self.values.borrow().get(name).cloned())
    }

    fn write(&self, name: &str, value: Option<&str>) -> crate::Result<()> {
        if self.fail_writes {
            return Err(storage_error());
        }
        match value {
            Some(value) => {
                self.values.borrow_mut().insert(name.into(), value.into());
            }
            None => {
                self.values.borrow_mut().remove(name);
            }
        }
        Ok(())
    }
}

fn pending() -> Session {
    Session {
        pending: Some(PendingFlow {
            state: "expected-state".into(),
            deadline: 400,
            exchanging: false,
        }),
        ..Default::default()
    }
}

fn connection(token: &str) -> Connection {
    connection_from_reply(serde_json::from_value(serde_json::json!({
        "code": 200, "access_token": token, "token_type": "Bearer", "expires_in": 259200,
        "user": { "userId": 1, "userName": "shimu", "nickName": "拾木", "balance": 3.5 }
    })).unwrap(), 100).unwrap()
}

fn callback(query: &str) -> Url {
    callback_url(&format!("{REDIRECT_URI}?{query}")).unwrap()
}

#[test]
fn authorization_uses_fixed_registration_and_minimal_scope() {
    let url = Url::parse(&authorization_url("random-state")).unwrap();
    assert_eq!(url.origin().ascii_serialization(), HOST);
    assert_eq!(url.path(), "/oauth/authorize");
    let query: HashMap<_, _> = url.query_pairs().collect();
    assert_eq!(query["client_id"], CLIENT_ID);
    assert_eq!(query["redirect_uri"], REDIRECT_URI);
    assert_eq!(query["scope"], "user:info,post:read,file:download");
    assert_eq!(query["state"], "random-state");
    assert!(!query.contains_key("client_secret"));
}

#[test]
fn credential_adapter_reuses_the_shared_store_and_provider_namespace() {
    let host = secrets::testing::with_store();
    let store = CredentialStore;
    let serialized =
        serde_json::to_string(&connection(&"a".repeat(7000))).unwrap();
    oauth_credentials::write(&store, Some(&serialized)).unwrap();
    assert_eq!(
        oauth_credentials::read(&store).unwrap().unwrap(),
        serialized
    );
    let keys = host.store().unwrap().keys();
    assert!(keys.len() > 1);
    assert!(
        keys.iter()
            .all(|key| key
                .starts_with("axolotl-account:third-party:shimu:oauth"))
    );
    oauth_credentials::write(&store, None).unwrap();
    assert!(host.store().unwrap().keys().is_empty());
}

#[test]
fn unavailable_or_locked_stores_cannot_report_successful_disconnect() {
    {
        let _host = secrets::testing::without_store();
        assert!(CredentialStore.read("oauth").is_err());
        assert!(CredentialStore.write("oauth", Some("token")).is_err());
        assert!(oauth_credentials::write(&CredentialStore, None).is_err());
    }
    let host = secrets::testing::with_store();
    CredentialStore.write("oauth", Some("token")).unwrap();
    host.store().unwrap().refuse_deletes();
    assert!(oauth_credentials::write(&CredentialStore, None).is_err());
    assert_eq!(
        CredentialStore.read("oauth").unwrap().as_deref(),
        Some("token")
    );
}

#[test]
fn only_the_registered_callback_shape_is_accepted() {
    for url in [
        "https://oauth/callback?code=secret",
        "axolotl://oauth/other?code=secret",
        "axolotl://oauth/callback/extra?code=secret",
        "axolotl://user@oauth/callback?code=secret",
        "axolotl://oauth:1234/callback?code=secret",
        "axolotl://oauth/callback?code=secret#fragment",
        "axolotl://oauth.evil/callback?code=secret",
    ] {
        assert!(callback_url(url).is_none(), "{url}");
    }
    assert!(callback_url(REDIRECT_URI).is_some());
}

#[test]
fn missing_mismatched_and_duplicate_state_never_consume_a_flow() {
    for query in [
        "code=secret",
        "state=wrong&code=secret",
        "state=expected-state&state=expected-state&code=secret",
        "state=expected-state&code=first&code=second",
        "state=expected-state&code=secret&error=access_denied",
        "state=expected-state&code=",
    ] {
        let mut session = pending();
        assert!(session.claim(&callback(query), 100).is_none());
        assert!(!session.pending.unwrap().exchanging);
    }
}

#[test]
fn callbacks_are_single_use_and_late_or_unsolicited_callbacks_are_ignored() {
    let url = callback("state=expected-state&code=secret");
    let mut session = pending();
    assert_eq!(session.claim(&url, 100).as_deref(), Some("secret"));
    assert!(session.claim(&url, 100).is_none());
    let mut session = pending();
    assert!(session.claim(&url, 400).is_none());
    assert_eq!(session.outcome, Some(AuthorizationStatus::TimedOut));
    assert!(Session::default().claim(&url, 100).is_none());
}

#[test]
fn denial_is_terminal_and_does_not_exchange_a_code() {
    let mut session = pending();
    assert!(
        session
            .claim(&callback("state=expected-state&error=access_denied"), 100)
            .is_none()
    );
    assert!(session.pending.is_none());
    assert_eq!(session.outcome, Some(AuthorizationStatus::Denied));
}

#[test]
fn cancelled_or_replaced_flows_cannot_restore_credentials() {
    let store = MemoryStore::default();
    let mut session = pending();
    session.claim(&callback("state=expected-state&code=secret"), 100);
    session.cancel("expected-state");
    session.complete(
        "expected-state",
        Ok(connection("secret-token")),
        &store,
        100,
    );
    assert!(read_connection(&store).unwrap().is_none());
    session = pending();
    session.pending.as_mut().unwrap().state = "new-state".into();
    session.complete(
        "expected-state",
        Ok(connection("secret-token")),
        &store,
        100,
    );
    assert!(read_connection(&store).unwrap().is_none());
}

#[test]
fn stale_cancellation_does_not_cancel_a_new_flow() {
    let mut session = pending();
    session.cancel("previous-state");
    assert_eq!(session.pending.as_ref().unwrap().state, "expected-state");
    session.cancel("expected-state");
    assert!(session.pending.is_none());
}

#[test]
fn credentials_are_committed_before_success_and_storage_failure_preserves_old_connection()
 {
    let mut session = pending();
    session.claim(&callback("state=expected-state&code=secret"), 100);
    let mut store = MemoryStore::default();
    session.complete(
        "expected-state",
        Ok(connection("old-token")),
        &store,
        100,
    );
    assert!(session.pending.is_none());
    assert_eq!(session.outcome, Some(AuthorizationStatus::Succeeded));
    let account = summary(read_connection(&store).unwrap(), &session, 100);
    assert_eq!(account.authorization, Some(AuthorizationStatus::Succeeded));
    assert_eq!(account.flow_id.as_deref(), Some("expected-state"));
    assert_eq!(
        read_connection(&store).unwrap().unwrap().access_token,
        "old-token"
    );
    store.fail_writes = true;
    session = pending();
    session.claim(&callback("state=expected-state&code=secret"), 100);
    session.complete(
        "expected-state",
        Ok(connection("new-token")),
        &store,
        100,
    );
    assert_eq!(session.outcome, Some(AuthorizationStatus::Failed));
    assert_eq!(
        read_connection(&store).unwrap().unwrap().access_token,
        "old-token"
    );
}

#[test]
fn expiry_preserves_identity_and_ipc_summary_never_contains_credentials() {
    let result = summary(
        Some(connection("secret-token")),
        &Session::default(),
        259300,
    );
    assert_eq!(result.status, AccountStatus::Expired);
    assert_eq!(result.user.as_ref().unwrap().user_name, "shimu");
    let value = serde_json::to_string(&result).unwrap();
    assert!(!value.contains("secret-token"));
    assert!(!value.contains("access_token"));
    assert!(!value.contains("balance"));
}

#[test]
fn malformed_exchange_responses_never_become_connected_accounts() {
    for (token, token_type, ttl, status) in [
        ("", "Bearer", 100, 200),
        ("token", "Basic", 100, 200),
        ("token", "Bearer", 0, 200),
        ("token", "Bearer", 100, 401),
    ] {
        let value = TokenReply {
            code: status,
            access_token: token.into(),
            token_type: token_type.into(),
            expires_in: ttl,
            user: AccountProfile {
                user_id: 1,
                user_name: "user".into(),
                nick_name: None,
            },
        };
        assert!(connection_from_reply(value, 100).is_err());
    }
}
