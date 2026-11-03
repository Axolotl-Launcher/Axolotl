-- content_favorites was replaced by resource_favorites and backfilled in
-- 20261011140000, processes lost its last reader, and telemetry_error_daily
-- stopped being written when error telemetry was disabled.
DROP TRIGGER IF EXISTS resource_content_favorites_insert;
DROP TRIGGER IF EXISTS resource_content_favorites_update;

-- The views name content_favorites, so they are rebuilt without it: the
-- identity triggers read resource_legacy_projects on every content write.
DROP VIEW IF EXISTS resource_legacy_projects;
DROP VIEW IF EXISTS resource_favorite_links;
DROP TABLE IF EXISTS content_favorites;

CREATE VIEW resource_legacy_projects AS
SELECT DISTINCT source, project_id, content_type FROM (
    SELECT ref.provider AS source, ref.provider_project_id AS project_id, entry.project_type AS content_type
    FROM instance_content_provider_refs ref
    JOIN instance_content_entries entry ON entry.id = ref.content_entry_id
    UNION ALL
    SELECT provider, provider_project_id, project_type FROM instance_pack_members
    UNION ALL
    SELECT provider, provider_project_id, project_type FROM instance_pending_manual_downloads
    UNION ALL
    SELECT source, project_id, 'modpack' FROM resource_instance_pack_projects
)
WHERE
    (source = 'modrinth' AND length(project_id) = 8 AND project_id NOT GLOB '*[^a-zA-Z0-9]*')
    OR (source = 'curseforge' AND project_id NOT GLOB '*[^0-9]*'
        AND CAST(project_id AS INTEGER) BETWEEN 1 AND 4294967295
        AND CAST(CAST(project_id AS INTEGER) AS TEXT) = project_id)
    OR (source = 'mcarchive' AND length(project_id) = 36
        AND substr(project_id, 9, 1) = '-' AND substr(project_id, 14, 1) = '-'
        AND substr(project_id, 19, 1) = '-' AND substr(project_id, 24, 1) = '-'
        AND length(replace(project_id, '-', '')) = 32
        AND replace(project_id, '-', '') NOT GLOB '*[^a-fA-F0-9]*');

CREATE VIEW resource_favorite_links AS
SELECT favorite.source AS source, favorite.project_id, project.resource_id,
    favorite.content_type, favorite.saved_at
FROM resource_favorites favorite
JOIN resource_source_projects project
    ON project.source = favorite.source AND project.project_id = favorite.project_id;

DROP TABLE IF EXISTS processes;
DROP TABLE IF EXISTS telemetry_error_daily;
