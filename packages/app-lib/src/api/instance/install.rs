use crate::state::{JavaVersion, State};

pub async fn get_optimal_jre_key(
    instance_id: &str,
) -> crate::Result<Option<JavaVersion>> {
    let state = State::get().await?;
    let context =
        crate::state::instances::commands::get_instance_launch_context(
            instance_id,
            &state.pool,
        )
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::OtherError(format!(
                "Tried to resolve a nonexistent instance {instance_id}!"
            ))
        })?;
    let (minecraft, version_index) =
        crate::launcher::resolve_minecraft_manifest(
            &context.applied_content_set.game_version,
            &state,
        )
        .await?;
    let version = &minecraft.versions[version_index];
    let game_dir = crate::launcher::instance_runtime::InstanceRuntimeAdapter::for_instance(&context.instance, &state.directories)?.game_dir();
    let mmc_profile = crate::launcher::multimc::MultiMcProfile::load(
        &game_dir,
        &context.applied_content_set.game_version,
        context.applied_content_set.loader,
        context.applied_content_set.loader_version.as_deref(),
    )
    .await?;
    let metadata_loader = mmc_profile
        .as_ref()
        .map_or(context.applied_content_set.loader, |profile| {
            profile.metadata_loader(context.applied_content_set.loader)
        });
    let loader_version = crate::launcher::get_loader_version_from_profile(
        &context.applied_content_set.game_version,
        metadata_loader,
        context.applied_content_set.loader_version.as_deref(),
    )
    .await?;
    let mut version_info = crate::launcher::download::download_version_info(
        &state,
        version,
        metadata_loader,
        loader_version.as_ref(),
        None,
        None,
        None,
    )
    .await?;

    if let Some(profile) = &mmc_profile {
        let vanilla = if profile.replaces_minecraft()
            && metadata_loader != crate::state::ModLoader::Vanilla
        {
            Some(
                crate::launcher::download::download_version_info(
                    &state,
                    version,
                    crate::state::ModLoader::Vanilla,
                    None,
                    None,
                    None,
                    None,
                )
                .await?,
            )
        } else {
            None
        };
        profile
            .apply(
                &mut version_info,
                vanilla.as_ref(),
                &state.directories.libraries_dir(),
            )
            .await?;
    }

    let major_version = version_info
        .java_version
        .as_ref()
        .map_or(8, |java| java.major_version);

    crate::api::jre::find_java_for_version(major_version).await
}
