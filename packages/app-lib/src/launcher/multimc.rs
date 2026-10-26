//! MultiMC component profiles retained alongside imported game data.

mod jarmod;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use daedalus::minecraft::{
    Argument, ArgumentType, DownloadType, JavaVersion, Library, VersionInfo,
};
use serde_json::{Value, json};

use super::direct_ensure::safe_maven_relative_path;
use crate::pack::import::mmc::MMCPack;
use crate::state::ModLoader;
use crate::util::io;

pub(crate) const PROFILE_DIR: &str = ".axolotl/multimc";

pub(crate) struct MultiMcProfile {
    root: PathBuf,
    pack: MMCPack,
    patches: Vec<(String, Value)>,
    jar_mods: Vec<jarmod::Artifact>,
    main_jar: Option<jarmod::Artifact>,
}

fn invalid(message: impl Into<String>) -> crate::Error {
    crate::ErrorKind::InputError(message.into()).into()
}

fn loader_uid(loader: ModLoader) -> Option<&'static str> {
    match loader {
        ModLoader::Forge => Some("net.minecraftforge"),
        ModLoader::NeoForge => Some("net.neoforged"),
        ModLoader::Fabric | ModLoader::LegacyFabric => {
            Some("net.fabricmc.fabric-loader")
        }
        ModLoader::Quilt => Some("org.quiltmc.quilt-loader"),
        ModLoader::Cleanroom => Some("com.cleanroommc"),
        _ => None,
    }
}

impl MultiMcProfile {
    pub(crate) async fn load(
        game_dir: &Path,
        game_version: &str,
        loader: ModLoader,
        loader_version: Option<&str>,
    ) -> crate::Result<Option<Self>> {
        if !game_dir.is_dir() {
            return Ok(None);
        }
        let retained = game_dir.join(PROFILE_DIR);
        let root = if retained.join("mmc-pack.json").is_file() {
            retained
        } else {
            // A linked MultiMC instance points at its minecraft directory.
            let game_dir = io::canonicalize(game_dir)?;
            let Some(parent) = game_dir.parent() else {
                return Ok(None);
            };
            if !matches!(
                game_dir.file_name().and_then(|name| name.to_str()),
                Some("minecraft" | ".minecraft")
            ) || !parent.join("mmc-pack.json").is_file()
            {
                return Ok(None);
            }
            parent.to_path_buf()
        };
        let profile = Self::from_root(root).await?;
        if profile.patches.is_empty()
            || !profile.matches(game_version, loader, loader_version)
        {
            return Ok(None);
        }
        Ok(Some(profile))
    }

    pub(crate) async fn from_root(root: PathBuf) -> crate::Result<Self> {
        let pack: MMCPack = serde_json::from_slice(
            &io::read(root.join("mmc-pack.json")).await?,
        )?;
        if pack.format_version != 1 {
            return Err(invalid(
                "Unsupported MultiMC component format version",
            ));
        }
        let mut patches = Vec::new();
        let mut jar_mods = Vec::new();
        let mut main_jar = None;
        let mut seen = HashSet::new();
        // Modern mmc-pack.json explicitly stores component order. The order
        // field inside individual patches belongs to the older file format.
        for component in pack
            .components
            .iter()
            .filter(|component| !component.disabled)
        {
            if !safe_maven_relative_path(&component.uid)
                || component.uid.contains('/')
                || !seen.insert(component.uid.clone())
            {
                return Err(invalid(format!(
                    "Invalid or duplicate MultiMC component: {}",
                    component.uid
                )));
            }
            let path =
                root.join("patches").join(format!("{}.json", component.uid));
            if !path.is_file() {
                // These components are supplied by Axolotl's normal Minecraft
                // and loader metadata. Other components must never disappear.
                if !component.uid.starts_with("net.legacyfabric")
                    && !component.uid.starts_with("com.cleanroommc")
                    && !component.uid.to_ascii_lowercase().contains("optifine")
                    && !component
                        .uid
                        .to_ascii_lowercase()
                        .contains("optifabric")
                    && !component
                        .uid
                        .to_ascii_lowercase()
                        .contains("liteloader")
                    && !(component.uid == "org.lwjgl3"
                        && !root.join("patches/net.minecraft.json").is_file())
                    && !matches!(
                        component.uid.as_str(),
                        "net.minecraft"
                            | "org.lwjgl"
                            | "net.minecraftforge"
                            | "net.neoforged"
                            | "net.fabricmc.fabric-loader"
                            | "net.fabricmc.intermediary"
                            | "org.quiltmc.quilt-loader"
                    )
                {
                    return Err(invalid(format!(
                        "MultiMC component {} is missing its patch",
                        component.uid
                    )));
                }
                continue;
            }
            let patch: Value = serde_json::from_slice(&io::read(&path).await?)?;
            if !patch.is_object()
                || patch
                    .get("formatVersion")
                    .and_then(Value::as_u64)
                    .is_some_and(|version| version != 1)
                || patch
                    .get("uid")
                    .and_then(Value::as_str)
                    .is_some_and(|uid| uid != component.uid)
            {
                return Err(invalid(format!(
                    "Invalid MultiMC patch: {}",
                    component.uid
                )));
            }
            if let Some(mods) = patch.get("mods")
                && !mods
                    .as_array()
                    .ok_or_else(|| invalid("Invalid MultiMC mods list"))?
                    .is_empty()
            {
                return Err(invalid(
                    "MultiMC managed loader mods are not supported",
                ));
            }
            jar_mods.extend(jarmod::parse_jar_mods(&patch)?);
            if let Some(raw) = patch.get("mainJar") {
                main_jar =
                    Some(jarmod::Artifact::parse(raw.clone(), "libraries")?);
            }
            patches.push((component.uid.clone(), patch));
        }
        Ok(Self {
            root,
            pack,
            patches,
            jar_mods,
            main_jar,
        })
    }

    fn matches(
        &self,
        game_version: &str,
        loader: ModLoader,
        loader_version: Option<&str>,
    ) -> bool {
        let active = || {
            self.pack
                .components
                .iter()
                .filter(|component| !component.disabled)
        };
        if !active().any(|component| {
            component.uid == "net.minecraft"
                && component.version.as_deref() == Some(game_version)
        }) {
            return false;
        }
        let primary = active().find(|component| {
            matches!(
                component.uid.as_str(),
                "net.minecraftforge"
                    | "net.neoforged"
                    | "net.fabricmc.fabric-loader"
                    | "org.quiltmc.quilt-loader"
            ) || component.uid.starts_with("com.cleanroommc")
        });
        match (loader_uid(loader), primary) {
            (Some(uid), Some(component)) => {
                component.uid.starts_with(uid)
                    && loader_version.is_none_or(|version| {
                        component.version.as_deref() == Some(version)
                    })
            }
            (None, None) => true,
            _ => false,
        }
    }

    pub(crate) fn metadata_loader(&self, loader: ModLoader) -> ModLoader {
        if loader_uid(loader)
            .is_some_and(|uid| self.patches.iter().any(|(id, _)| id == uid))
        {
            ModLoader::Vanilla
        } else {
            loader
        }
    }

    pub(crate) fn replaces_minecraft(&self) -> bool {
        self.patches.iter().any(|(uid, _)| uid == "net.minecraft")
    }

    pub(crate) fn cache_key(&self, version: &str) -> String {
        // Copying an instance must retain access to its installed client.
        let digest = sha1_smol::Sha1::from(
            serde_json::to_vec(&self.patches)
                .expect("JSON patch values serialize"),
        )
        .hexdigest();
        format!("multimc-{digest}-{version}")
    }

    pub(crate) fn local_loader_versions(
        &self,
    ) -> crate::Result<
        std::collections::HashMap<
            crate::pack::install_from::PackDependency,
            String,
        >,
    > {
        use crate::pack::install_from::PackDependency;
        let dependencies =
            crate::pack::import::mmc::mmc_dependencies(&self.pack)?;
        let mut local = std::collections::HashMap::new();
        for (dependency, loader) in [
            (PackDependency::Forge, ModLoader::Forge),
            (PackDependency::NeoForge, ModLoader::NeoForge),
            (PackDependency::FabricLoader, ModLoader::Fabric),
            (PackDependency::QuiltLoader, ModLoader::Quilt),
            (PackDependency::Cleanroom, ModLoader::Cleanroom),
            (PackDependency::LegacyFabric, ModLoader::LegacyFabric),
        ] {
            if self.metadata_loader(loader) == ModLoader::Vanilla
                && let Some(version) = dependencies.get(&dependency)
            {
                local.insert(dependency, version.clone());
            }
        }
        Ok(local)
    }

    /// Patch only this instance's in-memory profile, never shared version JSONs.
    pub(crate) async fn apply(
        &self,
        info: &mut VersionInfo,
        vanilla: Option<&VersionInfo>,
        libraries_dir: &Path,
    ) -> crate::Result<()> {
        if self.patches.iter().any(|(uid, patch)| {
            uid == "net.minecraft"
                && (patch.get("libraries").is_some()
                    || patch.get("+libraries").is_some())
        }) {
            // Keep standard loader additions when only Minecraft was replaced.
            // When the loader is patched too, the baseline is already vanilla.
            info.libraries.retain(|library| {
                vanilla.is_some_and(|vanilla| {
                    !vanilla
                        .libraries
                        .iter()
                        .any(|base| base.name == library.name)
                })
            });
        }
        let mut extra_jvm = Vec::new();
        if !self.jar_mods.is_empty() {
            extra_jvm.extend([
                "-Dfml.ignoreInvalidMinecraftCertificates=true".to_string(),
                "-Dfml.ignorePatchDiscrepancies=true".to_string(),
            ]);
        }
        let mut tweakers = Vec::new();
        let mut traits = HashSet::new();
        for (uid, patch) in &self.patches {
            traits.extend(strings(patch, "+traits")?);
            if let Some(main_class) =
                patch.get("mainClass").and_then(Value::as_str)
            {
                // A stock loader's main class takes precedence over Minecraft.
                if uid != "net.minecraft" || vanilla.is_none() {
                    info.main_class = main_class.to_string();
                }
            }
            if let Some(arguments) =
                patch.get("minecraftArguments").and_then(Value::as_str)
                && (uid != "net.minecraft" || vanilla.is_none())
            {
                info.minecraft_arguments = Some(arguments.to_string());
                if let Some(arguments) = info.arguments.as_mut() {
                    arguments.remove(&ArgumentType::Game);
                }
            }
            if uid == "net.minecraft" {
                if let Some(index) = patch.get("assetIndex") {
                    info.asset_index = serde_json::from_value(index.clone())?;
                    info.assets = info.asset_index.id.clone();
                }
                if let Some(artifact) =
                    patch.pointer("/mainJar/downloads/artifact")
                    && patch
                        .pointer("/mainJar/MMC-hint")
                        .and_then(Value::as_str)
                        != Some("local")
                    && artifact
                        .get("url")
                        .and_then(Value::as_str)
                        .is_some_and(|url| !url.is_empty())
                    && artifact
                        .get("sha1")
                        .and_then(Value::as_str)
                        .is_some_and(|hash| !hash.is_empty())
                    && artifact
                        .get("size")
                        .and_then(Value::as_u64)
                        .is_some_and(|size| size > 0)
                {
                    // The standard client downloader requires complete
                    // integrity metadata. Other main JARs are resolved by
                    // prepare_runtime with optional hashes and sizes.
                    info.downloads.insert(
                        DownloadType::Client,
                        serde_json::from_value(artifact.clone())?,
                    );
                }
            }
            if let Some(java) = patch.get("javaVersion") {
                info.java_version = Some(serde_json::from_value(java.clone())?);
            } else if let Some(majors) =
                patch.get("compatibleJavaMajors").and_then(Value::as_array)
            {
                let major = majors
                    .iter()
                    .filter_map(Value::as_u64)
                    .filter(|major| *major > 0 && *major <= u32::MAX as u64)
                    .min()
                    .ok_or_else(|| {
                        invalid("MultiMC patch has no compatible Java versions")
                    })?;
                info.java_version = Some(JavaVersion {
                    component: "multimc".to_string(),
                    major_version: major as u32,
                });
            }
            extra_jvm.extend(strings(patch, "+jvmArgs")?);
            tweakers.extend(strings(patch, "+tweakers")?);
            if let Some(arguments) = patch.get("arguments") {
                let arguments: std::collections::HashMap<
                    ArgumentType,
                    Vec<Argument>,
                > = serde_json::from_value(arguments.clone())?;
                for (kind, values) in arguments {
                    let target =
                        info.arguments.get_or_insert_with(Default::default);
                    if kind == ArgumentType::Game {
                        info.minecraft_arguments = None;
                    }
                    target.entry(kind).or_default().extend(values);
                }
            }
            for key in ["libraries", "+libraries"] {
                if let Some(libraries) = patch.get(key) {
                    for raw in libraries.as_array().ok_or_else(|| {
                        invalid("Invalid MultiMC libraries list")
                    })? {
                        let library =
                            self.library(raw.clone(), libraries_dir).await?;
                        // MultiMC replaces matching Maven modules in place.
                        let identity = library_identity(&library.name);
                        if let Some(existing) =
                            info.libraries.iter_mut().find(|existing| {
                                library_identity(&existing.name) == identity
                                    && serde_json::to_value(&existing.rules)
                                        .ok()
                                        == serde_json::to_value(&library.rules)
                                            .ok()
                            })
                        {
                            *existing = library;
                        } else {
                            info.libraries.push(library);
                        }
                    }
                }
            }
        }
        // MultiMC's legacy no-applet launch invokes Minecraft.main directly.
        // Keeping Mojang's LaunchWrapper here would select its applet tweaker
        // even for replacement clients such as the BTA installer.
        if traits.contains("legacyLaunch")
            && traits.contains("noapplet")
            && !self
                .patches
                .iter()
                .any(|(_, patch)| patch.get("mainClass").is_some())
            && info.main_class == "net.minecraft.launchwrapper.Launch"
        {
            info.main_class = "net.minecraft.client.Minecraft".to_string();
        }
        if !extra_jvm.is_empty() {
            let arguments = info.arguments.get_or_insert_with(Default::default);
            // Old Minecraft metadata has no JVM argument list. Supplying one
            // disables the launcher's defaults, so retain the required setup.
            let jvm = arguments.entry(ArgumentType::Jvm).or_insert_with(|| {
                [
                    "-Djava.library.path=${natives_directory}",
                    "-cp",
                    "${classpath}",
                ]
                .into_iter()
                .map(|arg| Argument::Normal(arg.to_string()))
                .collect()
            });
            jvm.extend(extra_jvm.into_iter().map(Argument::Normal));
        }
        if !tweakers.is_empty() {
            if let Some(arguments) = info
                .arguments
                .as_mut()
                .and_then(|arguments| arguments.get_mut(&ArgumentType::Game))
            {
                for tweaker in tweakers {
                    arguments.extend([
                        Argument::Normal("--tweakClass".to_string()),
                        Argument::Normal(tweaker),
                    ]);
                }
            } else {
                let arguments =
                    info.minecraft_arguments.get_or_insert_with(String::new);
                for tweaker in tweakers {
                    arguments.push_str(&format!(
                        " --tweakClass {}",
                        shlex::try_quote(&tweaker)
                            .map_err(|_| invalid("Invalid MultiMC tweaker"))?
                    ));
                }
            }
        }
        info.id = self.cache_key(&info.id);
        Ok(())
    }

    async fn library(
        &self,
        mut raw: Value,
        libraries_dir: &Path,
    ) -> crate::Result<Library> {
        // MultiMC permits omitted checksums/sizes; daedalus expects these keys.
        if let Some(downloads) =
            raw.get_mut("downloads").and_then(Value::as_object_mut)
        {
            for (key, value) in downloads {
                if key == "artifact" {
                    normalize_download(value);
                } else if key == "classifiers"
                    && let Some(classifiers) = value.as_object_mut()
                {
                    for download in classifiers.values_mut() {
                        normalize_download(download);
                    }
                }
            }
        }
        let mut library: Library = serde_json::from_value(raw.clone())?;
        let relative = daedalus::get_path_from_artifact(&library.name)?;
        if !safe_maven_relative_path(&relative) {
            return Err(invalid("Invalid MultiMC library coordinate"));
        }
        if raw.get("MMC-hint").and_then(Value::as_str) == Some("local") {
            let filename = raw
                .get("MMC-filename")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| {
                    Path::new(&relative)
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .to_string()
                });
            if !safe_maven_relative_path(&filename) {
                return Err(invalid("Invalid MultiMC local library filename"));
            }
            let bytes =
                io::read(self.root.join("libraries").join(filename)).await?;
            let hash = sha1_smol::Sha1::from(&bytes).hexdigest();
            // Namespace bundled JARs by content so importing a patch cannot
            // overwrite another instance's library with the same coordinate.
            library.name = format!("com.axolotl.multimc:local-{hash}:1");
            library.downloadable = false;
            library.downloads = None;
            let destination = libraries_dir
                .join(daedalus::get_path_from_artifact(&library.name)?);
            if io::read(&destination).await.ok().as_deref()
                != Some(bytes.as_slice())
            {
                let lock =
                    crate::util::fetch::destination_download_lock(&destination);
                let _guard = lock.lock().await;
                io::create_dir_all(destination.parent().unwrap()).await?;
                let temporary =
                    crate::util::fetch::suffixed_path(&destination, ".part");
                io::write(&temporary, bytes).await?;
                crate::util::fetch::finalize_download(&temporary, &destination)
                    .await?;
            }
        }
        Ok(library)
    }
}

fn normalize_download(value: &mut Value) {
    if let Some(download) = value.as_object_mut() {
        download.entry("sha1").or_insert(json!(""));
        download.entry("size").or_insert(json!(0));
        download.entry("url").or_insert(json!(""));
    }
}

fn library_identity(name: &str) -> (String, String, String) {
    let mut parts = name.split(':');
    let group = parts.next().unwrap_or_default().to_string();
    let artifact = parts.next().unwrap_or_default().to_string();
    let _version = parts.next();
    (
        group,
        artifact,
        parts.next().unwrap_or_default().to_string(),
    )
}

fn strings(patch: &Value, key: &str) -> crate::Result<Vec<String>> {
    patch
        .get(key)
        .map(|value| {
            serde_json::from_value(value.clone()).map_err(crate::Error::from)
        })
        .transpose()
        .map(Option::unwrap_or_default)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn baseline() -> VersionInfo {
        serde_json::from_value(json!({
            "id": "1.7.10", "assets": "1.7.10",
            "assetIndex": {"id": "1.7.10", "sha1": "", "size": 0, "totalSize": 0, "url": ""},
            "downloads": {}, "libraries": [{"name": "org.lwjgl.lwjgl:lwjgl:2.9.1"}],
            "mainClass": "net.minecraft.client.main.Main",
            "minecraftArguments": "--username ${auth_player_name}",
            "minimumLauncherVersion": 0, "releaseTime": "2014-05-14T17:29:23Z",
            "time": "2014-05-14T17:29:23Z", "type": "release"
        })).unwrap()
    }

    pub(super) fn write_profile(
        root: &Path,
        components: Value,
        patches: &[(&str, Value)],
    ) {
        std::fs::create_dir_all(root.join("patches")).unwrap();
        std::fs::write(
            root.join("mmc-pack.json"),
            serde_json::to_vec(
                &json!({"formatVersion":1, "components":components}),
            )
            .unwrap(),
        )
        .unwrap();
        for (uid, patch) in patches {
            std::fs::write(
                root.join("patches").join(format!("{uid}.json")),
                serde_json::to_vec(patch).unwrap(),
            )
            .unwrap();
        }
    }

    fn gtnh_profile(root: &Path) {
        write_profile(
            root,
            json!([
                {"uid":"org.lwjgl3", "version":"3.3.3"},
                {"uid":"net.minecraft", "version":"1.7.10"},
                {"uid":"early.classpath"},
                {"uid":"net.minecraftforge", "version":"10.13.4.1614"},
                {"uid":"launchargs"},
                {"uid":"disabled", "disabled":true}
            ]),
            &[
                (
                    "org.lwjgl3",
                    json!({"order":-1, "libraries":[{"name":"org.lwjgl:lwjgl:3.3.3", "downloads":{"artifact":{"url":"https://example.com/lwjgl.jar"}}}]}),
                ),
                (
                    "net.minecraft",
                    json!({"order":-2, "mainClass":"net.minecraft.client.main.Main", "minecraftArguments":"--username ${auth_player_name}", "compatibleJavaMajors":[17,21,25], "libraries":[{"name":"com.google.guava:guava:15.0"}]}),
                ),
                (
                    "early.classpath",
                    json!({"+jvmArgs":["-Djava.system.class.loader=com.gtnewhorizons.retrofuturabootstrap.RfbSystemClassLoader", "--add-opens", "java.base/java.lang=ALL-UNNAMED", "--add-opens", "java.base/java.nio=ALL-UNNAMED"], "libraries":[{"name":"com.github.GTNewHorizons:lwjgl3ify:2.1.16:forgePatches", "MMC-hint":"local"}]}),
                ),
                (
                    "net.minecraftforge",
                    json!({"mainClass":"com.gtnewhorizons.retrofuturabootstrap.Main", "+tweakers":["cpw.mods.fml.common.launcher.FMLTweaker"], "libraries":[{"name":"com.google.guava:guava:17.0"}]}),
                ),
                (
                    "launchargs",
                    json!({"mainClass":"com.gtnewhorizons.retrofuturabootstrap.Main"}),
                ),
                ("disabled", json!({"mainClass":"wrong.Main"})),
            ],
        );
        std::fs::create_dir_all(root.join("libraries")).unwrap();
        std::fs::write(
            root.join("libraries/lwjgl3ify-2.1.16-forgePatches.jar"),
            b"bundled-patch",
        )
        .unwrap();
    }

    #[tokio::test]
    async fn multimc_lwjgl_platform_rules_preserve_supported_and_foreign_libraries()
     {
        let directory = tempfile::tempdir().unwrap();
        let platforms = [
            "freebsd",
            "linux-arm32",
            "linux-arm64",
            "linux-ppc64le",
            "linux",
            "osx",
            "osx-arm64",
            "windows",
            "windows-arm64",
        ];
        let libraries: Vec<_> = platforms.iter().map(|platform| json!({
            "name": format!("org.lwjgl:lwjgl-natives-{platform}:3.4.3"),
            "rules": [{"action": "allow", "os": {"name": platform}}],
            "downloads": {"artifact": {"url": format!("https://example.com/natives-{platform}.jar")}}
        })).collect();
        write_profile(
            directory.path(),
            json!([
                {"uid":"org.lwjgl3", "version":"3.4.3"},
                {"uid":"net.minecraft", "version":"1.7.10"}
            ]),
            &[("org.lwjgl3", json!({"libraries": libraries}))],
        );
        let profile = MultiMcProfile::from_root(directory.path().to_path_buf())
            .await
            .unwrap();
        let mut info = baseline();
        profile
            .apply(&mut info, None, &directory.path().join("cache"))
            .await
            .unwrap();
        let restored: VersionInfo =
            serde_json::from_value(serde_json::to_value(&info).unwrap())
                .unwrap();
        assert_eq!(restored.libraries.len(), platforms.len() + 1);
        for library in restored
            .libraries
            .iter()
            .filter(|library| library.rules.is_some())
        {
            let platform = library.rules.as_ref().unwrap()[0]
                .os
                .as_ref()
                .unwrap()
                .name
                .as_ref()
                .unwrap();
            let native =
                daedalus::minecraft::Os::native_arch(std::env::consts::ARCH);
            let expected = *platform == native || *platform == native.get_os();
            assert_eq!(
                super::super::parse_rules(
                    library.rules.as_ref().unwrap(),
                    std::env::consts::ARCH,
                    &super::super::QuickPlayType::None,
                    true
                ),
                expected
            );
        }
    }

    /// Run with AXOLOTL_TEST_MULTIMC_ARCHIVE pointing to a local MultiMC export.
    #[tokio::test]
    #[ignore = "requires an external local MultiMC archive"]
    async fn local_multimc_archive_profile_applies() {
        let path = PathBuf::from(
            std::env::var_os("AXOLOTL_TEST_MULTIMC_ARCHIVE")
                .expect("set AXOLOTL_TEST_MULTIMC_ARCHIVE"),
        );
        let detected =
            crate::api::pack::detect::detect_local_pack_sync(&path).unwrap();
        assert_eq!(
            detected.format,
            crate::api::pack::detect::LocalPackFormat::MmcExport
        );
        let directory = tempfile::tempdir().unwrap();
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).unwrap();
            let name = crate::api::pack::detect::decode_zip_entry_name(
                entry.name_raw(),
            );
            let Some(relative) = name.strip_prefix(&detected.base_folder)
            else {
                continue;
            };
            if entry.is_dir()
                || !(relative == "mmc-pack.json"
                    || relative.starts_with("patches/")
                        && relative.ends_with(".json")
                    || relative.starts_with("libraries/")
                        && relative.ends_with(".jar"))
            {
                continue;
            }
            assert!(safe_maven_relative_path(relative));
            let destination = directory.path().join(relative);
            std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
            std::io::copy(
                &mut entry,
                &mut std::fs::File::create(destination).unwrap(),
            )
            .unwrap();
        }
        let profile = MultiMcProfile::from_root(directory.path().to_path_buf())
            .await
            .unwrap();
        let mut info = baseline();
        profile
            .apply(&mut info, None, &directory.path().join("cache"))
            .await
            .unwrap();
        let restored: VersionInfo =
            serde_json::from_value(serde_json::to_value(&info).unwrap())
                .unwrap();
        assert!(!restored.libraries.is_empty());
        let selected: Vec<_> = restored
            .libraries
            .iter()
            .filter(|library| {
                library.rules.as_ref().is_none_or(|rules| {
                    super::super::parse_rules(
                        rules,
                        std::env::consts::ARCH,
                        &super::super::QuickPlayType::None,
                        true,
                    )
                })
            })
            .collect();
        assert!(!selected.is_empty());
        eprintln!(
            "Applied local MultiMC profile: {} libraries, {} selected for this platform, main class {}",
            restored.libraries.len(),
            selected.len(),
            restored.main_class
        );
    }

    #[tokio::test]
    async fn multimc_gtnh_bootstrap_libraries_and_arguments_are_preserved() {
        let directory = tempfile::tempdir().unwrap();
        gtnh_profile(directory.path());
        let profile = MultiMcProfile::from_root(directory.path().to_path_buf())
            .await
            .unwrap();
        assert_eq!(
            profile.metadata_loader(ModLoader::Forge),
            ModLoader::Vanilla
        );
        assert_eq!(
            profile
                .local_loader_versions()
                .unwrap()
                .get(&crate::pack::install_from::PackDependency::Forge)
                .map(String::as_str),
            Some("10.13.4.1614")
        );
        let cache = directory.path().join("cache");
        let mut info = baseline();
        profile.apply(&mut info, None, &cache).await.unwrap();
        assert_eq!(
            info.main_class,
            "com.gtnewhorizons.retrofuturabootstrap.Main"
        );
        assert_eq!(info.java_version.as_ref().unwrap().major_version, 17);
        assert!(
            info.minecraft_arguments.as_ref().unwrap().contains(
                "--tweakClass cpw.mods.fml.common.launcher.FMLTweaker"
            )
        );
        assert!(
            info.libraries
                .iter()
                .any(|library| library.name == "org.lwjgl:lwjgl:3.3.3")
        );
        assert!(
            !info
                .libraries
                .iter()
                .any(|library| library.name.contains("org.lwjgl.lwjgl")
                    || library.name.ends_with("guava:15.0"))
        );
        let local = info
            .libraries
            .iter()
            .find(|library| !library.downloadable)
            .unwrap();
        assert_eq!(
            std::fs::read(
                cache.join(
                    daedalus::get_path_from_artifact(&local.name).unwrap()
                )
            )
            .unwrap(),
            b"bundled-patch"
        );
        let arguments = serde_json::to_value(
            info.arguments
                .as_ref()
                .unwrap()
                .get(&ArgumentType::Jvm)
                .unwrap(),
        )
        .unwrap();
        let arguments = arguments.as_array().unwrap();
        assert_eq!(
            arguments
                .iter()
                .filter(|argument| *argument == "--add-opens")
                .count(),
            2
        );
        assert!(arguments.contains(&json!("-cp")));
        assert!(arguments.contains(&json!("${classpath}")));
        assert_eq!(info.id, profile.cache_key("1.7.10"));
    }

    #[tokio::test]
    async fn multimc_retained_profile_survives_source_removal_and_version_changes()
     {
        let directory = tempfile::tempdir().unwrap();
        let game_dir = directory.path().join("game");
        gtnh_profile(&game_dir.join(PROFILE_DIR));
        assert!(
            MultiMcProfile::load(
                &game_dir,
                "1.7.10",
                ModLoader::Forge,
                Some("10.13.4.1614")
            )
            .await
            .unwrap()
            .is_some()
        );
        assert!(
            MultiMcProfile::load(
                &game_dir,
                "1.20.1",
                ModLoader::Forge,
                Some("47.0.0")
            )
            .await
            .unwrap()
            .is_none()
        );
        assert!(
            MultiMcProfile::load(&game_dir, "1.7.10", ModLoader::Vanilla, None)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            MultiMcProfile::load(
                &game_dir,
                "1.7.10",
                ModLoader::Forge,
                Some("10.13.4.1558")
            )
            .await
            .unwrap()
            .is_none()
        );
        let linked_root = directory.path().join("linked");
        gtnh_profile(&linked_root);
        std::fs::create_dir(linked_root.join(".minecraft")).unwrap();
        assert!(
            MultiMcProfile::load(
                &linked_root.join(".minecraft"),
                "1.7.10",
                ModLoader::Forge,
                None
            )
            .await
            .unwrap()
            .is_some()
        );
    }

    #[tokio::test]
    async fn multimc_standard_export_uses_normal_runtime_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let retained = directory.path().join(PROFILE_DIR);
        write_profile(
            &retained,
            json!([{"uid":"net.minecraft", "version":"1.21.1"}, {"uid":"org.lwjgl3", "version":"3.3.3"}, {"uid":"net.fabricmc.fabric-loader", "version":"0.16.0"}]),
            &[],
        );
        assert!(
            MultiMcProfile::load(
                directory.path(),
                "1.21.1",
                ModLoader::Fabric,
                Some("0.16.0")
            )
            .await
            .unwrap()
            .is_none()
        );
    }

    #[tokio::test]
    async fn multimc_rejects_missing_components_and_local_path_traversal() {
        let directory = tempfile::tempdir().unwrap();
        write_profile(directory.path(), json!([{"uid":"missing"}]), &[]);
        assert!(
            MultiMcProfile::from_root(directory.path().to_path_buf())
                .await
                .is_err()
        );
        write_profile(directory.path(), json!([{"uid":"../outside"}]), &[]);
        assert!(
            MultiMcProfile::from_root(directory.path().to_path_buf())
                .await
                .is_err()
        );
        gtnh_profile(directory.path());
        let profile = MultiMcProfile::from_root(directory.path().to_path_buf())
            .await
            .unwrap();
        for filename in [
            "../outside.jar",
            "C:/outside.jar",
            "\\\\server\\outside.jar",
        ] {
            assert!(profile.library(json!({"name":"example:lib:1", "MMC-hint":"local", "MMC-filename":filename}), &directory.path().join("cache")).await.is_err());
        }
        std::fs::remove_file(
            directory
                .path()
                .join("libraries/lwjgl3ify-2.1.16-forgePatches.jar"),
        )
        .unwrap();
        assert!(
            profile
                .apply(&mut baseline(), None, &directory.path().join("cache"))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn multimc_minecraft_patch_keeps_standard_loader_and_platform_variants()
     {
        let directory = tempfile::tempdir().unwrap();
        write_profile(
            directory.path(),
            json!([{"uid":"net.minecraft", "version":"1.7.10"}, {"uid":"net.minecraftforge", "version":"10.13.4.1614"}]),
            &[(
                "net.minecraft",
                json!({"mainClass":"vanilla.Main", "minecraftArguments":"vanilla", "libraries":[
                    {"name":"example:platform:1", "rules":[{"action":"allow", "os":{"name":"windows"}}]},
                    {"name":"example:platform:2", "rules":[{"action":"allow", "os":{"name":"osx"}}]}
                ]}),
            )],
        );
        let profile = MultiMcProfile::from_root(directory.path().to_path_buf())
            .await
            .unwrap();
        assert_eq!(profile.metadata_loader(ModLoader::Forge), ModLoader::Forge);
        let vanilla = baseline();
        let mut info = baseline();
        info.main_class = "forge.Main".to_string();
        info.minecraft_arguments =
            Some("--tweakClass forge.Tweaker".to_string());
        info.libraries.push(
            serde_json::from_value(
                json!({"name":"net.minecraftforge:forge:1.7.10"}),
            )
            .unwrap(),
        );
        profile
            .apply(&mut info, Some(&vanilla), directory.path())
            .await
            .unwrap();
        assert_eq!(info.main_class, "forge.Main");
        assert_eq!(
            info.minecraft_arguments.as_deref(),
            Some("--tweakClass forge.Tweaker")
        );
        assert_eq!(info.libraries.len(), 3);
        assert!(
            info.libraries
                .iter()
                .any(|library| library.name.starts_with("net.minecraftforge:"))
        );
    }

    #[tokio::test]
    #[ignore = "Set AXOLOTL_TEST_MULTIMC_ARCHIVE to a local GTNH MultiMC export"]
    async fn multimc_real_gtnh_archive() {
        use std::io::Read;
        let archive_path = std::env::var_os("AXOLOTL_TEST_MULTIMC_ARCHIVE")
            .expect("local archive path");
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(archive_path).unwrap())
                .unwrap();
        let manifest = archive
            .file_names()
            .find(|name| {
                name.ends_with("/mmc-pack.json") || *name == "mmc-pack.json"
            })
            .unwrap()
            .to_string();
        let prefix = manifest.strip_suffix("mmc-pack.json").unwrap();
        let directory = tempfile::tempdir().unwrap();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).unwrap();
            let Some(relative) =
                entry.name().strip_prefix(prefix).map(str::to_string)
            else {
                continue;
            };
            if entry.is_dir()
                || !(relative == "mmc-pack.json"
                    || relative.starts_with("patches/")
                    || relative.starts_with("libraries/"))
            {
                continue;
            }
            assert!(safe_maven_relative_path(&relative));
            let path = directory.path().join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
        let profile = MultiMcProfile::from_root(directory.path().to_path_buf())
            .await
            .unwrap();
        assert_eq!(
            profile.metadata_loader(ModLoader::Forge),
            ModLoader::Vanilla
        );
        let mut info = baseline();
        let cache = directory.path().join("cache");
        profile.apply(&mut info, None, &cache).await.unwrap();
        assert_eq!(
            info.main_class,
            "com.gtnewhorizons.retrofuturabootstrap.Main"
        );
        assert_eq!(info.java_version.as_ref().unwrap().major_version, 17);
        assert!(!info.libraries.iter().any(|library| {
            library.name.starts_with("net.minecraft:launchwrapper:")
                || library.name.starts_with("org.lwjgl.lwjgl:")
        }));
        let local = info
            .libraries
            .iter()
            .find(|library| !library.downloadable)
            .unwrap();
        let local_path =
            cache.join(daedalus::get_path_from_artifact(&local.name).unwrap());
        let mut jar =
            zip::ZipArchive::new(std::fs::File::open(&local_path).unwrap())
                .unwrap();
        assert!(jar.by_name("com/gtnewhorizons/retrofuturabootstrap/RfbSystemClassLoader.class").is_ok());
        assert!(
            jar.by_name("com/gtnewhorizons/retrofuturabootstrap/Main.class")
                .is_ok()
        );
        let jvm = serde_json::to_value(
            &info.arguments.as_ref().unwrap()[&ArgumentType::Jvm],
        )
        .unwrap();
        assert!(jvm.as_array().unwrap().contains(&json!("-Djava.system.class.loader=com.gtnewhorizons.retrofuturabootstrap.RfbSystemClassLoader")));
        assert!(
            jvm.as_array()
                .unwrap()
                .iter()
                .filter(|argument| *argument == "--add-opens")
                .count()
                > 20
        );
        if let Some(java_home) = std::env::var_os("AXOLOTL_TEST_JAVA_HOME") {
            let java_home = PathBuf::from(java_home);
            let source = directory.path().join("MultiMcBootstrapProbe.java");
            std::fs::write(&source, r#"public class MultiMcBootstrapProbe {
                public static void main(String[] args) throws Exception {
                    ClassLoader loader = ClassLoader.getSystemClassLoader();
                    if (!loader.getClass().getName().equals("com.gtnewhorizons.retrofuturabootstrap.RfbSystemClassLoader")) {
                        throw new AssertionError(loader.getClass().getName());
                    }
                    if (!(loader instanceof java.net.URLClassLoader)) throw new AssertionError("URLClassLoader required");
                    Class.forName("com.gtnewhorizons.retrofuturabootstrap.Main", false, loader);
                    System.out.println("GTNH bootstrap and URLClassLoader verified on Java " + System.getProperty("java.version"));
                }
            }"#).unwrap();
            let compiler = java_home.join("bin").join(if cfg!(windows) {
                "javac.exe"
            } else {
                "javac"
            });
            let compiled = std::process::Command::new(compiler)
                .arg(&source)
                .output()
                .unwrap();
            assert!(
                compiled.status.success(),
                "{}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            let java = java_home.join("bin").join(if cfg!(windows) {
                "java.exe"
            } else {
                "java"
            });
            let classpath =
                std::env::join_paths([directory.path(), local_path.as_path()])
                    .unwrap();
            let mut command = std::process::Command::new(java);
            command
                .current_dir(directory.path())
                .arg("-cp")
                .arg(classpath);
            for argument in info.arguments.as_ref().unwrap()[&ArgumentType::Jvm]
                .iter()
                .skip(3)
            {
                let Argument::Normal(argument) = argument else {
                    panic!("unconditional GTNH JVM argument")
                };
                command.arg(argument);
            }
            let result = command.arg("MultiMcBootstrapProbe").output().unwrap();
            assert!(
                result.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            println!("{}", String::from_utf8_lossy(&result.stdout));
        }
        println!(
            "GTNH profile resolved: {} libraries, main class {}, Java {}",
            info.libraries.len(),
            info.main_class,
            info.java_version.unwrap().major_version
        );
    }
}
