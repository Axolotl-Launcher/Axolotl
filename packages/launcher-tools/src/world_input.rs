use std::net::Ipv6Addr;

pub fn valid_world_id(id: &str) -> bool {
    if id.is_empty()
        || id.len() > 255
        || matches!(id, "." | "..")
        || id.ends_with([' ', '.'])
        || id.chars().any(|c| {
            c.is_control()
                || matches!(
                    c,
                    '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*'
                )
        })
    {
        return false;
    }
    let stem = id.split('.').next().unwrap_or("").to_ascii_uppercase();
    !matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$"
    ) && !(stem.len() == 4
        && (stem.starts_with("COM") || stem.starts_with("LPT"))
        && matches!(stem.as_bytes()[3], b'1'..=b'9'))
}

pub fn valid_address(address: &str) -> bool {
    if address.is_empty()
        || address.len() > 255
        || address.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return false;
    }
    if let Some(rest) = address.strip_prefix('[') {
        let Some((host, suffix)) = rest.split_once(']') else {
            return false;
        };
        return host.parse::<Ipv6Addr>().is_ok()
            && (suffix.is_empty()
                || suffix.strip_prefix(':').is_some_and(valid_port));
    }
    if address.matches(':').count() > 1 {
        return address.parse::<Ipv6Addr>().is_ok();
    }
    let (host, port) = address
        .split_once(':')
        .map_or((address, None), |(host, port)| (host, Some(port)));
    !host.is_empty()
        && host.trim_end_matches('.').split('.').all(|label| {
            !label.is_empty()
                && label
                    .chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
        })
        && port.is_none_or(valid_port)
}
fn valid_port(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|c| c.is_ascii_digit())
        && value.parse::<u16>().is_ok_and(|port| port > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn world_ids_are_one_portable_folder_name() {
        for id in ["我的世界", "New World", "世界 (2)", ".hidden-world"] {
            assert!(valid_world_id(id), "{id}");
        }
        for id in [
            "",
            ".",
            "..",
            "../world",
            "world/other",
            "C:\\world",
            "save:stream",
            "CON",
            "nul.txt",
            "LPT1",
            "world.",
            "world ",
        ] {
            assert!(!valid_world_id(id), "{id}");
        }
    }
    #[test]
    fn minecraft_addresses_accept_localhost_ports_and_ipv6_without_urls() {
        for address in [
            "localhost",
            "127.0.0.1:25565",
            "mc.example.com",
            "mc.example.com.",
            "[::1]:25565",
            "2001:db8::1",
            "[2001:db8::1]",
        ] {
            assert!(valid_address(address), "{address}");
        }
        for address in [
            "",
            "https://example.com",
            "host:0",
            "host:65536",
            "host:",
            "host/path",
            "user@host",
            "host port",
            "[bad]:25565",
            "host:+1",
            ".example",
        ] {
            assert!(!valid_address(address), "{address}");
        }
    }
}
