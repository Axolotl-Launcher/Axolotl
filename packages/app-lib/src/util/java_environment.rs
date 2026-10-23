//! Keep launcher rendering workarounds out of Java and game launch hooks.
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) const LAUNCHER_RENDERING_ENVIRONMENT: &[&str] = &[
    "WEBKIT_DISABLE_DMABUF_RENDERER",
    "__NV_DISABLE_EXPLICIT_SYNC",
];

pub(crate) fn configure_child_environment(command: &mut Command) {
    for variable in LAUNCHER_RENDERING_ENVIRONMENT {
        command.env_remove(variable);
    }

    // Cargo injects loader search paths for the launcher build, not the JVM.
    #[cfg(target_os = "macos")]
    if std::env::var_os("CARGO").is_some() {
        command.env_remove("DYLD_LIBRARY_PATH");
        command.env_remove("DYLD_FALLBACK_LIBRARY_PATH");
    }
}

pub(crate) fn configure_java_environment(command: &mut Command) {
    configure_child_environment(command);
    // Instance/server JVM options must not be replaced by inherited options.
    command.env_remove("_JAVA_OPTIONS");
}

pub(crate) fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    configure_java_environment(&mut command);
    command
}

pub(crate) fn tokio_command(
    program: impl AsRef<OsStr>,
) -> tokio::process::Command {
    command(program).into()
}

pub(crate) fn configure_pty_java_environment(
    command: &mut portable_pty::CommandBuilder,
) {
    for variable in LAUNCHER_RENDERING_ENVIRONMENT {
        command.env_remove(variable);
    }
    command.env_remove("_JAVA_OPTIONS");
}

/// Server consoles and installers require the console entry point from the
/// selected runtime. Java discovery uses javaw.exe for desktop game launches.
pub(crate) fn console_java_executable(program: &str) -> crate::Result<PathBuf> {
    let path = Path::new(program);
    #[cfg(windows)]
    if path.file_name().is_some_and(|name| {
        name.eq_ignore_ascii_case("javaw.exe")
            || name.eq_ignore_ascii_case("javaw")
    }) {
        let console = path.with_file_name("java.exe");
        // A bare executable is resolved through PATH by the process launcher.
        // Explicit paths must stay in the selected Java installation.
        if path
            .parent()
            .is_some_and(|parent| !parent.as_os_str().is_empty())
            && !console.is_file()
        {
            return Err(crate::ErrorKind::InputError(format!(
                "The selected Java runtime has no console executable: {}. Select its java.exe to use the server console.",
                console.display(),
            )).as_error());
        }
        return Ok(console);
    }
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::ffi::OsString;

    #[test]
    fn console_java_preserves_explicit_console_and_custom_executables() {
        for program in ["java", "java.exe", "custom-java", "runtime/bin/java"] {
            assert_eq!(
                console_java_executable(program).unwrap(),
                PathBuf::from(program)
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn console_java_uses_the_console_sibling_and_rejects_missing_siblings() {
        let runtime = tempfile::tempdir().unwrap();
        let windowed = runtime.path().join("javaw.exe");
        let console = runtime.path().join("java.exe");
        std::fs::write(&windowed, []).unwrap();
        assert!(console_java_executable(windowed.to_str().unwrap()).is_err());
        std::fs::write(&console, []).unwrap();
        assert_eq!(
            console_java_executable(windowed.to_str().unwrap()).unwrap(),
            console
        );
        assert_eq!(
            console_java_executable(
                runtime.path().join("JAVAW.EXE").to_str().unwrap()
            )
            .unwrap(),
            console
        );
        assert_eq!(
            console_java_executable("javaw.exe").unwrap(),
            PathBuf::from("java.exe")
        );
        assert_eq!(
            console_java_executable("javaw").unwrap(),
            PathBuf::from("java.exe")
        );
    }

    #[test]
    fn removes_launcher_workarounds_without_clearing_os_or_gpu_environment() {
        let mut command = Command::new("java");
        for (key, value) in [
            ("WEBKIT_DISABLE_DMABUF_RENDERER", "1"),
            ("__NV_DISABLE_EXPLICIT_SYNC", "1"),
            ("_JAVA_OPTIONS", "-Xmx32M"),
            ("APPDATA", "C:/Users/Test/AppData/Roaming"),
            ("LOCALAPPDATA", "C:/Users/Test/AppData/Local"),
            ("USERPROFILE", "C:/Users/Test"),
            ("PATH", "java/bin"),
            ("DISPLAY", ":0"),
            ("WAYLAND_DISPLAY", "wayland-0"),
            ("DRI_PRIME", "1"),
            ("__NV_PRIME_RENDER_OFFLOAD", "1"),
        ] {
            command.env(key, value);
        }
        configure_java_environment(&mut command);
        let changes: BTreeMap<_, _> = command.get_envs().collect();
        for key in LAUNCHER_RENDERING_ENVIRONMENT
            .iter()
            .copied()
            .chain(["_JAVA_OPTIONS"])
        {
            assert_eq!(changes.get(OsStr::new(key)), Some(&None));
        }
        for key in [
            "APPDATA",
            "LOCALAPPDATA",
            "USERPROFILE",
            "PATH",
            "DISPLAY",
            "WAYLAND_DISPLAY",
            "DRI_PRIME",
            "__NV_PRIME_RENDER_OFFLOAD",
        ] {
            assert!(
                changes
                    .get(OsStr::new(key))
                    .is_some_and(|value| value.is_some())
            );
        }
    }

    #[test]
    fn explicit_instance_environment_can_override_the_clean_baseline() {
        let mut command = command("java");
        command.env("__NV_DISABLE_EXPLICIT_SYNC", "explicit-user-choice");
        let value = command
            .get_envs()
            .find(|(key, _)| *key == "__NV_DISABLE_EXPLICIT_SYNC")
            .unwrap()
            .1;
        assert_eq!(value, Some(OsStr::new("explicit-user-choice")));
    }

    #[test]
    fn pty_servers_remove_the_same_launcher_variables() {
        let mut command = portable_pty::CommandBuilder::new("java");
        for key in LAUNCHER_RENDERING_ENVIRONMENT
            .iter()
            .copied()
            .chain(["_JAVA_OPTIONS"])
        {
            command.env(key, "inherited-launcher-value");
        }
        command.env("APPDATA", "preserved-user-directory");
        configure_pty_java_environment(&mut command);
        let variables: BTreeMap<_, _> =
            command.iter_full_env_as_str().collect();
        for key in LAUNCHER_RENDERING_ENVIRONMENT
            .iter()
            .copied()
            .chain(["_JAVA_OPTIONS"])
        {
            assert!(!variables.contains_key(key));
        }
        assert_eq!(variables.get("APPDATA"), Some(&"preserved-user-directory"));
    }

    #[test]
    fn child_environment_probe() {
        if std::env::var_os("AXOLOTL_JAVA_ENVIRONMENT_TEST").is_none() {
            return;
        }
        for key in LAUNCHER_RENDERING_ENVIRONMENT
            .iter()
            .copied()
            .chain(["_JAVA_OPTIONS"])
        {
            assert!(
                std::env::var_os(key).is_none(),
                "unexpected child variable: {key}"
            );
        }
        assert_eq!(
            std::env::var_os("APPDATA"),
            Some(OsString::from("test-roaming"))
        );
        assert_eq!(
            std::env::var_os("LOCALAPPDATA"),
            Some(OsString::from("test-local"))
        );
        assert_eq!(
            std::env::var_os("USERPROFILE"),
            Some(OsString::from("test-profile"))
        );
    }

    #[test]
    fn spawned_child_receives_os_variables_without_launcher_workarounds() {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            "util::java_environment::tests::child_environment_probe",
            "--nocapture",
        ]);
        command.env("AXOLOTL_JAVA_ENVIRONMENT_TEST", "1");
        command.env("APPDATA", "test-roaming");
        command.env("LOCALAPPDATA", "test-local");
        command.env("USERPROFILE", "test-profile");
        for key in LAUNCHER_RENDERING_ENVIRONMENT
            .iter()
            .copied()
            .chain(["_JAVA_OPTIONS"])
        {
            command.env(key, "launcher-only");
        }
        configure_java_environment(&mut command);
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}
