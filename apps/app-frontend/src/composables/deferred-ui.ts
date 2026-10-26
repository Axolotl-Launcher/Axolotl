import { markRaw, type Ref, ref, watch } from 'vue'

const INTERACTIONS = [
    'pointerdown',
    'keydown',
    'wheel',
    'touchstart',
    'dragenter',
    'focusin',
] as const

type Methods = Record<string, (...args: unknown[]) => unknown>

/** Runs `callback` when the page is idle, or after `timeout` at the latest; returns a cancel. */
function whenIdle(callback: () => void, timeout: number): () => void {
    // WebKit (the macOS webview) may not have requestIdleCallback.
    if (typeof window.requestIdleCallback === 'function') {
        const handle = window.requestIdleCallback(callback, { timeout })
        return () => window.cancelIdleCallback(handle)
    }
    const handle = window.setTimeout(callback, timeout)
    return () => window.clearTimeout(handle)
}

/**
 * UI the first screen does not show (dialogs, overlays and flows that open later).
 * Startup loads and renders only the first screen; this UI mounts on the first
 * interaction, when the app is idle (`idleTimeout`, null to wait for interaction),
 * or as soon as code needs one of its components, whichever comes first.
 */
export function createDeferredUi(idleTimeout: number | null) {
    const mounted = ref(false)
    let cancelIdle: (() => void) | null = null

    function mount() {
        if (mounted.value) return
        mounted.value = true
        for (const type of INTERACTIONS) window.removeEventListener(type, mount, true)
        cancelIdle?.()
    }

    for (const type of INTERACTIONS) {
        window.addEventListener(type, mount, { capture: true, passive: true })
    }
    if (idleTimeout !== null) cancelIdle = whenIdle(mount, idleTimeout)

    /** The component once it has mounted, mounting the deferred UI first. */
    function when<T>(component: Ref<T | null | undefined>): Promise<T> {
        mount()
        if (component.value) return Promise.resolve(component.value)
        return new Promise((resolve) => {
            const stop = watch(component, (instance) => {
                if (!instance) return
                stop()
                resolve(instance)
            })
        })
    }

    /**
     * Stands in for a deferred component where code holds on to it: each method
     * call waits for the component to mount, then forwards and returns its result.
     */
    function handle<T extends object>(component: Ref<T | null | undefined>): T {
        return markRaw(
            new Proxy({} as T, {
                get(target, key) {
                    if (typeof key === 'symbol' || key === 'then' || key.startsWith('__v')) {
                        return Reflect.get(target, key)
                    }
                    return (...args: unknown[]) =>
                        when(component).then((instance) =>
                            (instance as unknown as Methods)[key](...args),
                        )
                },
            }),
        )
    }

    return { mounted, mount, when, handle }
}
