use super::*;

fn image(data: &str) -> ModelImage {
    ModelImage {
        mime_type: "image/png".into(),
        data: data.into(),
    }
}
fn message(role: &str, call: &str) -> ChatMessage {
    let mut result = assistant();
    result.role = role.into();
    result.content = format!("result-{call}");
    result.tool_call_id = Some(call.into());
    result.images = vec![image("aW1hZ2U=")];
    result
}
fn turn(message: ChatMessage) -> ChatTurn {
    ChatTurn {
        messages: vec![message],
        tools: Vec::new(),
    }
}

#[test]
fn chat_and_ollama_keep_parallel_results_adjacent_then_send_all_images() {
    let mut first = message("tool", "call1");
    first.tool_error = true;
    let second = message("tool", "call2");
    let mut user = message("user", "question");
    user.images[0].mime_type = "image/jpeg".into();
    for ollama in [false, true] {
        let output = openai_messages(
            &[first.clone(), second.clone(), user.clone()],
            ollama,
        );
        assert_eq!(output.len(), 4);
        assert_eq!(output[0]["role"], "tool");
        assert_eq!(output[1]["role"], "tool");
        assert_eq!(output[0]["tool_call_id"], "call1");
        assert_eq!(output[1]["tool_call_id"], "call2");
        assert!(output[0]["content"].as_str().unwrap().contains("failed"));
        assert_eq!(output[2]["role"], "user");
        if ollama {
            assert_eq!(output[2]["images"], json!(["aW1hZ2U=", "aW1hZ2U="]));
            assert_eq!(output[3]["images"], json!(["aW1hZ2U="]));
        } else {
            assert_eq!(output[2]["content"].as_array().unwrap().len(), 3);
            assert_eq!(
                output[2]["content"][1]["image_url"]["url"],
                "data:image/png;base64,aW1hZ2U="
            );
            assert_eq!(
                output[3]["content"][1]["image_url"]["url"],
                "data:image/jpeg;base64,aW1hZ2U="
            );
        }
    }
}

#[test]
fn responses_preserves_encrypted_opaque_items_and_multimodal_tool_outputs() {
    let opaque = json!([{"type":"reasoning","id":"reasoning-id","encrypted_content":"encrypted"},{"type":"function_call","id":"provider-id","call_id":"call1","name":"screenshot","arguments":"{}"}]);
    let mut model = assistant();
    model.opaque = opaque.clone();
    let mut tool = message("tool", "call1");
    tool.tool_error = true;
    let (_, output) = responses_messages(&[model, tool]);
    assert_eq!(&output[..2], opaque.as_array().unwrap().as_slice());
    assert_eq!(output[2]["call_id"], "call1");
    assert_eq!(output[2]["output"][0]["type"], "input_text");
    assert!(
        output[2]["output"][0]["text"]
            .as_str()
            .unwrap()
            .contains("failed")
    );
    assert_eq!(
        output[2]["output"][1],
        json!({"type":"input_image","image_url":"data:image/png;base64,aW1hZ2U="})
    );
}

#[test]
fn anthropic_tool_errors_keep_image_sources_and_parallel_result_ids() {
    let mut first = message("tool", "call1");
    first.tool_error = true;
    let (_, output) = anthropic_messages(&[first, message("tool", "call2")]);
    let blocks = &output[0]["content"];
    assert_eq!(blocks.as_array().unwrap().len(), 2);
    assert_eq!(blocks[0]["tool_use_id"], "call1");
    assert_eq!(blocks[0]["is_error"], true);
    assert_eq!(blocks[1]["is_error"], false);
    assert_eq!(
        blocks[0]["content"][1]["source"],
        json!({"type":"base64","media_type":"image/png","data":"aW1hZ2U="})
    );
}

#[test]
fn validates_image_roles_mime_base64_and_decoded_size() {
    for role in ["user", "tool"] {
        assert!(validate(&turn(message(role, "call")), true).is_ok());
    }
    for role in ["assistant", "system", "foreign"] {
        assert!(validate(&turn(message(role, "call")), true).is_err());
    }
    for data in ["", "%%%", "data:image/png;base64,aW1hZ2U=", "aW1hZ2U"] {
        let mut user = message("user", "call");
        user.images[0].data = data.into();
        assert!(validate(&turn(user), true).is_err());
    }
    for mime in ["image/gif", "text/plain"] {
        let mut user = message("user", "call");
        user.images[0].mime_type = mime.into();
        assert!(validate(&turn(user), true).is_err());
    }
    let mut user = message("user", "call");
    user.images[0].data = base64::engine::general_purpose::STANDARD
        .encode(vec![1; MAX_IMAGE_BYTES + 1]);
    assert!(validate(&turn(user), true).is_err());
    let mut user = message("user", "call");
    user.tool_error = true;
    assert!(validate(&turn(user), true).is_err());
}

#[test]
fn agent_context_budget_does_not_expand_diagnostic_budget() {
    let mut user = message("user", "call");
    user.images.clear();
    user.content = "x".repeat(700_000);
    let large_text = turn(user.clone());
    assert!(validate(&large_text, false).is_err());
    assert!(validate(&large_text, true).is_ok());
    user.content = "question".into();
    let mut many = ChatTurn {
        messages: vec![user.clone(); 161],
        tools: vec![],
    };
    assert!(validate(&many, false).is_err());
    assert!(validate(&many, true).is_ok());
    many.messages = vec![user; 601];
    assert!(validate(&many, true).is_err());
    let mut text = assistant();
    text.role = "user".into();
    text.content = "x".repeat(16 * 1024 * 1024);
    assert!(validate(&turn(text), true).is_err());
}

#[test]
fn bedrock_unknown_profiles_keep_parallel_results_then_user_images() {
    let mut first = message("tool", "call1");
    first.content.clear();
    let mut second = message("tool", "call2");
    second.images[0].data = "c2Vjb25k".into();
    let mut model = assistant();
    model.content = "Observed the images".into();
    for profile in [
        "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/custom-alias",
        "custom-vision-alias",
        "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/anthropic.claude-sonnet-4-custom",
    ] {
        let output = super::super::bedrock::messages(
            &[first.clone(), second.clone(), model.clone()],
            bedrock_multimodal_tools(profile),
        );
        assert_eq!(output.len(), 2);
        assert_eq!(output[0]["role"], "user");
        let parts = output[0]["content"].as_array().unwrap();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[0]["toolResult"]["toolUseId"], "call1");
        assert_eq!(parts[1]["toolResult"]["toolUseId"], "call2");
        assert!(parts[0]["toolResult"].get("status").is_none());
        assert_eq!(
            parts[0]["toolResult"]["content"].as_array().unwrap().len(),
            1
        );
        assert_eq!(parts[3]["image"]["source"]["bytes"], "aW1hZ2U=");
        assert_eq!(parts[4]["image"]["source"]["bytes"], "c2Vjb25k");
        assert_eq!(output[1]["role"], "assistant");
    }
    let output = super::super::bedrock::messages(
        &[first, second],
        bedrock_multimodal_tools("us.anthropic.claude-sonnet-4-v1:0"),
    );
    assert_eq!(output[0]["content"].as_array().unwrap().len(), 2);
    assert_eq!(
        output[0]["content"][0]["toolResult"]["content"][0]["image"]["source"]
            ["bytes"],
        "aW1hZ2U="
    );
}

#[test]
fn image_guidance_requires_explicit_provider_image_rejection() {
    let rejected = "AI_PROVIDER_FAILED: Custom returned HTTP 400: This model does not support image inputs";
    let explained = image_error(input_error(rejected.into()), true).to_string();
    assert!(explained.contains("Choose a vision-capable model"));
    assert!(explained.contains(rejected));
    for text in [
        "AI_AUTHENTICATION_FAILED: HTTP 403 unsupported image access permission",
        "AI_RATE_LIMITED: HTTP 429 image quota exceeded",
        "Connection failed: unsupported image proxy",
        "AI_PROVIDER_FAILED: HTTP 400 invalid tool call ID",
    ] {
        let original = input_error(text.into()).to_string();
        assert_eq!(
            image_error(input_error(text.into()), true).to_string(),
            original
        );
    }
    assert_eq!(
        image_error(input_error(rejected.into()), false).to_string(),
        input_error(rejected.into()).to_string()
    );
}

#[tokio::test]
async fn malformed_or_oversized_images_fail_before_runtime_or_network_initialization()
 {
    let model = ChatModel {
        definition: provider_definition("openai").unwrap(),
        config: AiProviderConfig {
            provider_id: "openai".into(),
            custom_name: None,
            protocol: AiProtocol::Openai,
            enabled: true,
            endpoint: "http://127.0.0.1:1".into(),
            settings: HashMap::new(),
            has_api_key: false,
            configured_credentials: vec![],
            oauth_connected: false,
            models: vec![],
        },
        model_id: "configured-model".into(),
        session_id: Uuid::new_v4(),
    };
    let mut user = message("user", "call");
    user.images[0].data = "%%%".into();
    let error = model
        .turn_for_agent(turn(user), &|_, _| {})
        .await
        .unwrap_err();
    assert!(error.to_string().contains("invalid base64"));
    let mut user = message("user", "call");
    user.images[0].data = "A".repeat(MAX_IMAGE_BYTES.div_ceil(3) * 4 + 4);
    let error = model
        .turn_for_agent(turn(user), &|_, _| {})
        .await
        .unwrap_err();
    assert!(error.to_string().contains("7 MiB"));
}

#[test]
fn model_message_wire_retains_images_errors_and_opaque_with_legacy_defaults() {
    let incoming = json!({"role":"tool","content":"failure","tool_call_id":"c1","tool_error":true,"images":[{"mime_type":"image/webp","data":"aW1hZ2U="}],"opaque":{"signature":"retain"}});
    let model: ChatMessage = serde_json::from_value(incoming.clone()).unwrap();
    let wire = serde_json::to_value(model).unwrap();
    for key in ["images", "tool_error", "opaque", "tool_call_id"] {
        assert_eq!(wire[key], incoming[key]);
    }
    let legacy: ChatMessage =
        serde_json::from_value(json!({"role":"user","content":"legacy"}))
            .unwrap();
    assert!(!legacy.tool_error);
    assert!(legacy.images.is_empty());
}
