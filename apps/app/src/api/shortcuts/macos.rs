use crate::api::Result;
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    path::Path,
};
use url::Url;

pub(super) const SHORTCUT_EXTENSION: &str = "app";
pub(super) const SHORTCUT_ICON_EXTENSION: &str = "icns";

pub(super) async fn create_shortcut(
    profile_name: &str,
    launch_url: &Url,
    output_path: &Path,
    icon_path: Option<&Path>,
    _app_identifier: &str,
) -> Result<()> {
    let contents_dir = output_path.join("Contents");
    let macos_dir = contents_dir.join("MacOS");
    let resources_dir = contents_dir.join("Resources");
    tokio::fs::create_dir_all(&macos_dir).await?;
    tokio::fs::create_dir_all(&resources_dir).await?;

    let executable_path = macos_dir.join("launch");
    tokio::fs::write(
        &executable_path,
        format!(
            "#!/bin/sh\nexec /usr/bin/open {}\n",
            shell_quote(launch_url.as_str()),
        ),
    )
    .await?;

    let bundled_icon_path = resources_dir.join("icon.icns");
    if let Some(icon_path) = icon_path {
        let _ = tokio::fs::remove_file(&bundled_icon_path).await;
        std::os::unix::fs::symlink(icon_path, &bundled_icon_path)?;
    } else {
        tokio::fs::write(
            &bundled_icon_path,
            include_bytes!("../../../icons/icon.icns"),
        )
        .await?;
    }

    tokio::fs::write(
        contents_dir.join("Info.plist"),
        format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>launch</string>
    <key>CFBundleIdentifier</key>
    <string>{}</string>
    <key>CFBundleIconFile</key>
    <string>icon.icns</string>
    <key>CFBundleName</key>
    <string>{}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
</dict>
</plist>
"#,
            macos_shortcut_identifier(launch_url.as_str()),
            escape_xml(&format!("Launch {profile_name}")),
        ),
    )
    .await?;

    use std::os::unix::fs::PermissionsExt;

    let mut permissions =
        tokio::fs::metadata(&executable_path).await?.permissions();
    permissions.set_mode(0o755);
    tokio::fs::set_permissions(&executable_path, permissions).await?;

    Ok(())
}

fn macos_shortcut_identifier(launch_url: &str) -> String {
    let mut hasher = DefaultHasher::new();
    launch_url.hash(&mut hasher);

    format!("com.modrinth.instance-shortcut.{:x}", hasher.finish())
}

fn shell_quote(input: &str) -> String {
    format!("'{}'", input.replace('\'', "'\\''"))
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
