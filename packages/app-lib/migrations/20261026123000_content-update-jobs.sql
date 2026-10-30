CREATE TABLE content_update_jobs (
    plan_id TEXT PRIMARY KEY NOT NULL,
    job_id TEXT REFERENCES install_jobs(id) ON DELETE SET NULL
);
