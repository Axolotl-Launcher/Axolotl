---
name: tanstack-query
description: Implement or migrate Vue server-state queries and mutations with TanStack Query, including cache invalidation and reactive query keys. Use when changing remote data loading in a page or component.
---

Read the target package's instructions and [frontend development standard](../../../standards/frontend/DEVELOPMENT.md). Confirm a QueryClient is installed/provided and preserve the existing source of data; do not replace native/Tauri data with API-client requests just to fit this workflow. The website currently has no QueryClient integration. Do not add it merely because a file uses `useAsyncData`.

## Steps

1. **Read the component, callers and provider setup.** Identify existing query, `useAsyncData`, `useFetch` and imperative request patterns. Confirm server-state ownership before migrating it.
2. **Define query boundaries** with stable hierarchical keys, reactive parameters, lifecycle expectations and mutation ownership.
3. **Implement or migrate queries when query semantics fit:**
   - Preserve Nuxt SSR and first-paint behavior; migrate `useAsyncData` or `useFetch` only when the target runtime can support the same lifecycle.
   - Use the existing provider/fetcher for `queryFn`; inject the API client only where that is already the owning data source.
   - Reuse domain query-key factories when available. Include all response-defining parameters, including provider identity, in stable hierarchical keys.
   - Use `computed` query keys for reactive parameters.
   - Use the `enabled` option for conditional queries that depend on other data.
4. **Implement or migrate mutations when cache ownership is clear:**
   - Replace manual `try/catch` + `ref` patterns with `useMutation`.
   - Add `onSuccess` handlers that invalidate or update related query caches.
   - Use optimistic updates only when rollback and concurrent updates are well-defined.
5. **Clean up:**
   - Remove manual loading/error `ref()`s that are now handled by TanStack Query's return values (`isPending`, `isError`, `error`).
   - Remove manual `onMounted` fetch calls.
   - Preserve the target surface's lifecycle behavior and avoid introducing duplicate initial requests.
6. **Verify** loading/error/success behavior, route-shell prefetch parity, duplicate initial requests, and invalidation of affected queries. Run the relevant package tests.
