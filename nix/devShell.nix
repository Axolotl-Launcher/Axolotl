{
  inputs,
  callPackage,
  mkShell,
  mold,
  writeShellScriptBin,
  ...
}:
(
  let
    axolotl = callPackage ./axolotl.nix { inherit inputs; };
    update-gradle-deps = writeShellScriptBin "update-gradle-deps" ''
      ${axolotl.mitmCache.updateScript}
    '';
    update-pnpm-deps = writeShellScriptBin "update-pnpm-deps" (
      builtins.readFile ./update-pnpm-deps.sh
    );
  in
    mkShell {
      packages = [
        # The Linux linker that .cargo/config.toml selects.
        mold
        update-gradle-deps
        update-pnpm-deps
      ];
    }
)
