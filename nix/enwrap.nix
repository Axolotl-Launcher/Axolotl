{
  inputs,
  buildFHSEnv,
  callPackage,
  lib,

  launchEnv,
  ...
}:
let
  axolotl = callPackage ./axolotl.nix { inherit inputs; };
in
buildFHSEnv {
  name = "axolotl-launcher";
  targetPkgs =
    pkgs:
    builtins.concatLists [
      [ axolotl ]
      (with pkgs; [
        # For Axolotl
        libnotify
        gtk3
        webkitgtk_4_1
        libayatana-appindicator
        gdk-pixbuf
        cairo
        glib
        dbus
        libsoup_3
        glib-networking
        cacert
        # For Minecraft
        stdenv.cc.cc.lib
        ## native versions
        glfw3-minecraft
        openal
        ## openal
        alsa-lib
        libjack2
        libpulseaudio
        pipewire
        ## glfw
        libGL
        libx11
        libxcursor
        libxext
        libxrandr
        libxxf86vm
        wayland
        udev # oshi
        vulkan-loader # VulkanMod's lwjglt
        flite
        gamemode
        libusb1
      ])
    ];
  profile = ''
    set -o allexport
    ${lib.toShellVars launchEnv}
    set +o allexport
  '';
  runScript = ''
    "${axolotl}/bin/Axolotl Launcher"
  '';
}
