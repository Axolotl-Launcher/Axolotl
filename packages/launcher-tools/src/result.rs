use crate::ToolError;
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
    Image { data: String, mime_type: String },
}

#[derive(Clone, Debug, Serialize)]
pub struct ToolOutput {
    pub content: Vec<ContentBlock>,
    pub structured_content: Value,
    pub is_error: bool,
}

impl ToolOutput {
    pub fn structured(value: Value) -> Self {
        Self {
            content: vec![ContentBlock::Text {
                text: value.to_string(),
            }],
            structured_content: value,
            is_error: false,
        }
    }

    pub fn from_result(name: &str, result: Result<Value, ToolError>) -> Self {
        let result = result.and_then(|mut value| {
            if matches!(name, "ui_screenshot" | "screenshot_get") {
                Self::screenshot(value)
            } else if name == "operations_get"
                && value["status"] == "succeeded"
                && value["result"]["image"].is_object()
            {
                let mut image = Self::screenshot(value["result"].take())?;
                value["result"] = image.structured_content;
                image.structured_content = value;
                Ok(image)
            } else {
                Ok(Self::structured(value))
            }
        });
        match result {
            Ok(output) => output,
            Err(error) => Self {
                is_error: true,
                ..Self::structured(json!({"error": error}))
            },
        }
    }

    fn screenshot(mut value: Value) -> Result<Self, ToolError> {
        let image =
            value.get_mut("image").map(Value::take).ok_or_else(|| {
                ToolError::new(
                    "invalid_screenshot",
                    "Launcher did not return a screenshot",
                )
            })?;
        let data = image["data"]
            .as_str()
            .filter(|data| {
                data.starts_with("iVBORw0KGgo") && data.len() <= 7 * 1024 * 1024
            })
            .filter(|_| image["mime_type"] == "image/png")
            .ok_or_else(|| {
                ToolError::new(
                    "invalid_screenshot",
                    "Invalid or oversized launcher PNG",
                )
            })?;
        Ok(Self {
            content: vec![
                ContentBlock::Image {
                    data: data.into(),
                    mime_type: "image/png".into(),
                },
                ContentBlock::Text {
                    text: value["context"].to_string(),
                },
            ],
            structured_content: json!({"context": value["context"]}),
            is_error: false,
        })
    }
}
