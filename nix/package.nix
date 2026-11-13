{
  inputs,
  callPackage,
  lib,
  symlinkJoin,

  launchEnv ? { },
  ...
}:
let
  enwrap = callPackage ./enwrap.nix { inherit inputs launchEnv; };
  desktop = callPackage ./desktop.nix { inherit inputs; };
in
symlinkJoin {
  name = "axolotl-launcher";
  paths = [
    enwrap
    desktop
  ];
  meta = {
    description = "Axolotl Launcher: Your last launcher.";
    longDescription = ''
      Axolotl Launcher is a free, open-source, ad-free, cross-platform Minecraft Java Edition launcher for searching, installing, and updating mods, modpacks, resource packs, and shaders from Modrinth and CurseForge, with Axolotl Labs built in.
    '';
    homepage = "https://axlmc.org/";
    license = lib.licenses.gpl3Only;
    platforms = [
      "x86_64-linux"
      "aarch64-linux"
    ];
    mainProgram = "axolotl-launcher";
  };
}
