{
  inputs,
  buildNpmPackage,
  ...
}:
let
  pname = "blockbench";
  version = with builtins; (fromJSON (readFile ../third-party/blockbench/package.json)).version;
  src = ../third-party/blockbench;
in
buildNpmPackage {
  inherit pname version src;
  npmDepsHash = "sha256-EYtpxi9sTpn5Xpvf84UGAFkqJS+/p9vHwNUu/Vve4pg=";
  env = {
    ELECTRON_SKIP_BINARY_DOWNLOAD = 1;
  };
  dontNpmBuild = true;
  installPhase = ''
    mkdir -p "$out"
    cp -r node_modules "$out"
  '';
}
