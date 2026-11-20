{
  inputs,
  callPackage,
  lib,
  runCommand,
  symlinkJoin,

  launchEnv ? { },
  ...
}:
let
  enwrap = callPackage ./enwrap.nix { inherit inputs launchEnv; };
  desktop = callPackage ./desktop.nix { inherit inputs; };
  fileTypes = runCommand "axolotl-file-types" { } ''
    install -Dm644 ${../apps/app/linux/file-types.xml} "$out/share/mime/packages/red.ghs.axolotl.xml"
    install -Dm644 ${../packages/assets/file-icons/mrpack.svg} "$out/share/icons/hicolor/scalable/mimetypes/application-x-modrinth-modpack+zip.svg"
    install -Dm644 ${../packages/assets/file-icons/schem.svg} "$out/share/icons/hicolor/scalable/mimetypes/application-x-axolotl-schematic.svg"
  '';
in
symlinkJoin {
  name = "axolotl-launcher";
  paths = [
    enwrap
    desktop
    fileTypes
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
