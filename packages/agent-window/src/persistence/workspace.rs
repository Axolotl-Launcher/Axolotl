use super::*;
use std::collections::HashSet;

pub struct WorkspaceHead {
    pub workspace: Option<Value>,
    pub revision: i64,
}

/// Validates explicit workspace removals before the host cancels their tasks.
/// A missing row in a sanitized client projection never implies deletion.
pub fn validate_workspace_update(
    previous: Option<&Value>,
    workspace: &Value,
    deleted: &[String],
) -> Result<Vec<String>> {
    encode(workspace)?;
    if !workspace.is_object() {
        return Err(StoreError::Conflict("Invalid Agent workspace"));
    }
    let threads = workspace.get("threads").and_then(Value::as_array);
    let mut ids = HashSet::new();
    for thread in threads.into_iter().flatten() {
        let id = thread["id"]
            .as_str()
            .ok_or(StoreError::Conflict("Invalid task identity"))?;
        valid_id(id)?;
        if !ids.insert(id) {
            return Err(StoreError::Conflict("Duplicate task identity"));
        }
    }
    let prior_threads = previous.and_then(|value| value["threads"].as_array());
    let mut attempts = Vec::new();
    let mut removals = HashSet::new();
    for id in deleted {
        valid_id(id)?;
        if ids.contains(id.as_str()) || !removals.insert(id) {
            return Err(StoreError::Conflict("Invalid task removal"));
        }
        let thread = prior_threads
            .and_then(|threads| {
                threads.iter().find(|thread| thread["id"] == *id)
            })
            .ok_or(StoreError::Conflict(
                "Task removal was not in the acknowledged workspace",
            ))?;
        for turn in thread["turns"].as_array().into_iter().flatten() {
            if let Some(attempt) = turn["run"]["resume"]["attempt_id"].as_str()
            {
                valid_id(attempt)?;
                attempts.push(attempt.to_owned());
            }
        }
    }
    Ok(attempts)
}

impl AgentStore {
    pub async fn workspace_head(&self) -> Result<WorkspaceHead> {
        let row = sqlx::query(
            "SELECT payload, revision FROM workspace_state WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        match row {
            Some(row) => Ok(WorkspaceHead {
                workspace: Some(serde_json::from_str(row.get("payload"))?),
                revision: row.get("revision"),
            }),
            None => Ok(WorkspaceHead {
                workspace: None,
                revision: 0,
            }),
        }
    }

    pub async fn save_workspace(
        &self,
        workspace: &Value,
        expected: i64,
        deleted: &[String],
    ) -> Result<i64> {
        let payload = encode(workspace)?;
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT payload, revision FROM workspace_state WHERE id = 1",
        )
        .fetch_optional(&mut *tx)
        .await?;
        let revision = row
            .as_ref()
            .map(|row| row.get::<i64, _>("revision"))
            .unwrap_or(0);
        if expected < 0 || revision != expected {
            return Err(StoreError::Conflict(
                "Agent workspace changed; reload saved history before retrying",
            ));
        }
        let previous: Option<Value> = row
            .map(|row| serde_json::from_str(row.get("payload")))
            .transpose()?;
        validate_workspace_update(previous.as_ref(), workspace, deleted)?;
        for id in deleted {
            delete_thread_in(&mut tx, id).await?;
        }
        for thread in workspace["threads"].as_array().into_iter().flatten() {
            sqlx::query("INSERT INTO threads(id, payload) VALUES (?, ?) ON CONFLICT(id) DO UPDATE SET payload = excluded.payload, updated_at = unixepoch()")
                .bind(thread["id"].as_str().unwrap()).bind(encode(thread)?).execute(&mut *tx).await?;
        }
        let next = revision
            .checked_add(1)
            .ok_or(StoreError::Conflict("Workspace revision limit reached"))?;
        let updated = sqlx::query("INSERT INTO workspace_state(id, payload, revision) VALUES (1, ?, ?) ON CONFLICT(id) DO UPDATE SET payload = excluded.payload, revision = excluded.revision WHERE workspace_state.revision = ?")
            .bind(payload).bind(next).bind(expected).execute(&mut *tx).await?.rows_affected();
        if updated != 1 {
            return Err(StoreError::Conflict(
                "Agent workspace changed while saving",
            ));
        }
        tx.commit().await?;
        Ok(next)
    }
}
