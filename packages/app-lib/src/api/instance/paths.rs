use crate::launcher::instance_runtime::InstanceRuntimeAdapter;
use crate::state::State;
use crate::util::io;
use std::path::PathBuf;

#[tracing::instrument(skip_all, level = "info")]
pub async fn get_full_path(instance_id: &str) -> crate::Result<PathBuf> {
    let state = State::get().await?;
    let instance = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown instance".to_string())
        })?;

    // Directly associated instances never have a profile directory under
    // `profiles`; interactive APIs (open folder, worlds, servers, ...) must
    // operate on the linked `.minecraft` instead.
    let adapter = InstanceRuntimeAdapter::for_instance(
        &instance.instance,
        &state.directories,
    )?;
    Ok(io::canonicalize(adapter.game_dir())?)

    // `instance_game_dir` honours a per-instance `game_dir_override` before
    // falling back to the profile directory.
}

#[tracing::instrument(skip_all, level = "info")]
pub async fn get_mod_full_path(
    instance_id: &str,
    project_path: &str,
) -> crate::Result<PathBuf> {
    Ok(get_full_path(instance_id).await?.join(project_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{
        CreateDirectLinkInstance, CreateInstance, InstanceLink, ModLoader,
    };
    use std::path::Path;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// The launcher state is a process-wide singleton; initialize it once and
    /// reuse it so `State::get()` resolves inside these APIs. The state root
    /// is intentionally leaked (`.keep()`) because the shared state outlives
    /// this function.
    async fn global_state() -> Arc<State> {
        if !State::initialized() {
            let root = TempDir::new().unwrap().keep();
            let _ =
                State::init_for_test(root.to_string_lossy().to_string()).await;
        }
        State::get().await.unwrap()
    }

    async fn create_direct_link_fixture(
        label: &str,
    ) -> (TempDir, crate::state::InstanceMetadata) {
        let state = global_state().await;
        let minecraft = TempDir::new().unwrap();
        let version_dir = minecraft.path().join("versions").join(label);
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(
            version_dir.join(format!("{label}.json")),
            serde_json::to_vec_pretty(&serde_json::json!({
                "id": label,
                "inheritsFrom": "1.20.1",
                "mainClass": "net.minecraft.client.main.Main"
            }))
            .unwrap(),
        )
        .unwrap();

        let instance = crate::state::create_direct_link_instance(
            CreateDirectLinkInstance {
                name: None,
                launcher_type:
                    crate::api::pack::import::ImportLauncherType::Generic,
                base_path: minecraft.path().to_path_buf(),
                instance_folder: format!("versions/{label}"),
                instance_path: None,
                game_dir_mode: None,
            },
            &state,
        )
        .await
        .unwrap();
        let metadata = crate::state::get_instance(&instance.id, &state.pool)
            .await
            .unwrap()
            .expect("metadata for created fixture");
        (minecraft, metadata)
    }

    #[tokio::test]
    async fn direct_link_instance_resolves_to_isolated_version_directory() {
        let state = global_state().await;
        let (_minecraft, metadata) =
            create_direct_link_fixture("paths-demo").await;

        let resolved = get_full_path(&metadata.instance.id).await.unwrap();

        assert!(resolved.ends_with(Path::new("versions").join("paths-demo")));
        assert!(
            !state
                .directories
                .instances_dir()
                .join(&metadata.instance.path)
                .exists(),
            "no profile directory may be created for a direct-link instance"
        );
    }

    #[tokio::test]
    async fn shared_direct_link_instance_resolves_to_minecraft_root() {
        let state = global_state().await;
        let minecraft = TempDir::new().unwrap();
        let version_name = "paths-shared";
        let version_dir = minecraft.path().join("versions").join(version_name);
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(
            version_dir.join(format!("{version_name}.json")),
            serde_json::to_vec_pretty(&serde_json::json!({
                "id": version_name,
                "inheritsFrom": "1.20.1",
                "mainClass": "net.minecraft.client.main.Main"
            }))
            .unwrap(),
        )
        .unwrap();

        let instance = crate::state::create_direct_link_instance(
            CreateDirectLinkInstance {
                name: None,
                launcher_type:
                    crate::api::pack::import::ImportLauncherType::Generic,
                base_path: minecraft.path().to_path_buf(),
                instance_folder: Path::new("versions")
                    .join(version_name)
                    .to_string_lossy()
                    .to_string(),
                instance_path: None,
                game_dir_mode: Some(
                    crate::launcher::ExternalGameDirMode::Shared,
                ),
            },
            &state,
        )
        .await
        .unwrap();

        let resolved = get_full_path(&instance.id).await.unwrap();

        assert_eq!(resolved, io::canonicalize(minecraft.path()).unwrap());
    }

    #[tokio::test]
    async fn new_external_instances_resolve_before_version_json_exists() {
        let state = global_state().await;
        let minecraft = TempDir::new().unwrap();
        let dot_minecraft = minecraft.path().join(".minecraft");
        std::fs::create_dir_all(&dot_minecraft).unwrap();

        let isolated_dir =
            dot_minecraft.join("versions").join("fresh-isolated");
        let isolated = crate::state::instances::commands::create_instance(
            CreateInstance {
                name: "fresh-isolated".to_string(),
                path: None,
                game_version: "1.21.1".to_string(),
                loader: ModLoader::Vanilla,
                loader_version: None,
                icon_path: None,
                link: InstanceLink::Unmanaged,
                symlink_target: None,
                game_dir_override: Some(
                    isolated_dir.to_string_lossy().into_owned(),
                ),
            },
            &state,
        )
        .await
        .unwrap();
        assert_eq!(
            get_full_path(&isolated.id).await.unwrap(),
            io::canonicalize(&isolated_dir).unwrap()
        );
        let launch_context =
            crate::state::instances::commands::get_instance_launch_context(
                &isolated.id,
                &state.pool,
            )
            .await
            .unwrap()
            .expect("instance created by the install flow should be queryable");
        assert_eq!(launch_context.instance.id, isolated.id);
        assert_eq!(
            launch_context.instance.game_dir_override,
            Some(isolated_dir.to_string_lossy().into_owned())
        );

        let shared = crate::state::instances::commands::create_instance(
            CreateInstance {
                name: "fresh-shared".to_string(),
                path: None,
                game_version: "1.21.1".to_string(),
                loader: ModLoader::Vanilla,
                loader_version: None,
                icon_path: None,
                link: InstanceLink::Unmanaged,
                symlink_target: None,
                game_dir_override: Some(
                    dot_minecraft.to_string_lossy().into_owned(),
                ),
            },
            &state,
        )
        .await
        .unwrap();
        assert_eq!(
            get_full_path(&shared.id).await.unwrap(),
            io::canonicalize(&dot_minecraft).unwrap()
        );
    }

    #[tokio::test]
    async fn direct_link_file_browser_lists_linked_content() {
        let _state = global_state().await;
        let (minecraft, metadata) =
            create_direct_link_fixture("paths-browse").await;

        // The file browser lists directories relative to the resolved root.
        let version_dir =
            minecraft.path().join("versions").join("paths-browse");
        std::fs::create_dir_all(version_dir.join("mods")).unwrap();
        std::fs::write(
            version_dir.join("mods").join("browse-fixture.jar"),
            b"browse fixture",
        )
        .unwrap();

        let root = get_full_path(&metadata.instance.id).await.unwrap();
        let listed = root.join("mods").read_dir().unwrap();
        let names = listed
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec!["browse-fixture.jar".to_string()],
            "the browser root must expose the linked installation's content"
        );
    }

    #[tokio::test]
    async fn ordinary_instance_still_resolves_to_its_profile_directory() {
        let state = global_state().await;
        let metadata = crate::api::instance::create(
            format!("paths-normal {}", uuid::Uuid::new_v4()),
            "1.20.1".to_string(),
            crate::state::ModLoader::Vanilla,
            None,
            None,
            crate::state::InstanceLink::Unmanaged,
            None,
            None,
        )
        .await
        .unwrap();

        std::fs::create_dir_all(
            state
                .directories
                .instances_dir()
                .join(&metadata.instance.path),
        )
        .unwrap();

        let resolved = get_full_path(&metadata.instance.id).await.unwrap();
        assert_eq!(
            resolved,
            io::canonicalize(
                state
                    .directories
                    .instances_dir()
                    .join(&metadata.instance.path)
            )
            .unwrap()
        );
    }
}
