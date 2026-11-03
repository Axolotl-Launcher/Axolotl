use super::LauncherBackend;
use launcher_tools::{BackendContext, Command, ToolError};
use serde_json::{Value, json};

pub(super) async fn execute(
    backend: &LauncherBackend,
    command: Command,
    context: BackendContext,
) -> Result<Value, ToolError> {
    let app = backend.app.as_ref().ok_or_else(|| {
        ToolError::new("ui_unavailable", "Main launcher UI is unavailable")
    })?;
    let full = !matches!(command, Command::UiGetContext {});
    let action = match command {
        Command::UiGetContext {} => json!({"kind":"context"}),
        Command::UiScreenshot {} => json!({"kind":"screenshot"}),
        Command::UiNavigate { path } => json!({"kind":"navigate","path":path}),
        Command::UiClick {
            view_id,
            element_id,
        } => json!({"kind":"click","view_id":view_id,"element_id":element_id}),
        Command::UiType {
            view_id,
            element_id,
            text,
        } => {
            json!({"kind":"type","view_id":view_id,"element_id":element_id,"text":text})
        }
        Command::UiKey {
            view_id,
            key,
            shift,
            control,
        } => {
            json!({"kind":"key","view_id":view_id,"key":key,"shift":shift,"control":control})
        }
        Command::UiScroll {
            view_id,
            element_id,
            direction,
            pixels,
        } => {
            json!({"kind":"scroll","view_id":view_id,"element_id":element_id,"direction":direction,"pixels":pixels})
        }
        _ => {
            return Err(ToolError::new(
                "unsupported_command",
                "Not a UI operation",
            ));
        }
    };
    super::super::interface::request(app, context, full, action).await
}
