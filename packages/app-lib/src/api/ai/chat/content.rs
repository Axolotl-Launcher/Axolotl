use super::*;

pub(super) fn responses_tools(tools: &[Value]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            let function = &tool["function"];
            // Responses otherwise normalizes schemas toward strict mode. Registry
            // optional fields must stay optional, just as they are over MCP.
            json!({
                "type": "function",
                "name": function["name"],
                "description": function["description"],
                "parameters": function["parameters"],
                "strict": function["strict"].as_bool().unwrap_or(false),
            })
        })
        .collect()
}

pub(super) fn openai_messages(
    messages: &[ChatMessage],
    ollama: bool,
) -> Vec<Value> {
    let mut output = Vec::new();
    let mut tool_images = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        let mut value =
            json!({ "role": message.role, "content": text(message) });
        if message.role == "tool" {
            value["tool_call_id"] = json!(message.tool_call_id);
        }
        if !message.images.is_empty() {
            if message.role == "tool" {
                tool_images.extend(message.images.iter().cloned());
            } else if ollama {
                value["images"] = json!(
                    message
                        .images
                        .iter()
                        .map(|image| &image.data)
                        .collect::<Vec<_>>()
                );
            } else {
                value["content"] = json!(openai_parts(
                    &message.content,
                    &message.images,
                    false
                ));
            }
        }
        if !message.tool_calls.is_empty() {
            value["tool_calls"] = json!(message.tool_calls.iter().map(|call| json!({ "id": call.id, "type": "function", "function": { "name": call.name, "arguments": if ollama { call.args.clone() } else { json!(call.args.to_string()) } } })).collect::<Vec<_>>());
        }
        if message.role == "assistant" {
            for key in ["reasoning_content", "reasoning_details"] {
                if let Some(reasoning) =
                    message.opaque.get(key).filter(|value| !value.is_null())
                {
                    value[key] = reasoning.clone();
                }
            }
        }
        output.push(value);
        // Tool results for a parallel batch must remain adjacent. Chat Completions
        // cannot carry image blocks on a tool message, so follow the entire batch.
        if !tool_images.is_empty()
            && messages
                .get(index + 1)
                .is_none_or(|next| next.role != "tool")
        {
            let label = "Images returned by the preceding tool results, in the same order:";
            let images = if ollama {
                json!({"role":"user","content":label,"images":tool_images.iter().map(|image| &image.data).collect::<Vec<_>>()})
            } else {
                json!({"role":"user","content":openai_parts(label, &tool_images, false)})
            };
            output.push(images);
            tool_images.clear();
        }
    }
    output
}

pub(super) fn anthropic_messages(
    messages: &[ChatMessage],
) -> (String, Vec<Value>) {
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
            json!([{ "type": "tool_result", "tool_use_id": message.tool_call_id, "content": anthropic_parts(message), "is_error": message.tool_error }])
        } else if message.role == "assistant" && message.opaque.is_array() {
            message.opaque.clone()
        } else {
            let mut parts = anthropic_parts(message);
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

const MAX_IMAGE_BYTES: usize = 7 * 1024 * 1024;

pub(super) fn validate(turn: &ChatTurn, agent: bool) -> crate::Result<()> {
    let (messages, bytes, label) = if agent {
        (600, 16 * 1024 * 1024, "Agent")
    } else {
        (160, 700_000, "LogAgent")
    };
    if turn.messages.is_empty()
        || turn.messages.len() > messages
        || serde_json::to_vec(turn)?.len() > bytes
    {
        return Err(input_error(format!("{label} context budget exceeded")));
    }
    for message in &turn.messages {
        if !matches!(
            message.role.as_str(),
            "system" | "user" | "assistant" | "tool"
        ) {
            return Err(input_error(format!("Invalid {label} message role")));
        }
        if (!message.images.is_empty()
            && !matches!(message.role.as_str(), "user" | "tool"))
            || (message.tool_error && message.role != "tool")
        {
            return Err(input_error("Images belong to user/tool messages; tool_error belongs to tool messages".into()));
        }
        for image in &message.images {
            if !matches!(
                image.mime_type.as_str(),
                "image/png" | "image/jpeg" | "image/webp"
            ) {
                return Err(input_error(
                    "Unsupported image MIME type; use PNG, JPEG or WebP".into(),
                ));
            }
            if image.data.is_empty()
                || image.data.len() > MAX_IMAGE_BYTES.div_ceil(3) * 4
            {
                return Err(input_error(
                    "Image must contain nonempty base64 and be at most 7 MiB"
                        .into(),
                ));
            }
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(&image.data)
                .map_err(|_| {
                    input_error("Image contains invalid base64".into())
                })?;
            if decoded.is_empty() || decoded.len() > MAX_IMAGE_BYTES {
                return Err(input_error(
                    "Image must contain nonempty base64 and be at most 7 MiB"
                        .into(),
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn bedrock_multimodal_tools(model: &str) -> bool {
    if model.starts_with("arn:") {
        return false;
    }
    model.contains("amazon.nova")
        || model.contains("anthropic.claude-3")
        || model.contains("anthropic.claude-4")
        || model.contains("anthropic.claude-sonnet-4")
        || model.contains("anthropic.claude-opus-4")
        || model.contains("anthropic.claude-haiku-4")
}

pub(super) fn image_error(
    error: crate::Error,
    has_images: bool,
) -> crate::Error {
    let message = error.to_string();
    let lower = message.to_ascii_lowercase();
    if has_images
        && lower.contains("ai_provider_failed")
        && (lower.contains("image") || lower.contains("vision"))
        && ["not support", "unsupported", "doesn't support"]
            .iter()
            .any(|phrase| lower.contains(phrase))
    {
        return input_error(format!(
            "{message}\nThe selected provider/model rejected image input. Choose a vision-capable model or remove the images."
        ));
    }
    error
}

pub(super) fn text(message: &ChatMessage) -> String {
    if message.role == "tool" && message.tool_error {
        format!("Tool execution failed: {}", message.content)
    } else {
        message.content.clone()
    }
}

fn data_url(image: &ModelImage) -> String {
    format!("data:{};base64,{}", image.mime_type, image.data)
}

fn openai_parts(
    text: &str,
    images: &[ModelImage],
    responses: bool,
) -> Vec<Value> {
    let mut parts = Vec::new();
    if !text.is_empty() {
        parts.push(if responses {
            json!({"type":"input_text","text":text})
        } else {
            json!({"type":"text","text":text})
        });
    }
    for image in images {
        parts.push(if responses {
            json!({"type":"input_image","image_url":data_url(image)})
        } else {
            json!({"type":"image_url","image_url":{"url":data_url(image)}})
        });
    }
    parts
}

fn anthropic_parts(message: &ChatMessage) -> Vec<Value> {
    let mut parts = Vec::new();
    if !message.content.is_empty() {
        parts.push(json!({"type":"text","text":message.content}));
    }
    parts.extend(message.images.iter().map(|image| json!({"type":"image","source":{"type":"base64","media_type":image.mime_type,"data":image.data}})));
    parts
}

pub(super) fn responses_messages(
    messages: &[ChatMessage],
) -> (String, Vec<Value>) {
    let mut input = Vec::new();
    let mut instructions = String::new();
    for message in messages {
        match message.role.as_str() {
            "system" => { instructions.push_str(&message.content); instructions.push('\n'); }
            "tool" => input.push(json!({"type":"function_call_output","call_id":message.tool_call_id,"output":if message.images.is_empty() {json!(text(message))} else {json!(openai_parts(&text(message), &message.images, true))}})),
            "assistant" if message.opaque.is_array() => {
                for original in message.opaque.as_array().unwrap() {
                    let mut item = original.clone();
                    if item["type"] == "function_call"
                        && let Some(call) = message.tool_calls.iter().find(|call| item["call_id"] == call.id)
                    {
                        // Keep provider IDs/reasoning intact, but replay the
                        // canonical arguments that the host actually executed.
                        item["name"] = json!(call.name);
                        item["arguments"] = json!(call.args.to_string());
                    }
                    input.push(item);
                }
            }
            "assistant" => {
                if !message.content.is_empty() { input.push(json!({"role":"assistant","content":message.content})); }
                for call in &message.tool_calls { input.push(json!({"type":"function_call","call_id":call.id,"name":call.name,"arguments":call.args.to_string()})); }
            }
            _ => input.push(json!({"role":message.role,"content":if message.images.is_empty() {json!(message.content)} else {json!(openai_parts(&message.content, &message.images, true))}})),
        }
    }
    (instructions, input)
}

#[cfg(test)]
mod tests;
