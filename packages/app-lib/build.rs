use std::path::PathBuf;
use std::process::{Command, exit};
use std::{env, fs};

/// Build-time opt-in to a private launcher data directory. Read by
/// `theseus::brand::app_data_dir_identifier`, which appends it to the directory
/// name.
const DATA_DIR_SUFFIX_VAR: &str = "AXOLOTL_DATA_DIR_SUFFIX";

/// Public service defaults for downstream builds that have no `.env`.
/// Private Modrinth services remain disabled by Axolotl capabilities at runtime.
const MODRINTH_ENV_DEFAULTS: &[(&str, &str)] = &[
    ("MODRINTH_API_BASE_URL", "https://api.modrinth.com"),
    ("MODRINTH_API_URL", "https://api.modrinth.com/v2/"),
    ("MODRINTH_API_URL_V3", "https://api.modrinth.com/v3/"),
    (
        "MODRINTH_LAUNCHER_META_URL",
        "https://launcher-meta.modrinth.com/",
    ),
    ("MODRINTH_URL", "https://modrinth.com/"),
    ("MODRINTH_SOCKET_URL", "wss://disabled.invalid/"),
];

fn main() {
    // Only watch .env when it exists. A missing rerun-if-changed path keeps
    // Cargo treating the crate as dirty on every invocation.
    if PathBuf::from(".env").exists() {
        println!("cargo::rerun-if-changed=.env");
    }
    println!("cargo::rerun-if-env-changed=CURSEFORGE_API_KEY");
    for (name, _) in MODRINTH_ENV_DEFAULTS {
        println!("cargo::rerun-if-env-changed={name}");
    }
    println!("cargo::rerun-if-changed=java/gradle");
    println!("cargo::rerun-if-changed=java/src");
    println!("cargo::rerun-if-changed=java/build.gradle.kts");
    println!("cargo::rerun-if-changed=java/settings.gradle.kts");
    println!("cargo::rerun-if-changed=java/gradle.properties");

    #[cfg(target_os = "windows")]
    if env::var_os("CARGO_FEATURE_TAURI").is_some() {
        println!(
            "cargo::rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }

    set_env();
    build_java_jars();
}

fn set_env() {
    let curseforge_api_key = env::var("CURSEFORGE_API_KEY")
        .ok()
        .or_else(|| read_dotenv_literal("CURSEFORGE_API_KEY"));

    let dotenv_values: Vec<(String, String)> = dotenvy::dotenv_iter()
        .into_iter()
        .flatten()
        .flatten()
        .collect();

    for (var_name, var_value) in &dotenv_values {
        if var_name == "DATABASE_URL"
            || var_name == "CURSEFORGE_API_KEY"
            || var_name == DATA_DIR_SUFFIX_VAR
            || MODRINTH_ENV_DEFAULTS
                .iter()
                .any(|(name, _)| name == var_name)
        {
            // Handled explicitly below, where values are resolved with a
            // stable priority chain instead of being dumped as-is.
            continue;
        }

        println!("cargo::rustc-env={var_name}={var_value}");
    }

    // Single source of truth for env!() service URLs. Prefer local .env, then
    // an explicit process-env export, then public defaults. Always emit
    // rustc-env so Cargo does not mix process-env fingerprints with a
    // different baked value (which marked theseus dirty on every rebuild).
    for (name, default) in MODRINTH_ENV_DEFAULTS {
        let value = dotenv_values
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
            .or_else(|| env::var(name).ok().filter(|value| !value.is_empty()))
            .unwrap_or_else(|| (*default).to_string());
        println!("cargo::rustc-env={name}={value}");
    }

    if let Some(curseforge_api_key) = curseforge_api_key {
        println!("cargo::rustc-env=CURSEFORGE_API_KEY={curseforge_api_key}");
    }

    // Lets a local or test build keep its own data directory, so it cannot write
    // the database an installed launcher is using. Releases leave it unset and
    // resolve to the plain identifier.
    println!("cargo::rerun-if-env-changed={DATA_DIR_SUFFIX_VAR}");
    let data_dir_suffix = env::var(DATA_DIR_SUFFIX_VAR)
        .ok()
        .filter(|suffix| !suffix.is_empty())
        .or_else(|| {
            read_dotenv_literal(DATA_DIR_SUFFIX_VAR)
                .filter(|suffix| !suffix.is_empty())
        });

    if let Some(data_dir_suffix) = data_dir_suffix {
        // brand::data_dir_identifier drops every character that is not ASCII
        // alphanumeric or `-_.` and then trims dots and dashes off both ends, so
        // a suffix holding none of what survives sanitizes to nothing and the
        // build would fall back to the installed launcher's own data directory -
        // exactly the state this variable exists to avoid. Stop the build
        // instead of letting that happen silently.
        if !data_dir_suffix.chars().any(|character| {
            character.is_ascii_alphanumeric() || character == '_'
        }) {
            println!(
                "cargo::error={DATA_DIR_SUFFIX_VAR} leaves no usable directory name, so this build would use the installed launcher's data directory"
            );
            exit(1);
        }

        println!("cargo::rustc-env={DATA_DIR_SUFFIX_VAR}={data_dir_suffix}");
    }
}

fn read_dotenv_literal(name: &str) -> Option<String> {
    let contents = fs::read_to_string(".env").ok()?;

    contents.lines().find_map(|line| {
        let line = line.trim_start().strip_prefix("export ").unwrap_or(line);
        let (candidate, value) = line.split_once('=')?;
        if candidate.trim() != name {
            return None;
        }

        let value = value.trim();
        let value = if value.len() >= 2
            && ((value.starts_with('\'') && value.ends_with('\''))
                || (value.starts_with('"') && value.ends_with('"')))
        {
            &value[1..value.len() - 1]
        } else {
            value
        };

        (!value.is_empty()).then(|| value.to_string())
    })
}

fn newest_mtime(path: &PathBuf) -> Option<std::time::SystemTime> {
    let mut newest: Option<std::time::SystemTime> = None;
    if path.is_file() {
        return path.metadata().and_then(|m| m.modified()).ok();
    }
    if path.is_dir()
        && let Ok(entries) = fs::read_dir(path)
    {
        for entry in entries.flatten() {
            if let Some(time) = newest_mtime(&entry.path().to_path_buf()) {
                newest = Some(newest.map_or(time, |current| current.max(time)));
            }
        }
    }
    newest
}

fn java_jars_are_fresh(out_dir: &PathBuf) -> bool {
    let theseus_jar = out_dir.join("java/libs/theseus.jar");
    let authlib_jar = out_dir.join("java/libs/authlib-injector.jar");
    let Ok(theseus_time) = theseus_jar.metadata().and_then(|m| m.modified())
    else {
        return false;
    };
    let Ok(authlib_time) = authlib_jar.metadata().and_then(|m| m.modified())
    else {
        return false;
    };

    let input_paths = [
        PathBuf::from("java/src"),
        PathBuf::from("java/build.gradle.kts"),
        PathBuf::from("java/settings.gradle.kts"),
        PathBuf::from("java/gradle.properties"),
        PathBuf::from("java/gradle"),
    ];
    input_paths.iter().all(|input| {
        newest_mtime(input)
            .map(|input_time| {
                input_time <= theseus_time && input_time <= authlib_time
            })
            .unwrap_or(true)
    })
}

fn build_java_jars() {
    println!("cargo::rerun-if-env-changed=AXOLOTL_BUILD_RESOURCES");
    if let Some(resources) = env::var_os("AXOLOTL_BUILD_RESOURCES") {
        let resources = PathBuf::from(resources);
        let status = Command::new("node")
            .arg("scripts/axolotl/prepare-build-resources.mjs")
            .arg("--verify")
            .arg("--output")
            .arg(&resources)
            .current_dir("../..")
            .status()
            .expect("Failed to verify prepared Java resources");
        assert!(status.success(), "Prepared Java resources are invalid");
        println!(
            "cargo::rerun-if-changed={}",
            resources.join("manifest.json").display()
        );
        println!(
            "cargo::rustc-env=JAVA_JARS_DIR={}",
            resources.join("java").display()
        );
        return;
    }

    let out_dir =
        dunce::canonicalize(PathBuf::from(env::var_os("OUT_DIR").unwrap()))
            .unwrap();

    println!(
        "cargo::rustc-env=JAVA_JARS_DIR={}",
        out_dir.join("java/libs").display()
    );

    if java_jars_are_fresh(&out_dir) {
        return;
    }

    let gradle_path = fs::canonicalize(
        #[cfg(target_os = "windows")]
        "java\\gradlew.bat",
        #[cfg(not(target_os = "windows"))]
        "java/gradlew",
    )
    .unwrap();

    let mut command = Command::new(gradle_path);
    command
        .arg(format!(
            "-PaxolotlBuildDir={}",
            out_dir.join("java").display()
        ))
        .arg("build")
        .arg("--console=rich")
        .current_dir(dunce::canonicalize("java").unwrap());

    // A persistent Gradle daemon can inherit Cargo's build-script output pipe
    // on Windows. Cargo then waits forever for EOF after Gradle has completed.
    // CI runners are ephemeral and do not benefit from keeping a daemon.
    if cfg!(windows) || env::var_os("CI").is_some() {
        command.arg("--no-daemon");
    }

    let exit_status = command.status().expect("Failed to wait on Gradle build");

    if !exit_status.success() {
        println!("cargo::error=Gradle build failed with {exit_status}");
        exit(exit_status.code().unwrap_or(1));
    }
}
