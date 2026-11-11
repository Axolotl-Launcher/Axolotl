//! A minimal OAuth 2.0 authorization code grant flow redirection/reply loopback URI HTTP
//! server implementation, compliant with [RFC 6749]'s authorization code grant flow and
//! [RFC 8252]'s best current practices for OAuth 2.0 in native apps.
//!
//! This server is needed for the step 4 of the OAuth authentication dance represented in
//! figure 1 of [RFC 8252].
//!
//! Further reading: https://www.oauth.com/oauth2-servers/oauth-native-apps/redirect-urls-for-native-apps/
//!
//! [RFC 6749]: https://datatracker.ietf.org/doc/html/rfc6749
//! [RFC 8252]: https://datatracker.ietf.org/doc/html/rfc8252

use std::{
    net::SocketAddr,
    sync::{LazyLock, Mutex},
    time::Duration,
};

use hyper::body::Incoming;
use hyper_util::rt::{TokioIo, TokioTimer};
use theseus::ErrorKind;
use tokio::sync::{broadcast, oneshot};

static SERVER_SHUTDOWN: LazyLock<broadcast::Sender<()>> =
    LazyLock::new(|| broadcast::channel(1024).0);

/// Starts a temporary HTTP server on a registered loopback callback address.
pub async fn listen_fixed(
    address: SocketAddr,
    listen_socket_tx: oneshot::Sender<Result<SocketAddr, theseus::Error>>,
) -> Result<Option<AuthorizationCodeReply>, theseus::Error> {
    let listener = tokio::net::TcpListener::bind(address).await;
    listen_with_listener(listener, listen_socket_tx).await
}

async fn listen_with_listener(
    listener: Result<tokio::net::TcpListener, std::io::Error>,
    listen_socket_tx: oneshot::Sender<Result<SocketAddr, theseus::Error>>,
) -> Result<Option<AuthorizationCodeReply>, theseus::Error> {
    let listener = match listener {
        Ok(listener) => {
            listen_socket_tx
                .send(listener.local_addr().map_err(|e| {
                    ErrorKind::OtherError(format!(
                        "Failed to get auth code reply socket address: {e}"
                    ))
                    .into()
                }))
                .ok();

            listener
        }
        Err(e) => {
            let error_msg =
                format!("Failed to bind auth code reply socket: {e}");

            listen_socket_tx
                .send(Err(ErrorKind::OtherError(error_msg.clone()).into()))
                .ok();

            return Err(ErrorKind::OtherError(error_msg).into());
        }
    };

    let mut auth_code = Mutex::new(None);
    let mut shutdown_notification = SERVER_SHUTDOWN.subscribe();

    while auth_code.get_mut().unwrap().is_none() {
        let client_socket = tokio::select! {
            biased;
            _ = shutdown_notification.recv() => {
                break;
            }
            conn_accept_result = listener.accept() => {
                match conn_accept_result {
                    Ok((socket, _)) => socket,
                    Err(e) => {
                        tracing::warn!("Failed to accept auth code reply: {e}");
                        continue;
                    }
                }
            }
        };

        if let Err(e) = hyper::server::conn::http1::Builder::new()
            .keep_alive(false)
            .header_read_timeout(Duration::from_secs(5))
            .timer(TokioTimer::new())
            .auto_date_header(false)
            .serve_connection(
                TokioIo::new(client_socket),
                hyper::service::service_fn(|req| handle_reply(req, &auth_code)),
            )
            .await
        {
            tracing::warn!("Failed to handle auth code reply: {e}");
        }
    }

    Ok(auth_code.into_inner().unwrap())
}

/// Stops any active OAuth 2.0 authorization code grant flow reply listening HTTP servers.
pub fn stop_listeners() {
    SERVER_SHUTDOWN.send(()).ok();
}

pub struct AuthorizationCodeReply {
    pub code: String,
    pub state: Option<String>,
}

struct ReplyPageCopy {
    language: &'static str,
    success_title: &'static str,
    success_message: &'static str,
    error_title: &'static str,
    error_message: &'static str,
}

fn reply_page_copy(
    accept_language: Option<&hyper::header::HeaderValue>,
) -> ReplyPageCopy {
    let is_chinese = accept_language
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .any(|language| language.trim_start().starts_with("zh"))
        });

    if is_chinese {
        ReplyPageCopy {
            language: "zh-CN",
            success_title: "登录成功",
            success_message: "你已成功登录！现在可以关闭此页面。",
            error_title: "发生错误",
            error_message: "未找到授权代码。请重新尝试登录。",
        }
    } else {
        ReplyPageCopy {
            language: "en",
            success_title: "Success",
            success_message: "You have successfully signed in! You can close this page now.",
            error_title: "Error",
            error_message: "Authorization code not found. Please try signing in again.",
        }
    }
}

fn render_reply_page(language: &str, title: &str, message: &str) -> String {
    include_str!("auth_code_reply/page.html")
        .replace("lang=en", &format!("lang={language}"))
        .replace("{{title}}", title)
        .replace("{{message}}", message)
}

async fn handle_reply(
    req: hyper::Request<Incoming>,
    auth_code_out: &Mutex<Option<AuthorizationCodeReply>>,
) -> Result<hyper::Response<String>, hyper::http::Error> {
    if req.method() != hyper::Method::GET {
        return hyper::Response::builder()
            .status(hyper::StatusCode::METHOD_NOT_ALLOWED)
            .header("Allow", "GET")
            .body("".into());
    }

    let copy =
        reply_page_copy(req.headers().get(hyper::header::ACCEPT_LANGUAGE));

    // The authorization code is guaranteed to be sent as a "code" query parameter
    // in the request URI query string as per RFC 6749 § 4.1.2
    let auth_code = req.uri().query().and_then(|query_string| {
        let params: std::collections::HashMap<_, _> =
            url::form_urlencoded::parse(query_string.as_bytes()).collect();
        Some(AuthorizationCodeReply {
            code: params.get("code")?.to_string(),
            state: params.get("state").map(ToString::to_string),
        })
    });

    let response = if let Some(auth_code) = auth_code {
        *auth_code_out.lock().unwrap() = Some(auth_code);

        hyper::Response::builder()
            .status(hyper::StatusCode::OK)
            .header("Content-Type", "text/html;charset=utf-8")
            .body(render_reply_page(
                copy.language,
                copy.success_title,
                copy.success_message,
            ))
    } else {
        hyper::Response::builder()
            .status(hyper::StatusCode::BAD_REQUEST)
            .header("Content-Type", "text/html;charset=utf-8")
            .body(render_reply_page(
                copy.language,
                copy.error_title,
                copy.error_message,
            ))
    }?;

    Ok(response)
}
