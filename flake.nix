{
  description = "Axolotl Launcher: Your last launcher.";

  nixConfig = {
    extra-substituters = [
      "https://axlmc.cachix.org"
    ];
    extra-trusted-public-keys = [
      "axlmc.cachix.org-1:dmsRJqFQqLMfvRveerHV3DxDCW1wq1N+TX1+dryYWZU="
    ];
  };

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    self.submodules = true;
  };

  outputs = inputs: (
    let
      legacyPackages = builtins.mapAttrs (system: pkgs:
        import inputs.nixpkgs {
          inherit system;
          overlays = [ (inputs.rust-overlay.overlays.default) ];
        }
      ) inputs.nixpkgs.legacyPackages;
    in
      {
        devShells = builtins.mapAttrs (system: pkgs: {
          default = pkgs.callPackage ./nix/devShell.nix { inherit inputs; };
        }) legacyPackages;
        packages = builtins.mapAttrs (system: pkgs: rec {
          default = axolotl-launcher;
          axolotl-launcher = pkgs.callPackage ./nix/package.nix { inherit inputs; };
        }) legacyPackages;
        homeModules = import ./nix/home-module.nix { inherit inputs legacyPackages; };
      }
  );
}
