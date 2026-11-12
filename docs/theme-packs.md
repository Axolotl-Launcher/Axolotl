# Theme packs

Theme packs are locally installed archives that change the launcher's colors,
fonts, background, and can contribute pages to the launcher navigation. They
are shared as ordinary zip files; there is no store or signature.

## Archive layout

```
my-theme.zip
├── manifest.json
├── styles/            # optional CSS entered through manifest.styles
├── assets/            # background, UI font, page icons
└── pages/             # optional HTML pages referenced by manifest.pages
```

`manifest.json` is required at the archive root. Paths in the manifest are
relative, forward-slash separated, and must stay inside their declared
directory (`styles/`, `assets/`, `pages/`). Absolute paths, `..`, Windows
device names, and symlinks are rejected.

## Manifest

```json
{
  "schemaVersion": 1,
  "id": "example-theme",
  "name": { "en-US": "Example theme", "zh-CN": "示例主题" },
  "version": "1.0.0",
  "description": { "en-US": "A short description" },
  "engines": { "axolotl": ">=1.11.0" },
  "capabilities": ["tokens", "styles", "pages"],
  "tokens": {
    "light": { "--surface-1": "#ebebeb", "--surface-3": "#ffffff", "--color-text-primary": "#1a202c" },
    "dark": { "--surface-1": "#101215", "--surface-3": "#262b34", "--color-text-primary": "#ffffff" }
  },
  "styles": ["styles/theme.css"],
  "background": "assets/background.png",
  "fonts": { "ui": "assets/Inter.woff2" },
  "permissions": ["storage", "instances.read", "instances.logs", "network"],
  "network": { "allow": ["api.example.com", "*.example.net"] },
  "pages": [
    {
      "id": "home",
      "title": { "en-US": "Example" },
      "icon": "assets/icon.png",
      "entry": "pages/home.html",
      "order": 10,
      "requires": ["network"]
    }
  ]
}
```

- `id` uses `[a-z0-9._-]`; the `axolotl-` prefix is reserved.
- `engines.axolotl` is a semver requirement checked against the running
  launcher version.
- `tokens` must contain at least `--surface-1`, `--surface-3`, and
  `--color-text-primary` for both modes. Values cannot contain `url()`,
  `var()`, `@import`, braces, semicolons, or backslashes.
- `capabilities` gates which parts of the manifest are used: `tokens`,
  `styles`, `pages`.
- `permissions` gates what a page's bridge can request. A page may only list
  permissions the pack already declared.
- `network.allow` accepts exact hosts and `*.` subdomain wildcards. Bare `*`,
  non-HTTPS URLs, ports other than 443, and private/loopback addresses are
  rejected.
- CSS may reference local pack files, `data:` URLs, and `axolotl-theme:`
  assets. Remote URLs and `@import` are rejected.

## Permissions and the page bridge

Pack pages are loaded in an iframe with `sandbox="allow-scripts"` and a
unique opaque origin, and the response CSP forbids all direct network access.
A page talks to the launcher with `postMessage`:

```js
window.parent.postMessage(
  { type: 'axolotl-theme-request', id: crypto.randomUUID(), method: 'storage.read', params: { key: 'state' } },
  '*',
)
```

The launcher replies with
`{ type: 'axolotl-theme-response', id, ok, data | error }`. Available methods:

| Method | Permission | Notes |
| --- | --- | --- |
| `network.fetch` | `network` | HTTPS only, allow-listed host, GET/POST; request 1 MiB, response 8 MiB, 30 s timeout, 4 concurrent and 60 requests/minute per pack |
| `storage.list` / `read` / `write` / `delete` | `storage` | Per-pack directory, 256 files, 2 MiB per file |
| `instances.list` | `instances.read` | Read-only instance summaries |
| `instances.mods` | `instances.read` | Read-only mod list |
| `instances.logs` | `instances.logs` | Censored logs, last 256 KiB |

Tauri IPC is not available inside pack pages, and the launcher only answers
messages whose source is the page's own iframe.

## Limits

| Limit | Default |
| --- | --- |
| Archive size | 500 MiB |
| Extracted size | 500 MiB |
| Entries | 4,096 |

Override with `AXOLOTL_THEME_MAX_ARCHIVE_MB`,
`AXOLOTL_THEME_MAX_UNCOMPRESSED_MB` (both in megabytes), and
`AXOLOTL_THEME_MAX_ENTRIES`.

## Installing and exporting

Settings → Appearance → Theme suite manages installed packs. Importing
validates and extracts to a staging directory before swapping it into place.
Removing a pack leaves its data directory unless "also delete pack data" is
chosen. "Export snapshot" produces a zip of the current appearance: a copy of
the active pack without its data directory, or a token-only pack for built-in
and custom colors.
