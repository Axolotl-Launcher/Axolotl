# @modrinth/api-client

Platform-agnostic client used by Axolotl for public content-service APIs. It supports Nuxt, Tauri and generic Node/browser environments. Modrinth account login, friends and commercial hosting integrations have been removed from this fork; do not reintroduce their modules, node authentication or socket clients during upstream synchronization.

## Architecture

Requests follow `Module method → client.request() → feature chain → platform executeRequest()`.

- `src/core/` owns client, module, feature and upload abstractions.
- `src/platform/` implements generic, Nuxt, Tauri and XHR upload transport.
- `src/features/` owns request middleware such as retry, circuit breaking and optional authentication.
- `src/modules/` groups endpoint modules by service, including Labrinth, LauncherMeta, Paper, Purpur and Mclogs.
- `src/types/` owns client configuration, request, upload and error contracts.

Modules are lazy-loaded from `MODULE_REGISTRY` in `src/modules/index.ts`. Nested client properties and types are inferred from that registry. Keep implementations, registry entries, barrel exports and response types aligned when adding or removing an API.

```ts
const project = await client.labrinth.projects_v2.get('sodium')
const version = await client.labrinth.versions_v3.getVersion('version-id')
```

## Requests and authentication

API modules must use `this.client.request()` or `this.client.upload()` for HTTP calls. Do not call `$fetch`, `fetch` or another transport directly from modules: doing so bypasses the platform and middleware chain. The existing node-only ISO3166 data module is the exception.

Axolotl's desktop content client is anonymous. Generic authentication middleware may be used by an explicitly authenticated consumer, but must not implicitly read historical Modrinth launcher sessions or attach them to public requests.

For external services, pass the full base URL as `api` and set `skipAuth: true`:

```ts
return this.client.request<ResponseType>('/endpoint', {
  api: 'https://external-api.example',
  version: 1,
  method: 'GET',
  skipAuth: true,
})
```

Do not restore Archon/Kyros base URLs, node credentials, panel headers, WebSocket or server-sync transports to support an obsolete hosting caller. Current local-server capabilities belong to the desktop runtime and its platform providers.

## Vue consumers

Consumers provide the client through the [dependency injection standard](../../standards/frontend/DEPENDENCY_INJECTION.md). The desktop app uses `TauriModrinthClient`; shared components obtain the abstract client through `injectModrinthClient()` from `@modrinth/ui`. Do not assume the website provides every desktop layout contract.

```ts
const { labrinth } = injectModrinthClient()
const project = await labrinth.projects_v3.get(projectId)
```

## Types

Endpoint types must match the actual backend response: do not reshape or omit fields merely because the desktop UI does not use them. Keep them in the service's `types.ts` and expose them through the package root.

```ts
import type { Labrinth } from '@modrinth/api-client'

const project: Labrinth.Projects.v3.Project = await client.labrinth.projects_v3.get(projectId)
```

Removing an obsolete feature does not authorize changing unrelated public project, user or version response contracts.

## Uploads

The XHR upload layer provides progress and cancellation in browser-capable environments. Nuxt rejects uploads during SSR. Use the existing `UploadHandle<T>` contract: `promise`, `onProgress()` and `cancel()`.

```ts
const upload = client.upload<void>('/example-upload', {
  api: 'https://upload.example',
  version: 1,
  file,
  skipAuth: true,
})
upload.onProgress(({ progress }) => updateProgress(progress))
await upload.promise
```

Use explicit files or FormData according to the endpoint; preserve feature-chain handling and cancellation. See [the upload implementation](src/platform/xhr-upload-client.ts) for the transport contract.

## Validation and extension

Follow the [frontend development standard](../../standards/frontend/DEVELOPMENT.md). Run this package's `fix` and `lint` before a frontend commit, and `build` to verify JavaScript and TypeScript declaration output. There is no package `test` script; exercise affected consumers' existing tests when changing shared behavior.

For a new endpoint module, follow the [`api-module` skill](../../.agents/skills/api-module/SKILL.md) and [API module standard](../../standards/frontend/ADDING_API_MODULES.md).
