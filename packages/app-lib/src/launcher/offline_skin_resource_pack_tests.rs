use super::*;

#[test]
fn enables_offline_skin_pack_when_options_file_is_new() {
    let mut options = String::new();

    update_offline_skin_resource_pack_option(
        &mut options,
        crate::minecraft_skins::OfflineSkinPackOptions {
            enabled_pack_id: Some(
                crate::minecraft_skins::OFFLINE_SKIN_PACK_MODERN_ID,
            ),
        },
    )
    .unwrap();

    assert_eq!(
        options,
        "resourcePacks:[\"vanilla\",\"file/Axolotl Offline Skin.zip\"]"
    );
}

#[test]
fn adds_modern_offline_skin_pack_without_removing_existing_packs() {
    let mut options =
        "resourcePacks:[\"vanilla\",\"file/user-pack.zip\"]".to_string();

    update_offline_skin_resource_pack_option(
        &mut options,
        crate::minecraft_skins::OfflineSkinPackOptions {
            enabled_pack_id: Some(
                crate::minecraft_skins::OFFLINE_SKIN_PACK_MODERN_ID,
            ),
        },
    )
    .unwrap();

    assert_eq!(
        options,
        "resourcePacks:[\"vanilla\",\"file/user-pack.zip\",\"file/Axolotl Offline Skin.zip\"]"
    );
}

#[test]
fn removes_both_offline_skin_pack_id_variants() {
    let mut options = "resourcePacks:[\"vanilla\",\"Axolotl Offline Skin.zip\",\"file/Axolotl Offline Skin.zip\"]".to_string();

    update_offline_skin_resource_pack_option(
        &mut options,
        crate::minecraft_skins::OfflineSkinPackOptions {
            enabled_pack_id: None,
        },
    )
    .unwrap();

    assert_eq!(options, "resourcePacks:[\"vanilla\"]");
}
