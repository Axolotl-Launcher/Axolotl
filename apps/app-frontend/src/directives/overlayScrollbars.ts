import { OverlayScrollbars, type PartialOptions } from 'overlayscrollbars'
import type { ObjectDirective } from 'vue'

const defaultOverlayScrollbarsOptions = Object.freeze<PartialOptions>({
    scrollbars: {
        theme: 'os-theme-dark',
        autoHide: 'leave',
        autoHideSuspend: true,
    },
    update: {
        // Coalesce bursts of mutations and resizes into one update per
        // animation frame, but never delay an update by more than 100 ms.
        // Event and environment debounces keep the library defaults.
        debounce: {
            mutation: [0, 100],
            resize: [0, 100],
            event: [33, 99],
            env: [222, 666, true],
        },
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

export const overlayScrollbarsDirective: ObjectDirective<HTMLElement, PartialOptions | undefined> =
    {
        mounted(el, binding) {
            OverlayScrollbars(el, mergeOptions(binding.value))
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
