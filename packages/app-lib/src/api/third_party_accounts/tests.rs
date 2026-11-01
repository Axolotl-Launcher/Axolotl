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
            code_verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into(),
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
    let verifier = new_code_verifier().unwrap();
    let url =
        Url::parse(&authorization_url("random-state", &verifier)).unwrap();
    assert_eq!(url.origin().ascii_serialization(), HOST);
    assert_eq!(url.path(), "/oauth/authorize");
    let query: HashMap<_, _> = url.query_pairs().collect();
    assert_eq!(query["client_id"], CLIENT_ID);
    assert_eq!(query["redirect_uri"], REDIRECT_URI);
    assert_eq!(query["scope"], "user:info,post:read,file:download");
    assert_eq!(query["state"], "random-state");
    assert_eq!(query["response_type"], "code");
    assert_eq!(query["code_challenge_method"], "S256");
    assert_eq!(query["code_challenge"], code_challenge(&verifier));
    assert!(!query.contains_key("client_secret"));
    assert!(!query.contains_key("code_verifier"));
    let start = AuthorizationStart {
        flow_id: "random-state".into(),
        authorization_url: url.into(),
    };
    let ipc = serde_json::to_string(&start).unwrap();
    assert!(!ipc.contains(&verifier));
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
        assert!(!is_callback(url), "{url}");
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

#[test]
fn pending_authorization_survives_restart_but_a_claim_is_never_replayed() {
    let store = MemoryStore::default();
    let original = pending();
    original.persist_pending(&store).unwrap();
    let mut restarted = Session::restore(&store, 100).unwrap();
    assert_eq!(
        restarted.pending.as_ref().unwrap().code_verifier,
        original.pending.as_ref().unwrap().code_verifier,
    );
    let url = callback("state=expected-state&code=secret");
    assert_eq!(
        restarted
            .update(&store, |session| session.claim(&url, 100))
            .unwrap()
            .as_deref(),
        Some("secret"),
    );
    let mut interrupted_exchange = Session::restore(&store, 101).unwrap();
    assert!(interrupted_exchange.pending.is_none());
    assert_eq!(
        interrupted_exchange.outcome,
        Some(AuthorizationStatus::Failed)
    );
    assert!(interrupted_exchange.claim(&url, 101).is_none());
    assert!(store.read(FLOW_CREDENTIAL).unwrap().is_none());
}

#[test]
fn expired_and_cancelled_pending_flows_are_removed_from_credential_store() {
    let store = MemoryStore::default();
    pending().persist_pending(&store).unwrap();
    let expired = Session::restore(&store, 400).unwrap();
    assert!(expired.pending.is_none());
    assert_eq!(expired.outcome, Some(AuthorizationStatus::TimedOut));
    assert!(store.read(FLOW_CREDENTIAL).unwrap().is_none());
    let mut original = pending();
    original.persist_pending(&store).unwrap();
    original
        .update(&store, |session| session.cancel("expected-state"))
        .unwrap();
    assert!(Session::restore(&store, 100).unwrap().pending.is_none());
}

#[test]
fn a_claim_is_not_exchanged_if_it_cannot_be_persisted() {
    let mut session = pending();
    let store = MemoryStore {
        fail_writes: true,
        ..Default::default()
    };
    assert!(
        session
            .update(&store, |session| {
                session
                    .claim(&callback("state=expected-state&code=secret"), 100)
            })
            .is_err()
    );
    assert!(!session.pending.unwrap().exchanging);
}

#[test]
fn successful_checks_cannot_restore_expired_or_replaced_credentials() {
    let store = MemoryStore::default();
    let checked = connection("token");
    let mut updated = checked.clone();
    updated.user.nick_name = Some("Updated identity".into());
    let mut expired = checked.clone();
    expired.expires_at = 0;
    for current in [expired, connection("replacement")] {
        oauth_credentials::write(
            &store,
            Some(&serde_json::to_string(&current).unwrap()),
        )
        .unwrap();
        write_checked_connection(&store, &checked, &updated).unwrap();
        assert!(read_connection(&store).unwrap().as_ref() == Some(&current));
    }
    oauth_credentials::write(
        &store,
        Some(&serde_json::to_string(&checked).unwrap()),
    )
    .unwrap();
    write_checked_connection(&store, &checked, &updated).unwrap();
    assert!(read_connection(&store).unwrap().as_ref() == Some(&updated));
}

#[test]
fn invalid_callback_candidates_are_redacted_but_not_intercepted() {
    for url in [
        "axolotl://oauth/other?code=secret",
        "axolotl://oauth/callback?code=secret#fragment",
        "axolotl://user@oauth/callback?code=secret",
        "axolotl://oauth:1234/callback?code=secret",
    ] {
        assert!(is_callback_candidate(url));
        assert!(!is_callback(url));
    }
    assert!(is_callback(REDIRECT_URI));
    assert!(!is_callback_candidate("axolotl://settings"));
}

#[test]
fn s256_challenge_matches_rfc_7636_appendix_b() {
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    assert_eq!(
        code_challenge(verifier),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
}

#[test]
fn each_authorization_uses_a_fresh_os_random_pkce_verifier() {
    let mut seen = std::collections::HashSet::new();
    for _ in 0..32 {
        let verifier = new_code_verifier().unwrap();
        assert_eq!(verifier.len(), 43);
        assert!(valid_code_verifier(&verifier));
        assert_eq!(URL_SAFE_NO_PAD.decode(&verifier).unwrap().len(), 32);
        assert!(seen.insert(verifier));
    }
}

#[test]
fn exchange_uses_the_flow_verifier_without_a_shared_secret() {
    let verifier = new_code_verifier().unwrap();
    let form: HashMap<_, _> = token_form("authorization-code", &verifier)
        .unwrap()
        .into_iter()
        .collect();
    assert_eq!(form["code"], "authorization-code");
    assert_eq!(form["client_id"], CLIENT_ID);
    assert_eq!(form["redirect_uri"], REDIRECT_URI);
    assert_eq!(form["code_verifier"], verifier);
    assert!(!form.contains_key("client_secret"));
    assert!(!form.contains_key("code_challenge"));
    for invalid in ["", "short", &"a".repeat(129), &"!".repeat(43)] {
        assert!(token_form("code", invalid).is_err());
    }
}

#[test]
fn pre_pkce_pending_flows_are_discarded_without_losing_the_connection() {
    let store = MemoryStore::default();
    let saved = connection("existing-user-token");
    oauth_credentials::write(
        &store,
        Some(&serde_json::to_string(&saved).unwrap()),
    )
    .unwrap();
    store
        .write(
            FLOW_CREDENTIAL,
            Some(r#"{"state":"old-state","deadline":400,"exchanging":false}"#),
        )
        .unwrap();
    let mut restarted = Session::restore(&store, 100).unwrap();
    assert!(restarted.pending.is_none());
    assert_eq!(restarted.outcome, Some(AuthorizationStatus::Failed));
    assert!(store.read(FLOW_CREDENTIAL).unwrap().is_none());
    assert!(
        restarted
            .claim(&callback("state=old-state&code=secret"), 100)
            .is_none()
    );
    assert!(read_connection(&store).unwrap().as_ref() == Some(&saved));
    assert!(
        summary(read_connection(&store).unwrap(), &restarted, 100).available
    );
    // Discarding an incompatible flow leaves the session ready for a fresh login.
    assert!(
        restarted
            .update(&store, |session| session.pending = pending().pending)
            .is_ok()
    );
}

#[test]
fn pkce_proof_uses_the_shared_credential_manager_and_is_absent_from_summary() {
    let host = secrets::testing::with_store();
    let session = pending();
    session.persist_pending(&CredentialStore).unwrap();
    let serialized = CredentialStore.read(FLOW_CREDENTIAL).unwrap().unwrap();
    assert!(serialized.encode_utf16().count() <= 1280);
    assert_eq!(
        host.store().unwrap().keys(),
        vec![secrets::key(&["third-party", "shimu", FLOW_CREDENTIAL])]
    );
    let restored = Session::restore(&CredentialStore, 100).unwrap();
    assert_eq!(
        restored.pending.as_ref().unwrap().code_verifier,
        session.pending.as_ref().unwrap().code_verifier,
    );
    let ipc = serde_json::to_string(&summary(None, &restored, 100)).unwrap();
    assert!(!ipc.contains("code_verifier"));
    assert!(!ipc.contains(&restored.pending.as_ref().unwrap().code_verifier));
    assert!(restored.pending.is_some());
}
