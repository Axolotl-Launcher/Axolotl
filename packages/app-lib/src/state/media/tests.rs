use super::manifest::{ScreenshotIdentity, unique_renames};
use super::*;
use crate::state::MinecraftSkinVariant;
use crate::state::minecraft_skins::CustomMinecraftSkinInsertPosition;
use crate::state::persistence::import::Import;
use std::collections::HashSet;

async fn initialized(root: &std::path::Path) -> Media {
    let persistence = Persistence::new(root);
    let media = Media::new(&persistence);
    Import::new(&persistence, "media", media.document.clone())
        .unwrap()
        .run(
            || async { Ok(empty()) },
            |snapshot| validate(snapshot.data().unwrap()),
        )
        .await
        .unwrap();
    media
}

#[test]
fn equal_content_does_not_merge_screenshot_identities() {
    let mut old = ScreenshotIdentity::for_file("instance", "old.png");
    old.fingerprint = Some("same".into());
    assert_eq!(
        unique_renames(
            &[old.clone()],
            &HashSet::new(),
            &[("new.png".into(), "same".into())]
        )
        .len(),
        1
    );
    let mut sibling = ScreenshotIdentity::for_file("instance", "copy.png");
    sibling.fingerprint = old.fingerprint.clone();
    assert!(
        unique_renames(
            &[old.clone(), sibling],
            &HashSet::new(),
            &[("new.png".into(), "same".into())]
        )
        .is_empty()
    );
    assert!(
        unique_renames(
            &[old],
            &HashSet::new(),
            &[
                ("new.png".into(), "same".into()),
                ("copy.png".into(), "same".into())
            ]
        )
        .is_empty()
    );
}

#[tokio::test]
async fn stale_anchor_preserves_current_group_and_unknown_fields() {
    let temp = tempfile::tempdir().unwrap();
    let media = initialized(temp.path()).await;
    let mut identity = ScreenshotIdentity::for_file("instance", "image.png");
    media.anchor(vec![identity.clone()]).await.unwrap();
    media
        .create_group("group".into(), "Saved".into(), vec![identity.clone()])
        .await
        .unwrap();
    let id = identity.id.clone();
    media
        .update(move |data| {
            data["screenshots"][&id]["future_hint"] = json!({"keep":true});
            Ok(())
        })
        .await
        .unwrap();
    identity.fingerprint = Some("new hash".into());
    media.anchor(vec![identity.clone()]).await.unwrap();
    let current = media.read().await.unwrap();
    assert_eq!(current["screenshots"][&identity.id]["group_id"], "group");
    assert_eq!(
        current["screenshots"][&identity.id]["future_hint"],
        json!({"keep":true})
    );
    assert_eq!(
        current["screenshots"][&identity.id]["fingerprint"],
        "new hash"
    );
}

#[tokio::test]
async fn newer_identity_is_preserved_and_cannot_be_grouped_or_removed() {
    let temp = tempfile::tempdir().unwrap();
    let media = initialized(temp.path()).await;
    let identity = ScreenshotIdentity::for_file("instance", "image.png");
    media.anchor(vec![identity.clone()]).await.unwrap();
    let id = identity.id.clone();
    media
        .update(move |data| {
            data["screenshots"][&id]["schema_version"] = json!(2);
            Ok(())
        })
        .await
        .unwrap();
    assert!(media.screenshots().await.unwrap().is_empty());
    let original = tokio::fs::read(media.document.path()).await.unwrap();
    assert!(media.anchor(vec![identity]).await.is_err());
    assert!(media.forget("instance", "image.png").await.is_err());
    assert_eq!(
        tokio::fs::read(media.document.path()).await.unwrap(),
        original
    );
}

#[tokio::test]
async fn deleted_authority_does_not_revive_legacy_media() {
    let temp = tempfile::tempdir().unwrap();
    let media = initialized(temp.path()).await;
    tokio::fs::remove_file(media.document.path()).await.unwrap();
    assert!(media.read().await.is_err());
    assert!(
        media
            .create_group("new".into(), "Group".into(), vec![])
            .await
            .is_err()
    );
}

#[tokio::test]
async fn skins_keep_bytes_order_and_offline_selection_in_independent_storage() {
    let temp = tempfile::tempdir().unwrap();
    let media = initialized(temp.path()).await;
    let player = uuid::Uuid::new_v4();
    let bytes = b"opaque legacy PNG bytes";
    media
        .skin_add(
            player,
            "first",
            bytes,
            MinecraftSkinVariant::Slim,
            None,
            CustomMinecraftSkinInsertPosition::Bottom,
        )
        .await
        .unwrap();
    media
        .skin_add(
            player,
            "top",
            bytes,
            MinecraftSkinVariant::Classic,
            None,
            CustomMinecraftSkinInsertPosition::Top,
        )
        .await
        .unwrap();
    assert_eq!(media.skin_texture("first").await.unwrap(), bytes);
    assert_eq!(
        media
            .skins(player)
            .await
            .unwrap()
            .iter()
            .map(|skin| skin.texture_key.as_str())
            .collect::<Vec<_>>(),
        vec!["top", "first"]
    );
    media
        .set_offline_skin(
            player,
            Some("first".into()),
            MinecraftSkinVariant::Slim,
            None,
        )
        .await
        .unwrap();
    media.skin_remove(player, "first").await.unwrap();
    assert_eq!(
        media
            .offline_skin(player)
            .await
            .unwrap()
            .unwrap()
            .texture_key,
        "first"
    );
    assert_eq!(media.skin_texture("first").await.unwrap(), bytes);
}

#[tokio::test]
async fn opaque_location_cannot_be_shadowed_by_grouping_or_relocation() {
    let temp = tempfile::tempdir().unwrap();
    let media = initialized(temp.path()).await;
    media.update(|data|{data["screenshots"]["future"]=json!({"id":"future","instance_id":"instance","file_name":"image.png","schemaVersion":2});Ok(())}).await.unwrap();
    let original = tokio::fs::read(media.document.path()).await.unwrap();
    let identity = ScreenshotIdentity::for_file("instance", "image.png");
    assert!(media.memberships(vec![(identity, None)]).await.is_err());
    assert_eq!(
        tokio::fs::read(media.document.path()).await.unwrap(),
        original
    );
    let other = ScreenshotIdentity::for_file("instance", "other.png");
    media.anchor(vec![other.clone()]).await.unwrap();
    let before = tokio::fs::read(media.document.path()).await.unwrap();
    assert!(
        media
            .relocate(&other.id, "instance", "image.png")
            .await
            .is_err()
    );
    assert_eq!(
        tokio::fs::read(media.document.path()).await.unwrap(),
        before
    );
}
