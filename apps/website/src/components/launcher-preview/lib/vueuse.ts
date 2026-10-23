/**
 * `@vueuse/core` for the shared layouts the preview copies into its window. The
 * site has no dependency on it; `@modrinth/ui` does, and this re-exports that
 * copy through @modrinth/ui's node_modules link, as `three.ts` does for three.js.
 */
export * from '../../../../../../packages/ui/node_modules/@vueuse/core/index.mjs'
