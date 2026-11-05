// `vue-router-original` is a Vite alias for the real package (see demo/vite.config.ts),
// so the shim can re-export it while `vue-router` itself points at the shim.
declare module 'vue-router-original' {
    export * from 'vue-router'
}
