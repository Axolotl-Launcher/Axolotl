use crate::*;
use futures::future::BoxFuture;
use serde_json::{Map, Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU32, Ordering},
};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct TestBackend {
    calls: Mutex<Vec<(Command, String)>>,
    block: AtomicBool,
    fail: AtomicBool,
    panic: AtomicBool,
    finished: AtomicU32,
    resume: tokio::sync::Notify,
    plans: ScopedRegistry<String>,
}

impl Backend for TestBackend {
    fn execute(
        &self,
        command: Command,
        context: BackendContext,
    ) -> BoxFuture<'_, Result<Value, ToolError>> {
        Box::pin(async move {
            self.calls
                .lock()
                .unwrap()
                .push((command.clone(), context.session_id.clone()));
            if self.block.load(Ordering::SeqCst) {
                self.resume.notified().await;
            }
            assert!(
                !self.panic.load(Ordering::SeqCst),
                "deliberate backend panic"
            );
            self.finished.fetch_add(1, Ordering::SeqCst);
            if self.fail.load(Ordering::SeqCst) {
                return Err(ToolError::new("test_failure", "Backend failed"));
            }
            match command {
                Command::InstanceDeletePreview { .. } => {
                    let id = self.plans.insert(
                        &context.session_id,
                        0,
                        "reviewed".into(),
                    )?;
                    Ok(json!({"preview_id":id}))
                }
                Command::InstanceDelete { preview_id } => Ok(
                    json!({"deleted":self.plans.take(&context.session_id, preview_id, 0)?}),
                ),
                Command::UiScreenshot {}
                | Command::ScreenshotGet { .. }
                | Command::SeedRenderTile { .. } => Ok(
                    json!({"image":{"data":"iVBORw0KGgoAAA","mime_type":"image/png"},"context":{"view_id":"view"}}),
                ),
                _ => Ok(json!({"executed":true})),
            }
        })
    }
}

fn setup() -> (Arc<ToolRuntime>, Arc<TestBackend>) {
    let backend = Arc::new(TestBackend::default());
    (
        Arc::new(ToolRuntime::new(LauncherInfo::new("test"), backend.clone())),
        backend,
    )
}

fn context(full: bool) -> ExecutionContext {
    ExecutionContext::new(
        ExecutionScope::local(full, CancellationToken::new()),
        CancellationToken::new(),
    )
}

async fn call(
    runtime: &Arc<ToolRuntime>,
    context: &ExecutionContext,
    name: &str,
    arguments: Value,
) -> ToolOutput {
    runtime
        .call(
            name,
            arguments.as_object().unwrap().clone(),
            context.clone(),
        )
        .await
        .unwrap()
}

async fn wait_for(predicate: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !predicate() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

async fn poll(
    runtime: &Arc<ToolRuntime>,
    context: &ExecutionContext,
    id: &str,
    status: &str,
) -> ToolOutput {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let output = call(
                runtime,
                context,
                "operations_get",
                json!({"operation_id":id}),
            )
            .await;
            if output.structured_content["status"] == status {
                return output;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}

fn sample(rule: &Value) -> Value {
    if let Some(choices) = rule["enum"].as_array() {
        return choices[0].clone();
    }
    match rule["type"].as_str().unwrap() {
        "string" => match rule["format"].as_str() {
            Some("uuid") => json!(uuid::Uuid::nil()),
            Some("sha256") => json!("a".repeat(64)),
            Some("image-filename") => json!("image.png"),
            Some("log-filename") => json!("latest.log"),
            Some("account-id") => json!(format!("account:{}", "a".repeat(64))),
            _ => json!("test"),
        },
        "integer" => rule.get("minimum").cloned().unwrap_or(json!(1)),
        "boolean" => json!(true),
        "array" => Value::Array(
            (0..rule["minItems"].as_u64().unwrap_or(1))
                .map(|_| sample(&rule["items"]))
                .collect(),
        ),
        _ => panic!("unsupported registry schema: {rule}"),
    }
}

#[test]
fn every_definition_has_a_unique_name_valid_schema_and_typed_dispatch() {
    let mut names = std::collections::HashSet::new();
    for entry in registry::catalog() {
        assert!(names.insert(entry.name.as_ref()));
        assert!(
            !entry.input_schema["properties"]
                .as_object()
                .unwrap()
                .contains_key("session_id")
        );
        let arguments: Map<_, _> = entry.input_schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|key| {
                let name = key.as_str().unwrap();
                (name.into(), sample(&entry.input_schema["properties"][name]))
            })
            .collect();
        registry::validate(entry, &arguments).unwrap_or_else(|error| {
            panic!("{}: {}", entry.name, error.message)
        });
        if entry.access != Access::Public
            && !entry.name.starts_with("operations_")
        {
            serde_json::from_value::<Command>(
                json!({"name":entry.name,"arguments":arguments}),
            )
            .unwrap_or_else(|error| panic!("{}: {error}", entry.name));
        }
    }
    assert_eq!(names.len(), 267);
}

#[test]
fn scheduling_metadata_preserves_the_pre_refactor_contract() {
    let text: String = registry::catalog()
        .iter()
        .map(|entry| {
            format!(
                "{}:{}:{}\n",
                entry.name,
                entry.execution.deferred,
                entry.execution.interrupts_mutation
            )
        })
        .collect();
    let fingerprint = text.bytes().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    assert_eq!(fingerprint, 0x167cae213a1af557);
}

#[test]
fn backend_context_revalidates_live_permissions_and_cancellation() {
    let permission = Arc::new(AtomicBool::new(true));
    let cancel = CancellationToken::new();
    let scope = ExecutionScope::new("owner".into(), {
        let permission = permission.clone();
        let cancel = cancel.clone();
        move |full| {
            if full && !permission.load(Ordering::SeqCst) {
                return Err(ToolError::new(
                    "full_access_required",
                    "Permission removed",
                ));
            }
            Ok(cancel.clone())
        }
    });
    let context = BackendContext::new(scope);
    context.authorize(true).unwrap();
    permission.store(false, Ordering::SeqCst);
    assert_eq!(
        context.authorize(true).unwrap_err().code,
        "full_access_required"
    );
    context.authorize(false).unwrap();
    cancel.cancel();
    assert_eq!(
        context.authorize(false).unwrap_err().code,
        "session_revoked"
    );
}

#[tokio::test]
async fn local_calls_need_no_mcp_session_and_validate_before_dispatch() {
    let (runtime, backend) = setup();
    let context = context(false);
    let output = call(&runtime, &context, "instance_list", json!({})).await;
    assert!(!output.is_error);
    assert_eq!(backend.calls.lock().unwrap()[0].1, context.scope.id());
    for (name, args) in [
        ("instance_list", json!({"session_id":"spoof"})),
        (
            "instance_rename",
            json!({"instance_id":"instance","name":""}),
        ),
        (
            "file_read_text",
            json!({"target_kind":"instance","target_id":"instance","path":"../secret"}),
        ),
        ("not_a_tool", json!({})),
        ("start_session", json!({})),
    ] {
        assert!(
            runtime
                .call(name, args.as_object().unwrap().clone(), context.clone())
                .await
                .is_err(),
            "{name}"
        );
    }
    assert_eq!(backend.calls.lock().unwrap().len(), 1);
    let output = call(&runtime, &context, "ui_screenshot", json!({})).await;
    assert_eq!(
        output.structured_content["error"]["code"],
        "full_access_required"
    );
    assert!(output.is_error);
}

#[tokio::test]
async fn reviewed_plans_and_operations_are_owned_and_single_use_without_connections()
 {
    let (runtime, _) = setup();
    let owner = context(true);
    let other = context(true);
    let preview = call(
        &runtime,
        &owner,
        "instance_delete_preview",
        json!({"instance_id":"instance"}),
    )
    .await;
    let id = &preview.structured_content["preview_id"];
    let foreign = call(
        &runtime,
        &other,
        "instance_delete",
        json!({"preview_id":id}),
    )
    .await;
    let foreign_id =
        foreign.structured_content["operation_id"].as_str().unwrap();
    let failed = poll(&runtime, &other, foreign_id, "failed").await;
    assert_eq!(
        failed.structured_content["result"]["error"]["code"],
        "plan_not_found"
    );
    let result = call(
        &runtime,
        &owner,
        "instance_delete",
        json!({"preview_id":id}),
    )
    .await;
    let operation = result.structured_content["operation_id"].as_str().unwrap();
    assert_eq!(
        poll(&runtime, &owner, operation, "succeeded")
            .await
            .structured_content["result"]["deleted"],
        "reviewed"
    );
    assert_eq!(
        call(
            &runtime,
            &other,
            "operations_get",
            json!({"operation_id":operation})
        )
        .await
        .structured_content["error"]["code"],
        "not_found"
    );
    let repeated = call(
        &runtime,
        &owner,
        "instance_delete",
        json!({"preview_id":id}),
    )
    .await;
    assert_eq!(
        poll(
            &runtime,
            &owner,
            repeated.structured_content["operation_id"]
                .as_str()
                .unwrap(),
            "failed"
        )
        .await
        .structured_content["result"]["error"]["code"],
        "plan_not_found"
    );
}

#[tokio::test]
async fn queued_writes_revalidate_permissions_and_cancel_before_admission() {
    let (runtime, backend) = setup();
    let allowed = Arc::new(AtomicBool::new(true));
    let token = CancellationToken::new();
    let permission = allowed.clone();
    let authorization_token = token.clone();
    let owner = ExecutionContext::new(
        ExecutionScope::new("trusted-owner".into(), move |full| {
            if full && !permission.load(Ordering::SeqCst) {
                return Err(ToolError::new(
                    "full_access_required",
                    "Permission removed",
                ));
            }
            Ok(authorization_token.clone())
        }),
        CancellationToken::new(),
    );
    let permit = runtime.mutations.lock().await;
    let output = call(&runtime, &owner, "file_trash", json!({"target_kind":"instance","target_id":"instance","path":"file.txt","expected_sha256":"a".repeat(64)})).await;
    let id = output.structured_content["operation_id"].as_str().unwrap();
    allowed.store(false, Ordering::SeqCst);
    drop(permit);
    assert_eq!(
        poll(&runtime, &owner, id, "failed")
            .await
            .structured_content["result"]["error"]["code"],
        "full_access_required"
    );
    assert!(backend.calls.lock().unwrap().is_empty());

    let permit = runtime.mutations.lock().await;
    let other = context(true);
    let future = tokio::spawn({
        let runtime = runtime.clone();
        let other = other.clone();
        async move {
            call(
                &runtime,
                &other,
                "instance_rename",
                json!({"instance_id":"instance","name":"New"}),
            )
            .await
        }
    });
    tokio::task::yield_now().await;
    other.request_cancel.cancel();
    let output = future.await.unwrap();
    assert_eq!(output.structured_content["error"]["code"], "cancelled");
    drop(permit);
    assert!(backend.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn dropping_a_write_caller_drains_admitted_work_and_revocation_cancels_queued_work()
 {
    let (runtime, backend) = setup();
    backend.block.store(true, Ordering::SeqCst);
    let cancel = CancellationToken::new();
    let owner = ExecutionContext::new(
        ExecutionScope::local(true, cancel.clone()),
        CancellationToken::new(),
    );
    let running = tokio::spawn({
        let runtime = runtime.clone();
        let owner = owner.clone();
        async move {
            call(
                &runtime,
                &owner,
                "instance_rename",
                json!({"instance_id":"instance","name":"New"}),
            )
            .await
        }
    });
    wait_for(|| backend.calls.lock().unwrap().len() == 1).await;
    let queued = call(
        &runtime,
        &owner,
        "instance_launch",
        json!({"instance_id":"instance"}),
    )
    .await;
    let id = queued.structured_content["operation_id"].as_str().unwrap();
    running.abort();
    cancel.cancel();
    wait_for(|| {
        runtime.operations.lock().unwrap().status_for_test(id)
            == Some("cancelled")
    })
    .await;
    backend.resume.notify_one();
    wait_for(|| backend.finished.load(Ordering::SeqCst) == 1).await;
    assert_eq!(backend.calls.lock().unwrap().len(), 1);
    assert_eq!(
        call(&runtime, &owner, "instance_list", json!({}))
            .await
            .structured_content["error"]["code"],
        "session_revoked"
    );
}

#[tokio::test]
async fn reads_cancel_and_backend_panics_leave_terminal_operation_records() {
    let (runtime, backend) = setup();
    backend.block.store(true, Ordering::SeqCst);
    let owner = context(true);
    let reading = tokio::spawn({
        let runtime = runtime.clone();
        let owner = owner.clone();
        async move { call(&runtime, &owner, "instance_list", json!({})).await }
    });
    wait_for(|| !backend.calls.lock().unwrap().is_empty()).await;
    owner.request_cancel.cancel();
    assert_eq!(
        reading.await.unwrap().structured_content["error"]["code"],
        "cancelled"
    );
    assert_eq!(backend.finished.load(Ordering::SeqCst), 0);
    backend.block.store(false, Ordering::SeqCst);
    backend.panic.store(true, Ordering::SeqCst);
    let owner = context(true);
    let output = call(
        &runtime,
        &owner,
        "instance_launch",
        json!({"instance_id":"instance"}),
    )
    .await;
    let failed = poll(
        &runtime,
        &owner,
        output.structured_content["operation_id"].as_str().unwrap(),
        "failed",
    )
    .await;
    assert_eq!(
        failed.structured_content["result"]["error"]["code"],
        "execution_failed"
    );
}

#[tokio::test]
async fn results_preserve_images_metadata_and_execution_errors() {
    let (runtime, backend) = setup();
    let owner = context(true);
    let image = call(&runtime, &owner, "ui_screenshot", json!({})).await;
    assert!(
        matches!(&image.content[0], ContentBlock::Image { mime_type, .. } if mime_type=="image/png")
    );
    assert_eq!(image.structured_content["context"]["view_id"], "view");
    assert!(!image.structured_content.to_string().contains("iVBOR"));
    let deferred = ToolOutput::from_result(
        "operations_get",
        Ok(
            json!({"status":"succeeded","operation_id":"op","result":{"image":{"data":"iVBORw0KGgoAAA","mime_type":"image/png"},"context":{"tile":1}}}),
        ),
    );
    assert!(matches!(deferred.content[0], ContentBlock::Image { .. }));
    assert_eq!(deferred.structured_content["operation_id"], "op");
    assert_eq!(deferred.structured_content["result"]["context"]["tile"], 1);
    assert!(!deferred.structured_content.to_string().contains("iVBOR"));
    assert!(ToolOutput::from_result("ui_screenshot",Ok(json!({"image":{"data":"javascript:bad","mime_type":"image/png"}}))).is_error);
    backend.fail.store(true, Ordering::SeqCst);
    let error = call(&runtime, &owner, "instance_list", json!({})).await;
    assert!(error.is_error);
    assert_eq!(error.structured_content["error"]["code"], "test_failure");
    let backend_context = BackendContext::new(owner.scope);
    backend_context.authorize(true).unwrap();
}
