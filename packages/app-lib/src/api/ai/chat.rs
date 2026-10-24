//! Model turns for tool-driven diagnostics. Credentials never leave Rust.

mod bedrock;
mod google;
mod stream;

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    pub tool_call_id: Option<String>,
    #[serde(default)]
    pub opaque: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatTurn {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<Value>,
}

pub struct ChatModel {
    definition: &'static AiProviderDefinition,
    config: AiProviderConfig,
    model_id: String,
    // A model is retained for all turns of one LogAgent analysis.
    session_id: Uuid,
}

impl ChatModel {
    pub async fn prepare(
        provider_id: &str,
        model_id: &str,
    ) -> crate::Result<Self> {
        let state = get_state().await?;
        let config = state
            .providers
            .into_iter()
            .find(|config| config.provider_id == provider_id)
            .ok_or_else(|| {
                input_error(
                    "Select an AI provider in analysis settings".to_string(),
                )
            })?;
        if !state.settings.enabled
            || !config.enabled
            || !config
                .models
                .iter()
                .any(|model| model.enabled && model.id == model_id)
        {
            return Err(input_error("Enable the selected AI provider and model before using local LogAgent".to_string()));
        }
        if provider_id == "githubcopilot"
            && model_id.eq_ignore_ascii_case("auto")
        {
            return Err(input_error("LOCAL_LOGAGENT_UNSUPPORTED: Select an explicit tool-capable Copilot model instead of Auto.".to_string()));
        }
        Ok(Self {
            definition: provider_definition(provider_id)?,
            config,
            model_id: model_id.to_string(),
            session_id: Uuid::new_v4(),
        })
    }

    pub async fn turn(
        &self,
        turn: ChatTurn,
        emit: &(impl Fn(&str, &str) + Sync),
    ) -> crate::Result<ChatMessage> {
        if turn.messages.is_empty()
            || turn.messages.len() > 160
            || serde_json::to_vec(&turn)?.len() > 700_000
        {
            return Err(input_error(
                "LogAgent context budget exceeded".to_string(),
            ));
        }
        if turn.messages.iter().any(|message| {
            !matches!(
                message.role.as_str(),
                "system" | "user" | "assistant" | "tool"
            )
        }) {
            return Err(input_error(
                "Invalid LogAgent message role".to_string(),
            ));
        }
        let state = State::get().await?;
        let proxy = state.proxy_config().await?;
        let client = proxy
            .apply(
                reqwest::Client::builder()
                    .timeout(Duration::from_secs(120))
                    .user_agent(crate::launcher_user_agent()),
            )?
            .build()?;
        let mut token =
            read_credential(self.definition.id, API_KEY_CREDENTIAL)?
                .unwrap_or_default();
        let mut account_id = None;
        if self.definition.auth_type == AiAuthType::OAuthDeviceFlow {
            let credentials =
                fresh_oauth_credentials(self.definition.id).await?;
            token = if self.definition.id == "githubcopilot" {
                credentials.bearer_token.unwrap_or_default()
            } else {
                credentials.access_token
            };
            account_id = credentials.account_id;
        }
        if self.definition.auth_type != AiAuthType::None
            && token.is_empty()
            && self.definition.id != "vertexai"
            && self.config.protocol != AiProtocol::Bedrock
        {
            return Err(input_error(format!(
                "Configure credentials for {} before using LogAgent",
                self.definition.name
            )));
        }
        if self.config.protocol == AiProtocol::Google {
            return google::turn(self, &client, token, &turn, emit).await;
        }
        if self.config.protocol == AiProtocol::Bedrock {
            let result = bedrock::turn(self, &client, token, &turn).await?;
            emit("delta", &result.content);
            return Ok(result);
        }
        let copilot = self.definition.id == "githubcopilot";
        if self.definition.id == "chatgpt"
            || (copilot && github_copilot_uses_responses(&self.model_id))
        {
            return self
                .responses(&client, token, account_id, &turn, emit)
                .await;
        }
        if self.config.protocol == AiProtocol::Anthropic
            || (copilot && self.model_id.contains("claude"))
        {
            let (system, messages) = anthropic_messages(&turn.messages);
            let tools = turn.tools.iter().map(|tool| json!({ "name": tool["function"]["name"], "description": tool["function"]["description"], "input_schema": tool["function"]["parameters"] })).collect::<Vec<_>>();
            let builder = client
                .post(endpoint_with_path(&self.config.endpoint, "/v1/messages"))
                .header("anthropic-version", "2023-06-01");
            let builder =
                opencode_headers(builder, self.definition.id, self.session_id);
            let builder = if copilot {
                copilot_headers(builder.bearer_auth(token), None)
            } else {
                builder.header("x-api-key", token)
            };
            let mut body = json!({ "model": self.model_id, "max_tokens": 4096, "stream": true, "system": system, "messages": messages });
            if !tools.is_empty() {
                body["tools"] = json!(tools);
            }
            return stream::read(
                builder.json(&body).send().await?,
                self.definition.name,
                stream::Format::Anthropic,
                emit,
            )
            .await;
        }
        let ollama = self.config.protocol == AiProtocol::Ollama;
        let endpoint = if self.config.protocol == AiProtocol::Cloudflare {
            let account = self
                .config
                .settings
                .get("account_id")
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    input_error(
                        "Configure the Cloudflare account ID".to_string(),
                    )
                })?;
            format!(
                "{}/accounts/{}/ai/v1/chat/completions",
                self.config.endpoint.trim_end_matches('/'),
                urlencoding::encode(account)
            )
        } else if self.config.protocol == AiProtocol::Azure {
            let deployment = self
                .config
                .settings
                .get("deployment")
                .filter(|value| !value.is_empty())
                .unwrap_or(&self.model_id);
            let version = self
                .config
                .settings
                .get("api_version")
                .map(String::as_str)
                .unwrap_or("2024-10-21");
            format!(
                "{}/openai/deployments/{}/chat/completions?api-version={}",
                self.config.endpoint.trim_end_matches('/'),
                urlencoding::encode(deployment),
                urlencoding::encode(version)
            )
        } else {
            endpoint_with_path(
                &self.config.endpoint,
                if ollama {
                    "/api/chat"
                } else {
                    "/chat/completions"
                },
            )
        };
        let mut body = json!({ "model": self.model_id, "messages": openai_messages(&turn.messages, ollama), "tools": turn.tools, "stream": true });
        if turn.tools.is_empty() {
            body.as_object_mut().unwrap().remove("tools");
        } else if !ollama {
            body["tool_choice"] = json!("auto");
        }
        let mut builder = opencode_headers(
            client.post(endpoint).json(&body),
            self.definition.id,
            self.session_id,
        );
        if !token.is_empty() {
            builder = if self.config.protocol == AiProtocol::Azure
                || (self.config.protocol == AiProtocol::AzureAi
                    && self.definition.id != "github")
            {
                builder.header("api-key", token)
            } else {
                builder.bearer_auth(token)
            };
        }
        if copilot {
            builder = copilot_headers(builder, None);
        }
        stream::read(
            builder.send().await?,
            self.definition.name,
            if ollama {
                stream::Format::Ollama
            } else {
                stream::Format::Openai
            },
            emit,
        )
        .await
    }

    async fn responses(
        &self,
        client: &reqwest::Client,
        token: String,
        account_id: Option<String>,
        turn: &ChatTurn,
        emit: &(impl Fn(&str, &str) + Sync),
    ) -> crate::Result<ChatMessage> {
        let mut input = Vec::new();
        let mut instructions = String::new();
        for message in &turn.messages {
            match message.role.as_str() {
                "system" => { instructions.push_str(&message.content); instructions.push('\n'); }
                "tool" => input.push(json!({ "type": "function_call_output", "call_id": message.tool_call_id, "output": message.content })),
                "assistant" if message.opaque.is_array() => input.extend(message.opaque.as_array().unwrap().iter().cloned()),
                "assistant" => {
                    if !message.content.is_empty() { input.push(json!({ "role": "assistant", "content": message.content })); }
                    for call in &message.tool_calls { input.push(json!({ "type": "function_call", "call_id": call.id, "name": call.name, "arguments": call.args.to_string() })); }
                }
                _ => input.push(json!({ "role": message.role, "content": message.content })),
            }
        }
        let tools = turn.tools.iter().map(|tool| json!({ "type": "function", "name": tool["function"]["name"], "description": tool["function"]["description"], "parameters": tool["function"]["parameters"] })).collect::<Vec<_>>();
        let lite = self.definition.id == "chatgpt"
            && matches!(
                self.model_id.as_str(),
                "gpt-5.6-luna" | "gpt-5.6-sol" | "gpt-5.6-terra"
            );
        let body = if lite {
            input.insert(0, json!({ "role": "developer", "type": "message", "content": instructions }));
            input.insert(0, json!({ "role": "developer", "type": "additional_tools", "tools": tools }));
            json!({ "model": self.model_id, "input": input, "tool_choice": "auto", "parallel_tool_calls": false, "reasoning": { "context": "all_turns" }, "store": false, "stream": true })
        } else {
            json!({ "model": self.model_id, "instructions": instructions, "input": input, "tools": tools, "tool_choice": "auto", "parallel_tool_calls": false, "include": ["reasoning.encrypted_content"], "store": false, "stream": true })
        };
        let mut builder = client
            .post(endpoint_with_path(&self.config.endpoint, "/responses"))
            .bearer_auth(token)
            .header("Accept", "text/event-stream")
            .json(&body);
        if self.definition.id == "chatgpt" {
            builder = builder
                .header(
                    "ChatGPT-Account-Id",
                    account_id.ok_or_else(|| {
                        input_error(
                            "Reconnect ChatGPT before using LogAgent"
                                .to_string(),
                        )
                    })?,
                )
                .header("originator", "lobehub")
                .header("session-id", Uuid::new_v4().to_string())
                .header("version", env!("CARGO_PKG_VERSION"));
        } else {
            builder = copilot_headers(builder, None);
        }
        if lite {
            builder = builder
                .header("x-openai-internal-codex-responses-lite", "true");
        }
        let response = builder.send().await?;
        if !response.status().is_success() {
            checked_json(response, self.definition.name).await?;
            return Err(input_error("Invalid Responses result".to_string()));
        }
        parse_response_stream(response, emit).await
    }
}

async fn parse_response_stream(
    response: reqwest::Response,
    emit: &(impl Fn(&str, &str) + Sync),
) -> crate::Result<ChatMessage> {
    use futures::StreamExt;
    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();
    let mut total = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        total += chunk.len();
        if total > 4 * 1024 * 1024 {
            return Err(input_error(
                "LogAgent model output exceeded its limit".to_string(),
            ));
        }
        buffer.extend_from_slice(&chunk);
        while let Some(end) = buffer.iter().position(|byte| *byte == b'\n') {
            let line = String::from_utf8_lossy(&buffer[..end]).to_string();
            buffer.drain(..=end);
            if let Some(data) = line.strip_prefix("data:")
                && let Ok(value) = serde_json::from_str::<Value>(data.trim())
            {
                match value["type"].as_str() {
                    Some("response.output_text.delta") => {
                        if let Some(text) = value["delta"].as_str() {
                            emit("delta", text);
                        }
                    }
                    Some(
                        "response.reasoning_summary_text.delta"
                        | "response.reasoning_text.delta",
                    ) => emit("thinking", ""),
                    Some("response.completed") => {
                        return parse_responses(&value["response"]);
                    }
                    Some(
                        "response.failed" | "response.incomplete" | "error",
                    ) => {
                        return Err(input_error(
                            "LogAgent model response failed or was incomplete"
                                .to_string(),
                        ));
                    }
                    _ => {}
                }
            }
        }
    }
    Err(input_error(
        "LogAgent Responses stream ended before completion".to_string(),
    ))
}

fn assistant() -> ChatMessage {
    ChatMessage {
        role: "assistant".to_string(),
        content: String::new(),
        tool_calls: Vec::new(),
        tool_call_id: None,
        opaque: Value::Null,
    }
}

fn validate_output(message: ChatMessage) -> crate::Result<ChatMessage> {
    if message.content.is_empty() && message.tool_calls.is_empty() {
        return Err(input_error("The selected model returned no text or tool calls; choose a tool-capable model".to_string()));
    }
    if message.tool_calls.iter().any(|call| {
        call.id.is_empty() || call.name.is_empty() || !call.args.is_object()
    }) {
        return Err(input_error(
            "The selected model returned invalid tool calls".to_string(),
        ));
    }
    Ok(message)
}

fn openai_messages(messages: &[ChatMessage], ollama: bool) -> Vec<Value> {
    messages.iter().map(|message| {
        let mut value = json!({ "role": message.role, "content": message.content });
        if message.role == "tool" { value["tool_call_id"] = json!(message.tool_call_id); }
        if !message.tool_calls.is_empty() {
            value["tool_calls"] = json!(message.tool_calls.iter().map(|call| json!({ "id": call.id, "type": "function", "function": { "name": call.name, "arguments": if ollama { call.args.clone() } else { json!(call.args.to_string()) } } })).collect::<Vec<_>>());
        }
        if message.role == "assistant" {
            for key in ["reasoning_content", "reasoning_details"] { if let Some(reasoning) = message.opaque.get(key).filter(|value| !value.is_null()) { value[key] = reasoning.clone(); } }
        }
        value
    }).collect()
}

fn parse_openai(value: &Value) -> crate::Result<ChatMessage> {
    let mut result = assistant();
    result.content = value["content"].as_str().unwrap_or_default().to_string();
    if let Some(calls) = value["tool_calls"].as_array() {
        for call in calls {
            let args = &call["function"]["arguments"];
            result.tool_calls.push(ToolCall {
                id: call["id"]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| Uuid::new_v4().to_string()),
                name: call["function"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                args: if let Some(text) = args.as_str() {
                    serde_json::from_str(text)?
                } else {
                    args.clone()
                },
            });
        }
    }
    result.opaque = json!({ "reasoning_content": value["reasoning_content"], "reasoning_details": value["reasoning_details"] });
    validate_output(result)
}

fn anthropic_messages(messages: &[ChatMessage]) -> (String, Vec<Value>) {
    let system = messages
        .iter()
        .filter(|message| message.role == "system")
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let mut output: Vec<Value> = Vec::new();
    for message in messages.iter().filter(|message| message.role != "system") {
        let role = if message.role == "assistant" {
            "assistant"
        } else {
            "user"
        };
        let content = if message.role == "tool" {
            json!([{ "type": "tool_result", "tool_use_id": message.tool_call_id, "content": message.content }])
        } else if message.role == "assistant" && message.opaque.is_array() {
            message.opaque.clone()
        } else {
            let mut parts = Vec::new();
            if !message.content.is_empty() {
                parts.push(json!({ "type": "text", "text": message.content }));
            }
            for call in &message.tool_calls {
                parts.push(json!({ "type": "tool_use", "id": call.id, "name": call.name, "input": call.args }));
            }
            json!(parts)
        };
        if let Some(last) =
            output.last_mut().filter(|last| last["role"] == role)
        {
            last["content"]
                .as_array_mut()
                .unwrap()
                .extend(content.as_array().unwrap().iter().cloned());
        } else {
            output.push(json!({ "role": role, "content": content }));
        }
    }
    (system, output)
}

fn parse_anthropic(value: &Value) -> crate::Result<ChatMessage> {
    let mut result = assistant();
    if let Some(parts) = value["content"].as_array() {
        for part in parts {
            match part["type"].as_str() {
                Some("text") => result
                    .content
                    .push_str(part["text"].as_str().unwrap_or_default()),
                Some("tool_use") => result.tool_calls.push(ToolCall {
                    id: part["id"].as_str().unwrap_or_default().to_string(),
                    name: part["name"].as_str().unwrap_or_default().to_string(),
                    args: part["input"].clone(),
                }),
                _ => {}
            }
        }
    }
    result.opaque = value["content"].clone();
    validate_output(result)
}

fn parse_responses(value: &Value) -> crate::Result<ChatMessage> {
    let mut result = assistant();
    if let Some(parts) = value["output"].as_array() {
        for part in parts {
            match part["type"].as_str() {
                Some("function_call") => result.tool_calls.push(ToolCall {
                    id: part["call_id"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    name: part["name"].as_str().unwrap_or_default().to_string(),
                    args: serde_json::from_str(
                        part["arguments"].as_str().unwrap_or("{}"),
                    )?,
                }),
                Some("message") => {
                    if let Some(content) = part["content"].as_array() {
                        for block in content {
                            if let Some(text) = block["text"].as_str() {
                                result.content.push_str(text);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    result.opaque = value["output"].clone();
    validate_output(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn responses_stream_handles_fragmented_utf8_and_requires_completion()
    {
        let completed = json!({ "type": "response.completed", "response": { "output": [{ "type": "message", "content": [{ "type": "output_text", "text": "诊断" }] }, { "type": "function_call", "call_id": "c1", "name": "read_log_file", "arguments": "{\"file_id\":\"log-1\"}" }] } });
        let text = format!(
            "event: response.function_call_arguments.delta\ndata: {{\"type\":\"response.function_call_arguments.delta\",\"delta\":\"partial\"}}\n\ndata: {completed}\n\n"
        );
        let chunks = text
            .as_bytes()
            .chunks(7)
            .map(|chunk| Ok::<_, std::io::Error>(chunk.to_vec()))
            .collect::<Vec<_>>();
        let body = reqwest::Body::wrap_stream(futures::stream::iter(chunks));
        let result = parse_response_stream(
            reqwest::Response::from(http::Response::new(body)),
            &|_, _| {},
        )
        .await
        .unwrap();
        assert_eq!(result.content, "诊断");
        assert_eq!(result.tool_calls[0].args["file_id"], "log-1");
        let incomplete = reqwest::Response::from(http::Response::new(
            reqwest::Body::from(
                "data: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\n",
            ),
        ));
        assert!(parse_response_stream(incomplete, &|_, _| {}).await.is_err());
    }

    #[test]
    fn openai_round_trip_retains_tool_ids_arguments_and_reasoning() {
        let value = json!({ "content": null, "reasoning_content": "opaque provider reasoning", "tool_calls": [{"id": "call1", "function": {"name": "read_log_file", "arguments": "{\"file_id\":\"log-1\"}"}}] });
        let message = parse_openai(&value).unwrap();
        assert_eq!(message.tool_calls[0].args["file_id"], "log-1");
        let output = openai_messages(&[message], false);
        assert_eq!(
            output[0]["tool_calls"].as_array().unwrap(),
            &value["tool_calls"]
                .as_array()
                .unwrap()
                .iter()
                .map(|call| {
                    let mut call = call.clone();
                    call["type"] = json!("function");
                    call
                })
                .collect::<Vec<_>>()
        );
        assert_eq!(output[0]["reasoning_content"], value["reasoning_content"]);
    }

    #[test]
    fn anthropic_keeps_signatures_and_groups_parallel_tool_results() {
        let value = json!({"content": [{"type": "thinking", "thinking": "reason", "signature": "signed"}, {"type": "tool_use", "id": "call1", "name": "list_log_files", "input": {}}]});
        let message = parse_anthropic(&value).unwrap();
        let mut tool = assistant();
        tool.role = "tool".into();
        tool.content = "result".into();
        tool.tool_call_id = Some("call1".into());
        let (_, output) = anthropic_messages(&[message, tool.clone(), tool]);
        assert_eq!(output[0]["content"], value["content"]);
        assert_eq!(output[1]["content"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn responses_keeps_encrypted_reasoning_and_rejects_invalid_tool_args() {
        let value = json!({"output": [{"type": "reasoning", "encrypted_content": "opaque"}, {"type": "function_call", "call_id": "call1", "name": "list_log_files", "arguments": "{}"}]});
        let result = parse_responses(&value).unwrap();
        assert_eq!(result.opaque, value["output"]);
        assert_eq!(result.tool_calls[0].id, "call1");
        assert!(parse_openai(&json!({"tool_calls": [{"id":"call1", "function":{"name":"read_log_file", "arguments":"[]"}}]})).is_err());
    }
}
