---
name: api-module
description: Add a new API endpoint module to packages/api-client from an OpenAPI schema. Use when adding new backend endpoints, creating API client modules, or when an openapi.yml is provided.
---

Read [API client instructions](../../../packages/api-client/AGENTS.md) and the [API module standard](../../../standards/frontend/ADDING_API_MODULES.md). The schema is supplied by the user; ask for its location only when it cannot be identified from the request.

## Steps

1. **Read the OpenAPI schema named in the request** — identify endpoint paths, server URLs, request/response shapes and path parameters.
2. **Read the standard above** for naming conventions, type rules, and the module registration pattern.
3. **Determine the service, domain and version** from the schema's server and route definitions; place each module under `modules/<service>/<domain>/v<N>.ts`.
4. **Define types in `types.ts`** — types must match the API response 1:1. Use the OpenAPI schema as the source of truth. Do not reshape or rename fields.
5. **Create the module class** — extend `AbstractModule`, implement each endpoint as a method, and route requests through `this.client.request()` or `.upload()` as appropriate. Follow the request options in the standard.
6. **Register in `MODULE_REGISTRY`** — add the module entry so it's auto-instantiated on the client.
7. **Export types** from `modules/types.ts` when adding a service namespace. Existing service namespaces are already re-exported.
8. **Verify** — check that the module compiles and the types are accessible from `@modrinth/api-client`.
