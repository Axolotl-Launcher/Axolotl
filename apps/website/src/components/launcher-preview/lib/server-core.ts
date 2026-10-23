/**
 * `@modrinth/server`, the desktop app's server core (server types, EULA and
 * server.properties handling, status rules). It is a dependency-free source
 * package of the workspace that the site does not list as a dependency, so the
 * preview reads it by path, as it reads three.js through @modrinth/ui.
 */
export * from '../../../../../../packages/server/index.ts'
