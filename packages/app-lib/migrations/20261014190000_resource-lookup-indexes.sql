CREATE INDEX resource_pending_download_project
ON instance_pending_manual_downloads(provider, provider_project_id, project_type);

CREATE INDEX resource_link_modrinth_project
ON instance_links(link_kind, modrinth_project_id);

CREATE INDEX resource_link_content_project
ON instance_links(link_kind, content_project_id);

CREATE INDEX resource_link_curseforge_project
ON instance_links(link_kind, CAST(curseforge_project_id AS TEXT));
