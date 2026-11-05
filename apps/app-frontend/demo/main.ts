/**
 * Entry of the web demo: the real launcher frontend, with the Tauri bridge replaced
 * by an in-browser backend before any app module loads.
 *
 * The demo posts `{ type: 'axolotl-demo:ready' }` to an embedding page once the
 * launcher's first screen shows.
 */
import { installDemoBackend } from './backend'
import { installDemoNetwork } from './backend/network'

installDemoNetwork()
installDemoBackend()

type MountedRoot = HTMLElement & {
    __vue_app__?: { config: { globalProperties: { $router?: { isReady(): Promise<void> } } } }
}

const mountedApp = () => (document.querySelector('#app') as MountedRoot | null)?.__vue_app__

/**
 * Resolves once the launcher's splash screen (SplashScreen.vue, the only `.cube-bg`)
 * has shown and faded out. Watching starts before the app loads so it is never missed.
 */
function splashFinished() {
    return new Promise<void>((resolve) => {
        let shown = false
        const observer = new MutationObserver(() => {
            const present = document.querySelector('.cube-bg') !== null
            shown ||= present
            if (shown && !present) {
                observer.disconnect()
                resolve()
            }
        })
        observer.observe(document.documentElement, { childList: true, subtree: true })
    })
}

const splash = splashFinished()

void import('../src/main.js').then(async () => {
    await mountedApp()?.config.globalProperties.$router?.isReady()
    // Uncovering the demo during the launcher's own splash would show a second loading screen.
    await Promise.race([splash, new Promise((resolve) => setTimeout(resolve, 30_000))])
    requestAnimationFrame(() =>
        requestAnimationFrame(() => window.parent.postMessage({ type: 'axolotl-demo:ready' }, '*')),
    )
})
