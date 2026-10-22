export * from './auto-icons'
export * from './common-messages'
export * from './events'
export * from './file-extensions'
export * from './file-tree'
export * from './game-modes'
export * from './keybinding'
export * from './keybinding-catalog'
export * from './loaders'
export * from './log-share'
export * from './notices'
export * from './savable'
export * from './search'
export * from './server-content-installing'
export * from './server-search'
export * from './tag-messages'
export * from './truncate'
export * from './version-compatibility'
export * from './vue-children'
export type { SkinRendererConfig } from './webgl/skin-rendering'

/** Loads the WebGL implementation only when a skin renderer is needed. */
export function loadSkinRendering() {
    return import('./webgl/skin-rendering')
}
