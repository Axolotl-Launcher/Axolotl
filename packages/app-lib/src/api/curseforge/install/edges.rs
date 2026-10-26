//! CurseForge install edges.

use super::*;
use crate::State;
use crate::state::{
    ContentProvider, ContentProviderRef, DependencyResolutionPlan,
};
use std::collections::HashSet;

pub(crate) async fn persist_resolution_plan_dependency_edges(
    instance_id: &str,
    plan: &DependencyResolutionPlan,
    installed: &HashSet<ContentProviderRef>,
    state: &State,
) -> crate::Result<()> {
    if plan.edges.is_empty() {
        return Ok(());
    }
    let database_permit = state.acquire_install_db_permit().await?;
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let scope = crate::state::instances::commands::resolve_content_scope(
        instance_id,
        None,
        state,
    )
    .await?;
    let mut tx = state.pool.begin().await?;
    for edge in &plan.edges {
        if !installed.contains(&edge.parent) || !installed.contains(&edge.child)
        {
            continue;
        }
        let (Some(parent_release), Some(child_release)) = (
            edge.parent.database_release_id(),
            edge.child.database_release_id(),
        ) else {
            continue;
        };
        let Some(parent_entry) = crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
            &scope.content_set_id,
            edge.parent.provider(),
            &edge.parent.database_project_id(),
            &parent_release,
            &state.pool,
        )
        .await?
        else {
            continue;
        };
        let Some(child_entry) = crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
            &scope.content_set_id,
            edge.child.provider(),
            &edge.child.database_project_id(),
            &child_release,
            &state.pool,
        )
        .await?
        else {
            continue;
        };
        let now = chrono::Utc::now();
        crate::state::instances::adapters::sqlite::content_rows::upsert_content_dependency_edge_in_transaction(
            &crate::state::instances::ContentDependencyEdge {
                id: format!("content-dependency:{}", uuid::Uuid::new_v4()),
                content_set_id: scope.content_set_id.clone(),
                parent_entry_id: parent_entry.id,
                child_entry_id: child_entry.id,
                evidence_provider: edge.evidence_provider,
                parent_provider: edge.parent.provider(),
                child_provider: edge.child.provider(),
                dependency_kind: edge.relation,
                parent_project_id: edge.parent.database_project_id(),
                parent_release_id: parent_release,
                child_project_id: edge.child.database_project_id(),
                child_release_id: child_release,
                created_at: now,
                modified_at: now,
            },
            &mut tx,
        )
        .await?;
    }
    tx.commit().await?;
    drop(database_permit);
    Ok(())
}

pub(crate) async fn persist_curseforge_dependency_edges(
    instance_id: &str,
    candidates: &[CurseForgeDependencyEdgeCandidate],
    state: &State,
) -> crate::Result<()> {
    if candidates.is_empty() {
        return Ok(());
    }
    let database_permit = state.acquire_install_db_permit().await?;
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let scope = crate::state::instances::commands::resolve_content_scope(
        instance_id,
        None,
        state,
    )
    .await?;
    let mut tx = state.pool.begin().await?;
    for candidate in candidates {
        let Some(parent_entry) =
            crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
                &scope.content_set_id,
                ContentProvider::CurseForge,
                &candidate.parent_project_id.to_string(),
                &candidate.parent_file_id.to_string(),
                &state.pool,
            )
            .await?
        else {
            continue;
        };
        let Some(child_entry) =
            crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
                &scope.content_set_id,
                ContentProvider::CurseForge,
                &candidate.child_project_id.to_string(),
                &candidate.child_file_id.to_string(),
                &state.pool,
            )
            .await?
        else {
            continue;
        };
        crate::state::instances::adapters::sqlite::content_rows::set_content_entry_auto_dependency_in_transaction(
            &child_entry.id,
            true,
            &mut tx,
        )
        .await?;
        let now = chrono::Utc::now();
        crate::state::instances::adapters::sqlite::content_rows::upsert_content_dependency_edge_in_transaction(
            &crate::state::instances::ContentDependencyEdge {
                id: format!("content-dependency:{}", uuid::Uuid::new_v4()),
                content_set_id: scope.content_set_id.clone(),
                parent_entry_id: parent_entry.id,
                child_entry_id: child_entry.id,
                evidence_provider: ContentProvider::CurseForge,
                parent_provider: ContentProvider::CurseForge,
                child_provider: ContentProvider::CurseForge,
                dependency_kind: candidate.dependency_kind,
                parent_project_id: candidate.parent_project_id.to_string(),
                parent_release_id: candidate.parent_file_id.to_string(),
                child_project_id: candidate.child_project_id.to_string(),
                child_release_id: candidate.child_file_id.to_string(),
                created_at: now,
                modified_at: now,
            },
            &mut tx,
        )
        .await?;
    }
    tx.commit().await?;
    drop(database_permit);
    Ok(())
}

pub(crate) async fn persist_modrinth_fallback_dependency_edges(
    instance_id: &str,
    fallbacks: &[(&ModrinthFallbackPlan, HashSet<String>)],
    state: &State,
) -> crate::Result<()> {
    if fallbacks.is_empty() {
        return Ok(());
    }
    let database_permit = state.acquire_install_db_permit().await?;
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let scope = crate::state::instances::commands::resolve_content_scope(
        instance_id,
        None,
        state,
    )
    .await?;
    let mut tx = state.pool.begin().await?;

    for (fallback, installed_versions) in fallbacks {
        for dependency in &fallback.plan.dependencies {
            if !installed_versions.contains(&dependency.version_id) {
                continue;
            }
            let Some(parent_version_id) =
                dependency.dependent_on_version_id.as_deref()
            else {
                continue;
            };
            let parent = if parent_version_id
                == fallback.plan.primary.version_id
            {
                crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
                    &scope.content_set_id,
                    ContentProvider::CurseForge,
                    &fallback.parent_project_id.to_string(),
                    &fallback.parent_file_id.to_string(),
                    &state.pool,
                )
                .await?
                .map(|entry| (
                    entry,
                    ContentProvider::CurseForge,
                    fallback.parent_project_id.to_string(),
                    fallback.parent_file_id.to_string(),
                ))
            } else if let Some(parent_content) = fallback
                .plan
                .dependencies
                .iter()
                .find(|content| content.version_id == parent_version_id)
            {
                crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
                    &scope.content_set_id,
                    ContentProvider::Modrinth,
                    &parent_content.project_id,
                    &parent_content.version_id,
                    &state.pool,
                )
                .await?
                .map(|entry| (
                    entry,
                    ContentProvider::Modrinth,
                    parent_content.project_id.clone(),
                    parent_content.version_id.clone(),
                ))
            } else {
                None
            };
            let Some((
                parent_entry,
                parent_provider,
                parent_project_id,
                parent_release_id,
            )) = parent
            else {
                continue;
            };
            let Some(child_entry) = crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
                &scope.content_set_id,
                ContentProvider::Modrinth,
                &dependency.project_id,
                &dependency.version_id,
                &state.pool,
            )
            .await?
            else {
                continue;
            };
            let now = chrono::Utc::now();
            crate::state::instances::adapters::sqlite::content_rows::upsert_content_dependency_edge_in_transaction(
                &crate::state::instances::ContentDependencyEdge {
                    id: format!("content-dependency:{}", uuid::Uuid::new_v4()),
                    content_set_id: scope.content_set_id.clone(),
                    parent_entry_id: parent_entry.id,
                    child_entry_id: child_entry.id,
                    evidence_provider: ContentProvider::Modrinth,
                    parent_provider,
                    child_provider: ContentProvider::Modrinth,
                    dependency_kind: crate::state::instances::ContentDependencyKind::Required,
                    parent_project_id,
                    parent_release_id,
                    child_project_id: dependency.project_id.clone(),
                    child_release_id: dependency.version_id.clone(),
                    created_at: now,
                    modified_at: now,
                },
                &mut tx,
            )
            .await?;
        }
    }
    tx.commit().await?;
    drop(database_permit);
    Ok(())
}
