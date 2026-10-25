//! Platform-related code
use daedalus::minecraft::{Os, OsRule};

/// (e.g. "Run as administrator" on Windows).
#[cfg(target_os = "windows")]
pub fn is_process_elevated() -> bool {
    use std::ffi::c_void;
    use std::ptr::null_mut;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn OpenProcessToken(
            process_handle: *mut c_void,
            desired_access: u32,
            token_handle: *mut *mut c_void,
        ) -> i32;
        fn GetTokenInformation(
            token_handle: *mut c_void,
            token_information_class: u32,
            token_information: *mut c_void,
            token_information_length: u32,
            return_length: *mut u32,
        ) -> i32;
    }

    const TOKEN_QUERY: u32 = 0x8;
    const TOKEN_ELEVATION_TYPE: u32 = 18;
    const TOKEN_ELEVATION_TYPE_FULL: u32 = 2;

    // SAFETY: GetCurrentProcess returns a pseudo-handle that must not be
    // closed. The token handle is queried and closed on every path.
    unsafe {
        let process = GetCurrentProcess();
        let mut token = null_mut();
        if OpenProcessToken(process, TOKEN_QUERY, &raw mut token) == 0
            || token.is_null()
        {
            return false;
        }

        let mut elevation_type: u32 = 0;
        let mut size = 0u32;
        let success = GetTokenInformation(
            token,
            TOKEN_ELEVATION_TYPE,
            &raw mut elevation_type as *mut c_void,
            std::mem::size_of::<u32>() as u32,
            &raw mut size,
        ) != 0;
        let _ = CloseHandle(token);
        success && elevation_type == TOKEN_ELEVATION_TYPE_FULL
    }
}

#[cfg(not(target_os = "windows"))]
pub fn is_process_elevated() -> bool {
    false
}

// Bit width
#[cfg(target_pointer_width = "64")]
pub const ARCH_WIDTH: &str = "64";

#[cfg(target_pointer_width = "32")]
pub const ARCH_WIDTH: &str = "32";

// Platform rule handling
fn normalize_architecture(java_arch: &str) -> &str {
    if java_arch.eq_ignore_ascii_case("amd64") {
        "x86_64"
    } else if java_arch.eq_ignore_ascii_case("i386")
        || java_arch.eq_ignore_ascii_case("i686")
    {
        "x86"
    } else if java_arch.eq_ignore_ascii_case("arm64") {
        "aarch64"
    } else if java_arch.eq_ignore_ascii_case("arm32") {
        "arm"
    } else {
        java_arch
    }
}

pub fn os_rule(
    rule: &OsRule,
    java_arch: &str,
    // Minecraft updated over 1.18.2 (supports MacOS Natively)
    _minecraft_updated: bool,
) -> bool {
    let mut rule_match = true;
    let normalized_arch = normalize_architecture(java_arch);

    if let Some(expected) = rule.arch.as_deref() {
        let actual = normalized_arch.to_ascii_lowercase();
        rule_match &= match expected.to_ascii_lowercase().as_str() {
            "x86" | "i386" | "i686" => {
                matches!(actual.as_str(), "x86" | "i386" | "i686")
            }
            "x86_64" | "amd64" => {
                matches!(actual.as_str(), "x86_64" | "amd64")
            }
            "arm" | "arm32" => actual.starts_with("arm") && actual != "arm64",
            "aarch64" | "arm64" => {
                matches!(actual.as_str(), "aarch64" | "arm64")
            }
            expected => expected == actual,
        };
    }

    if let Some(name) = &rule.name {
        let native = Os::native_arch(normalized_arch);
        rule_match &= name == &Os::Universal
            || name == &native
            || name == &native.get_os();
    }

    // `rule.version` is ignored because it's not usually seen on real recent
    // Minecraft version manifests, its alleged regex syntax is undefined and is
    // likely to not match `Regex`'s, and the way to get the value to match it
    // against is allegedly calling `System.getProperty("os.version")`, which
    // on Windows the OpenJDK implements by fetching the kernel32.dll version,
    // an approach that no public Rust library implements. Moreover, launchers
    // such as PrismLauncher also ignore this field. Code references:
    // - https://github.com/openjdk/jdk/blob/948ade8e7003a41683600428c8e3155c7ed798db/src/java.base/windows/native/libjava/java_props_md.c#L556
    // - https://github.com/PrismLauncher/PrismLauncher/blob/1c20faccf88999474af70db098a4c10e7a03af33/launcher/minecraft/Rule.h#L77
    // - https://github.com/FillZpp/sys-info-rs/blob/60ecf1470a5b7c90242f429934a3bacb6023ec4d/c/windows.c#L23-L38

    rule_match
}

/// Package manager associated with a Linux distribution family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Apt,
    Pacman,
    Dnf,
    Zypper,
    Nix,
    Apk,
}

impl PackageManager {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Apt => "apt",
            Self::Pacman => "pacman",
            Self::Dnf => "dnf",
            Self::Zypper => "zypper",
            Self::Nix => "nix",
            Self::Apk => "apk",
        }
    }
}

/// Coarse Linux distribution family, used for platform-specific behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistroFamily {
    Debian,
    Arch,
    Fedora,
    Suse,
    NixOs,
    Alpine,
    Unknown,
}

impl DistroFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Debian => "debian",
            Self::Arch => "arch",
            Self::Fedora => "fedora",
            Self::Suse => "suse",
            Self::NixOs => "nixos",
            Self::Alpine => "alpine",
            Self::Unknown => "unknown",
        }
    }

    pub fn package_manager(self) -> Option<PackageManager> {
        match self {
            Self::Debian => Some(PackageManager::Apt),
            Self::Arch => Some(PackageManager::Pacman),
            Self::Fedora => Some(PackageManager::Dnf),
            Self::Suse => Some(PackageManager::Zypper),
            Self::NixOs => Some(PackageManager::Nix),
            Self::Alpine => Some(PackageManager::Apk),
            Self::Unknown => None,
        }
    }

    fn from_identifiers(id: &str, id_like: &str) -> Self {
        let tokens: Vec<&str> = std::iter::once(id)
            .chain(std::iter::once(id_like))
            .flat_map(str::split_whitespace)
            .collect();
        let any =
            |names: &[&str]| tokens.iter().any(|token| names.contains(token));
        let opensuse = tokens.iter().any(|token| {
            token.starts_with("opensuse")
                || matches!(*token, "suse" | "sles" | "sled")
        });

        if any(&["nixos"]) {
            return Self::NixOs;
        }
        if any(&["alpine", "postmarketos"]) {
            return Self::Alpine;
        }
        if opensuse {
            return Self::Suse;
        }
        if any(&[
            "arch",
            "archlinux",
            "manjaro",
            "endeavouros",
            "artix",
            "garuda",
            "cachyos",
            "arcolinux",
            "blackarch",
            "rebornos",
            "archcraft",
        ]) {
            return Self::Arch;
        }
        if any(&[
            "debian",
            "ubuntu",
            "linuxmint",
            "lmde",
            "pop",
            "elementary",
            "kali",
            "raspbian",
            "devuan",
            "mx",
            "zorin",
            "neon",
            "deepin",
            "parrot",
            "trisquel",
            "pureos",
            "peppermint",
            "bodhi",
            "tails",
        ]) {
            return Self::Debian;
        }
        if any(&[
            "fedora",
            "rhel",
            "centos",
            "rocky",
            "almalinux",
            "nobara",
            "bazzite",
            "ultramarine",
            "mageia",
            "openmandriva",
        ]) {
            return Self::Fedora;
        }
        Self::Unknown
    }
}

/// The Linux distribution this process runs on, as declared by
/// `/etc/os-release`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxDistro {
    /// `ID` field, e.g. `arch`.
    pub id: String,
    /// Human readable name, preferring `PRETTY_NAME`, then `NAME` with the
    /// distribution version appended.
    pub name: String,
    pub version_id: Option<String>,
    pub version: Option<String>,
    pub family: DistroFamily,
    pub package_manager: Option<PackageManager>,
}

fn unescape_quoted(value: &str) -> String {
    let mut unescaped = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            unescaped.push(character);
            continue;
        }
        match chars.next() {
            Some('"') => unescaped.push('"'),
            Some('\\') => unescaped.push('\\'),
            Some('$') => unescaped.push('$'),
            Some('`') => unescaped.push('`'),
            Some(other) => {
                unescaped.push('\\');
                unescaped.push(other);
            }
            None => unescaped.push('\\'),
        }
    }
    unescaped
}

fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 {
        let quote = bytes[0];
        if (quote == b'"' || quote == b'\'') && bytes[bytes.len() - 1] == quote
        {
            let inner = &value[1..value.len() - 1];
            return if quote == b'"' {
                unescape_quoted(inner)
            } else {
                inner.to_string()
            };
        }
    }
    value.to_string()
}

fn os_release_fields(
    content: &str,
) -> std::collections::BTreeMap<String, String> {
    let mut fields = std::collections::BTreeMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, raw)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        fields.insert(key.to_string(), unquote(raw.trim()));
    }
    fields
}

/// Parse the contents of `/etc/os-release` into a distribution description.
/// Returns `None` when the file carries no distribution identity.
pub fn parse_linux_distro(content: &str) -> Option<LinuxDistro> {
    let fields = os_release_fields(content);
    let id = fields
        .get("ID")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            fields
                .get("NAME")
                .map(|value| value.trim().to_ascii_lowercase())
                .filter(|value| !value.is_empty())
        })?;
    let id_like = fields
        .get("ID_LIKE")
        .map(|value| value.trim().to_string())
        .unwrap_or_default();
    let family = DistroFamily::from_identifiers(&id, &id_like);

    let version_id = fields
        .get("VERSION_ID")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let version = fields
        .get("VERSION")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    // Prefer PRETTY_NAME, which already embeds the distribution version. Fall
    // back to NAME plus VERSION/VERSION_ID so the version is not lost.
    let name = fields
        .get("PRETTY_NAME")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let name = fields
                .get("NAME")
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())?;
            Some(match version.as_deref().or(version_id.as_deref()) {
                Some(version) => format!("{name} {version}"),
                None => name,
            })
        })
        .unwrap_or_else(|| id.clone());

    Some(LinuxDistro {
        package_manager: family.package_manager(),
        id,
        name,
        version_id,
        version,
        family,
    })
}

static LINUX_DISTRO: std::sync::OnceLock<Option<LinuxDistro>> =
    std::sync::OnceLock::new();

/// The distribution this process runs on, read once from `/etc/os-release`.
/// Always `None` on non-Linux platforms.
pub fn detect_linux_distro() -> Option<&'static LinuxDistro> {
    LINUX_DISTRO
        .get_or_init(|| {
            #[cfg(target_os = "linux")]
            {
                std::fs::read_to_string("/etc/os-release")
                    .ok()
                    .and_then(|content| parse_linux_distro(&content))
            }
            #[cfg(not(target_os = "linux"))]
            {
                None
            }
        })
        .as_ref()
}

static KERNEL_RELEASE: std::sync::OnceLock<Option<String>> =
    std::sync::OnceLock::new();

/// The running kernel release, e.g. `7.2.11-zen1-1-zen`. Always `None` on
/// non-Linux platforms.
pub fn kernel_release() -> Option<&'static str> {
    KERNEL_RELEASE
        .get_or_init(|| {
            #[cfg(target_os = "linux")]
            {
                std::fs::read_to_string("/proc/sys/kernel/osrelease")
                    .ok()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
            }
            #[cfg(not(target_os = "linux"))]
            {
                None
            }
        })
        .as_deref()
}

static IS_WSL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Whether this process runs inside Windows Subsystem for Linux.
pub fn is_wsl() -> bool {
    *IS_WSL.get_or_init(|| {
        #[cfg(target_os = "linux")]
        {
            let has_env =
                ["WSL_DISTRO_NAME", "WSL_INTEROP"].iter().any(|key| {
                    std::env::var_os(key).is_some_and(|value| !value.is_empty())
                });
            has_env
                || std::fs::read_to_string("/proc/version")
                    .map(|value| {
                        value.to_ascii_lowercase().contains("microsoft")
                    })
                    .unwrap_or(false)
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    })
}

/// Whether this Linux system is Debian-based. Unknown distributions fall back
/// to the conventional Debian markers so derivatives without a recognized
/// `ID`/`ID_LIKE` keep the previous behavior.
pub fn is_debian_family() -> bool {
    #[cfg(target_os = "linux")]
    {
        if let Some(distro) = detect_linux_distro()
            && distro.family != DistroFamily::Unknown
        {
            return distro.family == DistroFamily::Debian;
        }
        ["/etc/debian_version", "/etc/apt", "/usr/bin/apt-get"]
            .iter()
            .any(|path| std::path::Path::new(path).exists())
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

pub fn classpath_separator(java_arch: &str) -> &'static str {
    match Os::native_arch(java_arch) {
        Os::Osx
        | Os::OsxArm64
        | Os::Linux
        | Os::LinuxArm32
        | Os::LinuxArm64
        | Os::LinuxPpc64le
        | Os::Freebsd
        | Os::Universal
        | Os::Unknown => ":",
        Os::Windows | Os::WindowsArm64 => ";",
    }
}

#[cfg(test)]
mod tests {
    use super::{DistroFamily, PackageManager, parse_linux_distro};

    fn distro(content: &str) -> super::LinuxDistro {
        parse_linux_distro(content).expect("os-release content should parse")
    }

    #[test]
    fn arch_linux_uses_its_pretty_name_without_a_version() {
        let parsed = distro(
            "NAME=\"Arch Linux\"\n\
             PRETTY_NAME=\"Arch Linux\"\n\
             ID=arch\n\
             BUILD_ID=rolling\n",
        );

        assert_eq!(parsed.id, "arch");
        assert_eq!(parsed.name, "Arch Linux");
        assert_eq!(parsed.version_id, None);
        assert_eq!(parsed.version, None);
        assert_eq!(parsed.family, DistroFamily::Arch);
        assert_eq!(parsed.package_manager, Some(PackageManager::Pacman));
    }

    #[test]
    fn ubuntu_is_debian_family_and_keeps_its_release_name() {
        let parsed = distro(
            "NAME=\"Ubuntu\"\n\
             VERSION=\"26.04 LTS (Resolute Raccoon)\"\n\
             ID=ubuntu\n\
             ID_LIKE=debian\n\
             PRETTY_NAME=\"Ubuntu 26.04 LTS\"\n\
             VERSION_ID=\"26.04\"\n",
        );

        assert_eq!(parsed.name, "Ubuntu 26.04 LTS");
        assert_eq!(parsed.version_id.as_deref(), Some("26.04"));
        assert_eq!(parsed.family, DistroFamily::Debian);
        assert_eq!(parsed.package_manager, Some(PackageManager::Apt));
    }

    #[test]
    fn nixos_is_detected_from_its_bare_id() {
        let parsed = distro(
            "NAME=NixOS\n\
             ID=nixos\n\
             PRETTY_NAME=\"NixOS 26.11 (Xantusia)\"\n\
             VERSION_ID=\"26.11\"\n",
        );

        assert_eq!(parsed.name, "NixOS 26.11 (Xantusia)");
        assert_eq!(parsed.family, DistroFamily::NixOs);
        assert_eq!(parsed.package_manager, Some(PackageManager::Nix));
    }

    #[test]
    fn opensuse_matches_by_identifier_prefix() {
        let parsed = distro(
            "NAME=\"openSUSE Tumbleweed\"\n\
             ID=opensuse-tumbleweed\n\
             ID_LIKE=\"opensuse suse\"\n",
        );

        assert_eq!(parsed.family, DistroFamily::Suse);
        assert_eq!(parsed.package_manager, Some(PackageManager::Zypper));
    }

    #[test]
    fn id_like_contributes_family_detection() {
        let parsed =
            distro("NAME=\"Custom Roll\"\nID=customroll\nID_LIKE=arch\n");

        assert_eq!(parsed.family, DistroFamily::Arch);
    }

    #[test]
    fn missing_pretty_name_falls_back_to_name_and_version() {
        let parsed = distro(
            "NAME=\"Some Distro\"\n\
             ID=somedistro\n\
             VERSION_ID=\"1.2\"\n",
        );

        assert_eq!(parsed.name, "Some Distro 1.2");
        assert_eq!(parsed.family, DistroFamily::Unknown);
        assert_eq!(parsed.package_manager, None);
    }

    #[test]
    fn missing_id_falls_back_to_a_lowercased_name() {
        let parsed = distro("NAME=\"Some Distro\"\n");

        assert_eq!(parsed.id, "some distro");
        assert_eq!(parsed.name, "Some Distro");
    }

    #[test]
    fn quoted_values_are_unescaped() {
        let parsed = distro(
            "ID=quoted\n\
             PRETTY_NAME=\"Quoted \\\"Distro\\\" Name\"\n\
             NAME='Single Quoted'\n",
        );

        assert_eq!(parsed.name, "Quoted \"Distro\" Name");
    }

    #[test]
    fn empty_or_comment_only_content_has_no_distribution() {
        assert_eq!(parse_linux_distro(""), None);
        assert_eq!(parse_linux_distro("# only a comment\n"), None);
        assert_eq!(parse_linux_distro("NO_EQUALS_SIGN\n"), None);
    }
}
