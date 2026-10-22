#!/usr/bin/env bash
# Recompute the `pnpmDeps` fixed-output hash in nix/axolotl.nix from the current
# pnpm lockfile. Run it from the repository root, e.g.
# `nix develop . --command update-pnpm-deps`.
set -euo pipefail

readonly repository_root="$PWD"
readonly target="$repository_root/nix/axolotl.nix"
readonly pnpm_deps_pattern='-axolotl-pnpm-deps\.drv$'

if [[ ! -f "$target" ]]; then
  echo "error: run update-pnpm-deps from the Axolotl repository root" >&2
  exit 1
fi

build_log="$(mktemp)"
trap 'rm -f "$build_log"' EXIT

# Instantiate the launcher once and pick the pnpm dependency derivation out of
# its derivation closure, so only that fixed-output derivation gets built.
wrapper_drv="$(nix eval --raw --accept-flake-config "$repository_root#axolotl-launcher.drvPath")" || {
  echo "error: cannot evaluate ${repository_root}#axolotl-launcher.drvPath" >&2
  exit 1
}

mapfile -t pnpm_drvs < <(nix-store -qR "$wrapper_drv" | grep -E -e "$pnpm_deps_pattern" || true)

if [[ "${#pnpm_drvs[@]}" -ne 1 ]]; then
  echo "error: expected exactly one pnpm dependency derivation in the closure of $wrapper_drv, found ${#pnpm_drvs[@]}" >&2
  exit 1
fi

if nix build --no-link "${pnpm_drvs[0]}^*" >"$build_log" 2>&1; then
  echo "pnpm dependency hash is up to date"
  exit 0
fi

current_hash="$(grep -m1 -oE 'specified: +sha256-[A-Za-z0-9+/=]+' "$build_log" | sed 's/.*specified: *//' || true)"
new_hash="$(grep -m1 -oE 'got: +sha256-[A-Za-z0-9+/=]+' "$build_log" | sed 's/.*got: *//' || true)"

if [[ -z "$current_hash" || -z "$new_hash" ]]; then
  cat "$build_log" >&2
  echo "error: could not determine the current and new pnpm dependency hashes" >&2
  exit 1
fi

# The recorded hash is the content hash of the pnpm store, so the value alone
# identifies the line to rewrite; no surrounding context is needed to locate it.
sed -i "s|$current_hash|$new_hash|" "$target"

if ! grep -qF "hash = \"$new_hash\";" "$target"; then
  echo "error: the pnpm dependency hash in $target was not updated" >&2
  exit 1
fi

echo "pnpm dependency hash updated to $new_hash"
