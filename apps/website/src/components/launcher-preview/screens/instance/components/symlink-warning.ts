import { computed, type ComputedRef, inject, provide, ref, watch } from 'vue'

import { usePreviewDomain } from '../../../lib/state'

const PROVIDE_KEY = Symbol('launcher-preview-symlink-warning-dismiss')

export interface SymlinkWarningDismissState {
    isHidden: ComputedRef<boolean>
    dismissTemp: () => void
    dismissPermanently: () => void
}

/**
 * Port of `composables/useSymlinkWarningDismiss.ts`. The app keeps the
 * permanently dismissed instances in localStorage; the preview keeps them for
 * the lifetime of the window.
 */
export function useSymlinkWarningDismiss(instanceId: () => string | null | undefined) {
    const dismissed = usePreviewDomain('instance-symlink-warning', () => ({
        value: ref<Set<string>>(new Set()),
    }))
    const locallyHidden = ref(false)

    const isDismissed = computed(() => {
        const id = instanceId()
        return id ? dismissed.value.has(id) : false
    })

    const isHidden = computed(() => locallyHidden.value || isDismissed.value)

    watch(instanceId, () => {
        locallyHidden.value = false
    })

    const state: SymlinkWarningDismissState = {
        isHidden,
        dismissTemp() {
            locallyHidden.value = true
        },
        dismissPermanently() {
            const id = instanceId()
            if (!id) return
            dismissed.value = new Set([...dismissed.value, id])
        },
    }

    provide(PROVIDE_KEY, state)
    return state
}

export function injectSymlinkWarningDismiss(): SymlinkWarningDismissState | null {
    return inject<SymlinkWarningDismissState | null>(PROVIDE_KEY, null)
}
