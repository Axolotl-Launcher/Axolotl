//! What the translation cache promises: a fetched translation is kept under
//! its key until it ages out or the cache is cleared.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::translation_cache::{
    TranslationCache, TranslationCacheRecord,
};

fn entry(translation: &str, created_at: i64) -> TranslationCacheRecord {
    TranslationCacheRecord {
        translation: translation.to_string(),
        created_at,
    }
}

#[tokio::test]
async fn a_translation_round_trips_and_ages_out() {
    let (_directory, handle) = support::derived().await;
    let key = "digest".to_string();

    TranslationCache::set(&handle, &key, Some(&entry("translated", 10)))
        .await
        .unwrap();
    assert_eq!(
        TranslationCache::get(&handle, &key).await.unwrap(),
        Some(entry("translated", 10))
    );

    TranslationCache::delete_created_before(&handle, 11)
        .await
        .unwrap();
    assert_eq!(TranslationCache::get(&handle, &key).await.unwrap(), None);

    TranslationCache::set(&handle, &key, Some(&entry("again", 20)))
        .await
        .unwrap();
    TranslationCache::set_all(&handle, &[]).await.unwrap();
    assert!(TranslationCache::get_all(&handle).await.unwrap().is_empty());
}
