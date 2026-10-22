# Architecture

The shared UI package used by both `apps/website` (Nuxt 3) and `apps/app-frontend` (Vue 3 + Tauri). Components here must be platform-agnostic — use dependency injection for platform-specific behavior.

## Folder Structure

```
src/
├── components/       # Vue components organized by feature domain
├── composables/      # Vue 3 composition API hooks
├── layouts/          # Self-contained page layouts (see below)
├── providers/        # Dependency injection contexts (createContext pattern)
├── utils/            # Utility functions and constants
├── locales/          # Locale files (FormatJS)
├── styles/           # Tailwind CSS utilities
```

Each subdirectory under `components/` has an `index.ts` barrel file. All public API is re-exported from the root `index.ts`, which is the only entry consumers should import from — reaching into `@modrinth/ui/src/*` is blocked by ESLint.

### `src/layouts/`

Self-contained page layouts, currently all consumed by the desktop application:

- **`shared/`** — Reusable layout modules with their own components, composables, providers, and types. Each module is a self-contained unit (e.g. `shared/content-tab/` contains the content/mods tab layout with its own `layout.vue`, `components/`, `composables/`, `providers/`, and `types.ts`).
- **`wrapped/`** — Intended for page-level components mirroring route structures. It is currently empty (its hosting pages were removed); the website does not consume any shared layout, as it provides none of the DI contracts they require.

Files inside `layouts/` use the `#ui/*` import alias (resolved via the `"imports"` field in `package.json`) to reference other `src/` modules like `#ui/components/base/buttons/Button.vue` or `#ui/composables/i18n`.

### Platform independence

Components here must stay platform-agnostic: platform capabilities arrive through the `providers/` DI contracts. Importing `@tauri-apps/*` from this package is blocked by ESLint, because the Nuxt website consumes it too and has no Tauri runtime.

# Code Guidelines

### Tailwind Configuration

All frontend packages share a Tailwind preset at `packages/tooling-config/tailwind/tailwind-preset.ts`. This package's `tailwind.config.ts` extends it:

```ts
import preset from '@modrinth/tooling-config/tailwind/tailwind-preset.ts'
```

CSS custom properties are defined in `packages/assets/styles/variables.scss` with light, dark, and OLED theme variants.

### Color Usage Rules

Use `surface-*` variables for backgrounds and strokes. The [surface standard](../../standards/frontend/SURFACE_SYSTEM.md) is authoritative for elevation and separators, including ordinary cards, cards on panels, table rows and stronger outlines. Follow the [frontend development standard](../../standards/frontend/DEVELOPMENT.md) for utility classes, validation and commit-time formatting.

**For text colors:**

| Class                               | Usage                            |
| ----------------------------------- | -------------------------------- |
| `text-[var(--color-text-primary)]`  | Primary headings                 |
| `text-[var(--color-text-default)]`  | Default body text                |
| `text-[var(--color-text-tertiary)]` | Reduced emphasis, secondary info |

**Brand and semantic colors** not all exposed as Figma variables — refer to `packages/assets/styles/variables.scss` for the full set:

- `bg-{color}`, `text-{color}` etc. — Primary brand colors
- `bg-{color}-highlight` — 25% opacity semantic highlights

**Color palette** (each with shades 50–950): red, orange, green, blue, purple, gray. Platform-specific colors also exist (fabric, forge, quilt, neoforge, etc.).

## Dependency Injection

This package defines the DI layer using `createContext` from `src/providers/index.ts`. Read the [dependency injection standard](../../standards/frontend/DEPENDENCY_INJECTION.md) before adding or changing a contract.

Key providers exported from this package:

- `provideModrinthClient` / `injectModrinthClient` — API client
- `provideNotificationManager` / `injectNotificationManager` — Notifications

## Vue Template Rules

### Multi-statement event handlers

Never use newline-separated statements in Vue template event handlers like `@click`. Vue's template compiler cannot parse multi-line expressions separated only by newlines. Always use semicolons on a single line:

```vue
<!-- BAD: will cause "Unexpected token" parse error -->
@click=" foo = true $emit('bar') "

<!-- GOOD -->
@click="foo = true; $emit('bar')"
```
