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

- `id` uses `[a-z0-9._-]`, starts with a letter or digit, does not end with `.`, is not a Windows device name, and the `axolotl-` prefix is reserved.
- `engines.axolotl` is a semver requirement checked against the running
  launcher version.
- `tokens` must contain at least `--surface-1`, `--surface-3`, and
  `--color-text-primary` for both modes. Values cannot contain `url()`,
  `var()`, `@import`, braces, semicolons, or backslashes.
- `capabilities` gates which parts of the manifest are used: `tokens`,
  `styles`, `pages`. A pack that declares tokens or stylesheets must list the
  matching capability, and `background`/`fonts.ui` require `tokens` or
  `styles`; otherwise installation is rejected.
- `permissions` gates what a page's bridge can request. A page may only list
  permissions the pack already declared.
- `network.allow` accepts exact hosts and `*.` subdomain wildcards. Bare `*`,
  non-HTTPS URLs, ports other than 443, and private/loopback addresses are
  rejected.
- CSS may reference local pack files, `data:` URLs, and `axolotl-theme:`
  assets. Remote URLs and `@import` are rejected.

## Permissions and the page bridge

Pack pages are hosted in a dedicated child webview labeled
`theme-page-*`. That webview has its own Tauri capability, which grants only
the `theme_pack_page_request` command, so a page cannot reach filesystem,
install, process, or any other launcher command. Application commands are
separately gated by the `app` capability, which only the main launcher webview
receives. The CSP also forbids direct network access; every request must go
through the launcher.

Pages call the frozen `window.axolotlTheme` object:

```js
const state = await window.axolotlTheme.storage.read('state')
const results = await window.axolotlTheme.network.fetch({
  url: 'https://api.example.com/items',
  method: 'GET',
})
const logs = await window.axolotlTheme.instances.logs(instanceId)
```

Available methods:

| Method | Permission | Notes |
| --- | --- | --- |
| `network.fetch({ url, method, headers?, body? })` | `network` | HTTPS only, allow-listed host, GET/POST; request 1 MiB, response 8 MiB, 30 s timeout, 4 concurrent and 60 requests/minute per pack; connects directly and ignores system HTTP proxies |
| `storage.list()` / `read(key)` / `write(key, contents)` / `delete(key)` | `storage` | Per-pack directory, 256 files enforced on write, writes serialized against the quota, 2 MiB per file, 10 MiB total; keys use 1-128 ASCII letters, digits, `.`, `_` or `-`, may not start or end with `.` or name a Windows device |
| `instances.list()` | `instances.read` | Read-only instance summaries |
| `instances.mods(instanceId)` | `instances.read` | Read-only mod list |
| `instances.logs(instanceId)` | `instances.logs` | Censored logs, last 256 KiB |

The bridge derives the pack id from the child webview's server-side label, not
from page input, so a page cannot read another pack's storage or network
allowlist. Every bridge method also checks the `requires` list of the page
that was opened: declaring `permissions` on the pack is not enough, the page
must request the matching permission too. Navigation is limited to the
same-origin exact entry document that was opened, without a query or
fragment, so a page cannot navigate to another origin or to a page with a
different `requires` list.

Each pack gets its own deterministic origin: `http://axolotl-theme.localhost:<port>`
on Windows and `axolotl-theme://localhost:<port>` elsewhere, where the port is
derived from the pack id. The protocol rejects a request whose port does not
belong to the pack in its path, and installing a pack whose id would derive an
already-used port is refused. Browser storage (`localStorage`,
`sessionStorage`, IndexedDB) is therefore isolated per pack. Cross-origin
reads of another pack's stylesheets or scripts are blocked by the browser;
static assets are still public by nature, so do not put secrets in a pack
archive. Keep persistent page state in `axolotlTheme.storage`.

## Limits

| Limit | Default |
| --- | --- |
| Archive size | 500 MiB |
| Extracted size | 500 MiB |
| Entries | 4,096 |
| Stylesheet | 4 MiB per file |
| Served asset | 64 MiB per response |

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
