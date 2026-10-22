use super::*;

pub(super) async fn turn(
    model: &ChatModel,
    client: &reqwest::Client,
    token: String,
    turn: &ChatTurn,
    emit: &(impl Fn(&str, &str) + Sync),
) -> crate::Result<ChatMessage> {
    let mut builder = if model.definition.id == "vertexai" {
        let credentials = read_credential(
            model.definition.id,
            VERTEX_SERVICE_ACCOUNT_CREDENTIAL,
        )?
        .ok_or_else(|| {
            input_error("Configure Vertex AI credentials".to_string())
        })?;
        let (account, access_token) = vertex_access_token(&credentials).await?;
        let project = model
            .config
            .settings
            .get("project")
            .filter(|value| !value.is_empty())
            .unwrap_or(&account.project_id);
        let location = model
            .config
            .settings
            .get("location")
            .or_else(|| model.config.settings.get("region"))
            .map(String::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or("global");
        let base = if model.config.endpoint.is_empty() {
            if location == "global" {
                "https://aiplatform.googleapis.com".to_string()
            } else {
                format!("https://{location}-aiplatform.googleapis.com")
            }
        } else {
            model.config.endpoint.clone()
        };
        client.post(format!("{}/v1/projects/{}/locations/{}/publishers/google/models/{}:streamGenerateContent?alt=sse", base.trim_end_matches('/'), urlencoding::encode(project), urlencoding::encode(location), urlencoding::encode(&model.model_id))).bearer_auth(access_token)
    } else {
        client
            .post(format!(
                "{}/v1beta/models/{}:streamGenerateContent?alt=sse",
                model.config.endpoint.trim_end_matches('/'),
                urlencoding::encode(&model.model_id)
            ))
            .header("x-goog-api-key", token)
    };
    let (system, contents) = messages(&turn.messages);
    let declarations = turn.tools.iter().map(|tool| json!({ "name": tool["function"]["name"], "description": tool["function"]["description"], "parameters": tool["function"]["parameters"] })).collect::<Vec<_>>();
    let mut body = json!({ "systemInstruction": { "parts": [{ "text": system }] }, "contents": contents });
    if !declarations.is_empty() {
        body["tools"] = json!([{ "functionDeclarations": declarations }]);
    }
    builder = builder.json(&body);
    stream::read(
        builder.send().await?,
        model.definition.name,
        stream::Format::Google,
        emit,
    )
    .await
}

fn messages(messages: &[ChatMessage]) -> (String, Vec<Value>) {
    let mut contents: Vec<Value> = Vec::new();
    let mut system = String::new();
    for message in messages {
        if message.role == "system" {
            system.push_str(&message.content);
            system.push('\n');
            continue;
        }
        let parts = if message.role == "tool" {
            let call = messages
                .iter()
                .flat_map(|message| &message.tool_calls)
                .find(|call| Some(&call.id) == message.tool_call_id.as_ref());
            let mut response = json!({ "name": call.map(|call| call.name.as_str()).unwrap_or("unknown"), "response": { "result": message.content } });
            if let Some(call) = call
                && call.id.starts_with("google:")
            {
                response["id"] = json!(call.id.trim_start_matches("google:"));
            }
            json!([{ "functionResponse": response }])
        } else if message.role == "assistant" && message.opaque.is_array() {
            message.opaque.clone()
        } else {
            json!([{ "text": message.content }])
        };
        let role = if message.role == "assistant" {
            "model"
        } else {
            "user"
        };
        if let Some(last) =
            contents.last_mut().filter(|last| last["role"] == role)
        {
            last["parts"]
                .as_array_mut()
                .unwrap()
                .extend(parts.as_array().unwrap().iter().cloned());
        } else {
            contents.push(json!({ "role": role, "parts": parts }));
        }
    }
    (system, contents)
}

pub(super) fn parse(value: &Value) -> crate::Result<ChatMessage> {
    let parts = value["candidates"][0]["content"]["parts"]
        .as_array()
        .ok_or_else(|| {
            input_error("Google returned no LogAgent output".to_string())
        })?;
    let mut result = assistant();
    for part in parts {
        if part["thought"] != true
            && let Some(text) = part["text"].as_str()
        {
            result.content.push_str(text);
        }
        if let Some(call) = part.get("functionCall") {
            result.tool_calls.push(ToolCall {
                id: call["id"]
                    .as_str()
                    .map(|id| format!("google:{id}"))
                    .unwrap_or_else(|| Uuid::new_v4().to_string()),
                name: call["name"].as_str().unwrap_or_default().to_string(),
                args: call["args"].clone(),
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
    fn preserves_thought_signatures_call_identity_and_parallel_results() {
        let value = json!({"candidates":[{"content":{"parts":[{"thought":true,"text":"private thought"},{"functionCall":{"id":"g1","name":"read_log_file","args":{"file_id":"log-1"}},"thoughtSignature":"signed"}]}}]});
        let assistant = parse(&value).unwrap();
        assert!(assistant.content.is_empty());
        let mut result = super::super::assistant();
        result.role = "tool".into();
        result.content = "evidence".into();
        result.tool_call_id = Some("google:g1".into());
        let (_, contents) = messages(&[assistant, result.clone(), result]);
        assert_eq!(
            contents[0]["parts"],
            value["candidates"][0]["content"]["parts"]
        );
        assert_eq!(contents[1]["parts"][0]["functionResponse"]["id"], "g1");
        assert_eq!(
            contents[1]["parts"][0]["functionResponse"]["name"],
            "read_log_file"
        );
        assert_eq!(contents[1]["parts"].as_array().unwrap().len(), 2);
    }
}
