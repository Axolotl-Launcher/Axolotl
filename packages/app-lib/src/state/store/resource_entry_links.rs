//! The provider references present entries carry, resolved through the
//! resource identity graph.
//!
//! The rows behind `resource_entry_links` belong to the instance half, and the
//! entry's own references to the resource layer; this module reads them
//! together, because what the resource layer needs is the pair.

use super::Handle;

/// One reference of one entry: the resource the project maps to, the project
/// it names, and the release and file the reference carries when it has them.
///
/// The rows arrive origin first, so the first reference of an entry is the one
/// the entry was installed from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryLinkRow {
    pub entry_id: String,
    pub resource_id: String,
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
        String,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        "
            SELECT link.entry_id, link.resource_id, link.source,
                link.project_id, link.release_id, link.artifact_id
            FROM resource_entry_links link
            JOIN instance_content_entries entry ON entry.id = link.entry_id
            WHERE entry.instance_id = ?
            ORDER BY link.is_origin DESC
            ",
    )
    .bind(instance_id)
    .fetch_all(handle.pool())
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                entry_id,
                resource_id,
                source,
                project_id,
                release_id,
                artifact_id,
            )| EntryLinkRow {
                entry_id,
                resource_id,
                source,
                project_id,
                release_id,
                artifact_id,
            },
        )
        .collect())
}

/// One reference an entry names, by the file it installs: the project, and the
/// release and file the reference carries when it has them.
pub(crate) type KnownReferenceRow =
    (String, String, String, Option<String>, Option<String>);

/// The references the instance's present entries name, from the identity graph
/// and from the provider references of entries the graph does not know.
pub(crate) async fn known_references(
    handle: &Handle,
    instance_id: &str,
) -> crate::Result<Vec<KnownReferenceRow>> {
    let rows: Vec<KnownReferenceRow> = sqlx::query_as(
        "
        SELECT file.relative_path, link.source, link.project_id,
            link.release_id, link.artifact_id
        FROM resource_entry_links link
        JOIN instance_content_entries entry ON entry.id = link.entry_id
        JOIN instance_files file ON file.id = entry.file_id
        JOIN instances instance ON instance.id = entry.instance_id
            AND instance.applied_content_set_id = entry.content_set_id
        WHERE entry.instance_id = ?
        UNION ALL
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
    .bind(instance_id)
    .fetch_all(handle.pool())
    .await?;

    Ok(rows)
}
