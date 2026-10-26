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
    } else if java_arch.eq_ignore_ascii_case("loongarch64")
        || java_arch.eq_ignore_ascii_case("loongarch64el")
        || java_arch.eq_ignore_ascii_case("loong64")
    {
        "loongarch64"
    } else if java_arch.eq_ignore_ascii_case("riscv64")
        || java_arch.eq_ignore_ascii_case("riscv64gc")
    {
        "riscv64"
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
            "loongarch64" | "loongarch64el" | "loong64" => {
                actual == "loongarch64"
            }
            "riscv64" | "riscv64gc" => actual == "riscv64",
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

pub fn classpath_separator(java_arch: &str) -> &'static str {
    match Os::native_arch(java_arch) {
        Os::Osx
        | Os::OsxArm64
        | Os::Linux
        | Os::LinuxArm32
        | Os::LinuxArm64
        | Os::LinuxPpc64le
        | Os::LinuxLoongArch64
        | Os::LinuxRiscv64
        | Os::Freebsd
        | Os::Universal
        | Os::Unknown => ":",
        Os::Windows | Os::WindowsArm64 => ";",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arch_rule(arch: &str) -> OsRule {
        OsRule {
            name: None,
            version: None,
            arch: Some(arch.to_string()),
        }
    }

    #[test]
    fn architecture_aliases_normalize_to_canonical_names() {
        for alias in ["loongarch64", "LoongArch64", "loongarch64el", "loong64"]
        {
            assert_eq!(normalize_architecture(alias), "loongarch64");
        }
        for alias in ["riscv64", "RISCV64", "riscv64gc"] {
            assert_eq!(normalize_architecture(alias), "riscv64");
        }
    }

    #[test]
    fn loongarch64_and_riscv64_architecture_rules_match() {
        for arch in ["loongarch64", "loong64", "loongarch64el"] {
            assert!(os_rule(&arch_rule(arch), "loongarch64", true));
            assert!(!os_rule(&arch_rule(arch), "riscv64", true));
        }
        for arch in ["riscv64", "riscv64gc"] {
            assert!(os_rule(&arch_rule(arch), "riscv64", true));
            assert!(!os_rule(&arch_rule(arch), "loongarch64", true));
        }
    }

    #[test]
    fn linux_java_arch_aliases_use_the_unix_classpath_separator() {
        if std::env::consts::OS != "linux" {
            return;
        }
        for arch in ["loongarch64", "loong64", "riscv64", "riscv64gc"] {
            assert_eq!(classpath_separator(arch), ":");
        }
    }
}
