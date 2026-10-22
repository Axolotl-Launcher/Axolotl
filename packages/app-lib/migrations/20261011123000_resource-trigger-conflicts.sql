DROP TRIGGER resource_instance_content_provider_refs_insert;
CREATE TRIGGER resource_instance_content_provider_refs_insert
AFTER INSERT ON instance_content_provider_refs
BEGIN
    INSERT INTO resource_source_projects(source, project_id)
    SELECT DISTINCT legacy.source, legacy.project_id FROM resource_legacy_projects legacy
    WHERE legacy.source = NEW.provider AND legacy.project_id = NEW.provider_project_id
        AND NOT EXISTS (SELECT 1 FROM resource_source_projects known
            WHERE known.source = legacy.source AND known.project_id = legacy.project_id);
    INSERT INTO resources(id)
    SELECT project.resource_id FROM resource_source_projects project
    WHERE project.source = NEW.provider AND project.project_id = NEW.provider_project_id
        AND NOT EXISTS (SELECT 1 FROM resources known WHERE known.id = project.resource_id);
    INSERT INTO resource_source_types(source, project_id, content_type)
    SELECT legacy.source, legacy.project_id, legacy.content_type FROM resource_legacy_projects legacy
    WHERE legacy.source = NEW.provider AND legacy.project_id = NEW.provider_project_id
        AND NOT EXISTS (SELECT 1 FROM resource_source_types known
            WHERE known.source = legacy.source AND known.project_id = legacy.project_id AND known.content_type = legacy.content_type);
END;

DROP TRIGGER resource_instance_content_provider_refs_update;
CREATE TRIGGER resource_instance_content_provider_refs_update
AFTER UPDATE ON instance_content_provider_refs
BEGIN
    INSERT INTO resource_source_projects(source, project_id)
    SELECT DISTINCT legacy.source, legacy.project_id FROM resource_legacy_projects legacy
    WHERE legacy.source = NEW.provider AND legacy.project_id = NEW.provider_project_id
        AND NOT EXISTS (SELECT 1 FROM resource_source_projects known
            WHERE known.source = legacy.source AND known.project_id = legacy.project_id);
    INSERT INTO resources(id)
    SELECT project.resource_id FROM resource_source_projects project
    WHERE project.source = NEW.provider AND project.project_id = NEW.provider_project_id
        AND NOT EXISTS (SELECT 1 FROM resources known WHERE known.id = project.resource_id);
    INSERT INTO resource_source_types(source, project_id, content_type)
    SELECT legacy.source, legacy.project_id, legacy.content_type FROM resource_legacy_projects legacy
    WHERE legacy.source = NEW.provider AND legacy.project_id = NEW.provider_project_id
        AND NOT EXISTS (SELECT 1 FROM resource_source_types known
            WHERE known.source = legacy.source AND known.project_id = legacy.project_id AND known.content_type = legacy.content_type);
END;

DROP TRIGGER resource_content_favorites_insert;
CREATE TRIGGER resource_content_favorites_insert
AFTER INSERT ON content_favorites
BEGIN
    INSERT INTO resource_source_projects(source, project_id)
    SELECT DISTINCT legacy.source, legacy.project_id FROM resource_legacy_projects legacy
    WHERE legacy.source = NEW.provider AND legacy.project_id = NEW.project_id
        AND NOT EXISTS (SELECT 1 FROM resource_source_projects known
            WHERE known.source = legacy.source AND known.project_id = legacy.project_id);
    INSERT INTO resources(id)
    SELECT project.resource_id FROM resource_source_projects project
    WHERE project.source = NEW.provider AND project.project_id = NEW.project_id
        AND NOT EXISTS (SELECT 1 FROM resources known WHERE known.id = project.resource_id);
    INSERT INTO resource_source_types(source, project_id, content_type)
    SELECT legacy.source, legacy.project_id, legacy.content_type FROM resource_legacy_projects legacy
    WHERE legacy.source = NEW.provider AND legacy.project_id = NEW.project_id
        AND NOT EXISTS (SELECT 1 FROM resource_source_types known
            WHERE known.source = legacy.source AND known.project_id = legacy.project_id AND known.content_type = legacy.content_type);
END;

DROP TRIGGER resource_content_favorites_update;
CREATE TRIGGER resource_content_favorites_update
AFTER UPDATE ON content_favorites
BEGIN
    INSERT INTO resource_source_projects(source, project_id)
    SELECT DISTINCT legacy.source, legacy.project_id FROM resource_legacy_projects legacy
    WHERE legacy.source = NEW.provider AND legacy.project_id = NEW.project_id
        AND NOT EXISTS (SELECT 1 FROM resource_source_projects known
            WHERE known.source = legacy.source AND known.project_id = legacy.project_id);
    INSERT INTO resources(id)
    SELECT project.resource_id FROM resource_source_projects project
    WHERE project.source = NEW.provider AND project.project_id = NEW.project_id
        AND NOT EXISTS (SELECT 1 FROM resources known WHERE known.id = project.resource_id);
    INSERT INTO resource_source_types(source, project_id, content_type)
    SELECT legacy.source, legacy.project_id, legacy.content_type FROM resource_legacy_projects legacy
    WHERE legacy.source = NEW.provider AND legacy.project_id = NEW.project_id
        AND NOT EXISTS (SELECT 1 FROM resource_source_types known
            WHERE known.source = legacy.source AND known.project_id = legacy.project_id AND known.content_type = legacy.content_type);
END;

DROP TRIGGER resource_instance_pack_members_insert;
CREATE TRIGGER resource_instance_pack_members_insert
AFTER INSERT ON instance_pack_members
BEGIN
    INSERT INTO resource_source_projects(source, project_id)
    SELECT DISTINCT legacy.source, legacy.project_id FROM resource_legacy_projects legacy
    WHERE legacy.source = NEW.provider AND legacy.project_id = NEW.provider_project_id
        AND NOT EXISTS (SELECT 1 FROM resource_source_projects known
            WHERE known.source = legacy.source AND known.project_id = legacy.project_id);
    INSERT INTO resources(id)
    SELECT project.resource_id FROM resource_source_projects project
    WHERE project.source = NEW.provider AND project.project_id = NEW.provider_project_id
        AND NOT EXISTS (SELECT 1 FROM resources known WHERE known.id = project.resource_id);
    INSERT INTO resource_source_types(source, project_id, content_type)
    SELECT legacy.source, legacy.project_id, legacy.content_type FROM resource_legacy_projects legacy
    WHERE legacy.source = NEW.provider AND legacy.project_id = NEW.provider_project_id
        AND NOT EXISTS (SELECT 1 FROM resource_source_types known
            WHERE known.source = legacy.source AND known.project_id = legacy.project_id AND known.content_type = legacy.content_type);
END;

DROP TRIGGER resource_instance_pack_members_update;
CREATE TRIGGER resource_instance_pack_members_update
AFTER UPDATE ON instance_pack_members
BEGIN
    INSERT INTO resource_source_projects(source, project_id)
    SELECT DISTINCT legacy.source, legacy.project_id FROM resource_legacy_projects legacy
    WHERE legacy.source = NEW.provider AND legacy.project_id = NEW.provider_project_id
        AND NOT EXISTS (SELECT 1 FROM resource_source_projects known
            WHERE known.source = legacy.source AND known.project_id = legacy.project_id);
    INSERT INTO resources(id)
    SELECT project.resource_id FROM resource_source_projects project
    WHERE project.source = NEW.provider AND project.project_id = NEW.provider_project_id
        AND NOT EXISTS (SELECT 1 FROM resources known WHERE known.id = project.resource_id);
    INSERT INTO resource_source_types(source, project_id, content_type)
    SELECT legacy.source, legacy.project_id, legacy.content_type FROM resource_legacy_projects legacy
    WHERE legacy.source = NEW.provider AND legacy.project_id = NEW.provider_project_id
        AND NOT EXISTS (SELECT 1 FROM resource_source_types known
            WHERE known.source = legacy.source AND known.project_id = legacy.project_id AND known.content_type = legacy.content_type);
END;

DROP TRIGGER resource_instance_pending_manual_downloads_insert;
CREATE TRIGGER resource_instance_pending_manual_downloads_insert
AFTER INSERT ON instance_pending_manual_downloads
BEGIN
    INSERT INTO resource_source_projects(source, project_id)
    SELECT DISTINCT legacy.source, legacy.project_id FROM resource_legacy_projects legacy
    WHERE legacy.source = NEW.provider AND legacy.project_id = NEW.provider_project_id
        AND NOT EXISTS (SELECT 1 FROM resource_source_projects known
            WHERE known.source = legacy.source AND known.project_id = legacy.project_id);
    INSERT INTO resources(id)
    SELECT project.resource_id FROM resource_source_projects project
    WHERE project.source = NEW.provider AND project.project_id = NEW.provider_project_id
        AND NOT EXISTS (SELECT 1 FROM resources known WHERE known.id = project.resource_id);
    INSERT INTO resource_source_types(source, project_id, content_type)
    SELECT legacy.source, legacy.project_id, legacy.content_type FROM resource_legacy_projects legacy
    WHERE legacy.source = NEW.provider AND legacy.project_id = NEW.provider_project_id
        AND NOT EXISTS (SELECT 1 FROM resource_source_types known
            WHERE known.source = legacy.source AND known.project_id = legacy.project_id AND known.content_type = legacy.content_type);
END;

DROP TRIGGER resource_instance_pending_manual_downloads_update;
CREATE TRIGGER resource_instance_pending_manual_downloads_update
AFTER UPDATE ON instance_pending_manual_downloads
BEGIN
    INSERT INTO resource_source_projects(source, project_id)
    SELECT DISTINCT legacy.source, legacy.project_id FROM resource_legacy_projects legacy
    WHERE legacy.source = NEW.provider AND legacy.project_id = NEW.provider_project_id
        AND NOT EXISTS (SELECT 1 FROM resource_source_projects known
            WHERE known.source = legacy.source AND known.project_id = legacy.project_id);
    INSERT INTO resources(id)
    SELECT project.resource_id FROM resource_source_projects project
    WHERE project.source = NEW.provider AND project.project_id = NEW.provider_project_id
        AND NOT EXISTS (SELECT 1 FROM resources known WHERE known.id = project.resource_id);
    INSERT INTO resource_source_types(source, project_id, content_type)
    SELECT legacy.source, legacy.project_id, legacy.content_type FROM resource_legacy_projects legacy
    WHERE legacy.source = NEW.provider AND legacy.project_id = NEW.provider_project_id
        AND NOT EXISTS (SELECT 1 FROM resource_source_types known
            WHERE known.source = legacy.source AND known.project_id = legacy.project_id AND known.content_type = legacy.content_type);
END;

DROP TRIGGER resource_instance_links_insert;
CREATE TRIGGER resource_instance_links_insert
AFTER INSERT ON instance_links
BEGIN
    INSERT INTO resource_source_projects(source, project_id)
    SELECT DISTINCT legacy.source, legacy.project_id FROM resource_legacy_projects legacy
    JOIN resource_instance_pack_projects pack ON pack.source = legacy.source AND pack.project_id = legacy.project_id
    WHERE pack.instance_id = NEW.instance_id
        AND NOT EXISTS (SELECT 1 FROM resource_source_projects known
            WHERE known.source = legacy.source AND known.project_id = legacy.project_id);
    INSERT INTO resources(id)
    SELECT project.resource_id FROM resource_source_projects project
    JOIN resource_instance_pack_projects pack ON pack.source = project.source AND pack.project_id = project.project_id
    WHERE pack.instance_id = NEW.instance_id
        AND NOT EXISTS (SELECT 1 FROM resources known WHERE known.id = project.resource_id);
    INSERT INTO resource_source_types(source, project_id, content_type)
    SELECT legacy.source, legacy.project_id, legacy.content_type FROM resource_legacy_projects legacy
    JOIN resource_instance_pack_projects pack ON pack.source = legacy.source AND pack.project_id = legacy.project_id
    WHERE pack.instance_id = NEW.instance_id
        AND NOT EXISTS (SELECT 1 FROM resource_source_types known
            WHERE known.source = legacy.source AND known.project_id = legacy.project_id AND known.content_type = legacy.content_type);
END;

DROP TRIGGER resource_instance_links_update;
CREATE TRIGGER resource_instance_links_update
AFTER UPDATE ON instance_links
BEGIN
    INSERT INTO resource_source_projects(source, project_id)
    SELECT DISTINCT legacy.source, legacy.project_id FROM resource_legacy_projects legacy
    JOIN resource_instance_pack_projects pack ON pack.source = legacy.source AND pack.project_id = legacy.project_id
    WHERE pack.instance_id = NEW.instance_id
        AND NOT EXISTS (SELECT 1 FROM resource_source_projects known
            WHERE known.source = legacy.source AND known.project_id = legacy.project_id);
    INSERT INTO resources(id)
    SELECT project.resource_id FROM resource_source_projects project
    JOIN resource_instance_pack_projects pack ON pack.source = project.source AND pack.project_id = project.project_id
    WHERE pack.instance_id = NEW.instance_id
        AND NOT EXISTS (SELECT 1 FROM resources known WHERE known.id = project.resource_id);
    INSERT INTO resource_source_types(source, project_id, content_type)
    SELECT legacy.source, legacy.project_id, legacy.content_type FROM resource_legacy_projects legacy
    JOIN resource_instance_pack_projects pack ON pack.source = legacy.source AND pack.project_id = legacy.project_id
    WHERE pack.instance_id = NEW.instance_id
        AND NOT EXISTS (SELECT 1 FROM resource_source_types known
            WHERE known.source = legacy.source AND known.project_id = legacy.project_id AND known.content_type = legacy.content_type);
END;
