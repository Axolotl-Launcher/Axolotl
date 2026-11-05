CREATE TABLE model_turns (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    attempt_id TEXT NOT NULL REFERENCES attempts(id) ON DELETE CASCADE,
    request_id TEXT NOT NULL,
    input TEXT NOT NULL CHECK(length(CAST(input AS BLOB)) <= 16777216),
    reply TEXT CHECK(reply IS NULL OR length(CAST(reply AS BLOB)) <= 16777216),
    UNIQUE(attempt_id, request_id)
);
