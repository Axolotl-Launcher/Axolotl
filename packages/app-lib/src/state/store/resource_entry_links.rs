//! Installed provider references with optional cross-platform identities.

use super::Handle;
use crate::state::instances::adapters::sqlite::resource_rows::{
    ResourceStore, project_key,
};

/// One reference of one entry: the resource the project maps to, the project
/// it names, and the release and file the reference carries when it has them.
///
/// The rows arrive origin first, so the first reference of an entry is the one
/// the entry was installed from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryLinkRow {
    pub entry_id: String,
    pub resource_id: Option<String>,
    pub source: String,
    pub project_id: String,
    pub release_id: Option<String>,
    pub artifact_id: Option<String>,
}

/// Every present entry's references, origin first.
pub(crate) async fn for_instance(
    handle: &Handle,
    instance_id: &str,
) -> crate::Result<Vec<EntryLinkRow>> {
    let rows: Vec<(
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        "
            SELECT ref.content_entry_id, ref.provider, ref.provider_project_id,
                ref.provider_release_id, ref.provider_file_id
            FROM instance_content_provider_refs ref
            JOIN instance_content_entries entry ON entry.id = ref.content_entry_id
            WHERE entry.instance_id = ?
            ORDER BY ref.is_origin DESC, ref.provider, ref.provider_project_id
            ",
    )
    .bind(instance_id)
    .fetch_all(handle.pool())
    .await?;

    let projects = rows
        .iter()
        .filter_map(|(_, source, id, _, _)| project_key(source, id))
        .collect::<Vec<_>>();
    let identities = ResourceStore::new(handle.pool().clone())
        .enrich_many(&projects)
        .await;
    Ok(rows
        .into_iter()
        .filter_map(
            |(entry_id, source, project_id, release_id, artifact_id)| {
                let project = project_key(&source, &project_id)?;
                let resource_id =
                    identities.get(&project).map(|id| id.as_str().to_owned());
                Some(EntryLinkRow {
                    entry_id,
                    resource_id,
                    source,
                    project_id,
                    release_id,
                    artifact_id,
                })
            },
        )
        .collect())
}

/// One reference an entry names, by the file it installs: the project, and the
/// release and file the reference carries when it has them.
pub(crate) type KnownReferenceRow =
    (String, String, String, Option<String>, Option<String>);

/// The installed files and provider references in the applied content set.
pub(crate) async fn known_references(
    handle: &Handle,
    instance_id: &str,
) -> crate::Result<Vec<KnownReferenceRow>> {
    let rows: Vec<KnownReferenceRow> = sqlx::query_as(
        "
        SELECT file.relative_path, ref.provider AS source,
            ref.provider_project_id AS project_id,
            COALESCE(ref.provider_release_id, ref.provider_file_id)
                AS release_id,
            ref.provider_file_id AS artifact_id
        FROM instance_content_provider_refs ref
        JOIN instance_content_entries entry
            ON entry.id = ref.content_entry_id
        JOIN instance_files file ON file.id = entry.file_id
        JOIN instances instance ON instance.id = entry.instance_id
            AND instance.applied_content_set_id = entry.content_set_id
        WHERE entry.instance_id = ?
        ",
    )
    .bind(instance_id)
    .fetch_all(handle.pool())
    .await?;

    Ok(rows)
}
