CREATE TABLE resource_favorites (
    source TEXT NOT NULL,
    project_id TEXT NOT NULL,
    content_type TEXT NOT NULL,
    saved_at INTEGER NOT NULL,
    PRIMARY KEY (source, project_id),
    FOREIGN KEY (source, project_id) REFERENCES resource_source_projects(source, project_id)
);

INSERT INTO resource_favorites(source, project_id, content_type, saved_at)
SELECT favorite.provider, favorite.project_id, favorite.content_type, favorite.saved_at
FROM content_favorites AS favorite
JOIN resource_source_projects AS project
    ON project.source = favorite.provider AND project.project_id = favorite.project_id;
