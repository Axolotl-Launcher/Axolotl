//! What the settings columns their own modules own promise: a change through
//! them leaves the settings record as it was, and a change through the record
//! leaves them alone.

use super::support;
use crate::state::store::settings::{Settings, Singleton};
use crate::state::store::{Collection as _, atom};

#[tokio::test]
async fn the_owned_columns_read_back_and_leave_the_record_alone() {
    let handle = support::handle().await;
    let before = Settings::get(&handle, &Singleton).await.unwrap().unwrap();

    let mut unit = atom(&handle).await.unwrap();
    Settings::update_owned_in(&mut unit, |columns| {
        columns.proxy_mode = "custom".to_string();
        columns.proxy_url = "http://proxy.test:8080".to_string();
        columns.proxy_username = "user".to_string();
        columns.proxy_password = "secret".to_string();
        columns.curseforge_api_key = "key".to_string();
    })
    .await
    .unwrap();
    unit.commit().await.unwrap();

    let columns = Settings::owned_columns(&handle).await.unwrap();
    assert_eq!(columns.proxy_mode, "custom");
    assert_eq!(columns.proxy_url, "http://proxy.test:8080");
    assert_eq!(columns.proxy_username, "user");
    assert_eq!(columns.proxy_password, "secret");
    assert_eq!(columns.curseforge_api_key, "key");

    let after = Settings::get(&handle, &Singleton).await.unwrap().unwrap();
    assert_eq!(after, before);
}

#[tokio::test]
async fn a_change_through_the_record_leaves_the_owned_columns_alone() {
    let handle = support::handle().await;

    let mut unit = atom(&handle).await.unwrap();
    Settings::update_owned_in(&mut unit, |columns| {
        columns.proxy_url = "http://proxy.test:8080".to_string();
    })
    .await
    .unwrap();
    unit.commit().await.unwrap();

    let mut unit = atom(&handle).await.unwrap();
    Settings::update_in(&mut unit, |record| {
        record.locale = "fr".to_string();
    })
    .await
    .unwrap();
    unit.commit().await.unwrap();

    let columns = Settings::owned_columns(&handle).await.unwrap();
    assert_eq!(columns.proxy_url, "http://proxy.test:8080");

    let record = Settings::get(&handle, &Singleton).await.unwrap().unwrap();
    assert_eq!(record.locale, "fr");
}
