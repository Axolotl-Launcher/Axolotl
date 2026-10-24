use super::multiplayer::SESSION_TEST;
use super::*;
use tokio::net::TcpListener;

#[tokio::test]
#[ignore = "requires official relay access and a running local Minecraft server; set HONGSHI_MINECRAFT_PORT"]
async fn official_kernel_forwards_a_real_minecraft_connection() {
    let _serial = SESSION_TEST.lock().await;
    let port: u16 = std::env::var("HONGSHI_MINECRAFT_PORT")
        .expect("set the local Minecraft port")
        .parse()
        .unwrap();
    let local = async_minecraft_ping::ConnectionConfig::build("127.0.0.1")
        .with_port(port)
        .with_timeout(Duration::from_secs(10))
        .connect()
        .await
        .unwrap()
        .status()
        .await
        .unwrap();
    let version = local.status.version.name.clone();
    local.ping(12345).await.unwrap();
    let client = reqwest::Client::new();
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join(binary_name());
    install_from(
        &client,
        &download_endpoint_for(std::env::consts::OS, std::env::consts::ARCH)
            .unwrap(),
        &binary,
        0,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(compatible_binary(&binary).await);
    let (map, cached) = load_node_map_from(
        &client,
        NODE_ENDPOINT,
        &directory.path().join("nodes.json"),
        &Mutex::new(NodeCache::default()),
        true,
    )
    .await
    .unwrap();
    let nodes = join_all(
        map.into_iter()
            .map(|(name, address)| probe_node(name, address, cached)),
    )
    .await;
    let node = nodes
        .into_iter()
        .filter(|node| node.reachable)
        .min_by_key(|node| node.latency_ms)
        .expect("a reachable official relay");
    let (id, token, done) = reserve_session(Some(port), None).await.unwrap();
    let cancellation = token.clone();
    let (sender, receiver) = oneshot::channel();
    let actor = tokio::spawn(async move {
        let mut ready = Some(sender);
        let result = host_kernels(
            id,
            port,
            Some(node.name.clone()),
            &binary,
            vec![node],
            &cancellation,
            &mut ready,
        )
        .await;
        finish_session(id, &cancellation, result, &mut ready, done).await;
    });
    let acceptance = async {
        receiver.await??;
        let address =
            HONGSHI_STATE.lock().await.public_address.clone().unwrap();
        println!(
            "Official RedStone endpoint: {address}; local Minecraft: {version}"
        );
        if let Ok(path) = std::env::var("HONGSHI_ACCEPTANCE_ENDPOINT_FILE") {
            tokio::fs::write(path, &address).await?;
        }
        let (host, port) = address.rsplit_once(':').unwrap();
        let remote = async_minecraft_ping::ConnectionConfig::build(host)
            .with_port(port.parse().unwrap())
            .with_timeout(Duration::from_secs(15))
            .connect()
            .await?
            .status()
            .await?;
        assert_eq!(remote.status.version.name, version);
        remote.ping(54321).await?;
        if let Ok(seconds) = std::env::var("HONGSHI_ACCEPTANCE_HOLD_SECONDS") {
            tokio::time::sleep(Duration::from_secs(seconds.parse()?)).await;
        }
        Ok::<_, eyre::Report>(())
    }
    .await;
    multiplayer::shutdown().await.unwrap();
    actor.await.unwrap();
    assert!(HONGSHI_RUNTIME.lock().await.session.is_none());
    assert!(HONGSHI_STATE.lock().await.public_address.is_none());
    assert!(
        multiplayer::claim_provider(MultiplayerProvider::Hongshi)
            .await
            .is_err()
    );
    acceptance.unwrap();
}

fn fixture() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/api/hongshi/kernel-fixture.rs");
    assert!(
        std::process::Command::new("rustc")
            .arg("--edition=2024")
            .arg(source)
            .arg("-o")
            .arg(directory.path().join(binary_name()))
            .status()
            .unwrap()
            .success()
    );
    directory
}

fn test_node(address: &str) -> HongshiNode {
    HongshiNode {
        name: address.into(),
        address: address.into(),
        latency_ms: Some(1),
        reachable: true,
        cached: false,
    }
}

#[tokio::test]
async fn direct_binary_install_succeeds_then_cached_help_is_reused() {
    let _serial = SESSION_TEST.lock().await;
    let fixture = fixture();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(binary_name());
    let binary = std::fs::read(fixture.path().join(binary_name())).unwrap();
    let (url, server) = http_server(vec![(
        200,
        "Content-Type: application/octet-stream\r\n".into(),
        binary.clone(),
    )])
    .await;
    install_from(
        &reqwest::Client::new(),
        &url,
        &path,
        0,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), binary);
    assert!(compatible_binary(&path).await);
    assert!(compatible_binary(&path).await);
    assert_eq!(
        std::fs::read_to_string(directory.path().join("help-count")).unwrap(),
        "2"
    );
    std::io::Write::write_all(
        &mut std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap(),
        b"changed",
    )
    .unwrap();
    assert!(compatible_binary(&path).await);
    assert_eq!(
        std::fs::read_to_string(directory.path().join("help-count")).unwrap(),
        "3"
    );
    assert_eq!(server.await.unwrap().len(), 1);
    let blocked = directory.path().join("blocked");
    std::fs::create_dir_all(&blocked).unwrap();
    let (url, server) = http_server(vec![(200, String::new(), binary)]).await;
    assert!(
        install_from(
            &reqwest::Client::new(),
            &url,
            &blocked,
            0,
            &CancellationToken::new()
        )
        .await
        .is_err()
    );
    assert!(blocked.is_dir());
    server.await.unwrap();
}

#[tokio::test]
async fn node_attempts_fallback_only_in_automatic_mode_and_exit_zero_closes() {
    let _serial = SESSION_TEST.lock().await;
    let fixture = fixture();
    let binary = fixture.path().join(binary_name());
    for (first, manual, expected) in [
        ("error.example.com", false, "closed"),
        ("timeout.example.com", false, "closed"),
        ("error.example.com", true, "error"),
        ("abnormal.example.com", false, "error"),
        ("empty-close.example.com", false, "early-close"),
        ("immediate.example.com", false, "immediate-close"),
    ] {
        let (id, token, done) =
            reserve_session(Some(25565), None).await.unwrap();
        let (sender, receiver) = oneshot::channel();
        let mut ready = Some(sender);
        let result = host_kernels(
            id,
            25565,
            manual.then(|| first.into()),
            &binary,
            vec![test_node(first), test_node("closed.example.com")],
            &token,
            &mut ready,
        )
        .await;
        let snapshot = HONGSHI_STATE.lock().await.clone();
        match expected {
            "closed" => {
                assert!(result.is_ok());
                assert_eq!(snapshot.node.unwrap().name, "closed.example.com");
                assert_eq!(snapshot.status, HongshiStatus::Closed);
                assert!(receiver.await.unwrap().is_ok());
            }
            "early-close" => {
                assert!(result.is_ok());
                assert_eq!(snapshot.status, HongshiStatus::Closed);
                assert_eq!(snapshot.last_exit_code, Some(0));
                assert!(receiver.await.unwrap().is_err());
            }
            "immediate-close" => {
                assert!(result.is_ok());
                assert_eq!(snapshot.status, HongshiStatus::Closed);
                assert_eq!(snapshot.last_exit_code, Some(0));
                let _ = receiver.await.unwrap();
            }
            "error" => {
                assert!(result.is_err());
                assert_eq!(snapshot.node.unwrap().name, first);
                assert_eq!(
                    snapshot.last_exit_code,
                    Some(if manual { 1 } else { 2 })
                );
            }
            _ => unreachable!(),
        }
        assert!(HONGSHI_STATE.lock().await.public_address.is_none());
        finish_session(id, &token, result, &mut ready, done).await;
        assert!(HONGSHI_RUNTIME.lock().await.session.is_none());
    }
}

#[tokio::test]
async fn a_second_endpoint_cannot_replace_the_first_address() {
    let _serial = SESSION_TEST.lock().await;
    let fixture = fixture();
    let (id, token, done) = reserve_session(Some(25565), None).await.unwrap();
    let mut kernel = spawn_kernel(
        &fixture.path().join(binary_name()),
        &test_node("double.example.com"),
        25565,
        id,
    )
    .await
    .unwrap();
    let cancellation = token.clone();
    let (sender, receiver) = oneshot::channel();
    let actor = tokio::spawn(async move {
        let mut ready = Some(sender);
        assert!(matches!(
            supervise_kernel(
                &mut kernel,
                id,
                &cancellation,
                &mut ready,
                START_TIMEOUT
            )
            .await
            .unwrap(),
            Attempt::Cancelled
        ));
        finish_kernel(&mut kernel).await;
        assert!(kernel.child.try_wait().unwrap().is_some());
        assert!(kernel.readers.iter().all(|reader| reader.is_finished()));
        finish_session(
            id,
            &cancellation,
            Err(eyre::eyre!("cancelled")),
            &mut ready,
            done,
        )
        .await;
    });
    receiver.await.unwrap().unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        HONGSHI_STATE.lock().await.public_address.as_deref(),
        Some("relay.example.com:34575")
    );
    stop().await.unwrap();
    actor.await.unwrap();
    assert!(HONGSHI_STATE.lock().await.public_address.is_none());
}

#[tokio::test]
async fn missing_build_can_later_install_a_valid_binary_without_resetting_availability()
 {
    let _serial = SESSION_TEST.lock().await;
    let fixture = fixture();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(binary_name());
    let (url, server) = http_server(vec![
        (
            404,
            String::new(),
            br#"{"detail":{"message":"not uploaded yet"}}"#.to_vec(),
        ),
        (
            200,
            String::new(),
            std::fs::read(fixture.path().join(binary_name())).unwrap(),
        ),
    ])
    .await;
    let client = reqwest::Client::new();
    let token = CancellationToken::new();
    let error = install_from(&client, &url, &path, 0, &token)
        .await
        .unwrap_err();
    assert_eq!(
        classify(&error, HongshiErrorType::Unknown),
        HongshiErrorType::BuildUnavailable
    );
    assert!(!compatible_binary(&path).await);
    install_from(&client, &url, &path, 0, &token).await.unwrap();
    assert!(compatible_binary(&path).await);
    assert_eq!(server.await.unwrap().len(), 2);
}

#[tokio::test]
async fn room_preparation_reuses_a_compatible_v2_kernel_without_requesting_downloads()
 {
    let _serial = SESSION_TEST.lock().await;
    let fixture = fixture();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("v2").join(binary_name());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::copy(fixture.path().join(binary_name()), &path).unwrap();
    for _ in 0..2 {
        assert_eq!(
            prepare_binary_in(
                directory.path(),
                0,
                &CancellationToken::new(),
                false
            )
            .await
            .unwrap(),
            path
        );
    }
    assert_eq!(
        std::fs::read_to_string(path.parent().unwrap().join("help-count"))
            .unwrap(),
        "1"
    );
    assert!(
        !path
            .with_file_name(if cfg!(target_os = "windows") {
                "hongshic.download.exe"
            } else {
                "hongshic.download"
            })
            .exists()
    );
}

#[tokio::test]
async fn download_size_header_is_rejected_before_a_file_is_installed() {
    let directory = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0; 2048];
        let _bytes_read = stream.read(&mut request).await.unwrap();
        stream
			.write_all(
				format!(
					"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
					MAX_BINARY_SIZE + 1
				)
				.as_bytes(),
			)
			.await
			.unwrap();
    });
    let path = directory.path().join(binary_name());
    std::fs::write(&path, b"existing file").unwrap();
    let error = install_from(
        &reqwest::Client::new(),
        &url,
        &path,
        0,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("too large"));
    assert_eq!(std::fs::read(path).unwrap(), b"existing file");
    server.await.unwrap();
}

#[tokio::test]
async fn cancelling_staged_help_reaps_the_process_before_removing_download() {
    let _serial = SESSION_TEST.lock().await;
    let fixture = fixture();
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("slow-help"), b"").unwrap();
    let (url, server) = http_server(vec![(
        200,
        String::new(),
        std::fs::read(fixture.path().join(binary_name())).unwrap(),
    )])
    .await;
    let token = CancellationToken::new();
    let cancellation = token.clone();
    let marker = directory.path().join("help-started");
    let cancel = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(5), async {
            while !marker.exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        cancellation.cancel();
    });
    let error = install_from(
        &reqwest::Client::new(),
        &url,
        &directory.path().join(binary_name()),
        0,
        &token,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("cancelled"));
    assert!(
        !directory
            .path()
            .join(if cfg!(target_os = "windows") {
                "hongshic.download.exe"
            } else {
                "hongshic.download"
            })
            .exists()
    );
    cancel.await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn real_child_output_timeout_exit_and_cancellation_are_supervised() {
    let _serial = SESSION_TEST.lock().await;
    let fixture = fixture();
    let binary = fixture.path().join(binary_name());
    for (address, expected) in [
        ("closed.example.com", "closed"),
        ("immediate.example.com", "immediate"),
        ("error.example.com", "error"),
        ("invalid.example.com", "output"),
        ("timeout.example.com", "timeout"),
        ("noisy.example.com", "cancel"),
    ] {
        let (id, token, done) =
            reserve_session(Some(25565), None).await.unwrap();
        let mut kernel = spawn_kernel(&binary, &test_node(address), 25565, id)
            .await
            .unwrap();
        let (sender, receiver) = oneshot::channel::<eyre::Result<()>>();
        let mut ready = Some(sender);
        if expected == "cancel" {
            let cancel = token.clone();
            tokio::spawn(async move {
                receiver.await.unwrap().unwrap();
                cancel.cancel();
            });
        }
        let attempt = supervise_kernel(
            &mut kernel,
            id,
            &token,
            &mut ready,
            if expected == "timeout" {
                Duration::from_millis(20)
            } else {
                Duration::from_secs(5)
            },
        )
        .await
        .unwrap();
        match expected {
            "closed" => assert!(matches!(attempt, Attempt::Exited(0, true))),
            "immediate" => assert!(matches!(attempt, Attempt::Exited(0, _))),
            "error" => assert!(matches!(attempt, Attempt::Exited(1, false))),
            "output" => assert!(matches!(attempt, Attempt::OutputError(_))),
            "timeout" => assert!(matches!(attempt, Attempt::TimedOut)),
            "cancel" => assert!(matches!(attempt, Attempt::Cancelled)),
            _ => unreachable!(),
        }
        finish_kernel(&mut kernel).await;
        assert!(kernel.child.try_wait().unwrap().is_some());
        assert!(kernel.readers.iter().all(|reader| reader.is_finished()));
        finish_session(id, &token, Ok(()), &mut ready, done).await;
    }
}

#[tokio::test]
async fn switching_provider_cancels_and_reaps_pending_kernel_before_restart() {
    let _serial = SESSION_TEST.lock().await;
    let fixture = fixture();
    let binary = fixture.path().join(binary_name());
    let (id, token, done) = reserve_session(Some(25565), None).await.unwrap();
    let mut kernel =
        spawn_kernel(&binary, &test_node("timeout.example.com"), 25565, id)
            .await
            .unwrap();
    let actor_token = token.clone();
    let actor = tokio::spawn(async move {
        let mut ready = None;
        assert!(matches!(
            supervise_kernel(
                &mut kernel,
                id,
                &actor_token,
                &mut ready,
                START_TIMEOUT
            )
            .await
            .unwrap(),
            Attempt::Cancelled
        ));
        finish_kernel(&mut kernel).await;
        assert!(kernel.child.try_wait().unwrap().is_some());
        assert!(kernel.readers.iter().all(|reader| reader.is_finished()));
        finish_session(
            id,
            &actor_token,
            Err(eyre::eyre!("RedStone operation cancelled")),
            &mut ready,
            done,
        )
        .await;
    });
    tokio::time::timeout(
        Duration::from_secs(5),
        multiplayer::switch_provider(MultiplayerProvider::Terracotta),
    )
    .await
    .unwrap()
    .unwrap();
    actor.await.unwrap();
    assert!(token.is_cancelled());
    assert!(HONGSHI_RUNTIME.lock().await.session.is_none());
    assert!(HONGSHI_STATE.lock().await.public_address.is_none());
    let (next, token, done) = reserve_session(Some(25566), None).await.unwrap();
    assert!(next > id);
    update_state(id, |state| {
        state.public_address = Some("stale.example.com:12345".into())
    })
    .await;
    assert!(HONGSHI_STATE.lock().await.public_address.is_none());
    token.cancel();
    finish_session(
        next,
        &token,
        Err(eyre::eyre!("cancelled")),
        &mut None,
        done,
    )
    .await;
}

#[tokio::test]
async fn truncated_node_response_falls_back_to_validated_v2_disk_cache() {
    let directory = TestDirectory::new();
    let path = directory.0.join("nodes.json");
    std::fs::write(&path, br#"{"cached region":"relay.example.com"}"#).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0; 2048];
        let _bytes_read = stream.read(&mut request).await.unwrap();
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{").await.unwrap();
    });
    let (map, cached) = load_node_map_from(
        &reqwest::Client::new(),
        &url,
        &path,
        &Mutex::new(NodeCache::default()),
        true,
    )
    .await
    .unwrap();
    assert!(cached);
    assert_eq!(map["cached region"], "relay.example.com");
    server.await.unwrap();
}

#[tokio::test]
async fn node_fallback_ignores_legacy_and_invalid_v2_cache_files() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("nodes.json"),
        br#"{"legacy":"old.example.com"}"#,
    )
    .unwrap();
    let path = directory.path().join("v2/nodes.json");
    let (url, server) = http_server(vec![
        (503, String::new(), Vec::new()),
        (503, String::new(), Vec::new()),
        (503, String::new(), Vec::new()),
    ])
    .await;
    let client = reqwest::Client::new();
    let cache = Mutex::new(NodeCache::default());
    assert!(
        load_node_map_from(&client, &url, &path, &cache, true)
            .await
            .is_err()
    );
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, br#"{"invalid":"127.0.0.1"}"#).unwrap();
    assert!(
        load_node_map_from(&client, &url, &path, &cache, true)
            .await
            .is_err()
    );
    std::fs::write(&path, br#"{"v2":"new.example.com"}"#).unwrap();
    let (map, cached) = load_node_map_from(&client, &url, &path, &cache, true)
        .await
        .unwrap();
    assert!(cached);
    assert_eq!(map.len(), 1);
    assert_eq!(map["v2"], "new.example.com");
    assert_eq!(server.await.unwrap().len(), 3);
}

#[tokio::test]
async fn bound_game_port_changes_require_restart_and_game_exit_reaps_the_kernel()
 {
    let _serial = SESSION_TEST.lock().await;
    let fixture = fixture();
    observe_minecraft_log(
        "bound-game",
        "Game",
        "process",
        "Local game hosted on port 25565",
    )
    .await;
    let (id, token, done) =
        reserve_session(Some(25565), Some("bound-game".into()))
            .await
            .unwrap();
    let mut kernel = spawn_kernel(
        &fixture.path().join(binary_name()),
        &test_node("double.example.com"),
        25565,
        id,
    )
    .await
    .unwrap();
    let cancellation = token.clone();
    let (sender, receiver) = oneshot::channel();
    let actor = tokio::spawn(async move {
        let mut ready = Some(sender);
        assert!(matches!(
            supervise_kernel(
                &mut kernel,
                id,
                &cancellation,
                &mut ready,
                START_TIMEOUT
            )
            .await
            .unwrap(),
            Attempt::Cancelled
        ));
        finish_kernel(&mut kernel).await;
        assert!(kernel.child.try_wait().unwrap().is_some());
        assert!(kernel.readers.iter().all(|reader| reader.is_finished()));
        finish_session(
            id,
            &cancellation,
            Err(eyre::eyre!("cancelled")),
            &mut ready,
            done,
        )
        .await;
    });
    receiver.await.unwrap().unwrap();
    observe_minecraft_log(
        "bound-game",
        "Game",
        "process",
        "Local game hosted on port 25566",
    )
    .await;
    assert!(HONGSHI_STATE.lock().await.port_changed);
    assert_eq!(HONGSHI_STATE.lock().await.local_port, Some(25565));
    minecraft_process_finished("bound-game").await;
    actor.await.unwrap();
    assert!(HONGSHI_RUNTIME.lock().await.session.is_none());
    assert!(HONGSHI_STATE.lock().await.public_address.is_none());
    assert!(!HONGSHI_STATE.lock().await.port_changed);
    assert!(
        get_detected_ports()
            .await
            .iter()
            .all(|port| port.instance_id != "bound-game")
    );
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir()
            .join(format!("axolotl-hongshi-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn http_server(
    responses: Vec<(u16, String, Vec<u8>)>,
) -> (String, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let mut requests = Vec::new();
        for (status, headers, body) in responses {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            loop {
                let mut buffer = [0; 1024];
                let read = stream.read(&mut buffer).await.unwrap();
                request.extend_from_slice(&buffer[..read]);
                if read == 0
                    || request.windows(4).any(|bytes| bytes == b"\r\n\r\n")
                {
                    break;
                }
            }
            requests.push(String::from_utf8_lossy(&request).to_string());
            stream.write_all(format!("HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n", body.len()).as_bytes()).await.unwrap();
            stream.write_all(&body).await.unwrap();
        }
        requests
    });
    (url, task)
}

#[test]
fn all_platforms_use_direct_client_downloads() {
    for (os, arch, platform, remote_arch) in [
        ("windows", "x86_64", "windows", "amd64"),
        ("linux", "x86_64", "linux", "amd64"),
        ("linux", "aarch64", "linux", "arm64"),
        ("macos", "x86_64", "macos", "amd64"),
        ("macos", "aarch64", "macos", "arm64"),
    ] {
        assert_eq!(
            download_endpoint_for(os, arch).unwrap(),
            format!(
                "https://hongshi.site/api/download/client?platform={platform}&arch={remote_arch}"
            )
        );
    }
    assert!(download_endpoint_for("linux", "riscv64").is_err());
    assert!(binary_path().ends_with(Path::new("v2").join(binary_name())));
    assert!(node_cache_path().ends_with("v2/nodes.json"));
    assert_eq!(CONTROL_PORT, 8080);
}

#[test]
fn endpoint_is_independent_of_log_decoration() {
    for line in [
        "endpoint=nj.hongshi.site:34575",
        "INFO uuid=abc port=34575 endpoint=nj.hongshi.site:34575 extra=value",
        "\x1b[32mINFO\x1b[0m endpoint=nj.hongshi.site:34575\x1b[0m",
        "arbitrary prefix\n endpoint=nj.hongshi.site:34575",
    ] {
        assert_eq!(
            parse_endpoint(line).unwrap().as_deref(),
            Some("nj.hongshi.site:34575")
        );
    }
    assert_eq!(parse_endpoint("no endpoint here").unwrap(), None);
    assert_eq!(
        parse_endpoint("other_endpoint=nj.hongshi.site:34575").unwrap(),
        None
    );
    for bad in [
        "",
        "host:0",
        "host:65536",
        "host",
        "host:abc",
        "user@host:123",
        "host:123/path",
        "host:123?query",
        "127.0.0.1:123",
    ] {
        assert!(parse_endpoint(&format!("endpoint={bad}")).is_err(), "{bad}");
    }
}

#[test]
fn node_names_are_dynamic_and_empty_maps_are_valid() {
    let map =
        parse_node_map(br#"{"region renamed":"relay.example.com"}"#).unwrap();
    assert_eq!(map["region renamed"], "relay.example.com");
    assert!(parse_node_map(b"{}").unwrap().is_empty());
    for bad in [
        br#"{"bad":"127.0.0.1"}"#.as_slice(),
        br#"{"bad":"https://relay.example.com"}"#,
        b"[]",
        br#"{"":"relay.example.com"}"#,
    ] {
        assert!(parse_node_map(bad).is_err());
    }
}

#[test]
fn nested_api_errors_include_expected_build() {
    assert_eq!(
        api_error_message(
            reqwest::StatusCode::NOT_FOUND,
            r#"{"detail":{"error":"build not published","message":"not uploaded yet","expected_file":"hongshic-macos-arm64"}}"#
        ),
        "not uploaded yet (hongshic-macos-arm64)"
    );
    assert_eq!(
        api_error_message(
            reqwest::StatusCode::BAD_REQUEST,
            r#"{"detail":"bad platform"}"#
        ),
        "bad platform"
    );
}

#[tokio::test]
async fn missing_build_can_be_retried_and_does_not_replace_existing_file() {
    let directory = TestDirectory::new();
    let path = directory.0.join(binary_name());
    std::fs::write(&path, b"existing file").unwrap();
    let (url, server) = http_server(vec![(404, String::new(), br#"{"detail":{"message":"not uploaded","expected_file":"hongshic-macos-arm64"}}"#.to_vec()), (200, String::new(), b"invalid binary".to_vec())]).await;
    let client = reqwest::Client::new();
    let token = CancellationToken::new();
    let error = install_from(&client, &url, &path, 0, &token)
        .await
        .unwrap_err();
    assert_eq!(
        classify(&error, HongshiErrorType::Unknown),
        HongshiErrorType::BuildUnavailable
    );
    assert!(
        install_from(&client, &url, &path, 0, &token)
            .await
            .unwrap_err()
            .to_string()
            .contains("executable validation")
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"existing file");
    let requests = server.await.unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests.iter().all(|request| {
        request
            .to_ascii_lowercase()
            .contains("accept: application/json")
    }));
}

#[tokio::test]
async fn rate_limited_download_retries_once() {
    let directory = TestDirectory::new();
    let (url, server) = http_server(vec![
        (429, "Retry-After: 0\r\n".into(), b"{}".to_vec()),
        (
            429,
            "Retry-After: 0\r\n".into(),
            br#"{"detail":{"message":"rate limited"}}"#.to_vec(),
        ),
    ])
    .await;
    let error = install_from(
        &reqwest::Client::new(),
        &url,
        &directory.0.join(binary_name()),
        0,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("rate limited"));
    assert_eq!(server.await.unwrap().len(), 2);
}

#[tokio::test]
async fn rate_limit_wait_is_cancellable() {
    let directory = TestDirectory::new();
    let (url, server) =
        http_server(vec![(429, "Retry-After: 61\r\n".into(), b"{}".to_vec())])
            .await;
    let token = CancellationToken::new();
    let cancel = token.clone();
    tokio::spawn(async move {
        server.await.unwrap();
        cancel.cancel();
    });
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        install_from(
            &reqwest::Client::new(),
            &url,
            &directory.0.join(binary_name()),
            0,
            &token,
        ),
    )
    .await
    .unwrap();
    assert!(result.is_err());
    assert!(!directory.0.join(binary_name()).exists());
}

#[tokio::test]
async fn node_cache_refresh_empty_and_cooldown() {
    let directory = TestDirectory::new();
    let path = directory.0.join("nodes.json");
    let cache = Mutex::new(NodeCache::default());
    let (url, server) = http_server(vec![
        (
            200,
            String::new(),
            br#"{"first":"relay.example.com"}"#.to_vec(),
        ),
        (200, String::new(), b"{}".to_vec()),
        (429, "Retry-After: 61\r\n".into(), b"{}".to_vec()),
    ])
    .await;
    let client = reqwest::Client::new();
    assert_eq!(
        load_node_map_from(&client, &url, &path, &cache, false)
            .await
            .unwrap()
            .0
            .len(),
        1
    );
    assert_eq!(
        load_node_map_from(&client, &url, &path, &cache, false)
            .await
            .unwrap()
            .0
            .len(),
        1
    );
    assert!(
        load_node_map_from(&client, &url, &path, &cache, true)
            .await
            .unwrap()
            .0
            .is_empty()
    );
    assert!(
        parse_node_map(&std::fs::read(&path).unwrap())
            .unwrap()
            .is_empty()
    );
    assert!(
        load_node_map_from(&client, &url, &path, &cache, true)
            .await
            .unwrap()
            .1
    );
    assert!(
        load_node_map_from(&client, &url, &path, &cache, true)
            .await
            .unwrap()
            .0
            .is_empty()
    );
    assert_eq!(server.await.unwrap().len(), 3);
}

#[tokio::test]
async fn output_reader_handles_chunks_and_rejects_oversized_lines() {
    let directory = TestDirectory::new();
    let log = Arc::new(Mutex::new(
        tokio::fs::File::create(directory.0.join("output.log"))
            .await
            .unwrap(),
    ));
    let (mut writer, reader) = tokio::io::duplex(128);
    let (sender, mut receiver) = mpsc::channel(16);
    let task = tokio::spawn(read_output(reader, true, log, sender));
    writer.write_all(b"unrelated\nend").await.unwrap();
    writer
        .write_all(b"point=relay.example.com:34575\n")
        .await
        .unwrap();
    assert!(
        matches!(receiver.recv().await, Some(OutputEvent::Endpoint(address)) if address == "relay.example.com:34575")
    );
    let send = tokio::spawn(async move {
        let _ = writer.write_all(&vec![b'a'; MAX_OUTPUT_LINE + 1]).await;
    });
    assert!(matches!(receiver.recv().await, Some(OutputEvent::Error(_))));
    task.await.unwrap();
    send.await.unwrap();
}

#[tokio::test]
async fn stale_sessions_cannot_update_or_release_current_provider() {
    let _serial = SESSION_TEST.lock().await;
    let (id, token, done) = reserve_session(Some(25565), None).await.unwrap();
    assert!(reserve_session(Some(25565), None).await.is_err());
    update_state(id - 1, |state| state.public_address = Some("stale".into()))
        .await;
    assert!(HONGSHI_STATE.lock().await.public_address.is_none());
    let (old_done, _) = watch::channel(false);
    finish_session(
        id - 1,
        &CancellationToken::new(),
        Ok(()),
        &mut None,
        old_done,
    )
    .await;
    assert_eq!(
        HONGSHI_RUNTIME.lock().await.session.as_ref().unwrap().id,
        id
    );
    assert_eq!(
        multiplayer::get_state().await.active_provider,
        Some(MultiplayerProvider::Hongshi)
    );
    let stop = tokio::spawn(stop());
    token.cancelled().await;
    finish_session(id, &token, Err(eyre::eyre!("cancelled")), &mut None, done)
        .await;
    stop.await.unwrap().unwrap();
    assert_eq!(HONGSHI_STATE.lock().await.status, HongshiStatus::Idle);
    assert!(HONGSHI_RUNTIME.lock().await.session.is_none());
}

#[tokio::test]
async fn game_exit_cancels_bound_session_during_download() {
    let _serial = SESSION_TEST.lock().await;
    observe_minecraft_log(
        "redstone-test",
        "Test",
        "process",
        "Local game hosted on port 54321",
    )
    .await;
    assert!(
        get_detected_ports()
            .await
            .iter()
            .any(|port| port.instance_id == "redstone-test"
                && port.port == 54321)
    );
    let (id, token, done) =
        reserve_session(Some(54321), Some("redstone-test".into()))
            .await
            .unwrap();
    update_state(id, |state| state.status = HongshiStatus::Downloading).await;
    let exit = tokio::spawn(minecraft_process_finished("redstone-test"));
    token.cancelled().await;
    finish_session(id, &token, Err(eyre::eyre!("cancelled")), &mut None, done)
        .await;
    exit.await.unwrap();
    assert!(
        get_detected_ports()
            .await
            .iter()
            .all(|port| port.instance_id != "redstone-test")
    );
}
