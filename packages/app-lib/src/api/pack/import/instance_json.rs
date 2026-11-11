use std::io::Read;
use std::path::Path;

use serde_json::Value;
use tracing::debug;

pub struct InstanceInfo {
    pub vanilla_name: String,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub adjuncts: Vec<(String, Option<String>)>,
}

fn find_json(path: &Path) -> Option<(String, String)> {
    let name = path.file_name()?.to_string_lossy().to_string();
    let primary = path.join(format!("{name}.json"));
    debug!(
        "instance_json: path={} looking for primary={}",
        path.display(),
        primary.display()
    );
    if primary.exists() {
        debug!(
            "instance_json: path={} json={} (by name match)",
            path.display(),
            primary.display()
        );
        let content = std::fs::read_to_string(&primary).ok()?;
        debug!(
            "instance_json: path={} primary content (len={}, first_200={:?})",
            path.display(),
            content.len(),
            &content[..content.len().min(200)]
        );
        return Some((name, content));
    }
    debug!(
        "instance_json: path={} primary={} NOT FOUND, enumerating directory",
        path.display(),
        primary.display()
    );
    let mut json_files = Vec::new();
    if let Ok(dir) = std::fs::read_dir(path) {
        for entry in dir.flatten() {
            let p = entry.path();
            debug!(
                "instance_json: path={} entry={}",
                path.display(),
                p.display()
            );
            if p.extension().map(|e| e == "json").unwrap_or(false) {
                json_files.push(p);
            }
        }
    }
    debug!(
        "instance_json: path={} found {} json files",
        path.display(),
        json_files.len()
    );
    if json_files.len() == 1 {
        debug!(
            "instance_json: path={} json={} (sole json fallback)",
            path.display(),
            json_files[0].display()
        );
        let content = std::fs::read_to_string(&json_files[0]).ok()?;
        debug!(
            "instance_json: path={} sole json content (len={}, first_200={:?})",
            path.display(),
            content.len(),
            &content[..content.len().min(200)]
        );
        let name = json_files[0]
            .file_stem()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or(name);
        return Some((name, content));
    }
    // Multiple JSONs: try each one, return the first with a valid version.
    // A single unreadable or malformed candidate must not abort the loop.
    for jf in &json_files {
        let Ok(content) = std::fs::read_to_string(jf) else {
            debug!(
                "instance_json: path={} json={} unreadable, trying next",
                path.display(),
                jf.display()
            );
            continue;
        };
        debug!(
            "instance_json: path={} trying json={} (len={}, first_200={:?})",
            path.display(),
            jf.display(),
            content.len(),
            &content[..content.len().min(200)]
        );
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&content)
        else {
            debug!(
                "instance_json: path={} json={} invalid JSON, trying next",
                path.display(),
                jf.display()
            );
            continue;
        };
        let version =
            extract_version(&json, &content, &jf.with_extension("jar"), None);
        if !version.is_empty() {
            let fname = jf
                .file_stem()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| name.clone());
            debug!(
                "instance_json: path={} json={} (multiple-json pick, version={})",
                path.display(),
                jf.display(),
                version
            );
            return Some((fname, content));
        }
    }
    debug!(
        "instance_json: path={} multiple={} json files, none yielded a version",
        path.display(),
        json_files.len()
    );
    None
}

pub fn detect(path: &Path) -> Option<InstanceInfo> {
    let (name, content) = find_json(path)?;
    let json: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            debug!("instance_json: path={} parse_err={}", path.display(), e);
            return None;
        }
    };
    let client_jar = path.join(format!("{name}.jar"));
    let mut vanilla_name =
        extract_version(&json, &content, &client_jar, Some(&name));
    debug!(
        "instance_json: path={} extract_version returned {:?}",
        path.display(),
        vanilla_name
    );
    if vanilla_name.is_empty() {
        debug!(
            "instance_json: path={} version empty or Unknown",
            path.display()
        );
        return None;
    }
    vanilla_name = normalize_version(&vanilla_name);
    let mut loader = detect_loader(&content, &json).map(|(loader, version)| {
        let version = version.map(|version| {
            normalize_imported_loader_version(&loader, &vanilla_name, &version)
        });
        (loader, version)
    });
    // HMCL-family launchers ship Cleanroom instances as a plain Forge version
    // JSON that is named after Cleanroom and inject the
    // `com.cleanroommc:cleanroom` library at launch time, so the JSON content
    // alone looks exactly like Forge. Treat a Cleanroom marker in the version
    // folder name or id as authoritative and install Cleanroom natively
    // instead of Forge.
    if loader.as_ref().is_some_and(|(loader, _)| loader == "forge")
        && (name.to_ascii_lowercase().contains("cleanroom")
            || json.get("id").and_then(Value::as_str).is_some_and(|id| {
                id.to_ascii_lowercase().contains("cleanroom")
            }))
    {
        debug!(
            "instance_json: forge json carries a cleanroom version marker; treating as cleanroom"
        );
        loader = Some(("cleanroom".into(), None));
    }
    let adjuncts = detect_adjuncts(
        &content,
        loader.as_ref().map(|(loader, _)| loader.as_str()),
    )
    .into_iter()
    .map(|(loader, version)| {
        let version = version.map(|version| {
            normalize_imported_loader_version(&loader, &vanilla_name, &version)
        });
        (loader, version)
    })
    .collect();
    debug!(
        "instance_json: path={} version={} loader={:?}",
        path.display(),
        vanilla_name,
        loader.as_ref().map(|(t, _)| t.as_str())
    );
    Some(InstanceInfo {
        vanilla_name,
        loader: loader.as_ref().map(|(t, _)| t.clone()),
        loader_version: loader.and_then(|(_, v)| v),
        adjuncts,
    })
}

fn normalize_version(raw: &str) -> String {
    let mut v = raw.to_string();
    if (v.starts_with("20.") || v.starts_with("21.")) && !v.starts_with("1.") {
        v = format!("1.{v}");
    }
    v = v.replace("_unobfuscated", "");
    v = v.replace(" Unobfuscated", "");
    v.trim().to_string()
}

pub(crate) fn normalize_imported_loader_version(
    loader: &str,
    game_version: &str,
    detected_version: &str,
) -> String {
    let detected_version = detected_version.trim();
    let without_family = match loader {
        "fabric" | "legacy_fabric" => detected_version
            .strip_prefix("fabric-loader-")
            .or_else(|| detected_version.strip_prefix("fabric-")),
        "quilt" => detected_version
            .strip_prefix("quilt-loader-")
            .or_else(|| detected_version.strip_prefix("quilt-")),
        "forge" => detected_version.strip_prefix("forge-"),
        "neoforge" => detected_version
            .strip_prefix("neoforge-")
            .or_else(|| detected_version.strip_prefix("neo-")),
        _ => None,
    }
    .unwrap_or(detected_version);

    match loader {
        "fabric" | "legacy_fabric" | "quilt" => without_family
            .strip_suffix(&format!("-{game_version}"))
            .unwrap_or(without_family)
            .to_string(),
        "forge" | "neoforge" => {
            let stripped = without_family
                .strip_prefix(&format!("{game_version}-"))
                .unwrap_or(without_family);
            // Legacy Forge (e.g. 1.7.10) embeds the MC version as a trailing
            // suffix: `1.7.10-10.13.4.1614-1.7.10` -> `10.13.4.1614`. Strip it so
            // the id matches the metadata manifest. When the suffix is absent
            // (modern `1.20.1-47.4.22`), keep the already prefix-stripped
            // version rather than reverting to the raw detected value.
            stripped
                .strip_suffix(&format!("-{game_version}"))
                .unwrap_or(stripped)
                .to_string()
        }
        _ => without_family.to_string(),
    }
}

fn extract_version(
    json: &Value,
    json_str: &str,
    client_jar: &Path,
    folder_name: Option<&str>,
) -> String {
    // ① PCL download record clientVersion
    if let Some(v) = json.get("clientVersion").and_then(|v| v.as_str())
        && !v.is_empty()
    {
        debug!("extract_version: method=① clientVersion value={}", v);
        return v.to_string();
    }

    // ② HMCL patches[].version (id == "game")
    if let Some(patches) = json.get("patches").and_then(|v| v.as_array()) {
        for patch in patches {
            if patch.get("id").and_then(|v| v.as_str()) == Some("game")
                && let Some(ver) = patch.get("version").and_then(|v| v.as_str())
                && !ver.is_empty()
            {
                debug!(
                    "extract_version: method=② patches.game.version value={}",
                    ver
                );
                return ver.to_string();
            }
        }
    }

    // ③ arguments.game --fml.mcVersion (Forge/NeoForge)
    if let Some(args) = json
        .get("arguments")
        .and_then(|v| v.get("game"))
        .and_then(|v| v.as_array())
    {
        let mut mark = false;
        for arg in args {
            if mark && let Some(v) = arg.as_str() {
                debug!("extract_version: method=③ --fml.mcVersion value={}", v);
                return v.to_string();
            }
            if arg.as_str() == Some("--fml.mcVersion") {
                mark = true;
            }
        }
    }

    // ④ inheritsFrom (version inheritance) — must come before the `jar`
    // field, which is not always a version name.
    if let Some(v) = json.get("inheritsFrom").and_then(|v| v.as_str())
        && !v.is_empty()
    {
        debug!("extract_version: method=④ inheritsFrom value={}", v);
        return v.to_string();
    }

    // ⑤ libraries string regex fallback (Forge/OptiFine/FabricLike lib versions)
    // Use the original JSON string (from find_json) instead of re-serializing
    // the parsed Value, which would allocate a fresh string unnecessarily.
    if let Some(v) = extract_version_from_libraries(json_str) {
        debug!("extract_version: method=⑤ libraries value={}", v);
        return v;
    }

    // ⑥ version.json inside the client jar (18w47b and later). Official
    // version JSONs have none of the records above, and this also holds for a
    // copy renamed by a launcher.
    if let Some(v) = version_from_client_jar(client_jar) {
        debug!("extract_version: method=⑥ client jar value={}", v);
        return v;
    }

    // Everything below infers the version from a name, so it only runs when
    // the JSON and the jar record none.
    // ⑦ JSON id field → extract leading version
    if let Some(id) = json.get("id").and_then(|v| v.as_str())
        && let Some(v) = extract_version_from_id(id)
    {
        debug!("extract_version: method=⑦ id id={} value={}", id, v);
        return v;
    }

    // ⑧ jar field (legacy versions store the base game in `jar`)
    if let Some(v) = json.get("jar").and_then(|v| v.as_str())
        && !v.is_empty()
    {
        debug!("extract_version: method=⑧ jar value={}", v);
        return v.to_string();
    }

    // ⑨ folder name fallback (renamed / non-standard instances)
    if let Some(name) = folder_name
        && let Some(v) = extract_version_from_id(name)
    {
        debug!("extract_version: method=⑨ folder_name value={}", v);
        return v;
    }

    debug!("extract_version: method=✗ all methods failed");
    String::new()
}

/// Extracts Minecraft version from library artifact coordinates in the JSON string.
/// Matches PCLCE's approach scanning for Forge/OptiFine/FabricLike lib entries.
/// Order: NeoForge before Forge (NeoForge JSON often also contains forge references).
fn extract_version_from_libraries(content: &str) -> Option<String> {
    // NeoForge: net.neoforged:neoforge:1.20.1-44.0.3 → "1.20.1"
    // Try known Maven coordinate formats (neoforge before forge).
    for needle in [
        "net.neoforged:neoforge:",
        "net.neoforged.neoforge:neoforge:",
        "net.neoforged.fml:modern:",
    ] {
        if let Some(pos) = content.find(needle) {
            let after = &content[pos + needle.len()..];
            if let Some(end) = after.find(&['"', ',', '\n', '}'] as &[char]) {
                let ver = &after[..end];
                if let Some(dash) = ver.find('-') {
                    return Some(ver[..dash].to_string());
                }
                return Some(ver.to_string());
            }
        }
    }
    // Forge: minecraftforge:forge:1.8.9-11.15.1.1722 → "1.8.9"
    //        net.minecraftforge:forge:1.21.1-52.0.0 (modern Forge, 1.13+)
    for needle in ["minecraftforge:forge:", "net.minecraftforge:forge:"] {
        if let Some(pos) = content.find(needle) {
            let after = &content[pos + needle.len()..];
            if let Some(end) = after.find(&['"', ',', '\n', '}'] as &[char]) {
                let ver = &after[..end];
                if let Some(dash) = ver.find('-') {
                    return Some(ver[..dash].to_string());
                }
                return Some(ver.to_string());
            }
        }
    }
    // OptiFine: optifine:OptiFine:1.8.9_HD_U_H5 → "1.8.9"
    if let Some(pos) = content.find("optifine:OptiFine:") {
        let after = &content[pos + "optifine:OptiFine:".len()..];
        if let Some(end) = after.find(&['"', ',', '\n', '}'] as &[char]) {
            let ver = &after[..end];
            if let Some(underscore) = ver.find('_') {
                return Some(ver[..underscore].to_string());
            }
            return Some(ver.to_string());
        }
    }
    // Fabric-like: net.fabricmc:fabric-loader:0.15.11-1.20.1 → "1.20.1"
    if let Some(pos) = content.find("net.fabricmc:fabric-loader:") {
        let after = &content[pos + "net.fabricmc:fabric-loader:".len()..];
        if let Some(end) = after.find(&['"', ',', '\n', '}'] as &[char]) {
            let ver = &after[..end];
            if let Some(dash) = ver.rfind('-') {
                return Some(ver[dash + 1..].to_string());
            }
        }
    }
    // Fabric-like: net.fabricmc:intermediary:1.20.1 → "1.20.1"
    for needle in [
        "net.fabricmc:intermediary:",
        "net.legacyfabric:intermediary:",
        "org.quiltmc:hashed:",
    ] {
        if let Some(version) =
            try_extract_version_from_needle(content, needle, None)
        {
            return Some(version);
        }
    }
    None
}

/// Reads the game version the client jar records about itself. The id of some
/// versions carries a build hash: "1.14 Pre-Release 5 / c20937bb...".
fn version_from_client_jar(jar: &Path) -> Option<String> {
    let file = std::fs::File::open(jar).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let entry = archive.by_name("version.json").ok()?;
    let metadata: Value = serde_json::from_reader(entry.take(65_536)).ok()?;
    let id = metadata.get("id")?.as_str()?;
    let id = id.split(" / ").next().unwrap_or(id).trim();
    (!id.is_empty()).then(|| id.to_string())
}

/// Where the Mojang pre-release qualifier after `separator` ends, if there is
/// one: the `-snapshot-7` of "26.4-snapshot-7", the `-pre1` of "1.21-pre1" or the
/// ` Pre-Release 5` of "1.14 Pre-Release 5". These are part of the game version.
/// A qualifier needs its number, so a name like "1.20.1-rc-pack" is not one.
fn prerelease_qualifier_end(id: &str, separator: usize) -> Option<usize> {
    let rest = &id[separator + 1..];
    let lower = rest.to_ascii_lowercase();
    let label = ["snapshot", "pre-release", "pre", "rc"]
        .into_iter()
        .find(|label| lower.starts_with(label))?;
    let after_label = &rest[label.len()..];
    let number = after_label.strip_prefix(['-', ' ']).unwrap_or(after_label);
    let digits = number.len()
        - number
            .trim_start_matches(|c: char| c.is_ascii_digit())
            .len();
    if digits == 0 {
        return None;
    }
    let end = id.len() - number.len() + digits;
    id[end..]
        .chars()
        .next()
        .is_none_or(|next| matches!(next, '-' | '_' | ' '))
        .then_some(end)
}

/// Extracts leading version number from the instance id.
/// e.g. "1.8.9-forge-11.15.1.1722" → "1.8.9"
/// Keeps the pre-release qualifier of an official id, since the version manifest
/// lists "26.4-snapshot-7" and has no "26.4".
/// Skips hash-like ids (≥32 chars, no separators).
fn extract_version_from_id(id: &str) -> Option<String> {
    let ver = id.trim();
    if ver.is_empty() {
        return None;
    }
    if ver.len() >= 32
        && !ver.contains('.')
        && !ver.contains('-')
        && !ver.contains('_')
    {
        return None;
    }
    if let Some(first_sep) = ver.find(['-', '_', ' ']) {
        let end = prerelease_qualifier_end(ver, first_sep).unwrap_or(first_sep);
        let candidate = &ver[..end];
        if candidate.starts_with("1.") || candidate.starts_with('2') {
            return Some(candidate.to_string());
        }
    }
    if ver.starts_with("1.") || ver.starts_with('2') {
        return Some(ver.to_string());
    }
    None
}

///参考自PCL启动器
fn detect_loader(
    content: &str,
    json: &Value,
) -> Option<(String, Option<String>)> {
    let lower = content.to_lowercase();

    // LabyMod
    if lower.contains("labymod_data") {
        let version = json
            .get("labymod_data")
            .and_then(|v| v.get("version"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        return Some(("labymod".into(), version));
    }

    // Legacy Fabric
    if lower.contains("net.legacyfabric:intermediary") {
        let version = try_extract_version_from_needle(
            content,
            "net.fabricmc:fabric-loader:",
            None,
        );
        return Some(("legacy_fabric".into(), version));
    }

    // Fabric
    if lower.contains("net.fabricmc:fabric-loader") {
        let version = try_extract_version_from_needle(
            content,
            "net.fabricmc:fabric-loader:",
            None,
        );
        return Some(("fabric".into(), version));
    }

    // Quilt
    if lower.contains("org.quiltmc:quilt-loader") {
        let version = try_extract_version_from_needle(
            content,
            "org.quiltmc:quilt-loader:",
            None,
        );
        return Some(("quilt".into(), version));
    }

    // Cleanroom
    if lower.contains("com.cleanroommc:cleanroom:") {
        let version = try_extract_version_from_needle(
            content,
            "com.cleanroommc:cleanroom:",
            None,
        );
        return Some(("cleanroom".into(), version));
    }

    // Forge
    if lower.contains("minecraftforge") && !lower.contains("net.neoforge") {
        let version = try_extract_version_from_needle(
            content,
            "minecraftforge:forge:",
            None,
        )
        .or_else(|| {
            try_extract_version_from_needle(
                content,
                "net.minecraftforge:forge:",
                None,
            )
        })
        .or_else(|| {
            try_extract_version_from_needle(
                content,
                "net.minecraftforge:fmlloader:",
                None,
            )
        });
        return Some(("forge".into(), version));
    }

    // NeoForge
    if lower.contains("net.neoforge") {
        let version = try_extract_version_from_needle(
            content,
            "net.neoforged:neoforge:",
            None,
        )
        .or_else(|| {
            try_extract_version_from_needle(
                content,
                "net.neoforged.neoforge:neoforge:",
                None,
            )
        });
        return Some(("neoforge".into(), version));
    }

    // OptiFine
    if lower.contains("optifine") {
        let version = try_extract_version_from_needle(
            content,
            "optifine:OptiFine:",
            None,
        );
        if version.is_some() {
            return Some(("optifine".into(), version));
        }
    }

    // LiteLoader
    if lower.contains("liteloader") {
        return Some(("lite_loader".into(), None));
    }

    debug!("detect_loader: no known loader library found in JSON content");
    None
}

fn detect_adjuncts(
    content: &str,
    primary_loader: Option<&str>,
) -> Vec<(String, Option<String>)> {
    let lower = content.to_ascii_lowercase();
    let mut adjuncts = Vec::new();
    if primary_loader != Some("lite_loader") && lower.contains("liteloader") {
        adjuncts.push(("lite_loader".to_string(), None));
    }
    if primary_loader != Some("optifine") && lower.contains("optifine") {
        let version = try_extract_version_from_needle(
            content,
            "optifine:OptiFine:",
            None,
        )
        .or_else(|| {
            try_extract_version_from_needle(&lower, "optifine:optifine:", None)
        });
        if version.is_some() {
            adjuncts.push(("optifine".to_string(), version));
        }
    }
    if lower.contains("optifabric") {
        let version = try_extract_version_from_needle(
            &lower,
            "me.modmuss50:optifabric:",
            None,
        )
        .or_else(|| {
            try_extract_version_from_needle(
                &lower,
                "optifabric:optifabric:",
                None,
            )
        });
        adjuncts.push(("optifabric".to_string(), version));
    }
    adjuncts
}

/// Extracts the loader version string from JSON content by finding a needle
/// and reading until a terminator character.
fn try_extract_version_from_needle(
    content: &str,
    needle: &str,
    split_at: Option<char>,
) -> Option<String> {
    let pos = content.find(needle)?;
    let after = &content[pos + needle.len()..];
    let end = after.find(&['"', ',', '\n', '}'] as &[char])?;
    let ver = &after[..end];
    if let Some(ch) = split_at
        && let Some(pos) = ver.rfind(ch)
    {
        Some(ver[pos + 1..].to_string())
    } else {
        Some(ver.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exported_vanilla_with_custom_name_preserves_exact_client_version() {
        let directory = tempfile::tempdir().unwrap();
        let version_dir = directory.path().join("versions/Survival");
        std::fs::create_dir_all(&version_dir).unwrap();
        for version in ["1.21.1", "24w14a", "b1.7.3", "rd-132211", "c0.30_01c"]
        {
            let info: daedalus::minecraft::VersionInfo = serde_json::from_value(serde_json::json!({
                "id": version, "assets": "legacy",
                "assetIndex": {"id": "legacy", "sha1": "", "size": 0, "totalSize": 0, "url": ""},
                "downloads": {}, "libraries": [], "mainClass": "net.minecraft.client.main.Main",
                "minimumLauncherVersion": 0,
                "releaseTime": "2017-09-18T08:39:46Z", "time": "2021-12-15T15:04:05Z",
                "type": "release"
            })).unwrap();
            let portable = crate::launcher::external_version::project_manifest(
                &info, "Survival", version,
            )
            .unwrap();
            std::fs::write(
                version_dir.join("Survival.json"),
                serde_json::to_vec(&portable.document).unwrap(),
            )
            .unwrap();
            let detected =
                detect(&version_dir).expect("custom version is rediscovered");
            assert_eq!(detected.vanilla_name, version);
            assert_eq!(detected.loader, None);
        }
    }

    fn detect_from_json(content: &str) -> Option<(String, Option<String>)> {
        let json: Value = serde_json::from_str(content).expect("test JSON");
        detect_loader(content, &json)
    }

    fn assert_loader(
        content: &str,
        expected: &str,
        expected_version: Option<&str>,
    ) {
        let (loader, version) = detect_from_json(content)
            .unwrap_or_else(|| panic!("expected loader {expected}, got None"));
        assert_eq!(loader, expected);
        assert_eq!(version.as_deref(), expected_version);
    }

    #[test]
    fn test_forge() {
        assert_loader(
            r#"{
                "id": "1.21.1-forge-52.0.0",
                "libraries": [
                    {
                        "name": "net.minecraftforge:forge:1.21.1-52.0.0"
                    }
                ]
            }"#,
            "forge",
            Some("1.21.1-52.0.0"),
        );
    }

    #[test]
    fn normalizes_imported_loader_versions_for_central_resolution() {
        for (loader, game_version, detected, expected) in [
            ("fabric", "1.20.1", "0.15.11-1.20.1", "0.15.11"),
            ("quilt", "1.20.1", "0.26.4-1.20.1", "0.26.4"),
            (
                "forge",
                "1.7.10",
                "1.7.10-10.13.4.1614-1.7.10",
                "10.13.4.1614",
            ),
            ("forge", "1.20.1", "1.20.1-47.4.22", "47.4.22"),
            ("neoforge", "1.20.1", "1.20.1-44.0.3", "44.0.3"),
            ("neoforge", "1.21.4", "21.4.157", "21.4.157"),
        ] {
            assert_eq!(
                normalize_imported_loader_version(
                    loader,
                    game_version,
                    detected
                ),
                expected,
                "{loader} {game_version} {detected}"
            );
        }
    }

    #[test]
    fn test_neoforge() {
        assert_loader(
            r#"{
                "id": "1.20.1-neoforge-44.0.3",
                "libraries": [
                    {
                        "name": "net.neoforged:neoforge:1.20.1-44.0.3"
                    }
                ]
            }"#,
            "neoforge",
            Some("1.20.1-44.0.3"),
        );
    }

    #[test]
    fn test_fabric() {
        assert_loader(
            r#"{
                "id": "1.20.1-fabric-0.15.11",
                "libraries": [
                    {
                        "name": "net.fabricmc:fabric-loader:0.15.11-1.20.1"
                    }
                ]
            }"#,
            "fabric",
            Some("0.15.11-1.20.1"),
        );
    }

    #[test]
    fn test_quilt() {
        assert_loader(
            r#"{
                "id": "1.20.1-quilt-0.26.4",
                "libraries": [
                    {
                        "name": "org.quiltmc:quilt-loader:0.26.4-1.20.1"
                    }
                ]
            }"#,
            "quilt",
            Some("0.26.4-1.20.1"),
        );
    }

    #[test]
    fn test_optifine() {
        assert_loader(
            r#"{
                "id": "1.8.9-OptiFine",
                "libraries": [
                    {
                        "name": "optifine:OptiFine:1.8.9_HD_U_H5"
                    }
                ]
            }"#,
            "optifine",
            Some("1.8.9_HD_U_H5"),
        );
    }

    #[test]
    fn detects_forge_with_liteloader_and_optifine_adjuncts() {
        let content = r#"{
            "id": "1.12.2-forge-combined",
            "libraries": [
                { "name": "net.minecraftforge:forge:1.12.2-14.23.5.2860" },
                { "name": "com.mumfrey:liteloader:1.12.2-SNAPSHOT" },
                { "name": "optifine:OptiFine:1.12.2_HD_U_G5" }
            ]
        }"#;
        let json: Value = serde_json::from_str(content).unwrap();
        let primary = detect_loader(content, &json).unwrap();
        let adjuncts = detect_adjuncts(content, Some(&primary.0));

        assert_eq!(primary.0, "forge");
        assert_eq!(
            adjuncts,
            vec![
                ("lite_loader".to_string(), None),
                ("optifine".to_string(), Some("1.12.2_HD_U_G5".to_string())),
            ]
        );
    }

    #[test]
    fn test_legacy_fabric() {
        assert_loader(
            r#"{
                "id": "1.8.9-legacy-fabric-0.13.1.4",
                "libraries": [
                    {
                        "name": "net.legacyfabric:intermediary:1.8.9"
                    },
                    {
                        "name": "net.fabricmc:fabric-loader:0.13.1.4-1.8.9"
                    }
                ]
            }"#,
            "legacy_fabric",
            Some("0.13.1.4-1.8.9"),
        );
    }

    #[test]
    fn test_cleanroom() {
        assert_loader(
            r#"{
                "id": "1.12.2-cleanroom-7.1.0",
                "libraries": [
                    {
                        "name": "com.cleanroommc:cleanroom:1.12.2-7.1.0"
                    }
                ]
            }"#,
            "cleanroom",
            Some("1.12.2-7.1.0"),
        );
    }

    #[test]
    fn cleanroom_named_forge_version_detects_as_cleanroom() {
        let root = std::env::temp_dir().join(format!(
            "axolotl-instance-json-cleanroom-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let version_dir = root.join("versions").join("1.12.2-Cleanroom");
        std::fs::create_dir_all(&version_dir).expect("create version dir");
        std::fs::write(
            version_dir.join("1.12.2-Cleanroom.json"),
            r#"{
                "id": "1.12.2-Cleanroom",
                "mainClass": "net.minecraft.launchwrapper.Launch",
                "minecraftArguments": "--tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker",
                "libraries": [
                    {"name": "net.minecraftforge:forge:1.12.2-14.23.5.2864"}
                ]
            }"#,
        )
        .expect("write version json");

        let info = detect(&version_dir).expect("detects instance");
        assert_eq!(info.vanilla_name, "1.12.2");
        assert_eq!(info.loader.as_deref(), Some("cleanroom"));
        // The forge version is meaningless for Cleanroom; the import resolves
        // the Cleanroom version from metadata instead.
        assert_eq!(info.loader_version, None);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn forge_version_without_cleanroom_marker_stays_forge() {
        let root = std::env::temp_dir().join(format!(
            "axolotl-instance-json-forge-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let version_dir = root.join("versions").join("1.12.2-forge");
        std::fs::create_dir_all(&version_dir).expect("create version dir");
        std::fs::write(
            version_dir.join("1.12.2-forge.json"),
            r#"{
                "id": "1.12.2-forge-14.23.5.2864",
                "libraries": [
                    {"name": "net.minecraftforge:forge:1.12.2-14.23.5.2864"}
                ]
            }"#,
        )
        .expect("write version json");

        let info = detect(&version_dir).expect("detects instance");
        assert_eq!(info.loader.as_deref(), Some("forge"));
        assert_eq!(info.loader_version.as_deref(), Some("14.23.5.2864"));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn test_labymod() {
        assert_loader(
            r#"{
                "id": "1.20.1-labymod",
                "labymod_data": {
                    "version": "4.4.20"
                }
            }"#,
            "labymod",
            Some("4.4.20"),
        );
    }

    #[test]
    fn test_lite_loader() {
        assert_loader(
            r#"{
                "id": "1.12.2-LiteLoader-1.12.2-SNAPSHOT",
                "libraries": [
                    {
                        "name": "com.mumfrey:liteloader:1.12.2"
                    }
                ]
            }"#,
            "lite_loader",
            None,
        );
    }

    #[test]
    fn test_no_loader() {
        assert!(detect_from_json(r#"{"id": "1.20.4"}"#).is_none());
    }

    #[test]
    fn version_ids_keep_official_prerelease_qualifiers() {
        for (id, expected) in [
            ("26.4-snapshot-7", "26.4-snapshot-7"),
            ("26.1-pre-1", "26.1-pre-1"),
            ("26.1-rc-2", "26.1-rc-2"),
            ("1.21-pre1", "1.21-pre1"),
            ("1.21-rc1", "1.21-rc1"),
            ("1.14 Pre-Release 5", "1.14 Pre-Release 5"),
            ("1.RV-Pre1", "1.RV-Pre1"),
            ("26.4-snapshot-7-Survival", "26.4-snapshot-7"),
            ("1.21-pre1_unobfuscated", "1.21-pre1"),
        ] {
            assert_eq!(
                extract_version_from_id(id).as_deref(),
                Some(expected),
                "{id}"
            );
        }
    }

    #[test]
    fn version_ids_still_drop_loader_and_custom_suffixes() {
        for (id, expected) in [
            ("1.8.9-forge-11.15.1.1722", "1.8.9"),
            ("1.20.1-fabric-0.15.11", "1.20.1"),
            ("1.20.1-rc-pack", "1.20.1"),
            ("1.20.1-preview", "1.20.1"),
            ("1.20.1-pre1.5", "1.20.1"),
            ("26.4-Survival", "26.4"),
            ("1.21.11_unobfuscated", "1.21.11"),
        ] {
            assert_eq!(
                extract_version_from_id(id).as_deref(),
                Some(expected),
                "{id}"
            );
        }
    }

    #[test]
    fn official_snapshot_instance_keeps_its_full_version() {
        let dir = tempfile::tempdir().expect("temp dir");
        let version_dir = dir.path().join("26.4-snapshot-7");
        std::fs::create_dir(&version_dir).expect("create version dir");
        std::fs::write(
            version_dir.join("26.4-snapshot-7.json"),
            r#"{
                "id": "26.4-snapshot-7",
                "type": "snapshot",
                "mainClass": "net.minecraft.client.main.Main",
                "libraries": [{ "name": "com.mojang:brigadier:1.3.10" }]
            }"#,
        )
        .expect("write version json");

        let info = detect(&version_dir).expect("detect should succeed");
        assert_eq!(info.vanilla_name, "26.4-snapshot-7");
        assert_eq!(info.loader, None);
    }

    fn write_version_dir(
        root: &Path,
        name: &str,
        json: &str,
    ) -> std::path::PathBuf {
        let version_dir = root.join(name);
        std::fs::create_dir(&version_dir).expect("create version dir");
        std::fs::write(version_dir.join(format!("{name}.json")), json)
            .expect("write version json");
        version_dir
    }

    fn write_client_jar(version_dir: &Path, name: &str, version_json: &str) {
        use std::io::Write;
        let jar =
            std::fs::File::create(version_dir.join(format!("{name}.jar")))
                .expect("create jar");
        let mut zip = zip::ZipWriter::new(jar);
        zip.start_file(
            "version.json",
            zip::write::SimpleFileOptions::default(),
        )
        .expect("start entry");
        zip.write_all(version_json.as_bytes()).expect("write entry");
        zip.finish().expect("finish jar");
    }

    #[test]
    fn client_jar_outranks_a_version_inferred_from_a_name() {
        for (recorded, expected) in [
            ("26.4-snapshot-7", "26.4-snapshot-7"),
            (
                "1.14 Pre-Release 5 / c20937bb9cb248468de8a554f50f7f51",
                "1.14 Pre-Release 5",
            ),
        ] {
            let dir = tempfile::tempdir().expect("temp dir");
            let version_dir = write_version_dir(
                dir.path(),
                "1.20.1-Survival",
                r#"{ "id": "1.20.1-Survival", "type": "release" }"#,
            );
            write_client_jar(
                &version_dir,
                "1.20.1-Survival",
                &serde_json::json!({ "id": recorded }).to_string(),
            );

            let info = detect(&version_dir).expect("detect should succeed");
            assert_eq!(info.vanilla_name, expected);
        }
    }

    #[test]
    fn client_version_outranks_the_client_jar_and_every_name() {
        let dir = tempfile::tempdir().expect("temp dir");
        let version_dir = write_version_dir(
            dir.path(),
            "1.19.4-pack",
            r#"{ "id": "1.18.2-pack", "clientVersion": "1.21.1" }"#,
        );
        write_client_jar(&version_dir, "1.19.4-pack", r#"{ "id": "1.20.1" }"#);

        let info = detect(&version_dir).expect("detect should succeed");
        assert_eq!(info.vanilla_name, "1.21.1");
    }

    #[test]
    fn names_are_used_only_without_any_version_record() {
        let dir = tempfile::tempdir().expect("temp dir");
        for (folder, json, expected) in [
            ("renamed", r#"{ "id": "1.12.2-Survival" }"#, "1.12.2"),
            ("1.16.5-Survival", r#"{ "id": "Survival" }"#, "1.16.5"),
        ] {
            let version_dir = write_version_dir(dir.path(), folder, json);
            let info = detect(&version_dir).expect("detect should succeed");
            assert_eq!(info.vanilla_name, expected, "{folder}");
        }
    }

    #[test]
    fn flattened_fabric_json_reads_the_intermediary_version() {
        let dir = tempfile::tempdir().expect("temp dir");
        let version_dir = write_version_dir(
            dir.path(),
            "Fabric Pack",
            r#"{
                "id": "Fabric Pack",
                "libraries": [
                    { "name": "net.fabricmc:fabric-loader:0.15.11" },
                    { "name": "net.fabricmc:intermediary:24w14a" }
                ]
            }"#,
        );

        let info = detect(&version_dir).expect("detect should succeed");
        assert_eq!(info.vanilla_name, "24w14a");
        assert_eq!(info.loader.as_deref(), Some("fabric"));
    }

    #[test]
    fn test_detect_end_to_end() {
        let dir = tempfile::tempdir().expect("temp dir");
        let instance = dir.path().join(".minecraft");
        std::fs::create_dir(&instance).expect("create .minecraft dir");
        std::fs::write(
            instance.join(".minecraft.json"),
            r#"{
                "id": "1.20.1-fabric-0.15.11",
                "libraries": [
                    {
                        "name": "net.fabricmc:fabric-loader:0.15.11-1.20.1"
                    }
                ]
            }"#,
        )
        .expect("write instance json");

        let info = detect(&instance).expect("detect should succeed");
        assert_eq!(info.vanilla_name, "1.20.1");
        assert_eq!(info.loader.as_deref(), Some("fabric"));
        assert_eq!(info.loader_version.as_deref(), Some("0.15.11"));
    }
}
