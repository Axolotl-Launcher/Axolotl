/**
 * `@sfirew/minecraft-motd-parser`, which the shared content layout uses to show
 * server MOTDs. The site has no dependency on it; `@modrinth/ui` does, and this
 * re-exports that copy through @modrinth/ui's node_modules link, as `three.ts`
 * does for three.js.
 */
export * from '../../../../../../packages/ui/node_modules/@sfirew/minecraft-motd-parser/dist/index.mjs'
