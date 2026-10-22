use super::{ProxyConfig, ProxyTestResult};
use std::time::{Duration, Instant};

const TARGETS: [(&str, &str, u16); 3] = [
    (
        "Microsoft",
        "https://login.microsoftonline.com/consumers/v2.0/.well-known/openid-configuration",
        200,
    ),
    (
        "Minecraft Services",
        "https://api.minecraftservices.com/publickeys",
        200,
    ),
    (
        "Mojang Session",
        "https://sessionserver.mojang.com/session/minecraft/hasJoined",
        204,
    ),
];

pub async fn test_proxy_connection(
    config: &ProxyConfig,
) -> crate::Result<ProxyTestResult> {
    config.validate()?;
    let settings = crate::State::get_if_initialized()
        .map(|state| state.download_clients());
    let clients = crate::util::fetch::DownloadClients::build(
        config,
        settings
            .as_ref()
            .is_some_and(|clients| clients.ignore_ssl_errors),
        settings.as_ref().is_some_and(|clients| clients.doh_enabled),
    )?;
    let client = config
        .apply(
            reqwest::Client::builder()
                .dns_resolver(clients.system_dns)
                .danger_accept_invalid_certs(clients.ignore_ssl_errors)
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(6))
                .timeout(Duration::from_secs(12))
                .user_agent(crate::launcher_user_agent()),
        )?
        .build()?;
    Ok(probe(&client, &TARGETS).await)
}

async fn probe(
    client: &reqwest::Client,
    targets: &[(&str, &str, u16)],
) -> ProxyTestResult {
    let started = Instant::now();
    let results = futures::future::join_all(targets.iter().map(
        |(name, url, status)| async move {
            let result = match client.get(*url).send().await {
                Ok(response) if response.status().as_u16() == *status => {
                    return None;
                }
                Ok(response) => format!("HTTP {}", response.status().as_u16()),
                // Raw transport errors may include proxy credentials.
                Err(error) if error.is_timeout() => "timeout".to_string(),
                Err(_) => "connection failed".to_string(),
            };
            Some(format!("{name}: {result}"))
        },
    ))
    .await;
    let failures: Vec<_> = results.into_iter().flatten().collect();
    ProxyTestResult {
        success: failures.is_empty(),
        latency_ms: Some(
            started.elapsed().as_millis().min(u64::MAX as u128) as u64
        ),
        message: failures.join("; "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn checks_real_authentication_hosts_over_https() {
        for (_, url, _) in TARGETS {
            assert_eq!(reqwest::Url::parse(url).unwrap().scheme(), "https");
        }
        assert!(
            TARGETS
                .iter()
                .any(|(_, url, _)| url.contains("login.microsoftonline.com"))
        );
        assert!(
            TARGETS
                .iter()
                .any(|(_, url, _)| url.contains("sessionserver.mojang.com"))
        );
    }

    async fn server(code: u16) -> (String, tokio::task::JoinHandle<()>) {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/probe", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let (mut stream, _) =
                tokio::time::timeout(Duration::from_secs(3), listener.accept())
                    .await
                    .unwrap()
                    .unwrap();
            let mut bytes = Vec::new();
            while !bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                let mut buffer = [0; 1024];
                let n = tokio::time::timeout(
                    Duration::from_secs(3),
                    stream.read(&mut buffer),
                )
                .await
                .unwrap()
                .unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
            }
            stream.write_all(format!("HTTP/1.1 {code} Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
        });
        (url, task)
    }

    #[tokio::test]
    async fn a_successful_endpoint_cannot_hide_a_failed_or_redirected_endpoint()
    {
        for status in [204, 302, 403, 503] {
            let (first, first_server) = server(200).await;
            let (second, second_server) = server(status).await;
            let client = reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap();
            let result = probe(
                &client,
                &[("first", &first, 200), ("second", &second, 204)],
            )
            .await;
            assert_eq!(result.success, status == 204);
            if status != 204 {
                assert!(result.message.contains("second: HTTP"));
            }
            first_server.await.unwrap();
            second_server.await.unwrap();
        }
    }
}
