use super::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphWrite {
    pub task_id: String,
    pub index: i64,
    pub channel: String,
    pub value: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphCheckpoint {
    pub id: String,
    pub checkpoint: Value,
    pub metadata: Value,
    pub parent_id: Option<String>,
    #[serde(default)]
    pub writes: Vec<GraphWrite>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApprovalBatch {
    pub id: String,
    pub attempt_id: String,
    pub revision: i64,
    pub expires_at: i64,
    pub actions: Vec<Approval>,
}

impl AgentStore {
    /// Includes active runs: an SDK may catch a tool error and continue the same
    /// graph, so uncertainty cannot be scoped only to older attempts.
    pub async fn has_unsettled_writes(&self, thread_id: &str) -> Result<bool> {
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM executions e JOIN attempts a ON a.id = e.attempt_id WHERE a.thread_id = ? AND e.is_write = 1 AND e.state IN ('started', 'unknown')")
            .bind(thread_id).fetch_one(&self.pool).await?;
        Ok(count != 0)
    }

    pub async fn unsettled_writes(
        &self,
        thread_id: &str,
    ) -> Result<Vec<Value>> {
        let rows = sqlx::query("SELECT e.attempt_id, e.call_id, e.tool, e.arguments, e.result FROM executions e JOIN attempts a ON a.id = e.attempt_id WHERE a.thread_id = ? AND a.state IN ('completed', 'failed', 'cancelled', 'interrupted') AND e.is_write = 1 AND e.state IN ('started', 'unknown') ORDER BY e.rowid")
            .bind(thread_id).fetch_all(&self.pool).await?;
        rows.into_iter().map(|row| {
            let result: Option<Value> = row.get::<Option<String>, _>("result").map(|value| serde_json::from_str(&value)).transpose()?;
            Ok(serde_json::json!({"attempt_id":row.get::<String,_>("attempt_id"),"call_id":row.get::<String,_>("call_id"),"tool":row.get::<String,_>("tool"),"arguments":serde_json::from_str::<Value>(row.get("arguments"))?,"result":result.clone(),"operation_id":result.as_ref().and_then(|value|value["structured_content"]["operation_id"].as_str())}))
        }).collect()
    }

    /// Only the native executor may supply this observation, obtained directly
    /// from operations_get under the original operation owner's scope.
    pub async fn reconcile_operation(
        &self,
        attempt_id: &str,
        call_id: &str,
        observed: &Value,
    ) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let execution = get_execution(&mut tx, attempt_id, call_id)
            .await?
            .ok_or(StoreError::Conflict("Unknown execution"))?;
        let content = &observed["structured_content"];
        let operation = execution.result.as_ref().and_then(|value| {
            value["structured_content"]["operation_id"].as_str()
        });
        if operation.is_none()
            || content["operation_id"].as_str() != operation
            || content["tool"].as_str() != Some(execution.tool.as_str())
            || observed["is_error"] != false
        {
            return Err(StoreError::Conflict(
                "Operation observation does not match the persisted execution",
            ));
        }
        let state = match content["status"].as_str() {
            Some("succeeded") => "completed",
            Some("failed" | "cancelled") => "failed",
            _ => return Ok(false),
        };
        let mut result = observed.clone();
        result["is_error"] = Value::Bool(state == "failed");
        let changed = sqlx::query("UPDATE executions SET state = ?, result = ? WHERE attempt_id = ? AND call_id = ? AND state IN ('started', 'unknown')")
            .bind(state).bind(encode(&redact(result))?).bind(attempt_id).bind(call_id).execute(&mut *tx).await?.rows_affected() == 1;
        tx.commit().await?;
        Ok(changed)
    }

    /// Host-only compare-and-set of a retained execution receipt. The host must
    /// classify observations from the original native producer and its exact
    /// read-only job queries; this method does not accept worker/model evidence.
    /// `failed = None` retains an unresolved receipt and never grants replay.
    pub async fn reconcile_execution_receipt(
        &self,
        attempt_id: &str,
        call_id: &str,
        previous: &Value,
        observed: &Value,
        failed: Option<bool>,
    ) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let execution = get_execution(&mut tx, attempt_id, call_id)
            .await?
            .ok_or(StoreError::Conflict("Unknown execution"))?;
        if !matches!(execution.state.as_str(), "started" | "unknown") {
            return Ok(false);
        }
        if execution.result.as_ref() != Some(previous) {
            return Err(StoreError::Conflict(
                "Execution receipt changed during reconciliation",
            ));
        }
        if let Some(failed) = failed {
            if observed["is_error"] != failed {
                return Err(StoreError::Conflict(
                    "Completion status does not match native receipt",
                ));
            }
        }
        let state = match failed {
            Some(true) => "failed",
            Some(false) => "completed",
            None => execution.state.as_str(),
        };
        let changed = sqlx::query("UPDATE executions SET state = ?, result = ? WHERE attempt_id = ? AND call_id = ? AND state IN ('started', 'unknown')")
            .bind(state).bind(encode(&redact(observed.clone()))?).bind(attempt_id).bind(call_id)
            .execute(&mut *tx).await?.rows_affected() == 1;
        tx.commit().await?;
        Ok(changed)
    }

    /// Business completion is a projection of selected execution receipts, never
    /// of the model's final text. Missing receipts remain explicitly incomplete.
    pub async fn applied_result(&self, attempt_id: &str) -> Result<Value> {
        let live = self
            .attempt(attempt_id)
            .await?
            .is_some_and(|attempt| !attempt.state.is_terminal());
        let selections: Vec<String> = sqlx::query_scalar("SELECT selected FROM approval_batches WHERE attempt_id = ? AND selected IS NOT NULL ORDER BY rowid")
            .bind(attempt_id).fetch_all(&self.pool).await?;
        let mut selected = HashSet::new();
        for selection in selections {
            selected.extend(serde_json::from_str::<Vec<String>>(&selection)?);
        }
        let mut actions = Vec::new();
        let (mut applied, mut failed, mut unknown, mut not_executed) =
            (0, 0, 0, 0);
        for approval in self.approvals(attempt_id).await? {
            if !selected.contains(&approval.approval.call_id) {
                actions.push(serde_json::json!({"call_id":approval.approval.call_id,"tool":approval.approval.tool,"state":if approval.state == "pending" { "pending" } else { "rejected" }}));
                continue;
            }
            let execution = self
                .execution(attempt_id, &approval.approval.call_id)
                .await?;
            let state = match execution
                .as_ref()
                .map(|execution| execution.state.as_str())
            {
                Some("completed")
                    if execution
                        .as_ref()
                        .and_then(|e| e.result.as_ref())
                        .is_some_and(|r| r["is_error"] == false) =>
                {
                    applied += 1;
                    "applied"
                }
                Some("failed") => {
                    failed += 1;
                    "failed"
                }
                Some("started") if live => "running",
                Some(_) => {
                    unknown += 1;
                    "unknown"
                }
                None if live => "queued",
                None => {
                    not_executed += 1;
                    "not_executed"
                }
            };
            actions.push(serde_json::json!({"call_id":approval.approval.call_id,"tool":approval.approval.tool,"state":state}));
        }
        Ok(
            serde_json::json!({"selected":selected.len(),"applied":applied,"failed":failed,"unknown":unknown,"not_executed":not_executed,"complete":applied == selected.len(),"actions":actions}),
        )
    }

    pub async fn execution_evidence(
        &self,
        attempt_id: &str,
    ) -> Result<Vec<Value>> {
        let rows = sqlx::query("SELECT call_id, tool, arguments, is_write, state, result FROM executions WHERE attempt_id = ? ORDER BY rowid")
            .bind(attempt_id).fetch_all(&self.pool).await?;
        rows.into_iter().map(|row| {
            let arguments: Value = serde_json::from_str(row.get("arguments"))?;
            let result: Option<Value> = row.get::<Option<String>, _>("result").map(|value|serde_json::from_str(&value)).transpose()?;
            let targets = arguments.as_object().map(|args|args.iter().filter(|(key,value)| (key.ends_with("_id") || matches!(key.as_str(), "path" | "name")) && value.to_string().len() < 2048).map(|(key,value)|(key.clone(),value.clone())).collect::<serde_json::Map<_,_>>()).unwrap_or_default();
            Ok(serde_json::json!({"call_id":row.get::<String,_>("call_id"),"tool":row.get::<String,_>("tool"),"is_write":row.get::<bool,_>("is_write"),"state":row.get::<String,_>("state"),"arguments":if arguments.to_string().len() <= 8192 { Some(&arguments) } else { None },"observed":result.as_ref().map(|value|&value["structured_content"]).filter(|value|value.to_string().len() <= 8192),"targets":targets,"operation_id":result.as_ref().and_then(|value|value["structured_content"]["operation_id"].as_str())}))
        }).collect()
    }

    pub async fn ensure_thread(&self, id: &str, payload: &Value) -> Result<()> {
        valid_id(id)?;
        sqlx::query("INSERT OR IGNORE INTO threads(id, payload) VALUES (?, ?)")
            .bind(id)
            .bind(encode(payload)?)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn execution_progress(
        &self,
        attempt_id: &str,
        call_id: &str,
        result: &Value,
    ) -> Result<()> {
        sqlx::query("UPDATE executions SET result = ? WHERE attempt_id = ? AND call_id = ? AND state IN ('started', 'unknown')")
            .bind(encode(&redact(result.clone()))?).bind(attempt_id).bind(call_id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn conversation_messages(
        &self,
        thread_id: &str,
    ) -> Result<Vec<Value>> {
        let rows = sqlx::query("SELECT m.payload FROM messages m JOIN attempts a ON a.id = m.attempt_id WHERE m.thread_id = ? AND a.state = 'completed' ORDER BY m.rowid")
            .bind(thread_id).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| Ok(serde_json::from_str(row.get("payload"))?))
            .collect()
    }

    pub async fn workspace(&self) -> Result<Option<Value>> {
        Ok(self.workspace_head().await?.workspace)
    }

    pub async fn set_workspace(&self, payload: &Value) -> Result<()> {
        let head = self.workspace_head().await?;
        self.save_workspace(payload, head.revision, &[]).await?;
        Ok(())
    }

    pub async fn enqueue_attempt(
        &self,
        attempt: &Attempt,
        input: &Value,
    ) -> Result<bool> {
        valid_id(&attempt.id)?;
        valid_id(&attempt.thread_id)?;
        valid_id(&attempt.turn_id)?;
        if attempt.state != AttemptState::Queued {
            return Err(StoreError::Conflict("New jobs must be queued"));
        }
        let payload = encode(input)?;
        let mut tx = self.pool.begin().await?;
        if let Some(existing) = get_attempt(&mut tx, &attempt.id).await? {
            let original: Option<String> = sqlx::query_scalar(
                "SELECT payload FROM attempt_inputs WHERE attempt_id = ?",
            )
            .bind(&attempt.id)
            .fetch_optional(&mut *tx)
            .await?;
            if original.as_deref() != Some(&payload)
                || existing.thread_id != attempt.thread_id
                || existing.turn_id != attempt.turn_id
            {
                return Err(StoreError::Conflict(
                    "Run identifier reused with a different request",
                ));
            }
            return Ok(false);
        }
        sqlx::query("INSERT INTO attempts(id, thread_id, turn_id, state, computer_use) VALUES (?, ?, ?, 'queued', ?)")
            .bind(&attempt.id).bind(&attempt.thread_id).bind(&attempt.turn_id).bind(attempt.computer_use).execute(&mut *tx).await?;
        sqlx::query(
            "INSERT INTO attempt_inputs(attempt_id, payload) VALUES (?, ?)",
        )
        .bind(&attempt.id)
        .bind(payload)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn attempts(&self, thread_id: &str) -> Result<Vec<Attempt>> {
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM attempts WHERE thread_id = ? ORDER BY rowid",
        )
        .bind(thread_id)
        .fetch_all(&self.pool)
        .await?;
        let mut attempts = Vec::new();
        for id in ids {
            if let Some(attempt) = self.attempt(&id).await? {
                attempts.push(attempt);
            }
        }
        Ok(attempts)
    }

    pub async fn prepare_batch(&self, batch: &ApprovalBatch) -> Result<()> {
        valid_id(&batch.id)?;
        if batch.actions.is_empty()
            || batch.actions.len() > 256
            || batch.revision < 1
        {
            return Err(StoreError::Conflict("Invalid approval batch"));
        }
        let mut seen = HashSet::new();
        let mut tx = self.pool.begin().await?;
        require_running(&mut tx, &batch.attempt_id).await?;
        let now: i64 = sqlx::query_scalar("SELECT unixepoch()")
            .fetch_one(&mut *tx)
            .await?;
        if batch.expires_at <= now || batch.expires_at > now + 600 {
            return Err(StoreError::Conflict("Approval expiry is invalid"));
        }
        for action in &batch.actions {
            valid_id(&action.id)?;
            valid_id(&action.call_id)?;
            if action.attempt_id != batch.attempt_id
                || action.revision != batch.revision
                || action.expires_at != batch.expires_at
                || !seen.insert(&action.call_id)
            {
                return Err(StoreError::Conflict(
                    "Approval actions do not match the batch",
                ));
            }
            sqlx::query("INSERT INTO approvals(id, attempt_id, call_id, tool, arguments, revision, expires_at, state) VALUES (?, ?, ?, ?, ?, ?, ?, 'pending')")
                .bind(&action.id).bind(&action.attempt_id).bind(&action.call_id).bind(&action.tool).bind(encode_arguments(&action.arguments)?).bind(action.revision).bind(action.expires_at).execute(&mut *tx).await?;
        }
        sqlx::query("INSERT INTO approval_batches(id, attempt_id, revision, expires_at, actions, state) VALUES (?, ?, ?, ?, ?, 'pending')")
            .bind(&batch.id).bind(&batch.attempt_id).bind(batch.revision).bind(batch.expires_at).bind(serde_json::to_string(&batch.actions)?).execute(&mut *tx).await?;
        sqlx::query("UPDATE attempts SET state = 'awaiting' WHERE id = ?")
            .bind(&batch.attempt_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn batch(&self, id: &str) -> Result<Option<ApprovalBatch>> {
        sqlx::query("SELECT attempt_id, revision, expires_at, actions FROM approval_batches WHERE id = ?").bind(id).fetch_optional(&self.pool).await?.map(|row| Ok(ApprovalBatch {
            id: id.into(), attempt_id: row.get("attempt_id"), revision: row.get("revision"), expires_at: row.get("expires_at"), actions: serde_json::from_str(row.get("actions"))?,
        })).transpose()
    }

    pub async fn batch_pending(&self, id: &str) -> Result<bool> {
        Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM approval_batches WHERE id = ? AND state = 'pending')").bind(id).fetch_one(&self.pool).await?)
    }

    /// Resolves every action in one transaction and resumes the attempt only once.
    /// The caller schedules work only when true is returned. Duplicate UI IPC is inert.
    pub async fn decide_batch(
        &self,
        id: &str,
        revision: i64,
        selected: &[String],
    ) -> Result<bool> {
        let selected: HashSet<_> = selected.iter().collect();
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT attempt_id, revision, expires_at, actions, selected, state FROM approval_batches WHERE id = ?").bind(id).fetch_optional(&mut *tx).await?.ok_or(StoreError::Conflict("Approval batch not found"))?;
        if row.get::<i64, _>("revision") != revision {
            return Err(StoreError::Conflict("Approval version changed"));
        }
        let actions: Vec<Approval> = serde_json::from_str(row.get("actions"))?;
        if selected
            .iter()
            .any(|id| !actions.iter().any(|action| &action.call_id == *id))
        {
            return Err(StoreError::Conflict(
                "Selection contains an unreviewed action",
            ));
        }
        let ordered: Vec<_> = actions
            .iter()
            .filter(|action| selected.contains(&action.call_id))
            .map(|action| action.call_id.clone())
            .collect();
        let decision = serde_json::to_string(&ordered)?;
        if row.get::<String, _>("state") == "decided" {
            if row.get::<Option<String>, _>("selected").as_deref()
                == Some(&decision)
            {
                return Ok(false);
            }
            return Err(StoreError::Conflict(
                "Approval was already decided differently",
            ));
        }
        let attempt_id: String = row.get("attempt_id");
        let attempt = require_live(&mut tx, &attempt_id).await?;
        let now: i64 = sqlx::query_scalar("SELECT unixepoch()")
            .fetch_one(&mut *tx)
            .await?;
        if row.get::<String, _>("state") == "pending"
            && row.get::<i64, _>("expires_at") <= now
        {
            return Err(StoreError::ApprovalExpired);
        }
        if row.get::<String, _>("state") != "pending"
            || row.get::<i64, _>("expires_at") <= now
            || attempt.state != AttemptState::Awaiting
        {
            return Err(StoreError::Conflict(
                "Approval expired or run is no longer awaiting a decision",
            ));
        }
        for action in actions {
            let changed = sqlx::query("UPDATE approvals SET state = ? WHERE id = ? AND state = 'pending'")
                .bind(if selected.contains(&action.call_id) { "approved" } else { "rejected" }).bind(action.id).execute(&mut *tx).await?.rows_affected();
            if changed != 1 {
                return Err(StoreError::Conflict(
                    "Approval action is no longer pending",
                ));
            }
        }
        sqlx::query("UPDATE approval_batches SET state = 'decided', selected = ? WHERE id = ?").bind(decision).bind(id).execute(&mut *tx).await?;
        sqlx::query("UPDATE attempts SET state = 'running' WHERE id = ?")
            .bind(attempt_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn put_graph_checkpoint(
        &self,
        attempt_id: &str,
        namespace: &str,
        record: &GraphCheckpoint,
    ) -> Result<()> {
        if !record.writes.is_empty() {
            return Err(StoreError::Conflict(
                "Checkpoint writes use their own operation",
            ));
        }
        self.put_checkpoint(
            attempt_id,
            &graph_namespace(attempt_id, namespace)?,
            &record.id,
            &serde_json::to_value(record)?,
        )
        .await
    }

    pub async fn put_graph_writes(
        &self,
        attempt_id: &str,
        namespace: &str,
        checkpoint_id: &str,
        writes: &[GraphWrite],
    ) -> Result<()> {
        if writes.len() > 512 {
            return Err(StoreError::Conflict("Too many pending writes"));
        }
        let namespace = graph_namespace(attempt_id, namespace)?;
        let mut tx = self.pool.begin().await?;
        let attempt = require_live(&mut tx, attempt_id).await?;
        for write in writes {
            valid_id(&write.task_id)?;
            sqlx::query("INSERT INTO checkpoint_writes(thread_id, namespace, checkpoint_id, task_id, write_index, channel, payload) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(thread_id, namespace, checkpoint_id, task_id, write_index) DO UPDATE SET channel = excluded.channel, payload = excluded.payload WHERE excluded.write_index < 0")
                .bind(&attempt.thread_id).bind(&namespace).bind(checkpoint_id).bind(&write.task_id).bind(write.index).bind(&write.channel).bind(encode(&write.value)?).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn graph_checkpoints(
        &self,
        attempt_id: &str,
        namespace: &str,
        id: Option<&str>,
        before: Option<&str>,
        limit: u32,
    ) -> Result<Vec<GraphCheckpoint>> {
        let attempt = self
            .attempt(attempt_id)
            .await?
            .ok_or(StoreError::Conflict("Unknown attempt"))?;
        let namespace = graph_namespace(attempt_id, namespace)?;
        let rows = sqlx::query("SELECT payload FROM checkpoints WHERE thread_id = ? AND namespace = ? AND (? IS NULL OR checkpoint_id = ?) AND (? IS NULL OR checkpoint_id < ?) ORDER BY checkpoint_id DESC LIMIT ?")
            .bind(&attempt.thread_id).bind(&namespace).bind(id).bind(id).bind(before).bind(before).bind(limit.clamp(1, 1000)).fetch_all(&self.pool).await?;
        let mut records = Vec::new();
        for row in rows {
            let mut record: GraphCheckpoint =
                serde_json::from_str(row.get("payload"))?;
            let writes = sqlx::query("SELECT task_id, write_index, channel, payload FROM checkpoint_writes WHERE thread_id = ? AND namespace = ? AND checkpoint_id = ? ORDER BY rowid")
                .bind(&attempt.thread_id).bind(&namespace).bind(&record.id).fetch_all(&self.pool).await?;
            record.writes = writes
                .into_iter()
                .map(|row| {
                    Ok(GraphWrite {
                        task_id: row.get("task_id"),
                        index: row.get("write_index"),
                        channel: row.get("channel"),
                        value: serde_json::from_str(row.get("payload"))?,
                    })
                })
                .collect::<Result<_>>()?;
            records.push(record);
        }
        Ok(records)
    }
}

fn graph_namespace(attempt_id: &str, namespace: &str) -> Result<String> {
    if namespace.len() > 2048 {
        return Err(StoreError::Conflict("Checkpoint namespace too long"));
    }
    Ok(serde_json::to_string(&(attempt_id, namespace))?)
}
