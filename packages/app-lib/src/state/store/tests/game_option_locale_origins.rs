//! What the locale origins collection promises.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::game_option_locale_origins::{
    GameOptionLocaleOrigins, LocaleOriginKey, LocaleOriginRecord,
};

fn key(scope: &str, option_id: &str) -> LocaleOriginKey {
    LocaleOriginKey {
        scope: scope.to_string(),
        option_id: option_id.to_string(),
    }
}

fn observed() -> LocaleOriginRecord {
    LocaleOriginRecord {
        source_instance_id: Some("instance".to_string()),
        source_game_version: Some("1.21".to_string()),
        backfilled: false,
        observation: Some("{\"snapshot\":\"one\"}".to_string()),
        origin: None,
    }
}

#[tokio::test]
async fn an_origin_round_trips_and_clears() {
    let handle = support::handle().await;
    GameOptionLocaleOrigins::set(
        &handle,
        &key("", "external:key_one"),
        Some(&observed()),
    )
    .await
    .unwrap();

    assert_eq!(
        GameOptionLocaleOrigins::get(&handle, &key("", "external:key_one"))
            .await
            .unwrap(),
        Some(observed())
    );

    GameOptionLocaleOrigins::set(&handle, &key("", "external:key_one"), None)
        .await
        .unwrap();
    assert_eq!(
        GameOptionLocaleOrigins::get(&handle, &key("", "external:key_one"))
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn a_scope_reads_and_clears_only_its_own_origins() {
    let handle = support::handle().await;
    for (scope, option_id) in [
        ("", "external:key_one"),
        ("one", "external:key_two"),
        ("two", "external:key_three"),
    ] {
        GameOptionLocaleOrigins::set(
            &handle,
            &key(scope, option_id),
            Some(&observed()),
        )
        .await
        .unwrap();
    }

    let scoped =
        GameOptionLocaleOrigins::get_scoped(&handle, &"one".to_string())
            .await
            .unwrap();
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].0.option_id, "external:key_two");

    GameOptionLocaleOrigins::set_scoped(&handle, &"one".to_string(), &[])
        .await
        .unwrap();
    assert_eq!(
        GameOptionLocaleOrigins::get_all(&handle)
            .await
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn a_replacement_writes_exactly_what_it_was_given() {
    let handle = support::handle().await;
    GameOptionLocaleOrigins::set(
        &handle,
        &key("", "external:key_one"),
        Some(&observed()),
    )
    .await
    .unwrap();

    let replaced = key("", "external:key_two");
    GameOptionLocaleOrigins::set_all(
        &handle,
        &[(replaced.clone(), observed())],
    )
    .await
    .unwrap();

    assert_eq!(
        GameOptionLocaleOrigins::get(&handle, &key("", "external:key_one"))
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        GameOptionLocaleOrigins::get(&handle, &replaced)
            .await
            .unwrap(),
        Some(observed())
    );
}

#[tokio::test]
async fn a_scoped_replacement_refuses_a_key_outside_its_scope() {
    let handle = support::handle().await;
    let error = GameOptionLocaleOrigins::set_scoped(
        &handle,
        &"one".to_string(),
        &[(key("two", "external:key_one"), observed())],
    )
    .await
    .unwrap_err();

    assert!(matches!(
        error.raw.as_ref(),
        crate::ErrorKind::InputError(_)
    ));
}
