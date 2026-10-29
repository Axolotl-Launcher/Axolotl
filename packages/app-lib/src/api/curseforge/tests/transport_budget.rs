use super::*;
use std::io::Write;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn waiting_for_admission_has_a_deadline_and_does_not_leak_permits() {
    let semaphore = tokio::sync::Semaphore::new(1);
    let held = semaphore.acquire().await.unwrap();
    let result = api_permit(&semaphore, Duration::from_millis(20)).await;
    let error = result.unwrap_err();
    assert!(error.to_string().contains("timed out"));
    assert_eq!(
        crate::resources::ResourceError::from(error),
        crate::resources::ResourceError::Unavailable
    );
    assert_eq!(semaphore.available_permits(), 0);
    drop(held);
    let result: crate::Result<()> = metadata_deadline(
        tokio::time::Instant::now() + Duration::from_millis(20),
        async {
            let _permit =
                api_permit(&semaphore, Duration::from_secs(1)).await?;
            std::future::pending().await
        },
    )
    .await;
    assert_eq!(
        crate::resources::ResourceError::from(result.unwrap_err()),
        crate::resources::ResourceError::Unavailable
    );
    assert_eq!(semaphore.available_permits(), 1);
}

#[tokio::test]
async fn overall_budget_stops_a_body_that_keeps_the_read_timeout_alive() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 2048];
        socket.read(&mut request).await.unwrap();
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
            .await
            .unwrap();
        for _ in 0..50 {
            if socket.write_all(b"1\r\nx\r\n").await.is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    });
    let client = reqwest::Client::builder()
        .no_proxy()
        .read_timeout(Duration::from_secs(1))
        .build()
        .unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_millis(120);
    let response = client
        .get(format!("http://{address}"))
        .send()
        .await
        .unwrap();
    let result = response_body(response, deadline).await;
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("metadata request timed out")
    );
    server.abort();
    let _ = server.await;
}

#[test]
fn request_identity_distinguishes_filters_and_payloads_without_exposing_them() {
    let query = vec![
        ("pageSize".into(), "50".into()),
        ("index".into(), "0".into()),
    ];
    let mut reordered = query.clone();
    reordered.reverse();
    let body = Some(serde_json::json!({"fingerprints":[987654321, 123456789]}));
    let key =
        request_identity(&Method::POST, "/v1/fingerprints/432", &query, &body);
    assert_eq!(key.len(), 64);
    let first: Value =
        serde_json::from_str(r#"{"nested":{"b":2,"a":1},"z":0}"#).unwrap();
    let second: Value =
        serde_json::from_str(r#"{"z":0,"nested":{"a":1,"b":2}}"#).unwrap();
    assert_eq!(
        request_identity(&Method::POST, "/v1/mods", &[], &Some(first)),
        request_identity(&Method::POST, "/v1/mods", &[], &Some(second))
    );
    assert_eq!(
        key,
        request_identity(
            &Method::POST,
            "/v1/fingerprints/432",
            &reordered,
            &body
        )
    );
    reordered[0].1 = "50".into();
    assert_ne!(
        key,
        request_identity(
            &Method::POST,
            "/v1/fingerprints/432",
            &reordered,
            &body
        )
    );
    assert_ne!(
        key,
        request_identity(
            &Method::POST,
            "/v1/fingerprints/432",
            &query,
            &Some(serde_json::json!({"fingerprints":[123456789, 987654321]}))
        )
    );
}

struct TestWriter(Arc<Mutex<Vec<u8>>>);
impl Write for TestWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buffer);
        Ok(buffer.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn every_guard_exit_records_its_stage_and_outcome_without_sensitive_values() {
    let output = Arc::new(Mutex::new(Vec::new()));
    let writer = output.clone();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(move || TestWriter(writer.clone()))
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        for outcome in ["success", "error", "timeout", "canceled_or_dropped"] {
            let mut trace = RequestTrace::new(
                &Method::POST,
                "/v1/fingerprints/432",
                &[
                    ("searchFilter".into(), "secret-search".into()),
                    ("token".into(), "secret-token".into()),
                    ("pageSize".into(), "50".into()),
                ],
                &Some(
                    serde_json::json!({"fingerprints":[987654321], "secret":"secret-body"}),
                ),
                RequestOrigin::Resource,
            );
            trace.stage("body");
            if outcome != "canceled_or_dropped" {
                trace.outcome = outcome;
            }
        }
    });
    let output = String::from_utf8(output.lock().unwrap().clone()).unwrap();
    assert_eq!(
        output
            .matches("Finished CurseForge metadata request")
            .count(),
        4
    );
    for outcome in ["success", "error", "timeout", "canceled_or_dropped"] {
        assert!(output.contains(outcome));
    }
    assert!(output.contains("stage=\"body\""));
    assert!(output.contains("item_count=1"));
    for secret in ["secret-search", "secret-token", "secret-body", "987654321"]
    {
        assert!(!output.contains(secret));
    }
}
