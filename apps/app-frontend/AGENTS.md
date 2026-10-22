# Desktop Frontend

Vue 3 + Vite frontend for the Tauri desktop application. Follow the [frontend development standard](../../standards/frontend/DEVELOPMENT.md) for validation, commit-time formatting and styling.

## Boundaries

- Routes live in `src/pages/`; app-specific orchestration and native capabilities stay here. Shared UI belongs in `packages/ui` only when its behavior is platform-independent.
- Providers are wired through `src/providers/setup.ts` and `src/providers/setup/`. Read the [DI standard](../../standards/frontend/DEPENDENCY_INJECTION.md) before changing platform contracts.
- For content-management features, preserve provider-qualified identities across display, selection, persistence and installation.
- When adding, renaming or changing a native `invoke` call, read and follow the [Tauri command completion rules](../app/AGENTS.md#tauri-command-completion). Include the shell's handler, build-time command manifest and window permissions in the change; frontend mocks cannot establish that the command is allowed at runtime.

## Onboarding

For additions or material changes to a page, route, navigation entry, large user-facing component, core workflow, settings section or content-management feature, assess the first-use journey. Read [onboarding instructions](src/components/ui/onboarding/AGENTS.md) before changing guided behavior or targets, and whenever a feature requires a new or updated tour.

## Update Announcements

Only edit the announcement catalog when the user explicitly requests an update log and provides the exact version. Read [announcement instructions](src/announcements/AGENTS.md) before changing the catalog, announcement display, updater trigger or startup dialog priority.

## Verification

Use the existing `test` and `test:visual` scripts for affected behavior. Native calls in browser tests require the project's mocks; do not start the real application solely to replace a component test. The user performs visual and interaction acceptance.
