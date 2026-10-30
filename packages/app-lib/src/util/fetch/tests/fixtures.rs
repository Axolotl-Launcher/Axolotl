//! Fixtures shared by the fetch tests.

use super::super::super::download::constants::MAX_REDIRECT_LOCATION_BYTES;
use super::super::transfer::download_to_path_inner;
use super::super::*;
use crate::util::download::constants::{
    DOWNLOAD_SCHEDULING_GRACE, ROUTE_PROBE_CONCURRENCY,
};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock};
use std::time;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::Semaphore;

pub(super) static RANGE_SPLITTING_TEST_LOCK: LazyLock<AsyncMutex<()>> =
    LazyLock::new(|| AsyncMutex::new(()));

pub(super) static AUTO_SOURCE_TEST_LOCK: LazyLock<std::sync::Mutex<()>> =
    LazyLock::new(|| std::sync::Mutex::new(()));

pub(super) static H2_FALLBACK_TEST_LOCK: LazyLock<std::sync::Mutex<()>> =
    LazyLock::new(|| std::sync::Mutex::new(()));

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct RangeServerBehavior {
    pub(super) wrong_content_range: bool,
    pub(super) ignore_range: bool,
    pub(super) slow_body: bool,
    pub(super) fail_first_range: bool,
    pub(super) stall_first_range: bool,
    pub(super) vary_mod_validators: bool,
    pub(super) vary_range_validators: bool,
}

pub(super) async fn spawn_range_server(
    data: Arc<Vec<u8>>,
    behavior: RangeServerBehavior,
) -> (
    String,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
    tokio::task::JoinHandle<()>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(AtomicUsize::new(0));
    let request_count = requests.clone();
    let normal_requests = Arc::new(AtomicUsize::new(0));
    let normal_request_count = normal_requests.clone();
    let failed_range = Arc::new(AtomicBool::new(false));
    let stalled_range = Arc::new(AtomicBool::new(false));
    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            let data = data.clone();
            let requests = request_count.clone();
            let normal_requests = normal_request_count.clone();
            let failed_range = Arc::clone(&failed_range);
            let stalled_range = Arc::clone(&stalled_range);
            tokio::spawn(async move {
                requests.fetch_add(1, Ordering::Relaxed);
                let mut request = Vec::new();
                let mut buffer = [0_u8; 1024];
                loop {
                    let Ok(read) = stream.read(&mut buffer).await else {
                        return;
                    };
                    if read == 0 {
                        return;
                    }
                    request.extend_from_slice(&buffer[..read]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                let request =
                    String::from_utf8_lossy(&request).to_ascii_lowercase();
                let requested_range = request
                    .lines()
                    .find_map(|line| line.strip_prefix("range: bytes="));
                let (mut headers, body) = if let Some(range) =
                    requested_range.filter(|_| !behavior.ignore_range)
                {
                    let Some((start, end)) = range.split_once('-') else {
                        return;
                    };
                    let Ok(start) = start.parse::<u64>() else {
                        return;
                    };
                    let end = if end.is_empty() {
                        data.len() as u64 - 1
                    } else {
                        let Ok(end) = end.parse::<u64>() else {
                            return;
                        };
                        end
                    };
                    let body = &data[start as usize..=end as usize];
                    let reported_start = if behavior.wrong_content_range {
                        start.saturating_add(1)
                    } else {
                        start
                    };
                    (
                        format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {reported_start}-{end}/{}\r\nETag: \"fixture\"\r\nConnection: close\r\n\r\n",
                            body.len(),
                            data.len(),
                        ),
                        body,
                    )
                } else {
                    if requested_range.is_none() {
                        normal_requests.fetch_add(1, Ordering::Relaxed);
                    }
                    (
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"fixture\"\r\nConnection: close\r\n\r\n",
                            data.len(),
                        ),
                        &data[..],
                    )
                };
                if let Some(range) = requested_range
                    && (behavior.vary_range_validators
                        || (behavior.vary_mod_validators
                            && request
                                .lines()
                                .next()
                                .is_some_and(|line| line.contains(".jar "))))
                {
                    headers = headers.replace(
                        "\"fixture\"",
                        &format!("\"fixture-{range}\""),
                    );
                }
                if stream.write_all(headers.as_bytes()).await.is_err() {
                    return;
                }
                if requested_range.is_some()
                    && behavior.stall_first_range
                    && !stalled_range.swap(true, Ordering::Relaxed)
                {
                    tokio::time::sleep(DOWNLOAD_SCHEDULING_GRACE).await;
                    return;
                }
                if requested_range.is_some()
                    && behavior.fail_first_range
                    && !failed_range.swap(true, Ordering::Relaxed)
                {
                    let midpoint = body.len() / 2;
                    let _ = stream.write_all(&body[..midpoint]).await;
                    return;
                }
                for chunk in body.chunks(64 * 1024) {
                    if stream.write_all(chunk).await.is_err() {
                        return;
                    }
                    if behavior.slow_body {
                        tokio::time::sleep(time::Duration::from_millis(100))
                            .await;
                    }
                }
            });
        }
    });
    (
        format!("http://{address}/file"),
        requests,
        normal_requests,
        handle,
    )
}

pub(super) async fn spawn_stream_server(
    chunks: Vec<(Duration, Vec<u8>)>,
    content_length: usize,
    hold_open: bool,
) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let chunks = Arc::new(chunks);
    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            let chunks = Arc::clone(&chunks);
            tokio::spawn(async move {
                let mut request = Vec::new();
                let mut buffer = [0_u8; 1024];
                loop {
                    let Ok(read) = stream.read(&mut buffer).await else {
                        return;
                    };
                    if read == 0 {
                        return;
                    }
                    request.extend_from_slice(&buffer[..read]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                let headers = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {content_length}\r\nConnection: close\r\n\r\n"
                );
                if stream.write_all(headers.as_bytes()).await.is_err() {
                    return;
                }
                for (delay, chunk) in chunks.iter() {
                    tokio::time::sleep(*delay).await;
                    if stream.write_all(chunk).await.is_err() {
                        return;
                    }
                }
                if hold_open {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            });
        }
    });
    (format!("http://{address}/file"), handle)
}

pub(super) async fn spawn_json_server(
    body: String,
) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(AtomicUsize::new(0));
    let request_count = Arc::clone(&requests);
    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            let requests = Arc::clone(&request_count);
            let body = body.clone();
            tokio::spawn(async move {
                requests.fetch_add(1, Ordering::Relaxed);
                let mut buffer = [0_u8; 1024];
                let _ = stream.read(&mut buffer).await;
                let headers = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(headers.as_bytes()).await;
                let _ = stream.write_all(body.as_bytes()).await;
            });
        }
    });
    (
        format!("http://{address}/tag/game_version"),
        requests,
        handle,
    )
}

pub(super) struct HttpFixture {
    pub(super) status_line: &'static str,
    pub(super) extra_headers: &'static str,
    pub(super) body: Vec<u8>,
    pub(super) response_delay: Duration,
}

impl HttpFixture {
    pub(super) fn new(
        status_line: &'static str,
        body: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            status_line,
            extra_headers: "",
            body: body.into(),
            response_delay: Duration::ZERO,
        }
    }

    pub(super) fn with_headers(mut self, headers: &'static str) -> Self {
        self.extra_headers = headers;
        self
    }

    pub(super) fn with_delay(mut self, delay: Duration) -> Self {
        self.response_delay = delay;
        self
    }
}

pub(super) async fn spawn_http_fixture(
    fixture: HttpFixture,
) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(AtomicUsize::new(0));
    let request_count = Arc::clone(&requests);
    let status_line = fixture.status_line.to_string();
    let extra_headers = fixture.extra_headers.to_string();
    let body = Arc::new(fixture.body);
    let response_delay = fixture.response_delay;
    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            let requests = Arc::clone(&request_count);
            let status_line = status_line.clone();
            let extra_headers = extra_headers.clone();
            let body = Arc::clone(&body);
            tokio::spawn(async move {
                requests.fetch_add(1, Ordering::Relaxed);
                let mut request = Vec::new();
                let mut buffer = [0_u8; 1024];
                loop {
                    let Ok(read) = stream.read(&mut buffer).await else {
                        return;
                    };
                    if read == 0 {
                        return;
                    }
                    request.extend_from_slice(&buffer[..read]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                tokio::time::sleep(response_delay).await;
                let response = format!(
                    "HTTP/1.1 {status_line}\r\nContent-Length: {}\r\n{extra_headers}Connection: close\r\n\r\n",
                    body.len(),
                );
                if stream.write_all(response.as_bytes()).await.is_ok() {
                    let _ = stream.write_all(&body).await;
                }
            });
        }
    });
    (format!("http://{address}/file"), requests, handle)
}

pub(super) fn direct_test_route(
    url: String,
    source: DownloadRouteSource,
) -> DownloadRoute {
    DownloadRoute {
        url,
        source,
        is_mirror: source != DownloadRouteSource::Official,
        allow_sensitive_headers: source == DownloadRouteSource::Official,
        supports_range: false,
        proxy: ProxyPolicy::Direct,
    }
}

pub(super) async fn spawn_redirect_server(
    location: String,
    response_delay: Duration,
) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(AtomicUsize::new(0));
    let request_count = Arc::clone(&requests);
    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            let location = location.clone();
            let requests = Arc::clone(&request_count);
            tokio::spawn(async move {
                requests.fetch_add(1, Ordering::Relaxed);
                let mut request = Vec::new();
                let mut buffer = [0_u8; 1024];
                loop {
                    let Ok(read) = stream.read(&mut buffer).await else {
                        return;
                    };
                    if read == 0 {
                        return;
                    }
                    request.extend_from_slice(&buffer[..read]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                tokio::time::sleep(response_delay).await;
                let response = format!(
                    "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });
    (format!("http://{address}/redirect"), requests, handle)
}

pub(super) async fn test_download(
    url: &str,
    destination: &Path,
    expected_size: u64,
) -> crate::Result<DownloadResult> {
    download_to_path(
        DownloadRequest::new(url, ResourceClass::Other)
            .with_integrity(Integrity::default().with_size(expected_size)),
        destination,
        &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
        None,
    )
    .await
}

/// A redirect location the download will not follow: longer than the cap the
/// client allows, or outside ASCII.
pub(super) fn is_safe_redirect_location(location: &str) -> bool {
    location.len() <= MAX_REDIRECT_LOCATION_BYTES && location.is_ascii()
}

pub(crate) async fn download_endpoint_fixture(
    request: DownloadRequest,
    destination: &Path,
    semaphore: &FetchSemaphore,
) -> crate::Result<DownloadResult> {
    download_to_path_inner(request, destination, semaphore, None).await
}

pub(crate) async fn download_benchmark_fixture(
    request: DownloadRequest,
    destination: &Path,
    semaphore: &FetchSemaphore,
) -> crate::Result<DownloadResult> {
    BENCHMARK_TIMING
        .scope(
            (),
            download_to_path_inner(request, destination, semaphore, None),
        )
        .await
}
