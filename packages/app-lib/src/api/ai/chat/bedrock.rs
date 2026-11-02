use super::*;

pub(super) async fn turn(
    model: &ChatModel,
    client: &reqwest::Client,
    token: String,
    turn: &ChatTurn,
) -> crate::Result<ChatMessage> {
    let region = model
        .config
        .settings
        .get("region")
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("us-east-1");
    let base = if model.config.endpoint.is_empty() {
        format!("https://bedrock-runtime.{region}.amazonaws.com")
    } else {
        model.config.endpoint.clone()
    };
    let endpoint = format!(
        "{}/model/{}/converse",
        base.trim_end_matches('/'),
        urlencoding::encode(&model.model_id)
    );
    let system = turn
        .messages
        .iter()
        .filter(|message| message.role == "system")
        .map(|message| json!({"text": message.content}))
        .collect::<Vec<_>>();
    let tools = turn.tools.iter().map(|tool| json!({"toolSpec": {"name": tool["function"]["name"], "description": tool["function"]["description"], "inputSchema": {"json": tool["function"]["parameters"]}}})).collect::<Vec<_>>();
    let mut body = json!({ "system": system, "messages": messages(&turn.messages, content::bedrock_multimodal_tools(&model.model_id)), "inferenceConfig": { "maxTokens": 4096 } });
    if !tools.is_empty() {
        body["toolConfig"] = json!({ "tools": tools });
    }
    let body = body.to_string();
    let mut builder = client
        .post(&endpoint)
        .header("Content-Type", "application/json");
    if model.config.settings.get("auth_mode").map(String::as_str)
        == Some("api-key")
        || (!token.is_empty()
            && model.config.settings.get("auth_mode").map(String::as_str)
                != Some("aws-credentials"))
    {
        if token.is_empty() {
            return Err(input_error(
                "Configure the Bedrock API key".to_string(),
            ));
        }
        builder = builder.bearer_auth(token);
    } else {
        let access =
            read_credential(model.definition.id, AWS_ACCESS_KEY_ID_CREDENTIAL)?
                .ok_or_else(|| {
                    input_error("Configure Bedrock AWS credentials".to_string())
                })?;
        let secret = read_credential(
            model.definition.id,
            AWS_SECRET_ACCESS_KEY_CREDENTIAL,
        )?
        .ok_or_else(|| {
            input_error("Configure Bedrock AWS credentials".to_string())
        })?;
        let session =
            read_credential(model.definition.id, AWS_SESSION_TOKEN_CREDENTIAL)?;
        builder = sign_bedrock_request(
            builder,
            &endpoint,
            region,
            &body,
            &access,
            &secret,
            session.as_deref(),
        )?;
    }
    let value =
        checked_json(builder.body(body).send().await?, model.definition.name)
            .await?;
    parse(&value)
}

pub(super) fn messages(
    messages: &[ChatMessage],
    status_supported: bool,
) -> Vec<Value> {
    let mut output: Vec<Value> = Vec::new();
    let mut tool_images = Vec::new();
    for (index, message) in messages
        .iter()
        .enumerate()
        .filter(|(_, message)| message.role != "system")
    {
        let content = if message.role == "tool" {
            if !status_supported {
                tool_images.extend(message.images.iter().cloned());
            }
            let mut result_parts =
                parts(message, !status_supported, status_supported);
            if result_parts.is_empty() {
                result_parts.push(
                    json!({"text":"Tool completed with no text output."}),
                );
            }
            let mut result = json!({"toolUseId":message.tool_call_id,"content":result_parts});
            if status_supported {
                result["status"] = json!(if message.tool_error {
                    "error"
                } else {
                    "success"
                });
            }
            json!([{"toolResult":result}])
        } else if message.role == "assistant" && message.opaque.is_array() {
            message.opaque.clone()
        } else {
            json!(parts(message, false, true))
        };
        let role = if message.role == "assistant" {
            "assistant"
        } else {
            "user"
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
        // Unknown inference profile aliases may still route to vision models.
        // Keep their tool results adjacent, then attach images to the same user
        // Content rather than guessing support for nested toolResult.image.
        if !tool_images.is_empty()
            && messages
                .get(index + 1)
                .is_none_or(|next| next.role != "tool")
        {
            let mut images = super::assistant();
            images.role = "user".into();
            images.content = "Images returned by the preceding tool results, in the same order:".into();
            images.images = std::mem::take(&mut tool_images);
            output.last_mut().unwrap()["content"]
                .as_array_mut()
                .unwrap()
                .extend(parts(&images, false, true));
        }
    }
    output
}

fn parts(
    message: &ChatMessage,
    describe_error: bool,
    include_images: bool,
) -> Vec<Value> {
    let mut parts = Vec::new();
    let text = if describe_error {
        content::text(message)
    } else {
        message.content.clone()
    };
    if !text.is_empty() {
        parts.push(json!({"text":text}));
    }
    if include_images {
        parts.extend(message.images.iter().map(|image| json!({"image":{"format":image.mime_type.trim_start_matches("image/"),"source":{"bytes":image.data}}})));
    }
    parts
}

fn parse(value: &Value) -> crate::Result<ChatMessage> {
    let parts = value["output"]["message"]["content"]
        .as_array()
        .ok_or_else(|| {
            input_error("Bedrock returned no LogAgent output".to_string())
        })?;
    let mut result = assistant();
    for part in parts {
        if let Some(text) = part["text"].as_str() {
            result.content.push_str(text);
        }
        if let Some(call) = part.get("toolUse") {
            result.tool_calls.push(ToolCall {
                id: call["toolUseId"].as_str().unwrap_or_default().to_string(),
                name: call["name"].as_str().unwrap_or_default().to_string(),
                args: call["input"].clone(),
            });
        }
    }
    result.opaque = json!(parts);
    validate_output(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_signed_reasoning_and_groups_parallel_results() {
        let value = json!({"output":{"message":{"content":[{"reasoningContent":{"reasoningText":{"text":"reason","signature":"signed"}}},{"toolUse":{"toolUseId":"b1","name":"list_log_files","input":{}}}]}}});
        let assistant = parse(&value).unwrap();
        let mut result = super::super::assistant();
        result.role = "tool".into();
        result.content = "files".into();
        result.tool_call_id = Some("b1".into());
        let output = messages(&[assistant, result.clone(), result], false);
        assert_eq!(output[0]["content"], value["output"]["message"]["content"]);
        assert_eq!(output[1]["content"][0]["toolResult"]["toolUseId"], "b1");
        assert_eq!(output[1]["content"].as_array().unwrap().len(), 2);
    }
    #[test]
    fn tool_result_images_keep_bytes_and_error_status_with_text_fallback() {
        let mut tool = super::super::assistant();
        tool.role = "tool".into();
        tool.content = "capture failed".into();
        tool.tool_error = true;
        tool.tool_call_id = Some("call1".into());
        tool.images.push(ModelImage {
            mime_type: "image/jpeg".into(),
            data: "aW1hZ2U=".into(),
        });
        let output = messages(&[tool.clone()], true);
        assert_eq!(output[0]["content"][0]["toolResult"]["status"], "error");
        assert_eq!(
            output[0]["content"][0]["toolResult"]["content"][1],
            json!({"image":{"format":"jpeg","source":{"bytes":"aW1hZ2U="}}})
        );
        tool.images.clear();
        let output = messages(&[tool], false);
        let result = &output[0]["content"][0]["toolResult"];
        assert!(result.get("status").is_none());
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("failed")
        );
    }
}
