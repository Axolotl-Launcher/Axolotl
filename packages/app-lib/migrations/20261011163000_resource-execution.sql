CREATE TABLE resource_execution_journals (
    plan_id TEXT PRIMARY KEY NOT NULL REFERENCES resource_operation_plans(id) ON DELETE CASCADE,
    payload TEXT NOT NULL
);
CREATE TABLE resource_entry_graphs (
    entry_id TEXT PRIMARY KEY NOT NULL REFERENCES instance_content_entries(id) ON DELETE CASCADE,
    nodes TEXT NOT NULL,
    edges TEXT NOT NULL
);
