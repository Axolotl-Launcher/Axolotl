{
  inputs,
  legacyPackages,
  ...
}:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  inherit (lib)
    mkEnableOption
    mkOption
    mkIf
    types
    ;

  callPackage = legacyPackages.${pkgs.stdenv.hostPlatform.system}.callPackage;

  # A document is read as its own version plus a `data` object; a file that is
  # only the object reads as one this build cannot interpret.
  documentFile = data: {
    text = builtins.toJSON {
      schema_version = 0;
      data = data;
    };
  };
in
{
  options.programs.axolotl-launcher = {
    enable = mkEnableOption "Axolotl Launcher";
    launchEnv = mkOption {
      type = with lib.types; attrsOf anything;
      default = { };
      description = ''
        Environment variables or flags to be passed to Axolotl.
      '';
      example = {
        WEBKIT_DISABLE_DMABUF_RENDERER = 1;
      };
    };
    settings = mkOption {
      type = with types; attrsOf (attrsOf anything);
      default = { };
      description = ''
        Settings this deployment answers, one document per key.

        Each key becomes `<name>.json` under `axolotl-launcher` in the
        configuration home, which is where the launcher looks for a deployment's
        documents without being told. That layer sits above the database row and
        below the machine's own document, so a value here is what the launcher
        shows until the user changes that setting themselves.

        Nothing is checked here: a name the launcher has no domain for is a file
        it never reads, and a key its domain does not carry is dropped when that
        domain is deserialized.
      '';
      example = {
        appearance = {
          locale = "zh-CN";
          theme = "dark";
        };
        download.max_concurrent_downloads = 16;
        # A Java runtime the launcher may start the game with, and the one it
        # answers a major version with. `path` is the interpreter itself.
        java = {
          runtimes = [
            {
              path = "${pkgs.jdk21}/bin/java";
              major_version = 21;
              full_version = pkgs.jdk21.version;
              distribution = "Temurin";
            }
          ];
          defaults = [
            {
              major_version = 21;
              path = "${pkgs.jdk21}/bin/java";
            }
          ];
        };
      };
    };
  };

  config =
    with config.programs.axolotl-launcher;
    mkIf enable {
      # The launcher reads a deployment's documents from its own directory under
      # the configuration home, so each file is linked there rather than at the
      # top of the home directory.
      home.file = lib.mapAttrs' (
        name: data:
        lib.nameValuePair "${config.xdg.configHome}/axolotl-launcher/${name}.json" (documentFile data)
      ) settings;

      home.packages = [
        (callPackage ./package.nix {
          inherit inputs launchEnv;
        })
      ];
    };
}
