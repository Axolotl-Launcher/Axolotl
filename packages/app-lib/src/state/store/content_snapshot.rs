//! A content view and its revision read from one database snapshot.

use super::Handle;
use crate::resources::{ResourceId, SourceId};
use crate::state::instances::adapters::sqlite::{instance_rows, links};
use crate::state::instances::{
    ContentEntry, ContentSet, Instance, InstanceFile, InstanceLink, PackMember,
    PendingManualDownload,
};
use crate::state::{ContentProvider, ContentProviderRef};
use sqlx::Row;
use std::collections::HashMap;

pub(crate) struct SnapshotRecords {
    pub pack_presentation: Option<super::content_refresh_pack::PackRecord>,
    pub fingerprints: HashMap<
        String,
        super::content_file_fingerprints::FileFingerprintRecord,
    >,
    pub presentations: HashMap<
        String,
        super::content_refresh_presentations::PresentationRecord,
    >,
    pub instance: Instance,
    pub content_set: ContentSet,
    pub link: InstanceLink,
    pub files: Vec<InstanceFile>,
    pub entries: Vec<ContentEntry>,
    pub members: Vec<PackMember>,
    pub pending: Vec<PendingManualDownload>,
    pub dependencies:
        HashMap<String, crate::state::instances::ContentDependencyInfo>,
    pub references: HashMap<String, Vec<ContentProviderRef>>,
    pub origins: HashMap<String, ContentProvider>,
    pub artifacts:
        HashMap<String, crate::resources::installed::InstalledArtifactRecord>,
    pub entry_resources: HashMap<String, ResourceId>,
    pub pack_resources: HashMap<String, ResourceId>,
    pub pack_resource: Option<ResourceId>,
    pub preferred_source: SourceId,
    pub checks: Vec<(
        super::content_refresh_checks::RefreshCheckKey,
        super::content_refresh_checks::RefreshCheckRecord,
    )>,
}

pub(crate) async fn get(
    handle: &Handle,
    instance_id: &str,
) -> crate::Result<SnapshotRecords> {
    use super::Collection as _;

    use crate::state::instances::adapters::sqlite::{
        content_entries, content_sets, files, pack_members,
    };
    let mut tx = handle.pool().begin().await?;
    let instance = get_instance(&mut tx, instance_id).await?;
    let content_set =
        content_sets::get_applied_content_set(instance_id, &mut *tx)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(
                    "Instance has no applied content set".into(),
                )
            })?;
    #[cfg(test)]
    pause::between_reads().await;
    let link = links::get_instance_link(instance_id, &mut *tx).await?;
    let files = files::get_instance_files(instance_id, &mut *tx).await?;
    let entries =
        content_entries::get_content_entries(&content_set.id, &mut *tx).await?;
    let rows: Vec<pack_members::PackMemberRow> = sqlx::query_as("SELECT * FROM instance_pack_members WHERE content_set_id = ? ORDER BY created_at, id")
        .bind(&content_set.id).fetch_all(&mut *tx).await?;
    let members = rows
        .into_iter()
        .map(TryInto::try_into)
        .collect::<crate::Result<Vec<_>>>()?;
    let rows: Vec<pack_members::PendingManualDownloadRow> = sqlx::query_as("SELECT * FROM instance_pending_manual_downloads WHERE instance_id = ? AND state IN ('waiting', 'matched', 'error') ORDER BY created_at, id")
        .bind(instance_id).fetch_all(&mut *tx).await?;
    let pending = rows
        .into_iter()
        .map(TryInto::try_into)
        .collect::<crate::Result<Vec<_>>>()?;
    let mut dependencies =
        HashMap::<String, crate::state::instances::ContentDependencyInfo>::new(
        );
    for entry in &entries {
        if entry.auto_dependency {
            dependencies.insert(
                entry.id.clone(),
                crate::state::instances::ContentDependencyInfo {
                    auto_dependency: true,
                    orphaned: true,
                    ..Default::default()
                },
            );
        }
    }
    let edges = sqlx::query("SELECT parent_entry_id, child_entry_id, parent_provider, child_provider, parent_project_id, child_project_id, parent_release_id, child_release_id FROM instance_content_dependencies WHERE content_set_id = ? ORDER BY created_at, id")
        .bind(&content_set.id).fetch_all(&mut *tx).await?;
    for row in edges {
        let parent: String = row.try_get("parent_entry_id")?;
        let child: String = row.try_get("child_entry_id")?;
        let providers =
            ContentProvider::from_str(row.try_get("parent_provider")?)
                .and_then(|parent| {
                    Ok((
                        parent,
                        ContentProvider::from_str(
                            row.try_get("child_provider")?,
                        )?,
                    ))
                });
        let Ok((parent_provider, child_provider)) = providers else {
            tracing::warn!("Skipping invalid dependency provider");
            continue;
        };
        dependencies.entry(parent).or_default().requires.push(
            crate::state::instances::ContentDependencyRef {
                provider: child_provider,
                project_id: row.try_get("child_project_id")?,
                release_id: row.try_get("child_release_id")?,
            },
        );
        let info = dependencies.entry(child).or_default();
        info.orphaned = false;
        info.required_by
            .push(crate::state::instances::ContentDependencyRef {
                provider: parent_provider,
                project_id: row.try_get("parent_project_id")?,
                release_id: row.try_get("parent_release_id")?,
            });
    }
    let rows = sqlx::query("SELECT ref.content_entry_id, ref.provider, ref.provider_project_id, ref.provider_release_id, ref.provider_file_id, ref.is_origin FROM instance_content_provider_refs ref JOIN instance_content_entries entry ON entry.id = ref.content_entry_id WHERE entry.content_set_id = ? ORDER BY ref.content_entry_id, ref.is_origin DESC, ref.provider, ref.provider_project_id")
        .bind(&content_set.id).fetch_all(&mut *tx).await?;
    let mut references = HashMap::<String, Vec<ContentProviderRef>>::new();
    let mut origins = HashMap::new();
    for row in rows {
        let entry: String = row.try_get("content_entry_id")?;
        let reference = ContentProviderRef::from_database(
            row.try_get("provider")?,
            row.try_get("provider_project_id")?,
            row.try_get::<Option<String>, _>("provider_release_id")?
                .as_deref(),
            row.try_get::<Option<String>, _>("provider_file_id")?
                .as_deref(),
        );
        match reference {
            Ok(reference) => {
                if row.try_get::<bool, _>("is_origin")? {
                    origins.insert(entry.clone(), reference.provider());
                }
                references.entry(entry).or_default().push(reference);
            }
            Err(error) => {
                tracing::warn!(%entry, %error, "Skipping invalid content provider reference")
            }
        }
    }
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT exact.entry_id, exact.artifact FROM resource_installed_artifacts exact JOIN instance_content_entries entry ON entry.id = exact.entry_id WHERE entry.content_set_id = ?")
        .bind(&content_set.id).fetch_all(&mut *tx).await?;
    let artifacts: HashMap<String, crate::resources::installed::InstalledArtifactRecord> = rows.into_iter().filter_map(|(entry, payload)| {
        match serde_json::from_str(&payload) {
            Ok(record) => Some((entry, record)),
            Err(error) => { tracing::warn!(%entry, %error, "Skipping invalid installed artifact"); None }
        }
    }).collect();
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT entry.id, COALESCE(MAX(CASE WHEN ref.is_origin = 1 THEN link.resource_id END), MIN(link.resource_id)) FROM instance_content_entries entry JOIN instance_content_provider_refs ref ON ref.content_entry_id = entry.id LEFT JOIN resource_entry_links link ON link.reference_id = ref.id WHERE entry.content_set_id = ? GROUP BY entry.id HAVING (MAX(ref.is_origin) = 0 AND COUNT(link.resource_id) = COUNT(*) AND COUNT(DISTINCT link.resource_id) = 1) OR (MAX(ref.is_origin) = 1 AND SUM(CASE WHEN ref.is_origin = 1 AND link.resource_id IS NOT NULL THEN 1 ELSE 0 END) = 1)")
        .bind(&content_set.id).fetch_all(&mut *tx).await?;
    let mut entry_resources = resource_ids(rows);
    let mut projects = HashMap::new();
    for (entry, record) in &artifacts {
        if entry_resources.contains_key(entry) {
            continue;
        }
        let project = &record.artifact.reference.release.project;
        if !projects.contains_key(project) {
            let key =
                super::resource_source_projects::ResourceSourceProjectKey {
                    source: project.source.as_str().into(),
                    project_id: project.project_id.as_str().into(),
                };
            let resource =
                super::resource_source_projects::ResourceSourceProjects::read(
                    &mut tx, &key,
                )
                .await?
                .and_then(|record| ResourceId::new(record.resource_id).ok());
            projects.insert(project.clone(), resource);
        }
        if let Some(Some(resource)) = projects.get(project) {
            entry_resources.insert(entry.clone(), resource.clone());
        }
    }
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT link.member_id, link.resource_id FROM resource_pack_links link JOIN instance_pack_members member ON member.id = link.member_id WHERE member.content_set_id = ?")
        .bind(&content_set.id).fetch_all(&mut *tx).await?;
    let pack_resources = resource_ids(rows);
    let pack_resource: Option<String> = sqlx::query_scalar("SELECT resource_id FROM resource_instance_pack_links WHERE instance_id = ?")
        .bind(instance_id).fetch_optional(&mut *tx).await?;
    let pack_resource = pack_resource.and_then(|id| ResourceId::new(id).ok());
    let preferred: String = sqlx::query_scalar(
        "SELECT preferred_source FROM resource_preferences WHERE id = 1",
    )
    .fetch_one(&mut *tx)
    .await?;
    let preferred_source =
        SourceId::new(preferred).unwrap_or_else(|_| SourceId::modrinth());
    let checks =
        super::content_refresh_checks::ContentRefreshChecks::read_scoped(
            &mut tx,
            &instance_id.to_owned(),
        )
        .await?;
    let fingerprints =
        super::content_file_fingerprints::ContentFileFingerprints::read_scoped(
            &mut tx,
            &instance_id.to_owned(),
        )
        .await?
        .into_iter()
        .map(|(key, record)| (key.file_id, record))
        .collect();
    let presentations = super::content_refresh_presentations::ContentRefreshPresentations::read_scoped(&mut tx, &instance_id.to_owned()).await?
        .into_iter().map(|(key, record)| (key.file_id, record)).collect();
    tx.commit().await?;
    Ok(SnapshotRecords {
        pack_presentation: None,
        fingerprints,
        presentations,
        instance,
        content_set,
        link,
        files,
        entries,
        members,
        pending,
        dependencies,
        references,
        origins,
        artifacts,
        entry_resources,
        pack_resources,
        pack_resource,
        preferred_source,
        checks,
    })
}

/// The same snapshot with the saved pack metadata, which lives in the derived
/// database: those rows left this one, so they are read beside the transaction
/// that reads the rest instead of inside it.
pub(crate) async fn get_with_saved_pack(
    handle: &Handle,
    derived: &super::derived_db::DerivedDb,
    instance_id: &str,
) -> crate::Result<SnapshotRecords> {
    let mut records = get(handle, instance_id).await?;
    records.pack_presentation =
        super::content_refresh_pack::ContentRefreshPacks::saved(
            derived,
            &super::content_refresh_pack::PackKey {
                instance_id: instance_id.to_string(),
                content_set_id: records.content_set.id.clone(),
            },
        )
        .await?;
    Ok(records)
}

fn resource_ids(rows: Vec<(String, String)>) -> HashMap<String, ResourceId> {
    rows.into_iter().filter_map(|(key, value)| match ResourceId::new(value) {
        Ok(id) => Some((key, id)),
        Err(error) => { tracing::warn!(%key, %error, "Skipping invalid resource identity"); None }
    }).collect()
}

pub(crate) async fn get_instance(
    conn: &mut sqlx::SqliteConnection,
    instance_id: &str,
) -> crate::Result<Instance> {
    let row: instance_rows::InstanceRow =
        sqlx::query_as("SELECT * FROM instances WHERE id = ?")
            .bind(instance_id)
            .fetch_optional(&mut *conn)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError("Unknown instance".into())
            })?;
    let mut instance: Instance = row.try_into()?;
    let direct =
        instance_rows::get_direct_link_fields(instance_id, &mut *conn).await?;
    instance.linked_launcher = direct.launcher;
    instance.linked_launcher_root = direct.launcher_root;
    instance.linked_dot_minecraft = direct.dot_minecraft;
    instance.linked_version_id = direct.version_id;
    instance.linked_version_json_path = direct.version_json_path;
    instance.linked_game_dir_mode = direct.game_dir_mode;
    Ok(instance)
}

/// A point a test can pause the snapshot at, between the read of its revision
/// and the read of everything else.
///
/// A test build only: the snapshot has no way to stop partway otherwise.
#[cfg(test)]
pub(crate) mod pause {
    use std::cell::RefCell;
    use std::future::Future;
    use std::pin::Pin;

    /// What a test wants to happen in the gap.
    ///
    /// `Send` so a caller that reads a snapshot from a spawned task still has a
    /// `Send` future while the pause is in place.
    pub(crate) type Between = Pin<Box<dyn Future<Output = ()> + Send>>;

    thread_local! {
        /// The pause this thread put in place, if any.
        static PAUSED: RefCell<Option<Between>> = const { RefCell::new(None) };
    }

    /// Waits for whatever a test put in place, once.
    pub(crate) async fn between_reads() {
        let paused = PAUSED.with_borrow_mut(Option::take);
        if let Some(paused) = paused {
            paused.await;
        }
    }

    /// Puts a pause in place for as long as the guard lives.
    pub(crate) fn pause(between: Between) -> Pause {
        PAUSED.with_borrow_mut(|slot| *slot = Some(between));
        Pause
    }

    /// Takes the pause back, so a test that ends early leaves none behind for
    /// the next test on this thread.
    pub(crate) struct Pause;

    impl Drop for Pause {
        fn drop(&mut self) {
            PAUSED.with_borrow_mut(|slot| *slot = None);
        }
    }
}
