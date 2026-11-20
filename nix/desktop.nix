{
  inputs,
  makeDesktopItem,
  ...
}:
makeDesktopItem {
  categories = [ "Game" ];
  desktopName = "Axolotl Launcher";
  exec = "axolotl-launcher %f";
  icon = "${../apps/app/icons/icon.png}";
  mimeTypes = [
    "application/x-modrinth-modpack+zip"
    "application/x-axolotl-schematic"
    "x-scheme-handler/axolotl"
  ];
  name = "Axolotl Launcher";
  startupWMClass = "Axolotl Launcher";
  terminal = false;
  type = "Application";
}
