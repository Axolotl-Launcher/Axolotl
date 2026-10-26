# Shared Minecraft data

Vanilla Minecraft data reused across Lab features, together with the modules that read it:

- `assets/items/` — per-version item manifests (identifiers, readable names, atlas texture keys).
- `assets/tags/` — per-version vanilla item tags.
- `assets/vanilla/` — item name index, potion list and potion item list.
- `assets/texture-atlas.json` / `assets/texture-atlas.png` — item icon atlas and its layout.
- `resources.ts`, `item-names.ts`, `item-images.ts` — atlas and manifest loaders, name resolution and icon slicing.
- `versions.ts`, `types.ts` — shared version primitives, plus the version identifier and pack format types.

These files were moved here from `lab/recipe-generator`, which is only one of their consumers: `lab/seed-map` and `lab/command-generator` use the same data. Data provenance and licensing are recorded in [sources.json](sources.json) and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

New Lab features should import from this directory directly instead of copying these files.
