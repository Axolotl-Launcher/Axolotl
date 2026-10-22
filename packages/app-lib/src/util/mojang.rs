//! Routing helpers for Mojang services that can be reached through the
//! Fallen-Breath HTTP forwarding proxy.

use std::borrow::Cow;

use url::Url;

pub fn fallen_proxy_url(original: &str) -> Option<String> {
    let mut url = Url::parse(original).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let proxy_host = match host.as_str() {
        "authserver.mojang.com" => "auth.msp.fallenbreath.me",
        "sessionserver.mojang.com" => "session.msp.fallenbreath.me",
        "api.mojang.com"
            if url.path().starts_with("/users/profiles/minecraft") =>
        {
            "profiles.msp.fallenbreath.me"
        }
        "api.mojang.com" => "account.msp.fallenbreath.me",
        "api.minecraftservices.com" => "services.msp.fallenbreath.me",
        _ => return None,
    };

    url.set_host(Some(proxy_host)).ok()?;
    Some(url.into())
}

pub fn mojang_service_url(original: &str, use_mirror: bool) -> Cow<'_, str> {
    if use_mirror && let Some(mirror) = fallen_proxy_url(original) {
        return Cow::Owned(mirror);
    }
    Cow::Borrowed(original)
}

pub fn should_use_mojang_mirror() -> bool {
    crate::State::get_if_initialized()
        .is_some_and(|state| state.mojang_auth_use_mirror())
}

pub(crate) fn auth_proxy_java_args(
    args: &[String],
    use_mirror: bool,
) -> Vec<String> {
    let proxy_args = ["auth", "account", "session", "services", "profiles"]
        .map(|service| format!("-Dminecraft.api.{service}.host=https://{service}.msp.fallenbreath.me"));
    let mut result = if use_mirror {
        proxy_args.to_vec()
    } else {
        Vec::new()
    };
    result.extend(args.iter().filter(|arg| !proxy_args.contains(arg)).cloned());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_each_mojang_service_to_its_fallen_proxy_host() {
        let cases = [
            (
                "https://authserver.mojang.com/authenticate",
                "https://auth.msp.fallenbreath.me/authenticate",
            ),
            (
                "https://api.mojang.com/user/profile",
                "https://account.msp.fallenbreath.me/user/profile",
            ),
            (
                "https://sessionserver.mojang.com/session/minecraft/join",
                "https://session.msp.fallenbreath.me/session/minecraft/join",
            ),
            (
                "https://api.minecraftservices.com/launcher/login",
                "https://services.msp.fallenbreath.me/launcher/login",
            ),
            (
                "https://api.mojang.com/users/profiles/minecraft/Notch",
                "https://profiles.msp.fallenbreath.me/users/profiles/minecraft/Notch",
            ),
        ];

        for (original, expected) in cases {
            assert_eq!(fallen_proxy_url(original).as_deref(), Some(expected));
        }
    }

    #[test]
    fn leaves_unknown_hosts_untouched() {
        assert_eq!(
            fallen_proxy_url("https://api.modrinth.com/v2/project/foo"),
            None
        );
    }

    #[test]
    fn keeps_the_original_url_when_mirror_is_disabled() {
        let original = "https://api.minecraftservices.com/minecraft/profile";
        assert_eq!(mojang_service_url(original, false), original);
        assert_eq!(
            mojang_service_url(original, true),
            "https://services.msp.fallenbreath.me/minecraft/profile"
        );
    }
}
