/**
 * Entry of the web demo: the real launcher frontend, with the Tauri bridge replaced
 * by an in-browser backend before any app module loads.
 */
import { installDemoBackend } from './backend'
import { installDemoNetwork } from './backend/network'

installDemoNetwork()
installDemoBackend()

type MountedRoot = HTMLElement & {
    __vue_app__?: { config: { globalProperties: { $router?: { isReady(): Promise<void> } } } }
}

void import('../src/main.js').then(async () => {
    const app = (document.querySelector('#app') as MountedRoot | null)?.__vue_app__
    await app?.config.globalProperties.$router?.isReady()
    // Tell the embedding page once the first screen has painted.
    requestAnimationFrame(() =>
        requestAnimationFrame(() => window.parent.postMessage({ type: 'axolotl-demo:ready' }, '*')),
    )
})
