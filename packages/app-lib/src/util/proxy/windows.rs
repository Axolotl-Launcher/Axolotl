use reqwest::{ClientBuilder, NoProxy, Proxy, Url};

#[derive(Default)]
struct Environment {
    http: String,
    https: String,
    all: String,
    bypass: String,
    cgi: bool,
}

#[cfg(windows)]
fn system_protocol_list() -> Option<(String, String, Environment)> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let settings = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
        )
        .ok()?;
    if settings.get_value::<u32, _>("ProxyEnable").unwrap_or(0) == 0 {
        return None;
    }
    let server = settings
        .get_value::<String, _>("ProxyServer")
        .unwrap_or_default();
    // Reqwest already handles a single proxy address. Its Windows reader
    // treats a per-protocol list as one URI and silently discards it.
    if !server.split(';').any(|entry| {
        entry.split_once('=').is_some_and(|(key, _)| {
            matches!(
                key.trim().to_ascii_lowercase().as_str(),
                "http" | "https" | "socks" | "ftp"
            )
        })
    }) {
        return None;
    }
    let bypass = settings
        .get_value::<String, _>("ProxyOverride")
        .unwrap_or_default();
    let env = |upper, lower| {
        std::env::var(upper)
            .or_else(|_| std::env::var(lower))
            .unwrap_or_default()
    };
    Some((
        server,
        bypass,
        Environment {
            http: env("HTTP_PROXY", "http_proxy"),
            https: env("HTTPS_PROXY", "https_proxy"),
            all: env("ALL_PROXY", "all_proxy"),
            bypass: env("NO_PROXY", "no_proxy"),
            cgi: std::env::var_os("REQUEST_METHOD").is_some(),
        },
    ))
}

#[cfg(windows)]
pub(super) fn apply(builder: ClientBuilder) -> crate::Result<ClientBuilder> {
    let Some((server, bypass, environment)) = system_protocol_list() else {
        return Ok(builder);
    };
    apply_protocol_list(builder, &server, &bypass, environment)
}

#[cfg(windows)]
pub(super) fn native_matcher()
-> Option<hyper_util::client::proxy::matcher::Matcher> {
    let (server, bypass, environment) = system_protocol_list()?;
    Some(protocol_list_matcher(&server, &bypass, environment))
}

fn protocol_list_matcher(
    server: &str,
    bypass: &str,
    environment: Environment,
) -> hyper_util::client::proxy::matcher::Matcher {
    use hyper_util::client::proxy::matcher::Matcher;
    let (http, https, bypass) =
        protocol_list_values(server, bypass, environment);
    Matcher::builder()
        .http(http.as_ref().map(Url::as_str).unwrap_or_default())
        .https(https.as_ref().map(Url::as_str).unwrap_or_default())
        .no(bypass)
        .build()
}

fn endpoint(server: &str, scheme: &str) -> Option<Url> {
    let address = server
        .split(';')
        .filter_map(|entry| entry.split_once('='))
        .find(|(key, _)| key.trim().eq_ignore_ascii_case(scheme))?
        .1
        .trim();
    if address.is_empty() {
        return None;
    }
    // The list key describes the destination protocol. An HTTPS destination
    // normally uses CONNECT through a plain HTTP proxy such as Clash.
    let address = if address.contains("://") {
        address.to_string()
    } else {
        format!("http://{address}")
    };
    Proxy::all(&address).ok()?;
    Url::parse(&address).ok()
}

fn apply_protocol_list(
    builder: ClientBuilder,
    server: &str,
    bypass: &str,
    env: Environment,
) -> crate::Result<ClientBuilder> {
    let builder = builder.no_proxy();
    let (http, https, bypass) = protocol_list_values(server, bypass, env);
    let bypass = NoProxy::from_string(&bypass);
    // Standard per-protocol proxies preserve reqwest's credential handling and
    // IP/domain/CIDR bypass matching. Its custom proxy path ignores NoProxy.
    let mut builder = builder;
    if let Some(http) = http {
        builder = builder.proxy(Proxy::http(http)?.no_proxy(bypass.clone()));
    }
    if let Some(https) = https {
        builder = builder.proxy(Proxy::https(https)?.no_proxy(bypass));
    }
    Ok(builder)
}

fn protocol_list_values(
    server: &str,
    bypass: &str,
    env: Environment,
) -> (Option<Url>, Option<Url>, String) {
    if env.cgi {
        return (None, None, String::new());
    }
    let select = |scheme: &str, configured: &str| {
        if !configured.is_empty() {
            endpoint(&format!("{scheme}={configured}"), scheme)
        } else {
            endpoint(server, scheme)
        }
        .or_else(|| endpoint(&format!("{scheme}={}", env.all), scheme))
    };
    let http = select("http", &env.http);
    let https = select("https", &env.https);
    let bypass = if env.bypass.is_empty() {
        bypass
            .split(';')
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(",")
            .replace("*.", "")
    } else {
        env.bypass
    };
    (http, https, bypass)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_protocol_list_matches_environment_credentials_and_bypass() {
        let matcher = protocol_list_matcher(
            "http=127.0.0.1:7890;https=127.0.0.1:7891",
            "registry.invalid",
            Environment {
                https: "socks5h://user:pass@127.0.0.1:7892".into(),
                bypass: "127.0.0.0/8,.bypass.invalid".into(),
                ..Default::default()
            },
        );
        let selected = matcher
            .intercept(&"https://assets.invalid/file".parse().unwrap())
            .unwrap();
        assert_eq!(selected.uri(), "socks5h://127.0.0.1:7892/");
        assert_eq!(selected.raw_auth(), Some(("user", "pass")));
        assert!(
            matcher
                .intercept(&"https://127.0.0.2/file".parse().unwrap())
                .is_none()
        );
        assert!(
            matcher
                .intercept(&"https://a.bypass.invalid/file".parse().unwrap())
                .is_none()
        );
        assert!(
            matcher
                .intercept(&"https://registry.invalid/file".parse().unwrap())
                .is_some()
        );
        assert_eq!(
            matcher
                .intercept(&"http://assets.invalid/file".parse().unwrap())
                .unwrap()
                .uri(),
            "http://127.0.0.1:7890/"
        );
    }

    #[test]
    fn native_protocol_list_preserves_registry_bypass_and_cgi_behavior() {
        let matcher = protocol_list_matcher(
            "https=127.0.0.1:7890",
            "*.example.invalid;127.0.0.1",
            Environment::default(),
        );
        assert!(
            matcher
                .intercept(
                    &"https://assets.example.invalid/file".parse().unwrap()
                )
                .is_none()
        );
        let matcher = protocol_list_matcher(
            "https=127.0.0.1:7890",
            "",
            Environment {
                cgi: true,
                ..Default::default()
            },
        );
        assert!(
            matcher
                .intercept(&"https://assets.invalid/file".parse().unwrap())
                .is_none()
        );
    }

    #[test]
    fn protocol_keys_select_destinations_without_changing_proxy_transport() {
        let server = " HTTP = 127.0.0.1:7890 ; HTTPS = [::1]:7891 ; socks=127.0.0.1:7892 ";
        assert_eq!(
            endpoint(server, "http").unwrap().as_str(),
            "http://127.0.0.1:7890/"
        );
        assert_eq!(
            endpoint(server, "https").unwrap().as_str(),
            "http://[::1]:7891/"
        );
        assert!(endpoint("http=;https=invalid address", "https").is_none());
        assert!(endpoint("http=127.0.0.1:7890", "https").is_none());
        assert_eq!(
            endpoint("https=socks5h://127.0.0.1:7892", "https")
                .unwrap()
                .scheme(),
            "socks5h"
        );
    }

    async fn server() -> (std::net::SocketAddr, tokio::task::JoinHandle<String>)
    {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut stream, _) = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                listener.accept(),
            )
            .await
            .unwrap()
            .unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut buffer = [0; 1024];
                let n = tokio::time::timeout(
                    std::time::Duration::from_secs(3),
                    stream.read(&mut buffer),
                )
                .await
                .unwrap()
                .unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
                if bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok").await.unwrap();
            String::from_utf8(bytes).unwrap()
        });
        (address, task)
    }

    #[tokio::test]
    async fn protocol_list_routes_http_and_https_through_their_respective_ports()
     {
        let (http, http_task) = server().await;
        let (https, https_task) = server().await;
        let client = apply_protocol_list(
            reqwest::Client::builder(),
            &format!("http={http};https={https}"),
            "",
            Environment::default(),
        )
        .unwrap()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap();
        client
            .get("http://unresolvable.invalid/file")
            .send()
            .await
            .unwrap();
        // The fixture records CONNECT, then closes instead of doing TLS.
        assert!(
            client
                .get("https://unresolvable.invalid/file")
                .send()
                .await
                .is_err()
        );
        assert!(
            http_task
                .await
                .unwrap()
                .starts_with("GET http://unresolvable.invalid/file ")
        );
        assert!(
            https_task
                .await
                .unwrap()
                .starts_with("CONNECT unresolvable.invalid:443 ")
        );
    }

    #[tokio::test]
    async fn environment_proxy_and_credentials_override_the_registry() {
        let (address, task) = server().await;
        let client = apply_protocol_list(
            reqwest::Client::builder(),
            "http=127.0.0.1:1",
            "",
            Environment {
                http: format!("http://user:pass@{address}"),
                ..Default::default()
            },
        )
        .unwrap()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap();
        client
            .get("http://unresolvable.invalid/file")
            .send()
            .await
            .unwrap();
        assert!(
            task.await
                .unwrap()
                .to_ascii_lowercase()
                .contains("proxy-authorization: basic dxnlcjpwyxnz")
        );
    }

    #[tokio::test]
    async fn bypass_list_keeps_loopback_requests_direct() {
        let (address, task) = server().await;
        let client = apply_protocol_list(
            reqwest::Client::builder(),
            "http=127.0.0.1:1",
            "127.0.0.1;*.example.invalid;<local>",
            Environment::default(),
        )
        .unwrap()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap();
        client
            .get(format!("http://{address}/file"))
            .send()
            .await
            .unwrap();
        assert!(task.await.unwrap().starts_with("GET /file "));
    }

    #[tokio::test]
    async fn environment_bypass_takes_priority_and_all_proxy_handles_missing_protocols()
     {
        let (address, task) = server().await;
        let client = apply_protocol_list(
            reqwest::Client::builder(),
            "https=127.0.0.1:1",
            "*",
            Environment {
                all: format!("http://{address}"),
                bypass: "other.invalid".into(),
                ..Default::default()
            },
        )
        .unwrap()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap();
        client
            .get("http://unresolvable.invalid/file")
            .send()
            .await
            .unwrap();
        assert!(
            task.await
                .unwrap()
                .starts_with("GET http://unresolvable.invalid/file ")
        );
    }
}
