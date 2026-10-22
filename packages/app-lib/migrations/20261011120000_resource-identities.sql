CREATE TABLE resources (
    id TEXT PRIMARY KEY NOT NULL,
    redirect_to TEXT REFERENCES resources(id),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    CHECK (redirect_to IS NULL OR redirect_to <> id)
);

CREATE TABLE resource_source_projects (
    source TEXT NOT NULL,
    project_id TEXT NOT NULL,
    resource_id TEXT NOT NULL DEFAULT (lower(hex(randomblob(16)))),
    PRIMARY KEY (source, project_id),
    FOREIGN KEY (resource_id) REFERENCES resources(id) DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX resource_source_projects_resource ON resource_source_projects(resource_id);

CREATE TABLE resource_source_types (
    source TEXT NOT NULL,
    project_id TEXT NOT NULL,
    content_type TEXT NOT NULL,
    PRIMARY KEY (source, project_id, content_type),
    FOREIGN KEY (source, project_id) REFERENCES resource_source_projects(source, project_id)
);

CREATE TABLE resource_aliases (
    source TEXT NOT NULL,
    alias TEXT NOT NULL,
    project_id TEXT NOT NULL,
    PRIMARY KEY (source, alias, project_id),
    FOREIGN KEY (source, project_id) REFERENCES resource_source_projects(source, project_id)
);

CREATE TABLE resource_identity_evidence (
    evidence_id TEXT PRIMARY KEY NOT NULL,
    resource_id TEXT NOT NULL REFERENCES resources(id),
    payload TEXT NOT NULL,
    confirmed_at INTEGER NOT NULL
);

CREATE TABLE resource_preferences (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    preferred_source TEXT NOT NULL
);
INSERT INTO resource_preferences VALUES (1, 'modrinth');

CREATE VIEW resource_instance_pack_projects AS
SELECT instance_id, 'modrinth' AS source, modrinth_project_id AS project_id, modrinth_version_id AS release_id
FROM instance_links WHERE link_kind = 'modrinth_modpack'
UNION ALL
SELECT instance_id, 'modrinth', content_project_id, content_version_id
FROM instance_links WHERE link_kind = 'server_project_modpack'
UNION ALL
SELECT instance_id, 'curseforge', CAST(curseforge_project_id AS TEXT), CAST(curseforge_file_id AS TEXT)
FROM instance_links WHERE link_kind = 'curseforge_modpack';

CREATE VIEW resource_legacy_projects AS
SELECT DISTINCT source, project_id, content_type FROM (
    SELECT ref.provider AS source, ref.provider_project_id AS project_id, entry.project_type AS content_type
    FROM instance_content_provider_refs ref
    JOIN instance_content_entries entry ON entry.id = ref.content_entry_id
    UNION ALL
    SELECT provider, project_id, content_type FROM content_favorites
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

INSERT INTO resource_source_projects(source, project_id)
SELECT DISTINCT source, project_id FROM resource_legacy_projects;
INSERT INTO resources(id) SELECT resource_id FROM resource_source_projects;
INSERT INTO resource_source_types(source, project_id, content_type)
SELECT source, project_id, content_type FROM resource_legacy_projects;

CREATE VIEW resource_entry_links AS
SELECT ref.id AS reference_id, ref.content_entry_id AS entry_id, ref.is_origin,
    project.resource_id, ref.provider AS source, ref.provider_project_id AS project_id,
    ref.provider_release_id AS release_id, ref.provider_file_id AS artifact_id
FROM instance_content_provider_refs ref
JOIN resource_source_projects project
    ON project.source = ref.provider AND project.project_id = ref.provider_project_id;

CREATE VIEW resource_favorite_links AS
SELECT favorite.provider AS source, favorite.project_id, project.resource_id,
    favorite.content_type, favorite.saved_at
FROM content_favorites favorite
JOIN resource_source_projects project
    ON project.source = favorite.provider AND project.project_id = favorite.project_id;

CREATE VIEW resource_pack_links AS
SELECT member.id AS member_id, project.resource_id
FROM instance_pack_members member
JOIN resource_source_projects project
    ON project.source = member.provider AND project.project_id = member.provider_project_id;

CREATE VIEW resource_instance_pack_links AS
SELECT pack.instance_id, project.resource_id
FROM resource_instance_pack_projects pack
JOIN resource_source_projects project ON project.source = pack.source AND project.project_id = pack.project_id;

CREATE TRIGGER resource_instance_links_insert
AFTER INSERT ON instance_links
BEGIN
    INSERT OR IGNORE INTO resource_source_projects(source, project_id)
    SELECT legacy.source, legacy.project_id FROM resource_legacy_projects legacy
    JOIN resource_instance_pack_projects pack ON pack.source = legacy.source AND pack.project_id = legacy.project_id
    WHERE pack.instance_id = NEW.instance_id;
    INSERT OR IGNORE INTO resources(id)
    SELECT project.resource_id FROM resource_source_projects project
    JOIN resource_instance_pack_projects pack ON pack.source = project.source AND pack.project_id = project.project_id
    WHERE pack.instance_id = NEW.instance_id;
    INSERT OR IGNORE INTO resource_source_types(source, project_id, content_type)
    SELECT legacy.source, legacy.project_id, legacy.content_type FROM resource_legacy_projects legacy
    JOIN resource_instance_pack_projects pack ON pack.source = legacy.source AND pack.project_id = legacy.project_id
    WHERE pack.instance_id = NEW.instance_id;
END;

CREATE TRIGGER resource_instance_links_update
AFTER UPDATE ON instance_links
BEGIN
    INSERT OR IGNORE INTO resource_source_projects(source, project_id)
    SELECT legacy.source, legacy.project_id FROM resource_legacy_projects legacy
    JOIN resource_instance_pack_projects pack ON pack.source = legacy.source AND pack.project_id = legacy.project_id
    WHERE pack.instance_id = NEW.instance_id;
    INSERT OR IGNORE INTO resources(id)
    SELECT project.resource_id FROM resource_source_projects project
    JOIN resource_instance_pack_projects pack ON pack.source = project.source AND pack.project_id = project.project_id
    WHERE pack.instance_id = NEW.instance_id;
    INSERT OR IGNORE INTO resource_source_types(source, project_id, content_type)
    SELECT legacy.source, legacy.project_id, legacy.content_type FROM resource_legacy_projects legacy
    JOIN resource_instance_pack_projects pack ON pack.source = legacy.source AND pack.project_id = legacy.project_id
    WHERE pack.instance_id = NEW.instance_id;
END;

CREATE TRIGGER resource_instance_content_provider_refs_insert
AFTER INSERT ON instance_content_provider_refs
BEGIN
    INSERT OR IGNORE INTO resource_source_projects(source, project_id)
    SELECT source, project_id FROM resource_legacy_projects
    WHERE source = NEW.provider AND project_id = NEW.provider_project_id;
    INSERT OR IGNORE INTO resources(id)
    SELECT resource_id FROM resource_source_projects
    WHERE source = NEW.provider AND project_id = NEW.provider_project_id;
    INSERT OR IGNORE INTO resource_source_types(source, project_id, content_type)
    SELECT source, project_id, content_type FROM resource_legacy_projects
    WHERE source = NEW.provider AND project_id = NEW.provider_project_id;
END;

CREATE TRIGGER resource_instance_content_provider_refs_update
AFTER UPDATE ON instance_content_provider_refs
BEGIN
    INSERT OR IGNORE INTO resource_source_projects(source, project_id)
    SELECT source, project_id FROM resource_legacy_projects
    WHERE source = NEW.provider AND project_id = NEW.provider_project_id;
    INSERT OR IGNORE INTO resources(id)
    SELECT resource_id FROM resource_source_projects
    WHERE source = NEW.provider AND project_id = NEW.provider_project_id;
    INSERT OR IGNORE INTO resource_source_types(source, project_id, content_type)
    SELECT source, project_id, content_type FROM resource_legacy_projects
    WHERE source = NEW.provider AND project_id = NEW.provider_project_id;
END;

CREATE TRIGGER resource_content_favorites_insert
AFTER INSERT ON content_favorites
BEGIN
    INSERT OR IGNORE INTO resource_source_projects(source, project_id)
    SELECT source, project_id FROM resource_legacy_projects
    WHERE source = NEW.provider AND project_id = NEW.project_id;
    INSERT OR IGNORE INTO resources(id)
    SELECT resource_id FROM resource_source_projects
    WHERE source = NEW.provider AND project_id = NEW.project_id;
    INSERT OR IGNORE INTO resource_source_types(source, project_id, content_type)
    SELECT source, project_id, content_type FROM resource_legacy_projects
    WHERE source = NEW.provider AND project_id = NEW.project_id;
END;

CREATE TRIGGER resource_content_favorites_update
AFTER UPDATE ON content_favorites
BEGIN
    INSERT OR IGNORE INTO resource_source_projects(source, project_id)
    SELECT source, project_id FROM resource_legacy_projects
    WHERE source = NEW.provider AND project_id = NEW.project_id;
    INSERT OR IGNORE INTO resources(id)
    SELECT resource_id FROM resource_source_projects
    WHERE source = NEW.provider AND project_id = NEW.project_id;
    INSERT OR IGNORE INTO resource_source_types(source, project_id, content_type)
    SELECT source, project_id, content_type FROM resource_legacy_projects
    WHERE source = NEW.provider AND project_id = NEW.project_id;
END;

CREATE TRIGGER resource_instance_pack_members_insert
AFTER INSERT ON instance_pack_members
BEGIN
    INSERT OR IGNORE INTO resource_source_projects(source, project_id)
    SELECT source, project_id FROM resource_legacy_projects
    WHERE source = NEW.provider AND project_id = NEW.provider_project_id;
    INSERT OR IGNORE INTO resources(id)
    SELECT resource_id FROM resource_source_projects
    WHERE source = NEW.provider AND project_id = NEW.provider_project_id;
    INSERT OR IGNORE INTO resource_source_types(source, project_id, content_type)
    SELECT source, project_id, content_type FROM resource_legacy_projects
    WHERE source = NEW.provider AND project_id = NEW.provider_project_id;
END;

CREATE TRIGGER resource_instance_pack_members_update
AFTER UPDATE ON instance_pack_members
BEGIN
    INSERT OR IGNORE INTO resource_source_projects(source, project_id)
    SELECT source, project_id FROM resource_legacy_projects
    WHERE source = NEW.provider AND project_id = NEW.provider_project_id;
    INSERT OR IGNORE INTO resources(id)
    SELECT resource_id FROM resource_source_projects
    WHERE source = NEW.provider AND project_id = NEW.provider_project_id;
    INSERT OR IGNORE INTO resource_source_types(source, project_id, content_type)
    SELECT source, project_id, content_type FROM resource_legacy_projects
    WHERE source = NEW.provider AND project_id = NEW.provider_project_id;
END;

CREATE TRIGGER resource_instance_pending_manual_downloads_insert
AFTER INSERT ON instance_pending_manual_downloads
BEGIN
    INSERT OR IGNORE INTO resource_source_projects(source, project_id)
    SELECT source, project_id FROM resource_legacy_projects
    WHERE source = NEW.provider AND project_id = NEW.provider_project_id;
    INSERT OR IGNORE INTO resources(id)
    SELECT resource_id FROM resource_source_projects
    WHERE source = NEW.provider AND project_id = NEW.provider_project_id;
    INSERT OR IGNORE INTO resource_source_types(source, project_id, content_type)
    SELECT source, project_id, content_type FROM resource_legacy_projects
    WHERE source = NEW.provider AND project_id = NEW.provider_project_id;
END;

CREATE TRIGGER resource_instance_pending_manual_downloads_update
AFTER UPDATE ON instance_pending_manual_downloads
BEGIN
    INSERT OR IGNORE INTO resource_source_projects(source, project_id)
    SELECT source, project_id FROM resource_legacy_projects
    WHERE source = NEW.provider AND project_id = NEW.provider_project_id;
    INSERT OR IGNORE INTO resources(id)
    SELECT resource_id FROM resource_source_projects
    WHERE source = NEW.provider AND project_id = NEW.provider_project_id;
    INSERT OR IGNORE INTO resource_source_types(source, project_id, content_type)
    SELECT source, project_id, content_type FROM resource_legacy_projects
    WHERE source = NEW.provider AND project_id = NEW.provider_project_id;
END;
