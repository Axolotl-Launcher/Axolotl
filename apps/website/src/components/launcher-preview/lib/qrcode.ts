/**
 * `qrcode.vue` for the window-scoped `ShareModal`. The site has no dependency on
 * it; `@modrinth/ui` does, and this re-exports that copy through @modrinth/ui's
 * node_modules link, as `three.ts` does for three.js.
 */
export { default as QrcodeVue } from '../../../../../../packages/ui/node_modules/qrcode.vue/dist/qrcode.vue.esm.js'
