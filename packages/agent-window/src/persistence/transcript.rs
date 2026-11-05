use super::*;
use serde_json::json;
use std::collections::HashSet;

impl AgentStore {
    /// Durable input is immutable under a model request identity. Only wire
    /// message fields are archived; transport configuration never enters this API.
    pub async fn save_model_input(
        &self,
        attempt_id: &str,
        request_id: &str,
        messages: &[Value],
    ) -> Result<()> {
        valid_id(request_id)?;
        let messages = messages
            .iter()
            .map(wire_message)
            .collect::<Result<Vec<_>>>()?;
        if messages.is_empty() {
            return Err(StoreError::Conflict(
                "Model transcript input is empty",
            ));
        }
        let input = encode(&json!(messages))?;
        let mut tx = self.pool.begin().await?;
        if get_attempt(&mut tx, attempt_id).await?.is_none() {
            return Err(StoreError::Conflict("Unknown attempt"));
        }
        let existing: Option<String> = sqlx::query_scalar("SELECT input FROM model_turns WHERE attempt_id = ? AND request_id = ?")
            .bind(attempt_id).bind(request_id).fetch_optional(&mut *tx).await?;
        if let Some(existing) = existing {
            if existing != input {
                return Err(StoreError::Conflict(
                    "Model request identity reused with different messages",
                ));
            }
            return Ok(());
        }
        sqlx::query("INSERT INTO model_turns(attempt_id, request_id, input) VALUES (?, ?, ?)")
            .bind(attempt_id).bind(request_id).bind(input).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    /// A reply that finishes after cancellation is archived without reopening the
    /// attempt or emitting events. Duplicate delivery can never replace a reply.
    pub async fn save_model_reply(
        &self,
        attempt_id: &str,
        request_id: &str,
        reply: &Value,
    ) -> Result<()> {
        let reply = wire_message(reply)?;
        if reply["role"] != "assistant" {
            return Err(StoreError::Conflict(
                "Model reply must be an assistant message",
            ));
        }
        let reply = encode(&reply)?;
        let mut tx = self.pool.begin().await?;
        let existing: Option<Option<String>> = sqlx::query_scalar("SELECT reply FROM model_turns WHERE attempt_id = ? AND request_id = ?")
            .bind(attempt_id).bind(request_id).fetch_optional(&mut *tx).await?;
        match existing {
            None => {
                return Err(StoreError::Conflict(
                    "Model reply has no persisted input",
                ));
            }
            Some(Some(existing)) => {
                if existing != reply {
                    return Err(StoreError::Conflict(
                        "Model reply identity reused with different output",
                    ));
                }
                return Ok(());
            }
            Some(None) => {}
        }
        sqlx::query("UPDATE model_turns SET reply = ? WHERE attempt_id = ? AND request_id = ? AND reply IS NULL")
            .bind(reply).bind(attempt_id).bind(request_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    /// One latest full snapshot already contains the earlier conversation. Never
    /// concatenate every model request and multiply the same history each turn.
    pub async fn conversation_transcript(
        &self,
        thread_id: &str,
        provider_id: &str,
        model_id: &str,
    ) -> Result<Vec<Value>> {
        self.conversation_transcript_for_attempts(
            thread_id,
            provider_id,
            model_id,
            None,
        )
        .await
    }

    /// Visible turns select the conversation branch. Older execution receipts
    /// remain intact, but discarded replies never re-enter the model history.
    pub async fn conversation_transcript_for_attempts(
        &self,
        thread_id: &str,
        provider_id: &str,
        model_id: &str,
        history_attempts: Option<&[String]>,
    ) -> Result<Vec<Value>> {
        if let Some(ids) = history_attempts {
            for id in ids {
                valid_id(id)?;
            }
        }
        let history =
            history_attempts.map(serde_json::to_string).transpose()?;
        let row = sqlx::query("SELECT t.attempt_id, t.input, t.reply, i.payload AS configuration FROM model_turns t JOIN attempts a ON a.id = t.attempt_id LEFT JOIN attempt_inputs i ON i.attempt_id = a.id WHERE a.thread_id = ? AND (? IS NULL OR a.id IN (SELECT value FROM json_each(?))) AND a.state IN ('completed', 'failed', 'cancelled', 'interrupted') ORDER BY a.rowid DESC, t.sequence DESC LIMIT 1")
            .bind(thread_id).bind(&history).bind(&history).fetch_optional(&self.pool).await?;
        let Some(row) = row else {
            let rows = sqlx::query("SELECT m.payload FROM messages m JOIN attempts a ON a.id = m.attempt_id WHERE m.thread_id = ? AND (? IS NULL OR a.id IN (SELECT value FROM json_each(?))) AND a.state = 'completed' ORDER BY m.rowid")
                .bind(thread_id).bind(&history).bind(&history).fetch_all(&self.pool).await?;
            return rows
                .into_iter()
                .map(|row| Ok(serde_json::from_str(row.get("payload"))?))
                .collect();
        };
        let attempt_id: String = row.get("attempt_id");
        let mut messages: Vec<Value> = serde_json::from_str(row.get("input"))?;
        if let Some(reply) = row.get::<Option<String>, _>("reply") {
            messages.push(serde_json::from_str(&reply)?);
        }
        messages.retain(|message| message["role"] != "system");
        let mut complete = Vec::new();
        let mut index = 0;
        while index < messages.len() {
            let message = &messages[index];
            complete.push(message.clone());
            index += 1;
            if message["role"] != "assistant" {
                continue;
            }
            let Some(calls) = message["tool_calls"]
                .as_array()
                .filter(|calls| !calls.is_empty())
            else {
                continue;
            };
            let mut answered = HashSet::new();
            while index < messages.len() && messages[index]["role"] == "tool" {
                if let Some(id) = messages[index]["tool_call_id"].as_str() {
                    answered.insert(id.to_owned());
                }
                complete.push(messages[index].clone());
                index += 1;
            }
            for call in calls {
                let Some(id) = call["id"].as_str() else {
                    continue;
                };
                if !answered.contains(id) {
                    complete.push(
                        self.historical_receipt(&attempt_id, id, call).await?,
                    );
                }
            }
        }
        let configuration: Value = row
            .get::<Option<String>, _>("configuration")
            .map(|text| serde_json::from_str(&text))
            .transpose()?
            .unwrap_or(Value::Null);
        if configuration["provider_id"] == provider_id
            && configuration["model_id"] == model_id
        {
            return Ok(complete);
        }
        Ok(complete.iter().map(historical_message).collect())
    }

    async fn historical_receipt(
        &self,
        attempt_id: &str,
        call_id: &str,
        call: &Value,
    ) -> Result<Value> {
        let execution = self.execution(attempt_id, call_id).await?;
        let mut images = Vec::new();
        let mut failed = true;
        let content = match execution {
            Some(execution) if execution.tool == call["name"].as_str().unwrap_or_default() && execution.arguments == call["args"] => {
                if matches!(execution.state.as_str(), "completed" | "failed") && execution.result.is_some() {
                    let result = execution.result.unwrap();
                    failed = execution.state == "failed" || result["is_error"] != false;
                    let mut text = Vec::new();
                    if let Some(parts) = result["content"].as_array() {
                        for part in parts {
                            if part["type"] == "text" { if let Some(value) = part["text"].as_str() { text.push(value.to_owned()); } }
                            if part["type"] == "image" { images.push(json!({"mime_type":part["mime_type"],"data":part["data"]})); }
                        }
                    }
                    if text.is_empty() { text.push(result["structured_content"].to_string()); }
                    format!("Historical host execution receipt (do not replay this call): {}", text.join("\n"))
                } else { "Historical execution outcome is unconfirmed. Verify readonly; never replay this call.".into() }
            }
            Some(_) => "Historical call identity does not match its execution receipt. Outcome unconfirmed; verify readonly, never replay.".into(),
            None if call["name"] == "write_todos" => "Historical internal checklist result is unavailable. No business change is proven by this call.".into(),
            None => "Historical tool call was not executed. Do not replay this call; inspect current state before proposing new actions.".into(),
        };
        Ok(
            json!({"role":"tool","tool_call_id":call_id,"content":content,"images":images,"tool_error":failed}),
        )
    }
}

fn wire_message(message: &Value) -> Result<Value> {
    if !matches!(
        message["role"].as_str(),
        Some("system" | "user" | "assistant" | "tool")
    ) {
        return Err(StoreError::Conflict("Invalid transcript message role"));
    }
    let mut wire = serde_json::Map::new();
    for key in [
        "role",
        "content",
        "images",
        "tool_calls",
        "tool_call_id",
        "tool_error",
        "opaque",
    ] {
        if let Some(value) = message.get(key) {
            wire.insert(key.to_owned(), value.clone());
        }
    }
    let wire = Value::Object(wire);
    encode(&wire)?;
    Ok(wire)
}

fn historical_message(message: &Value) -> Value {
    let role = message["role"].as_str().unwrap_or("unknown");
    let mut content = format!(
        "Historical {role} message from a different provider/model (reference only):\n{}",
        message["content"].as_str().unwrap_or_default()
    );
    if let Some(calls) = message["tool_calls"].as_array() {
        for call in calls {
            content.push_str(&format!(
                "\nHistorical tool request {}: {}",
                call["name"].as_str().unwrap_or("unknown"),
                call["args"]
            ));
        }
    }
    let mut historical = json!({"role":"user","content":content});
    if let Some(images) = message.get("images") {
        historical["images"] = images.clone();
    }
    historical
}
