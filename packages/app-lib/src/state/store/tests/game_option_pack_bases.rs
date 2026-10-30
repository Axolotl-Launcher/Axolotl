//! What the pack bases collection promises.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::game_option_pack_bases::{
    GameOptionPackBases, PackBaseKey, PackBaseRecord, PackBaseSource,
};

fn key(instance_id: &str) -> PackBaseKey {
    PackBaseKey {
        instance_id: instance_id.to_string(),
    }
}

#[tokio::test]
async fn a_pack_base_round_trips_and_clears() {
    let handle = support::handle_with_instances(&["instance"]).await;
    let record = PackBaseRecord {
        pack_version_id: Some("version".to_string()),
        source: PackBaseSource::Overrides,
        sha1: Some("abc".to_string()),
        encoding: Some("utf-8".to_string()),
        document: Some(b"options".to_vec()),
    };

    GameOptionPackBases::set(&handle, &key("instance"), Some(&record))
        .await
        .unwrap();
    assert_eq!(
        GameOptionPackBases::get(&handle, &key("instance"))
            .await
            .unwrap(),
        Some(record)
    );
    assert_eq!(
        GameOptionPackBases::get(&handle, &key("instance"))
            .await
            .unwrap()
            .map(|record| record.source),
        Some(PackBaseSource::Overrides)
    );
    assert_eq!(
        GameOptionPackBases::get_all(&handle).await.unwrap().len(),
        1
    );

    GameOptionPackBases::set(&handle, &key("instance"), None)
        .await
        .unwrap();
    assert_eq!(
        GameOptionPackBases::get(&handle, &key("instance"))
            .await
            .unwrap(),
        None
    );
}
