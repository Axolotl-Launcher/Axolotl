import { OverlayScrollbars, type PartialOptions } from 'overlayscrollbars'
import type { ObjectDirective } from 'vue'

const defaultOverlayScrollbarsOptions = Object.freeze<PartialOptions>({
    scrollbars: {
        theme: 'os-theme-dark',
        autoHide: 'leave',
        autoHideSuspend: true,
    },
})

const mergeOptions = (options: PartialOptions = {}): PartialOptions => ({
    ...defaultOverlayScrollbarsOptions,
    ...options,
    scrollbars: {
        ...defaultOverlayScrollbarsOptions.scrollbars,
        ...(options.scrollbars ?? {}),
    },
})

/** Runs `callback` once the next frame has been painted. */
const afterNextPaint = (callback: () => void) =>
    requestAnimationFrame(() => {
        setTimeout(callback, 0)
    })

export const overlayScrollbarsDirective: ObjectDirective<HTMLElement, PartialOptions | undefined> =
    {
        mounted(el, binding) {
            // Initializing measures the element; inside the task that mounts it, that
            // forces the whole window's first layout. `data-overlayscrollbars-initialize`
            // keeps native scrollbars hidden until then.
            afterNextPaint(() => {
                if (el.isConnected) OverlayScrollbars(el, mergeOptions(binding.value))
            })
        },
        updated(el, binding) {
            if (binding.value === binding.oldValue) return
            const instance = OverlayScrollbars(el)
            instance?.options(mergeOptions(binding.value))
        },
        unmounted(el) {
            const instance = OverlayScrollbars(el)
            instance?.destroy()
        },
    }
