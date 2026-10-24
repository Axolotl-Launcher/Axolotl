import '@modrinth/ui/directives/tooltip-styles.ts'

import { tooltipDirective } from '@modrinth/ui'

/**
 * Registers the shared `v-tooltip` directive on the website.
 *
 * The site compiles `packages/ui` components directly, and some of them carry
 * `v-tooltip` (the theme selector in the settings modal is the live case).
 * Without this the directive would not resolve and those tooltips would do
 * nothing -- which is what happened while the directive belonged to the app
 * and the website never installed it.
 *
 * The directive's stylesheet ships with the directive itself, and its tokens
 * carry fallbacks, because the website declares neither the tooltip tokens nor
 * the radii the desktop app uses.
 *
 * Client-only: the directive attaches DOM listeners, and its hooks do not run
 * during SSR anyway.
 */
export default defineNuxtPlugin((nuxtApp) => {
    nuxtApp.vueApp.directive('tooltip', tooltipDirective)
})
