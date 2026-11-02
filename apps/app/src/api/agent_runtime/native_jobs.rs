//! Host-only completion policy. A successful transport or wrapper operation may
//! only admit a native job; only that job's authoritative observation settles it.
use serde_json::{Map, Value, json};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Query {
    pub tool: &'static str,
    pub arguments: Map<String, Value>,
}

#[derive(Debug, PartialEq)]
pub(super) enum Completion {
    Finished { failed: bool },
    Pending(Query),
    Attention(Query),
    Unconfirmed,
}

fn query(tool: &'static str, arguments: Value) -> Query {
    Query {
        tool,
        arguments: arguments.as_object().unwrap().clone(),
    }
}

fn uuid(value: &Value) -> Option<&str> {
    value
        .as_str()
        .filter(|value| uuid::Uuid::parse_str(value).is_ok())
}

fn install_action(tool: &str) -> bool {
    matches!(
        tool,
        "instance_create"
            | "instance_duplicate"
            | "instance_reinstall"
            | "instance_execute_import"
            | "instance_execute_upgrade"
            | "modpack_execute_install"
            | "resource_execute_plan"
            | "resource_execute_changes"
            | "resource_history_rollback"
            | "java_install"
            | "install_job_retry"
            | "install_job_resume"
            | "install_job_cancel"
            | "download_job_retry"
            | "download_job_resume"
            | "download_job_cancel"
            | "install_job_skip_missing"
            | "install_job_repair_and_retry"
            | "install_job_scan_missing_files"
    )
}

fn finish(failed: bool) -> Completion {
    Completion::Finished { failed }
}

/// `receipt` and `native_observation` are supplied only by the native executor,
/// never by worker events, assistant text, or model arguments.
pub(super) fn classify(
    tool: &str,
    arguments: &Value,
    receipt: &Value,
) -> Completion {
    let Some(entry) = launcher_tools::registry::get(tool) else {
        return Completion::Unconfirmed;
    };
    if receipt["is_error"] == true {
        return finish(true);
    }
    if receipt["is_error"] != false {
        return Completion::Unconfirmed;
    }
    let content = &receipt["structured_content"];
    let payload = if entry.execution.deferred {
        let Some(id) = uuid(&content["operation_id"]) else {
            return Completion::Unconfirmed;
        };
        // Admission has no tool field; observations must match the producing tool.
        if content.get("tool").is_some() && content["tool"] != tool {
            return Completion::Unconfirmed;
        }
        match content["status"].as_str() {
            Some("queued" | "running") => {
                return Completion::Pending(query(
                    "operations_get",
                    json!({"operation_id":id}),
                ));
            }
            Some("failed" | "cancelled") => return finish(true),
            Some("succeeded") if content["tool"] == tool => &content["result"],
            _ => return Completion::Unconfirmed,
        }
    } else {
        content
    };

    if install_action(tool) {
        let initial = if tool == "install_job_scan_missing_files" {
            &payload["job"]
        } else {
            payload
        };
        let Some(id) = uuid(&initial["job_id"]) else {
            return Completion::Unconfirmed;
        };
        // A completed scan that still needs manual files has not admitted a
        // resumed installation. Its own result is the scan counts, not install success.
        if tool == "install_job_scan_missing_files"
            && initial["status"] == "waiting_for_user"
        {
            return finish(false);
        }
        let follow = query("install_job_get", json!({"job_id":id}));
        let observed =
            if let Some(observation) = content.get("native_observation") {
                if observation["is_error"] != false
                    || observation["structured_content"]["job_id"] != id
                {
                    return Completion::Attention(follow);
                }
                &observation["structured_content"]
            } else {
                initial
            };
        let cancelling =
            matches!(tool, "install_job_cancel" | "download_job_cancel");
        return match observed["status"].as_str() {
            Some("succeeded") => finish(cancelling),
            Some("canceled") => finish(!cancelling),
            Some("failed" | "interrupted") => finish(true),
            Some("queued" | "running" | "canceling") => {
                Completion::Pending(follow)
            }
            _ => Completion::Attention(follow),
        };
    }

    if matches!(
        tool,
        "backup_create"
            | "backup_preview_restore"
            | "backup_restore"
            | "backup_cancel"
    ) {
        if tool == "backup_cancel" && payload["accepted"] == false {
            return finish(true);
        }
        if tool == "backup_cancel" && payload["accepted"] != true {
            return Completion::Unconfirmed;
        }
        let initial_id = if tool == "backup_cancel" {
            &arguments["backup_operation_id"]
        } else {
            &payload["backup_operation_id"]
        };
        let Some(id) = uuid(initial_id) else {
            return Completion::Unconfirmed;
        };
        let instance = if tool == "backup_restore" {
            &payload["instance_id"]
        } else {
            &arguments["instance_id"]
        };
        let Some(instance) = instance.as_str().filter(|id| !id.is_empty())
        else {
            return Completion::Unconfirmed;
        };
        let follow = query(
            "backup_get_operation",
            json!({"instance_id":instance,"backup_operation_id":id}),
        );
        let Some(observation) = content.get("native_observation") else {
            return Completion::Pending(follow);
        };
        let observed = &observation["structured_content"];
        if observation["is_error"] != false
            || observed["backup_operation_id"] != id
            || observed["instance_id"] != instance
        {
            return Completion::Attention(follow);
        }
        return match observed["state"].as_str() {
            Some("completed") => finish(tool == "backup_cancel"),
            Some("cancelled") => finish(tool != "backup_cancel"),
            Some("failed" | "interrupted") => finish(true),
            Some(
                "queued" | "scanning" | "hashing" | "saving" | "validating"
                | "materializing" | "applying",
            ) => Completion::Pending(follow),
            _ => Completion::Attention(follow),
        };
    }
    finish(false)
}

/// Keep the producing response and its IDs intact while adding native evidence.
pub(super) fn observe(
    receipt: &mut Value,
    query: &Query,
    output: Value,
) -> bool {
    if query.tool == "operations_get" {
        if output["is_error"] != false
            || output["structured_content"]["operation_id"]
                != query.arguments["operation_id"]
        {
            return false;
        }
        *receipt = output;
    } else {
        receipt["structured_content"]["native_observation"] = output;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    const ID: &str = "00000000-0000-4000-8000-000000000001";
    const OTHER: &str = "00000000-0000-4000-8000-000000000002";

    fn result(value: Value) -> Value {
        json!({"is_error":false,"structured_content":value})
    }
    fn wrapper(tool: &str, payload: Value) -> Value {
        result(
            json!({"operation_id":ID,"tool":tool,"status":"succeeded","result":payload}),
        )
    }

    #[test]
    fn queued_native_job_is_not_completed_by_wrapper_or_model_text() {
        let mut receipt =
            wrapper("java_install", json!({"job_id":OTHER,"status":"queued"}));
        let Completion::Pending(query) =
            classify("java_install", &json!({}), &receipt)
        else {
            panic!("admission is not completion")
        };
        assert_eq!(query.tool, "install_job_get");
        assert_eq!(query.arguments["job_id"], OTHER);
        receipt["structured_content"]["native_observation"] =
            json!({"message":"succeeded"});
        assert!(matches!(
            classify("java_install", &json!({}), &receipt),
            Completion::Attention(_)
        ));
        observe(
            &mut receipt,
            &query,
            result(json!({"job_id":OTHER,"status":"succeeded"})),
        );
        assert_eq!(
            classify("java_install", &json!({}), &receipt),
            finish(false)
        );
        assert_eq!(receipt["structured_content"]["result"]["job_id"], OTHER);
    }

    #[test]
    fn every_install_producer_requires_its_native_job() {
        for tool in [
            "instance_create",
            "instance_duplicate",
            "instance_reinstall",
            "instance_execute_import",
            "instance_execute_upgrade",
            "modpack_execute_install",
            "resource_execute_plan",
            "resource_execute_changes",
            "resource_history_rollback",
            "java_install",
            "install_job_retry",
            "install_job_resume",
            "install_job_cancel",
            "download_job_retry",
            "download_job_resume",
            "download_job_cancel",
            "install_job_skip_missing",
            "install_job_repair_and_retry",
        ] {
            let payload = json!({"job_id":OTHER,"status":"running"});
            let receipt = if tool == "instance_create" {
                result(payload)
            } else {
                wrapper(tool, payload)
            };
            assert!(
                matches!(
                    classify(tool, &json!({}), &receipt),
                    Completion::Pending(_)
                ),
                "{tool}"
            );
            let missing = if tool == "instance_create" {
                result(json!({}))
            } else {
                wrapper(tool, json!({}))
            };
            assert_eq!(
                classify(tool, &json!({}), &missing),
                Completion::Unconfirmed,
                "{tool}"
            );
        }
    }

    #[test]
    fn cancellation_requires_canceled_and_queries_returned_retry_identity() {
        for status in [
            "queued",
            "running",
            "canceling",
            "waiting_for_user",
            "succeeded",
            "failed",
            "interrupted",
            "canceled",
            "invented",
        ] {
            let receipt = wrapper(
                "download_job_cancel",
                json!({"job_id":OTHER,"status":status}),
            );
            let value = classify(
                "download_job_cancel",
                &json!({"job_id":ID}),
                &receipt,
            );
            match status {
                "canceled" => assert_eq!(value, finish(false)),
                "succeeded" | "failed" | "interrupted" => {
                    assert_eq!(value, finish(true))
                }
                "waiting_for_user" | "invented" => {
                    assert!(matches!(value, Completion::Attention(_)))
                }
                _ => assert!(matches!(value, Completion::Pending(_))),
            }
        }
        let receipt = wrapper(
            "download_job_retry",
            json!({"job_id":OTHER,"status":"running"}),
        );
        let Completion::Pending(query) =
            classify("download_job_retry", &json!({"job_id":ID}), &receipt)
        else {
            panic!()
        };
        assert_eq!(query.arguments["job_id"], OTHER);
    }

    #[test]
    fn backup_admission_cancel_and_instance_ownership_are_checked() {
        for tool in [
            "backup_create",
            "backup_preview_restore",
            "backup_restore",
            "backup_cancel",
        ] {
            let args =
                json!({"instance_id":"instance","backup_operation_id":ID});
            let payload = json!({"instance_id":"instance","backup_operation_id":ID,"accepted":true});
            let mut receipt = if tool == "backup_cancel" {
                result(payload)
            } else {
                wrapper(tool, payload)
            };
            let Completion::Pending(query) = classify(tool, &args, &receipt)
            else {
                panic!("{tool}")
            };
            assert_eq!(query.tool, "backup_get_operation");
            for state in [
                "queued",
                "scanning",
                "hashing",
                "saving",
                "validating",
                "materializing",
                "applying",
                "completed",
                "failed",
                "interrupted",
                "cancelled",
                "invented",
            ] {
                observe(
                    &mut receipt,
                    &query,
                    result(
                        json!({"instance_id":"instance","backup_operation_id":ID,"state":state}),
                    ),
                );
                let value = classify(tool, &args, &receipt);
                match state {
                    "completed" => {
                        assert_eq!(value, finish(tool == "backup_cancel"))
                    }
                    "cancelled" => {
                        assert_eq!(value, finish(tool != "backup_cancel"))
                    }
                    "failed" | "interrupted" => assert_eq!(value, finish(true)),
                    "invented" => {
                        assert!(matches!(value, Completion::Attention(_)))
                    }
                    _ => assert!(matches!(value, Completion::Pending(_))),
                }
            }
            observe(
                &mut receipt,
                &query,
                result(
                    json!({"instance_id":"foreign","backup_operation_id":ID,"state":"completed"}),
                ),
            );
            assert!(matches!(
                classify(tool, &args, &receipt),
                Completion::Attention(_)
            ));
        }
        assert_eq!(
            classify(
                "backup_cancel",
                &json!({}),
                &result(json!({"accepted":false}))
            ),
            finish(true)
        );
    }

    #[test]
    fn scan_and_preflight_validation_finish_without_claiming_installation() {
        let receipt = wrapper(
            "install_job_scan_missing_files",
            json!({"job":{"job_id":ID,"status":"waiting_for_user"},"imported_count":0}),
        );
        assert_eq!(
            classify("install_job_scan_missing_files", &json!({}), &receipt),
            finish(false)
        );
        let error = json!({"is_error":true,"structured_content":{"error":{"code":"plan_blocked"}}});
        assert_eq!(
            classify("resource_execute_plan", &json!({}), &error),
            finish(true)
        );
        // Unrelated results with job-like JSON do not become native install authority.
        assert_eq!(
            classify(
                "instance_get",
                &json!({}),
                &result(json!({"job_id":ID,"status":"running"}))
            ),
            finish(false)
        );
        let queued = result(json!({"operation_id":ID,"status":"queued"}));
        assert!(matches!(
            classify("java_install", &json!({}), &queued),
            Completion::Pending(_)
        ));
        assert_eq!(
            classify(
                "java_install",
                &json!({}),
                &wrapper("backup_create", json!({}))
            ),
            Completion::Unconfirmed
        );
    }

    #[test]
    fn recovery_can_requery_attention_but_foreign_observations_never_settle() {
        let mut receipt =
            result(json!({"job_id":ID,"status":"waiting_for_user"}));
        let Completion::Attention(query) =
            classify("instance_create", &json!({}), &receipt)
        else {
            panic!()
        };
        observe(
            &mut receipt,
            &query,
            result(json!({"job_id":OTHER,"status":"succeeded"})),
        );
        assert!(matches!(
            classify("instance_create", &json!({}), &receipt),
            Completion::Attention(_)
        ));
        observe(
            &mut receipt,
            &query,
            result(json!({"job_id":ID,"status":"succeeded"})),
        );
        assert_eq!(
            classify("instance_create", &json!({}), &receipt),
            finish(false)
        );

        let mut wrapper_receipt =
            result(json!({"operation_id":ID,"status":"queued"}));
        let Completion::Pending(query) =
            classify("java_install", &json!({}), &wrapper_receipt)
        else {
            panic!()
        };
        let before = wrapper_receipt.clone();
        assert!(!observe(
            &mut wrapper_receipt,
            &query,
            result(
                json!({"operation_id":OTHER,"tool":"java_install","status":"succeeded"})
            )
        ));
        assert_eq!(wrapper_receipt, before);
        assert!(!observe(
            &mut wrapper_receipt,
            &query,
            json!({"is_error":true})
        ));
        assert_eq!(wrapper_receipt, before);
    }
}
