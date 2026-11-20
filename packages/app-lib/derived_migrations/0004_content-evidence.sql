CREATE TABLE content_evidence (
    kind TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    record_id TEXT NOT NULL,
    guard TEXT NOT NULL,
    payload TEXT NOT NULL,
    PRIMARY KEY (kind, instance_id, record_id)
);
CREATE INDEX content_evidence_instance ON content_evidence(instance_id);
