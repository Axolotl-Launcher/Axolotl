import type { MaybeRef } from '@vueuse/core'
import { computed, inject, provide, ref, unref, watch } from 'vue'

const STORAGE_KEY = 'axolotl:symlink-warning-dismissed'
const PROVIDE_KEY = Symbol('symlinkWarningDismiss')

function loadDismissed(): Set<string> {
    try {
        const raw = localStorage.getItem(STORAGE_KEY)
        if (raw) {
            return new Set(JSON.parse(raw))
        }
    } catch {
        // ignore
    }
    return new Set()
}

function saveDismissed(set: Set<string>) {
    try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify([...set]))
    } catch {
        // ignore
    }
}

const dismissedInstances = ref<Set<string>>(loadDismissed())

export interface SymlinkWarningDismissState {
    isHidden: import('vue').ComputedRef<boolean>
    dismissTemp: () => void
    dismissPermanently: () => void
}

export function useSymlinkWarningDismiss(instanceId: MaybeRef<string | null | undefined>) {
    const locallyHidden = ref(false)

    const isDismissed = computed(() => {
        const id = unref(instanceId)
        if (!id) return false
        return dismissedInstances.value.has(id)
    })

    const isHidden = computed(() => locallyHidden.value || isDismissed.value)

    watch(
        () => unref(instanceId),
        () => {
            locallyHidden.value = false
        },
    )

    function dismissTemp() {
        locallyHidden.value = true
    }

    function dismissPermanently() {
        const id = unref(instanceId)
        if (!id) return
        const next = new Set(dismissedInstances.value)
        next.add(id)
        dismissedInstances.value = next
        saveDismissed(next)
    }

    const state: SymlinkWarningDismissState = {
        isHidden,
        dismissTemp,
        dismissPermanently,
    }

    provide(PROVIDE_KEY, state)

    return state
}

export function injectSymlinkWarningDismiss(): SymlinkWarningDismissState | null {
    return inject<SymlinkWarningDismissState | null>(PROVIDE_KEY, null)
}
