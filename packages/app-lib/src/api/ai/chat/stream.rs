use super::*;
use futures::StreamExt;

pub(super) enum Format {
    Openai,
    Anthropic,
    Ollama,
    Google,
}

/// Assemble complete protocol messages while forwarding only user-facing text.
pub(super) async fn read(
    response: reqwest::Response,
    provider: &str,
    format: Format,
    emit: &(impl Fn(&str, &str) + Sync),
) -> crate::Result<ChatMessage> {
    if !response.status().is_success() {
        checked_json(response, provider).await?;
        return Err(input_error("Invalid model stream".into()));
    }
    if response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("application/json"))
        && !matches!(format, Format::Ollama)
    {
        let value = checked_json(response, provider).await?;
        let result = match format {
            Format::Anthropic => parse_anthropic(&value)?,
            Format::Google => google::parse(&value)?,
            _ => parse_openai(&value["choices"][0]["message"])?,
        };
        emit("delta", &result.content);
        return Ok(result);
    }
    let mut parser = Parser::new(format);
    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();
    let mut total = 0usize;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        total = total.saturating_add(chunk.len());
        if total > 4 * 1024 * 1024 {
            return Err(input_error(
                "LogAgent model output exceeded its limit".into(),
            ));
        }
        buffer.extend_from_slice(&chunk);
        while let Some(end) = buffer.iter().position(|byte| *byte == b'\n') {
            let line = String::from_utf8_lossy(&buffer[..end]).to_string();
            buffer.drain(..=end);
            parser.line(&line, emit)?;
            if parser.done {
                return parser.finish();
            }
        }
    }
    if !buffer.is_empty() {
        parser.line(&String::from_utf8_lossy(&buffer), emit)?;
    }
    parser.finish()
}

struct Parser {
    format: Format,
    message: Value,
    blocks: std::collections::BTreeMap<usize, Value>,
    calls: std::collections::BTreeMap<usize, Value>,
    done: bool,
}

impl Parser {
    fn new(format: Format) -> Self {
        Self {
            format,
            message: json!({"content":""}),
            blocks: Default::default(),
            calls: Default::default(),
            done: false,
        }
    }
    fn append(value: &mut Value, text: &str) {
        let old = value.as_str().unwrap_or_default();
        *value = json!(format!("{old}{text}"));
    }
    fn line(
        &mut self,
        line: &str,
        emit: &(impl Fn(&str, &str) + Sync),
    ) -> crate::Result<()> {
        let data = if matches!(self.format, Format::Ollama) {
            line.trim()
        } else {
            let Some(data) = line.strip_prefix("data:") else {
                return Ok(());
            };
            data.trim()
        };
        if data.is_empty() {
            return Ok(());
        }
        if data == "[DONE]" {
            self.done = true;
            return Ok(());
        }
        let value: Value = serde_json::from_str(data)?;
        if value.get("error").is_some() || value["type"] == "error" {
            return Err(input_error(
                "LogAgent provider reported a streaming error".into(),
            ));
        }
        match self.format {
            Format::Openai => {
                let choice = &value["choices"][0];
                if matches!(
                    choice["finish_reason"].as_str(),
                    Some("length" | "content_filter")
                ) {
                    return Err(input_error(
                        "LogAgent model reply was cut short before completion"
                            .into(),
                    ));
                }
                if choice["finish_reason"].as_str().is_some() {
                    self.done = true;
                }
                let delta = &choice["delta"];
                if let Some(text) = delta["content"].as_str() {
                    Self::append(&mut self.message["content"], text);
                    emit("delta", text);
                }
                if delta.get("reasoning_content").is_some() {
                    Self::append(
                        &mut self.message["reasoning_content"],
                        delta["reasoning_content"].as_str().unwrap_or_default(),
                    );
                    emit("thinking", "");
                }
                if let Some(details) = delta["reasoning_details"].as_array() {
                    let output = self
                        .message
                        .as_object_mut()
                        .unwrap()
                        .entry("reasoning_details")
                        .or_insert(json!([]))
                        .as_array_mut()
                        .unwrap();
                    for detail in details {
                        let existing = output.iter_mut().find(|item| {
                            (detail.get("index").is_some()
                                && item.get("index") == detail.get("index"))
                                || (detail.get("id").is_some()
                                    && item.get("id") == detail.get("id"))
                        });
                        if let Some(existing) = existing {
                            if let Some(fields) = detail.as_object() {
                                for (key, value) in fields {
                                    if matches!(
                                        key.as_str(),
                                        "text"
                                            | "data"
                                            | "signature"
                                            | "summary"
                                    ) && value.is_string()
                                    {
                                        Self::append(
                                            &mut existing[key],
                                            value.as_str().unwrap(),
                                        );
                                    } else {
                                        existing[key] = value.clone();
                                    }
                                }
                            }
                        } else {
                            output.push(detail.clone());
                        }
                    }
                    emit("thinking", "");
                }
                if let Some(calls) = delta["tool_calls"].as_array() {
                    for call in calls {
                        let index =
                            call["index"].as_u64().unwrap_or(0) as usize;
                        let entry = self.calls.entry(index).or_insert(json!({"id":"", "function":{"name":"", "arguments":""}}));
                        if let Some(text) = call["id"].as_str() {
                            Self::append(&mut entry["id"], text);
                        }
                        for key in ["name", "arguments"] {
                            if let Some(text) = call["function"][key].as_str() {
                                Self::append(&mut entry["function"][key], text);
                            }
                        }
                    }
                }
            }
            Format::Anthropic => match value["type"].as_str() {
                Some("content_block_start") => {
                    self.blocks.insert(
                        value["index"].as_u64().unwrap_or(0) as usize,
                        value["content_block"].clone(),
                    );
                }
                Some("content_block_delta") => {
                    let block = self
                        .blocks
                        .entry(value["index"].as_u64().unwrap_or(0) as usize)
                        .or_insert(json!({}));
                    let delta = &value["delta"];
                    match delta["type"].as_str() {
                        Some("text_delta") => {
                            let text =
                                delta["text"].as_str().unwrap_or_default();
                            Self::append(&mut block["text"], text);
                            emit("delta", text);
                        }
                        Some("input_json_delta") => Self::append(
                            &mut block["partial_json"],
                            delta["partial_json"].as_str().unwrap_or_default(),
                        ),
                        Some("thinking_delta") => {
                            Self::append(
                                &mut block["thinking"],
                                delta["thinking"].as_str().unwrap_or_default(),
                            );
                            emit("thinking", "");
                        }
                        Some("signature_delta") => Self::append(
                            &mut block["signature"],
                            delta["signature"].as_str().unwrap_or_default(),
                        ),
                        _ => {}
                    }
                }
                Some("message_stop") => self.done = true,
                Some("message_delta")
                    if value["delta"]["stop_reason"] == "max_tokens" =>
                {
                    return Err(input_error(
                        "LogAgent model reply reached its output limit".into(),
                    ));
                }
                _ => {}
            },
            Format::Ollama => {
                if let Some(text) = value["message"]["content"].as_str() {
                    Self::append(&mut self.message["content"], text);
                    emit("delta", text);
                }
                if value["message"].get("thinking").is_some() {
                    emit("thinking", "");
                }
                if let Some(calls) = value["message"]["tool_calls"].as_array() {
                    for call in calls {
                        self.calls.insert(self.calls.len(), call.clone());
                    }
                }
                if value["done"] == true {
                    self.done = true;
                }
            }
            Format::Google => {
                let candidate = &value["candidates"][0];
                if let Some(reason) = candidate["finishReason"].as_str() {
                    if reason != "STOP" {
                        return Err(input_error("Google model reply ended before a complete diagnosis".into()));
                    }
                    self.done = true;
                }
                if let Some(parts) = candidate["content"]["parts"].as_array() {
                    for part in parts {
                        if part["thought"] == true {
                            emit("thinking", "");
                        } else if let Some(text) = part["text"].as_str() {
                            emit("delta", text);
                        }
                        self.blocks.insert(self.blocks.len(), part.clone());
                    }
                }
            }
        }
        Ok(())
    }
    fn finish(mut self) -> crate::Result<ChatMessage> {
        if !self.done {
            return Err(input_error(
                "LogAgent model stream ended before completion".into(),
            ));
        }
        match self.format {
            Format::Google => google::parse(
                &json!({"candidates":[{"content":{"parts":self.blocks.into_values().collect::<Vec<_>>()}}]}),
            ),
            Format::Anthropic => {
                let mut blocks = Vec::new();
                for (_, mut block) in self.blocks {
                    if let Some(partial) = block
                        .as_object_mut()
                        .and_then(|block| block.remove("partial_json"))
                    {
                        block["input"] = serde_json::from_str(
                            partial.as_str().unwrap_or("{}"),
                        )?;
                    }
                    blocks.push(block);
                }
                parse_anthropic(&json!({"content":blocks}))
            }
            _ => {
                self.message["tool_calls"] =
                    json!(self.calls.into_values().collect::<Vec<_>>());
                parse_openai(&self.message)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn google_stream_keeps_signed_tool_parts_and_emits_only_public_text() {
        let mut parser = Parser::new(Format::Google);
        let events = std::sync::Mutex::new(Vec::new());
        let emit = |kind: &str, text: &str| {
            events
                .lock()
                .unwrap()
                .push((kind.to_string(), text.to_string()))
        };
        for value in [
            json!({"candidates":[{"content":{"parts":[{"text":"private","thought":true,"thoughtSignature":"signed"}]}}]}),
            json!({"candidates":[{"content":{"parts":[{"text":"Checking logs."},{"functionCall":{"id":"g1","name":"list_log_files","args":{}},"thoughtSignature":"tool-signature"}]},"finishReason":"STOP"}]}),
        ] {
            parser.line(&format!("data: {value}"), &emit).unwrap();
        }
        let result = parser.finish().unwrap();
        assert_eq!(result.content, "Checking logs.");
        assert_eq!(result.opaque[0]["thoughtSignature"], "signed");
        assert_eq!(result.tool_calls[0].id, "google:g1");
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                ("thinking".into(), "".into()),
                ("delta".into(), "Checking logs.".into())
            ]
        );
    }

    #[test]
    fn reasoning_fragments_merge_for_protocol_replay_and_incomplete_streams_fail()
     {
        let mut parser = Parser::new(Format::Openai);
        for text in ["part one", "part two"] {
            parser.line(&format!("data: {}", json!({"choices":[{"delta":{"reasoning_details":[{"index":0,"type":"reasoning.encrypted","data":text}]}}]})), &|_, _| {}).unwrap();
        }
        parser.line(&format!("data: {}", json!({"choices":[{"delta":{"content":"Conclusion"},"finish_reason":"stop"}]})), &|_, _| {}).unwrap();
        assert_eq!(
            parser.finish().unwrap().opaque["reasoning_details"][0]["data"],
            "part onepart two"
        );
        assert!(Parser::new(Format::Openai).finish().is_err());
    }
    #[tokio::test]
    async fn streams_text_before_completion_and_assembles_split_tool_arguments()
    {
        let (tx, rx) =
            tokio::sync::mpsc::channel::<Result<Vec<u8>, std::io::Error>>(8);
        let body = reqwest::Body::wrap_stream(futures::stream::unfold(
            rx,
            |mut rx| async { rx.recv().await.map(|item| (item, rx)) },
        ));
        let (observed, mut output) = tokio::sync::mpsc::unbounded_channel();
        let handle = tokio::spawn(async move {
            read(
                reqwest::Response::from(http::Response::new(body)),
                "test",
                Format::Openai,
                &|kind, text| {
                    let _ = observed.send((kind.to_string(), text.to_string()));
                },
            )
            .await
        });
        tx.send(Ok(b"data: {\"choices\":[{\"delta\":{\"content\":\"Visible text\",\"reasoning_content\":\"private\",\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"function\":{\"name\":\"grep_log_file\",\"arguments\":\"{\\\"query\\\":\"}}]}}]}\n\n".to_vec())).await.unwrap();
        assert_eq!(
            output.recv().await.unwrap(),
            ("delta".into(), "Visible text".into())
        );
        assert_eq!(
            output.recv().await.unwrap(),
            ("thinking".into(), "".into())
        );
        assert!(!handle.is_finished());
        tx.send(Ok(b"data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"error\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\ndata: [DONE]\n\n".to_vec())).await.unwrap();
        drop(tx);
        let result = handle.await.unwrap().unwrap();
        assert_eq!(result.tool_calls[0].args["query"], "error");
        assert_eq!(result.opaque["reasoning_content"], "private");
    }
    #[test]
    fn anthropic_preserves_private_signed_blocks_without_emitting_them() {
        let mut parser = Parser::new(Format::Anthropic);
        let events = std::sync::Mutex::new(Vec::new());
        let emit = |kind: &str, text: &str| {
            events
                .lock()
                .unwrap()
                .push((kind.to_string(), text.to_string()))
        };
        for value in [
            json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"","signature":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"private"}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"signed"}}),
            json!({"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Conclusion"}}),
            json!({"type":"message_stop"}),
        ] {
            parser.line(&format!("data: {value}"), &emit).unwrap();
        }
        let result = parser.finish().unwrap();
        assert_eq!(result.opaque[0]["signature"], "signed");
        assert_eq!(result.content, "Conclusion");
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                ("thinking".into(), "".into()),
                ("delta".into(), "Conclusion".into())
            ]
        );
    }
}
