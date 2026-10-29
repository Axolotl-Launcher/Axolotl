//! What the favorites collection promises.

use super::support;
use crate::state::content_types::{
    ContentFavoriteProvider, ContentFavoriteType,
};
use crate::state::store::Collection as _;
use crate::state::store::content_favorites::{
    ContentFavoriteKey, ContentFavoriteRecord, ContentFavorites,
};
use chrono::Utc;
use std::time::Duration;

fn key(
    provider: ContentFavoriteProvider,
    project_id: &str,
) -> ContentFavoriteKey {
    ContentFavoriteKey {
        provider,
        project_id: project_id.to_string(),
    }
}

fn modrinth_key(project_id: &str) -> ContentFavoriteKey {
    ContentFavoriteKey {
        provider: ContentFavoriteProvider::Modrinth,
        project_id: project_id.to_string(),
    }
}

fn record(saved_at: i64) -> ContentFavoriteRecord {
    ContentFavoriteRecord {
        content_type: ContentFavoriteType::Mod,
        saved_at,
    }
}

#[tokio::test]
async fn a_favorite_round_trips_and_removes() {
    let handle = support::handle().await;
    let favorite = key(ContentFavoriteProvider::Modrinth, "sodium");
    let record = ContentFavoriteRecord {
        content_type: ContentFavoriteType::Mod,
        saved_at: 1_700_000_000_000,
    };

    ContentFavorites::set(&handle, &favorite, Some(&record))
        .await
        .unwrap();
    assert_eq!(
        ContentFavorites::get(&handle, &favorite).await.unwrap(),
        Some(record)
    );

    ContentFavorites::set(&handle, &favorite, None)
        .await
        .unwrap();
    assert_eq!(
        ContentFavorites::get(&handle, &favorite).await.unwrap(),
        None
    );
}

#[tokio::test]
async fn saving_a_favorite_again_keeps_the_moment_it_was_first_saved() {
    let handle = support::handle().await;
    let favorite = key(ContentFavoriteProvider::Modrinth, "sodium");

    ContentFavorites::set(
        &handle,
        &favorite,
        Some(&ContentFavoriteRecord {
            content_type: ContentFavoriteType::Mod,
            saved_at: 1,
        }),
    )
    .await
    .unwrap();
    ContentFavorites::set(
        &handle,
        &favorite,
        Some(&ContentFavoriteRecord {
            content_type: ContentFavoriteType::Shader,
            saved_at: 2,
        }),
    )
    .await
    .unwrap();

    assert_eq!(
        ContentFavorites::get(&handle, &favorite).await.unwrap(),
        Some(ContentFavoriteRecord {
            content_type: ContentFavoriteType::Shader,
            saved_at: 1,
        })
    );
}

#[tokio::test]
async fn a_whole_collection_round_trips() {
    let handle = support::handle().await;
    let records = vec![
        (
            key(ContentFavoriteProvider::Modrinth, "sodium"),
            ContentFavoriteRecord {
                content_type: ContentFavoriteType::Mod,
                saved_at: 2,
            },
        ),
        (
            key(ContentFavoriteProvider::Curseforge, "sapphire"),
            ContentFavoriteRecord {
                content_type: ContentFavoriteType::Shader,
                saved_at: 1,
            },
        ),
    ];

    ContentFavorites::set_all(&handle, &records).await.unwrap();

    let mut stored = ContentFavorites::get_all(&handle).await.unwrap();
    stored.sort_by(|left, right| left.0.project_id.cmp(&right.0.project_id));
    let mut expected = records;
    expected.sort_by(|left, right| left.0.project_id.cmp(&right.0.project_id));
    assert_eq!(stored, expected);
}

#[tokio::test]
async fn the_collection_rejects_values_it_does_not_know() {
    let handle = support::handle().await;

    assert!(
        sqlx::query(
            "INSERT INTO content_favorites
                (provider, project_id, content_type, saved_at)
             VALUES ('unknown', 'bad-provider', 'mod', 3)",
        )
        .execute(handle.pool())
        .await
        .is_err()
    );
    assert!(
        sqlx::query(
            "INSERT INTO content_favorites
                (provider, project_id, content_type, saved_at)
             VALUES ('modrinth', 'bad-type', 'modpack', 3)",
        )
        .execute(handle.pool())
        .await
        .is_err()
    );
}

#[tokio::test]
async fn a_favorite_whose_identity_cannot_be_read_is_skipped() {
    let handle = support::handle().await;
    ContentFavorites::set(
        &handle,
        &key(ContentFavoriteProvider::Modrinth, "sodium"),
        Some(&ContentFavoriteRecord {
            content_type: ContentFavoriteType::Mod,
            saved_at: 1,
        }),
    )
    .await
    .unwrap();
    // The table refuses these values, so they are forced in to stand for a
    // database a later build may have left behind.
    sqlx::raw_sql(
        "PRAGMA ignore_check_constraints = ON;
         INSERT INTO content_favorites
            (provider, project_id, content_type, saved_at)
         VALUES ('unknown', 'bad-provider', 'mod', 3),
            ('modrinth', 'bad-type', 'modpack', 4);
         PRAGMA ignore_check_constraints = OFF;",
    )
    .execute(handle.pool())
    .await
    .unwrap();

    let stored = ContentFavorites::get_all(&handle).await.unwrap();

    assert_eq!(
        stored,
        vec![(
            key(ContentFavoriteProvider::Modrinth, "sodium"),
            ContentFavoriteRecord {
                content_type: ContentFavoriteType::Mod,
                saved_at: 1,
            },
        )]
    );
}

#[tokio::test]
async fn a_batch_keeps_the_time_it_was_given() {
    let handle = support::handle().await;

    ContentFavorites::set_all(&handle, &[(modrinth_key("sodium"), record(7))])
        .await
        .unwrap();

    assert_eq!(
        ContentFavorites::get(&handle, &modrinth_key("sodium"))
            .await
            .unwrap(),
        Some(record(7))
    );
}

#[tokio::test]
async fn a_batch_refuses_the_same_key_twice() {
    let handle = support::handle().await;

    assert!(
        ContentFavorites::set_all(
            &handle,
            &[
                (modrinth_key("sodium"), record(1)),
                (modrinth_key("sodium"), record(2)),
            ],
        )
        .await
        .is_err()
    );
    assert!(ContentFavorites::get_all(&handle).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_scoped_replace_refuses_a_key_outside_its_scope() {
    let handle = support::handle().await;

    assert!(
        ContentFavorites::set_scoped(
            &handle,
            &modrinth_key("sodium"),
            &[(modrinth_key("lithium"), record(1))],
        )
        .await
        .is_err()
    );
    assert!(
        ContentFavorites::get(&handle, &modrinth_key("lithium"))
            .await
            .unwrap()
            .is_none()
    );
}
#[tokio::test]
async fn content_favorites_are_provider_qualified_and_idempotent() {
    let handle = support::handle().await;
    let modrinth = key(ContentFavoriteProvider::Modrinth, "same-id");
    let saved_at = Utc::now().timestamp_millis();
    ContentFavorites::set(&handle, &modrinth, Some(&record(saved_at)))
        .await
        .expect("add Modrinth favorite");
    let first = ContentFavorites::get(&handle, &modrinth)
        .await
        .expect("read Modrinth favorite")
        .expect("Modrinth favorite is stored");
    ContentFavorites::set(&handle, &modrinth, Some(&record(saved_at + 1)))
        .await
        .expect("add duplicate favorite");
    let duplicate = ContentFavorites::get(&handle, &modrinth)
        .await
        .expect("read duplicate favorite")
        .expect("duplicate favorite is stored");
    ContentFavorites::set(
        &handle,
        &key(ContentFavoriteProvider::Curseforge, "same-id"),
        Some(&ContentFavoriteRecord {
            content_type: ContentFavoriteType::Shader,
            saved_at,
        }),
    )
    .await
    .expect("add CurseForge favorite");

    let favorites = ContentFavorites::get_all(&handle)
        .await
        .expect("read favorites");
    assert_eq!(favorites.len(), 2);
    assert_eq!(duplicate.saved_at, first.saved_at);
    assert!(favorites.iter().any(|(favorite, _)| {
        favorite.provider == ContentFavoriteProvider::Modrinth
            && favorite.project_id == "same-id"
    }));
    assert!(favorites.iter().any(|(favorite, _)| {
        favorite.provider == ContentFavoriteProvider::Curseforge
            && favorite.project_id == "same-id"
    }));
}

#[tokio::test]
async fn removing_a_content_favorite_is_idempotent() {
    let handle = support::handle().await;
    ContentFavorites::set(
        &handle,
        &key(ContentFavoriteProvider::Modrinth, "missing"),
        None,
    )
    .await
    .expect("remove missing favorite");
    assert!(
        ContentFavorites::get_all(&handle)
            .await
            .expect("read favorites")
            .is_empty()
    );
}

#[tokio::test]
async fn readding_after_remove_gets_a_new_saved_time() {
    let handle = support::handle().await;
    let favorite = key(ContentFavoriteProvider::Modrinth, "sodium");
    ContentFavorites::set(
        &handle,
        &favorite,
        Some(&record(Utc::now().timestamp_millis())),
    )
    .await
    .expect("add favorite");
    let first = ContentFavorites::get(&handle, &favorite)
        .await
        .expect("read favorite")
        .expect("favorite is stored");
    ContentFavorites::set(&handle, &favorite, None)
        .await
        .expect("remove favorite");
    tokio::time::sleep(Duration::from_millis(2)).await;
    ContentFavorites::set(
        &handle,
        &favorite,
        Some(&record(Utc::now().timestamp_millis())),
    )
    .await
    .expect("re-add favorite");
    let second = ContentFavorites::get(&handle, &favorite)
        .await
        .expect("read favorite")
        .expect("favorite is stored again");

    assert!(second.saved_at > first.saved_at);
}

#[tokio::test]
async fn favorites_are_ordered_by_when_they_were_saved() {
    let handle = support::handle().await;
    for (provider, project_id, saved_at) in [
        (ContentFavoriteProvider::Modrinth, "older", 1),
        (ContentFavoriteProvider::Curseforge, "newer", 2),
    ] {
        ContentFavorites::set(
            &handle,
            &ContentFavoriteKey {
                provider,
                project_id: project_id.to_string(),
            },
            Some(&ContentFavoriteRecord {
                content_type: ContentFavoriteType::Mod,
                saved_at,
            }),
        )
        .await
        .expect("seed favorite");
    }

    let mut favorites = ContentFavorites::get_all(&handle)
        .await
        .expect("read favorites");
    favorites
        .sort_by(|(_, left), (_, right)| right.saved_at.cmp(&left.saved_at));
    assert_eq!(
        favorites
            .iter()
            .map(|(favorite, _)| favorite.project_id.as_str())
            .collect::<Vec<_>>(),
        vec!["newer", "older"]
    );
}
