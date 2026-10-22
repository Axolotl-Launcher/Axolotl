---
name: cross-platform-pages
description: Share a Vue page between the website and desktop app by placing it in packages/ui and defining only the required platform contracts. Use when moving or designing cross-platform pages and layouts.
---

Read [shared UI instructions](../../../packages/ui/AGENTS.md), the [DI standard](../../../standards/frontend/DEPENDENCY_INJECTION.md) and each affected app's instructions. The website currently consumes shared components without the desktop layout providers; do not assume it is ready to render an existing desktop layout. Implement a website route or provider only when the requested scope requires it.

## Steps

1. **Identify the page** from the user's request. Read its implementation and route shells; no special argument interpolation is required.
2. **Identify platform boundaries**: keep identical page implementations wrapped in `packages/ui`; use dependency injection only when website and desktop data sources differ.
3. **Choose the category based on the existing platform boundaries:**
   - **Wrapped** (`layouts/wrapped/`) — if the page uses the same API source on both platforms (e.g. web requests, not Tauri plugins). Just move the page component into `packages/ui` and import it from both frontends.
   - **Shared** (`layouts/shared/`) — if the page has different platform-owned data or capabilities (e.g. website uses the API client while the app uses Tauri commands). Requires a DI contract.
4. **For shared layouts:**
   - Define a DI contract interface in `providers/` capturing all platform-specific operations.
   - Create the layout component that injects the context and handles all UI logic.
   - Extract reusable stateful logic (search, filtering, selection) into `composables/`.
   - Implement the contract separately in each frontend (`apps/website/`, `apps/app-frontend/`).
5. **For wrapped pages:**
   - Move the page component into `packages/ui/src/layouts/wrapped/` matching the route structure.
   - Replace any platform-specific imports with shared utilities.
   - Import and render the wrapped page from both frontends as a simple component.
   - When a route shell prefetches TanStack Query data, preserve the same query keys and fetch functions in the layout.
6. **Verify** package exports, route-shell props, provider implementations and platform-specific lifecycle behavior. Run the affected package checks.
