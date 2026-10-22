{
  inputs,
  callPackage,
  fetchurl,
  fetchPnpmDeps,
  lib,
  makeShellWrapper,
  stdenv,

  cargo-tauri,
  gradle_9,
  jdk17,
  makeRustPlatform,
  nodejs_24,
  pkg-config,
  pnpmConfigHook,
  pnpm_10,
  rust-bin,
  wrapGAppsHook4,

  glib-networking,
  libayatana-appindicator,
  librsvg,
  webkitgtk_4_1,
  ...
}:

let
  pname = "axolotl";
  version = with builtins; (fromJSON (readFile ../apps/app-frontend/package.json)).version;
  src = with lib.fileset; toSource {
    root = ../.;
    fileset = unions [
      ../.cargo
      ../apps
      ../packages
      ../patches
      ../scripts
      ../third-party
      ../Cargo.lock
      ../Cargo.toml
      ../package.json
      ../pnpm-lock.yaml
      ../pnpm-workspace.yaml
      ../rust-toolchain.toml
      ../turbo.jsonc
    ];
  };

  blockbench = callPackage ./blockbench.nix { inherit inputs; };

  gradle_9_j17 = gradle_9.override {
    java = jdk17;
  };
  pnpm_10_33_2 = pnpm_10.overrideAttrs (old: rec {
    version = "10.33.2";
    src = fetchurl {
      url = "https://registry.npmjs.org/pnpm/-/pnpm-${version}.tgz";
      hash = "sha512-qQ+vb+6rca1sblf5Tg/hoS9dzCLNdU20CulZPraj4LaxLjVAIYuzeuCDQEsfLObbKkEh6XmCm0r/lLmfSdoc+A==";
    };
  });
  rustToolchain = rust-bin.fromRustupToolchainFile ../rust-toolchain.toml;
  rustPlatform = makeRustPlatform {
    cargo = rustToolchain;
    rustc = rustToolchain;
  };
in
  rustPlatform.buildRustPackage (finalAttrs: {
    inherit pname version src;
    # Deps prefetch
    cargoLock = {
      lockFile = ../Cargo.lock;
      outputHashes = {
        "tauri-plugin-updater-2.10.1" = "sha256-NiORbFiK91SGrAIfQtUCLwomKO5ZIX+nvW8/8ZODaB4=";
        "tauri-plugin-window-state-2.4.1" = "sha256-WPZ5HvSY5NCwjHXVgovBK4Xvf8Zl4Z8vJinKZvRJPhQ=";
      };
    };
    mitmCache = gradle_9_j17.fetchDeps {
      pkg = finalAttrs.finalPackage;
      data = ./mitmCache.json;
    };
    pnpmDeps = fetchPnpmDeps {
      inherit pname version src;
      pnpm = pnpm_10_33_2;
      fetcherVersion = 4;
      hash = "sha256-OLFtqud0O3et3N37hLHQ+qJhxoJ9qFPAKkOR9Bx40Ds=";
    };
    # Gradle
    gradleFlags = [
      "--no-configuration-cache"
      "-x"
      "spotlessJava"
    ];
    preGradleUpdate = ''
      cd packages/app-lib/java
    '';
    gradleUpdateTask = "nixDownloadDeps authlibInjector";
    # Workflow
    dontUseGradleBuild = true;
    dontUseGradleCheck = true;
    dontCargoCheck = true;
    # Axolotl
    nativeBuildInputs = [
      cargo-tauri.hook
      gradle_9_j17
      jdk17
      makeShellWrapper
      nodejs_24
      pkg-config
      pnpmConfigHook
      pnpm_10_33_2
      wrapGAppsHook4
    ];
    buildInputs = [
      glib-networking
      libayatana-appindicator
      librsvg
      webkitgtk_4_1
    ];
    preBuild = ''
      ln -s '${blockbench}/node_modules' 'third-party/blockbench/node_modules'
      makeShellWrapper '${gradle_9_j17}/bin/gradle' 'packages/app-lib/java/gradlew' \
        --add-flags "''${gradleFlags[*]}"      \
        --add-flags "''${gradleFlagsArray[*]}" \
    '';
    installPhase = ''
      runHook preInstall
      mkdir -p "$out/bin"
      cp "target/${stdenv.hostPlatform.rust.rustcTarget}/release/Axolotl Launcher" "$out/bin"
      runHook postInstall
    '';
  })
