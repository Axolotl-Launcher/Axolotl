/**
 * Entry of the web demo: the real launcher frontend, with the Tauri bridge replaced
 * by an in-browser backend before any app module loads.
 *
 * An embedding page can pick the starting theme with `?theme=dark|light|oled|system`
 * and switch it later by posting `{ type: 'axolotl-demo:theme', theme }`. The demo
 * posts `{ type: 'axolotl-demo:ready' }` to it once the launcher's first screen shows.
 * What the demo shows comes from `demo-data.json` next to this page (backend/demo-data.ts).
 */
import type { Pinia } from 'pinia'

import { useTheming } from '@/store/theme'

import { installDemoBackend } from './backend'
import { loadDemoData } from './backend/demo-data'
import { installDemoNetwork } from './backend/network'
import { demoTheme, saveDemoTheme } from './backend/settings'

installDemoNetwork()
installDemoBackend()

type MountedRoot = HTMLElement & {
    __vue_app__?: {
        config: {
            globalProperties: { $router?: { isReady(): Promise<void> }; $pinia?: Pinia }
        }
    }
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

window.addEventListener('message', (event) => {
    if (event.source !== window.parent || event.data?.type !== 'axolotl-demo:theme') return
    const theme = demoTheme(event.data.theme)
    if (!theme) return
    saveDemoTheme(theme)
    const pinia = mountedApp()?.config.globalProperties.$pinia
    if (pinia) useTheming(pinia).setThemeState(theme)
})

void loadDemoData()
    .then(() => import('../src/main.js'))
    .then(async () => {
        await mountedApp()?.config.globalProperties.$router?.isReady()
        // Uncovering the demo during the launcher's own splash would show a second loading screen.
        await Promise.race([splash, new Promise((resolve) => setTimeout(resolve, 30_000))])
        requestAnimationFrame(() =>
            requestAnimationFrame(() =>
                window.parent.postMessage({ type: 'axolotl-demo:ready' }, '*'),
            ),
        )
    })
