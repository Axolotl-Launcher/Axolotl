use super::*;
#[tokio::test]
async fn native_spawn_biome_and_render_dispatch_work_with_negative_coordinates()
{
    let backend = LauncherBackend::default();
    let scope = BackendContext::new(launcher_tools::ExecutionScope::local(
        true,
        tokio_util::sync::CancellationToken::new(),
    ));
    let spawn = execute(
        &backend,
        Command::SeedGetSpawn {
            seed: "12345".into(),
            edition: "java".into(),
            version: "1.21.1".into(),
        },
        scope.clone(),
    )
    .await
    .unwrap();
    assert!(spawn["x"].is_number());
    let biome = execute(
        &backend,
        Command::SeedGetBiome {
            seed: "12345".into(),
            edition: "java".into(),
            version: "1.21.1".into(),
            dimension: "overworld".into(),
            x: -16,
            y: 64,
            z: -16,
        },
        scope.clone(),
    )
    .await
    .unwrap();
    assert!(biome["biome_id"].as_i64().unwrap() >= 0);
    let tile = execute(
        &backend,
        Command::SeedRenderTile {
            seed: "12345".into(),
            edition: "java".into(),
            version: "1.21.1".into(),
            dimension: "overworld".into(),
            x: -16,
            z: -16,
            scale: "4".into(),
            width: 16,
            height: 16,
            elevation: None,
            terrain: Some(false),
            contours: Some(false),
        },
        scope.clone(),
    )
    .await
    .unwrap();
    let bytes = STANDARD
        .decode(tile["image"]["data"].as_str().unwrap())
        .unwrap();
    let decoded = image::load_from_memory(&bytes).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (16, 16));
    assert!(
        execute(
            &backend,
            Command::SeedFindFeatures {
                seed: "1".into(),
                edition: "java".into(),
                version: "1.21.1".into(),
                dimension: "overworld".into(),
                min_x: -30000000,
                min_z: 0,
                max_x: 30000000,
                max_z: 1,
                feature_mask: None,
                offset: None,
                limit: None
            },
            scope
        )
        .await
        .is_err()
    );
}
