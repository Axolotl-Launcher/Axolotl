/**
 * three.js for the preview's 3D views, which port app modules that import it.
 * The site has no three dependency of its own; `@modrinth/ui` does, and this
 * re-exports that same copy (through @modrinth/ui's node_modules link), so the
 * preview and @modrinth/ui's skin renderer share one instance. Only lazy route
 * chunks import this module.
 */
export * from '../../../../../../packages/ui/node_modules/three/build/three.module.js'
