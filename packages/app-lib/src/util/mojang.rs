//! Routing helpers for Mojang services that can be reached through an HTTP
//! forwarding proxy.

use std::borrow::Cow;
use url::Url;

/// A forwarding proxy that answers Mojang's service hosts under its own names.
///
/// Both proxies map the same services; adding one means adding a variant here,
/// its host table below, and a `DownloadSourceMode` that selects it.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum MojangProxy {
    /// Fallen-Breath's `*.msp.fallenbreath.me` forwarding proxy.
    Fallen,
    /// Sakurin's `*.mc.sakurin.cn` forwarding proxy.
    Sakurin,
}

/// The services a forwarding proxy mirrors, as named by the JVM properties
/// `-Dminecraft.api.<service>.host` that Minecraft reads at runtime.
///
/// `discovery` is the one Minecraft 26.3 and later reads: from Authlib 10.0.77
/// on, the game resolves `session`, `profiles` and the authentication keys from
/// a document this service serves, and ignores the other five properties. They
/// stay here because 1.16 through 26.2 still read them.
pub const MOJANG_SERVICES: [&str; 6] = [
    "auth",
    "account",
    "session",
    "services",
    "profiles",
    "discovery",
];

/// The path the discovery document is served under. `-Dminecraft.api
/// .discovery.host` takes a full URL rather than a bare host, so the path has
/// to travel with the value.
const DISCOVERY_PATH: &str = "minecraft/client";

impl MojangProxy {
    pub fn host(self, service: &str) -> Option<&'static str> {
        let host = match (self, service) {
            (Self::Fallen, "auth") => "auth.msp.fallenbreath.me",
            (Self::Fallen, "account") => "account.msp.fallenbreath.me",
            (Self::Fallen, "session") => "session.msp.fallenbreath.me",
            (Self::Fallen, "services") => "services.msp.fallenbreath.me",
            (Self::Fallen, "profiles") => "profiles.msp.fallenbreath.me",
            (Self::Fallen, "discovery") => "discovery.msp.fallenbreath.me",
            (Self::Sakurin, "auth") => "auth.mc.sakurin.cn",
            (Self::Sakurin, "account") => "account.mc.sakurin.cn",
            (Self::Sakurin, "session") => "session.mc.sakurin.cn",
            (Self::Sakurin, "services") => "services.mc.sakurin.cn",
            (Self::Sakurin, "profiles") => "profiles.mc.sakurin.cn",
            (Self::Sakurin, "discovery") => "discovery.mc.sakurin.cn",
            _ => return None,
        };
        Some(host)
    }

    /// The JVM value naming `service` on this proxy: a bare origin, except for
    /// `discovery`, whose value carries the path the document is served under.
    fn service_url(self, service: &str) -> Option<String> {
        let host = self.host(service)?;
        let path = if service == "discovery" {
            format!("/{DISCOVERY_PATH}")
        } else {
            String::new()
        };
        Some(format!("https://{host}{path}"))
    }

    /// Names the service a Mojang URL belongs to, from its host and path: each
    /// service answers under its own host, and `profiles` and `account` share
    /// `api.mojang.com`, distinguished by path.
    fn service_for(self, host: &str, path: &str) -> Option<&'static str> {
        match host {
            "authserver.mojang.com" => Some("auth"),
            "sessionserver.mojang.com" => Some("session"),
            "api.mojang.com"
                if path.starts_with("/users/profiles/minecraft") =>
            {
                Some("profiles")
            }
            "api.mojang.com" => Some("account"),
            "api.minecraftservices.com" => Some("services"),
            "discovery.minecraftservices.com" => Some("discovery"),
            _ => None,
        }
    }

    /// Rewrites a Mojang service URL onto this proxy's host, or `None` when the
    /// URL is not a proxied service.
    ///
    /// This covers the launcher's own requests only. The document `discovery`
    /// serves already names absolute URIs, so pointing the game at this proxy
    /// also requires the proxy to rewrite those URIs in the document it returns.
    pub fn rewrite(self, original: &str) -> Option<String> {
        let mut url = Url::parse(original).ok()?;
        let host = url.host_str()?.to_ascii_lowercase();
        let path = url.path().to_string();
        let service = self.service_for(&host, &path)?;
        url.set_host(Some(self.host(service)?)).ok()?;
        Some(url.into())
    }

    /// The JVM properties that point Minecraft at this proxy.
    fn java_args(self) -> Vec<String> {
        MOJANG_SERVICES
            .into_iter()
            .filter_map(|service| {
                self.service_url(service)
                    .map(|url| format!("-Dminecraft.api.{service}.host={url}"))
            })
            .collect()
    }
}

pub fn mojang_service_url(
    original: &str,
    proxy: Option<MojangProxy>,
) -> Cow<'_, str> {
    if let Some(mirror) = proxy.and_then(|proxy| proxy.rewrite(original)) {
        return Cow::Owned(mirror);
    }
    Cow::Borrowed(original)
}

/// The proxy Mojang requests currently go out through, if any.
pub fn selected_mojang_proxy() -> Option<MojangProxy> {
    crate::State::get_if_initialized()
        .and_then(|state| state.mojang_auth_proxy())
}

pub(crate) fn auth_proxy_java_args(
    args: &[String],
    proxy: Option<MojangProxy>,
) -> Vec<String> {
    let proxy_args = proxy.map(MojangProxy::java_args).unwrap_or_default();
    // Strip by property name rather than by whole argument: a user who wrote
    // `-Dminecraft.api.auth.host=` themselves must not end up with two values
    // for one property, where the JVM keeps whichever comes last.
    let prefixes: Vec<String> = MOJANG_SERVICES
        .iter()
        .map(|service| format!("-Dminecraft.api.{service}.host="))
        .collect();

    let mut result = proxy_args;
    result.extend(
        args.iter()
            .filter(|arg| {
                !prefixes.iter().any(|prefix| arg.starts_with(prefix))
            })
            .cloned(),
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every service a proxy mirrors, with the host suffix it answers under.
    const PROXIES: [(MojangProxy, &str); 2] = [
        (MojangProxy::Fallen, "msp.fallenbreath.me"),
        (MojangProxy::Sakurin, "mc.sakurin.cn"),
    ];

    #[test]
    fn rewrites_each_mojang_service_to_the_proxy_host() {
        let cases = [
            (
                "https://authserver.mojang.com/authenticate",
                "auth",
                "authenticate",
            ),
            (
                "https://api.mojang.com/user/profile",
                "account",
                "user/profile",
            ),
            (
                "https://sessionserver.mojang.com/session/minecraft/join",
                "session",
                "session/minecraft/join",
            ),
            (
                "https://api.minecraftservices.com/launcher/login",
                "services",
                "launcher/login",
            ),
            (
                "https://api.mojang.com/users/profiles/minecraft/Notch",
                "profiles",
                "users/profiles/minecraft/Notch",
            ),
            (
                "https://discovery.minecraftservices.com/minecraft/client",
                "discovery",
                "minecraft/client",
            ),
        ];

        for (proxy, suffix) in PROXIES {
            for (original, service, path) in cases {
                let expected = format!("https://{service}.{suffix}/{path}");
                assert_eq!(
                    proxy.rewrite(original).as_deref(),
                    Some(expected.as_str()),
                    "{proxy:?} should rewrite {original}"
                );
            }
        }
    }

    #[test]
    fn both_proxies_mirror_the_same_services() {
        for service in MOJANG_SERVICES {
            for (proxy, _) in PROXIES {
                assert!(
                    proxy.host(service).is_some(),
                    "{proxy:?} does not mirror {service}"
                );
            }
        }
        for (proxy, _) in PROXIES {
            assert_eq!(proxy.host("unknown"), None);
        }
    }

    #[test]
    fn leaves_unknown_hosts_untouched() {
        for (proxy, _) in PROXIES {
            assert_eq!(
                proxy.rewrite("https://api.modrinth.com/v2/project/foo"),
                None
            );
        }
    }

    #[test]
    fn keeps_the_original_url_when_no_proxy_is_selected() {
        let original = "https://api.minecraftservices.com/minecraft/profile";
        assert_eq!(mojang_service_url(original, None), original);
        assert_eq!(
            mojang_service_url(original, Some(MojangProxy::Fallen)),
            "https://services.msp.fallenbreath.me/minecraft/profile"
        );
        assert_eq!(
            mojang_service_url(original, Some(MojangProxy::Sakurin)),
            "https://services.mc.sakurin.cn/minecraft/profile"
        );
    }

    /// Minecraft 26.3 and later read only this one, and its value is a URL with
    /// the document's path rather than a bare origin.
    #[test]
    fn discovery_carries_the_document_path() {
        assert_eq!(
            MojangProxy::Fallen.service_url("discovery").as_deref(),
            Some("https://discovery.msp.fallenbreath.me/minecraft/client")
        );
        assert_eq!(
            MojangProxy::Sakurin.service_url("discovery").as_deref(),
            Some("https://discovery.mc.sakurin.cn/minecraft/client")
        );
        // Every other service stays a bare origin, as their properties expect.
        assert_eq!(
            MojangProxy::Sakurin.service_url("session").as_deref(),
            Some("https://session.mc.sakurin.cn")
        );
    }

    fn java_host_args(proxy: MojangProxy) -> Vec<String> {
        MOJANG_SERVICES
            .into_iter()
            .map(|service| {
                format!(
                    "-Dminecraft.api.{service}.host={}",
                    proxy.service_url(service).unwrap()
                )
            })
            .collect()
    }

    #[test]
    fn java_args_follow_the_selected_proxy() {
        for (proxy, _) in PROXIES {
            assert_eq!(
                auth_proxy_java_args(&[], Some(proxy)),
                java_host_args(proxy)
            );
        }
        assert!(auth_proxy_java_args(&[], None).is_empty());
    }

    #[test]
    fn java_args_replace_a_previous_proxy_and_keep_user_arguments() {
        let mut args = auth_proxy_java_args(
            &["-Xmx4G".to_string()],
            Some(MojangProxy::Fallen),
        );
        args.push("-Dcustom=keep".to_string());

        let switched = auth_proxy_java_args(&args, Some(MojangProxy::Sakurin));
        let expected: Vec<String> = [
            java_host_args(MojangProxy::Sakurin),
            vec!["-Xmx4G".to_string(), "-Dcustom=keep".to_string()],
        ]
        .concat();
        assert_eq!(switched, expected);

        let cleared = auth_proxy_java_args(&switched, None);
        assert_eq!(cleared, ["-Xmx4G", "-Dcustom=keep"]);
    }

    #[test]
    fn java_args_reject_a_user_supplied_host_property() {
        // A user may have copied the old five properties from a guide; none of
        // them, nor a stale discovery value, may survive beside the proxy's.
        let args = [
            "-Dminecraft.api.auth.host=https://example.test".to_string(),
            "-Dminecraft.api.discovery.host=https://example.test/minecraft/client"
                .to_string(),
            "-Xmx4G".to_string(),
        ];
        let expected: Vec<String> = [
            java_host_args(MojangProxy::Fallen),
            vec!["-Xmx4G".to_string()],
        ]
        .concat();
        assert_eq!(
            auth_proxy_java_args(&args, Some(MojangProxy::Fallen)),
            expected
        );
        assert_eq!(auth_proxy_java_args(&args, None), ["-Xmx4G"]);
    }
}
