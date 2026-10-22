# Axolotl Website

`apps/website` is the official Axolotl Launcher website. It is a Nuxt 3 static site using Vue 3, Tailwind CSS, and the shared UI and asset packages.

## Development

Run `pnpm website:dev` from the repository root for local development and `pnpm website:build` for the production build.

Follow the [frontend development standard](../../standards/frontend/DEVELOPMENT.md) for validation, formatting and styling. The user performs visual and interaction acceptance; do not run excessive browser automation unless requested.

The website has its own locale integration. Inspect `src/locales/` and the consuming component before applying shared UI localization patterns. It does not currently provide the desktop shared-layout DI contracts or a TanStack Query client; add those only when the requested feature needs them.

## Structure

- `src/pages/` contains file-based routes.
- `src/components/` contains website-specific components.
- `src/layouts/` contains page layouts.
- `src/locales/` contains website messages.
- Shared components and styles belong in `packages/ui` and `packages/assets` when both the website and desktop application can use them.
