import { injectNotificationManager } from '@modrinth/ui'
import { computed, type MaybeRefOrGetter, onMounted, ref, toValue } from 'vue'

import { clearSettingsOverride, getSettingsLayers, type SettingLayer } from '@/helpers/settings'

const layers = ref<Record<string, SettingLayer>>({})
let loading: Promise<void> | null = null
let failureReported = false

async function requestLayers() {
    try {
        const result = await getSettingsLayers()
        layers.value = Object.fromEntries(result.map((layer) => [layer.key, layer]))
        failureReported = false
    } finally {
        loading = null
    }
}

async function loadLayers(reportError: (error: unknown) => void) {
    if (!loading) loading = requestLayers()

    try {
        await loading
    } catch (error) {
        if (failureReported) return
        failureReported = true
        reportError(error)
    }
}

/** Re-reads the shared layer state after an operation rewrote the local documents. */
export async function reloadSettingsLayers(reportError: (error: unknown) => void) {
    loading = null
    await loadLayers(reportError)
}

export function usePinnedOverride(key: MaybeRefOrGetter<string>) {
    const { handleError } = injectNotificationManager()
    const restoring = ref(false)

    const layer = computed(() => layers.value[toValue(key)] ?? null)
    // An override needs both halves: the deployment pins the key, this machine
    // holds a value for it, and that value is not the one the deployment gives.
    // Local presence alone is every setting the user ever changed.
    const isOverridden = computed(() => {
        const current = layer.value
        if (!current?.provided_present || !current.local_present) return false
        return JSON.stringify(current.provided) !== JSON.stringify(current.effective)
    })

    onMounted(() => loadLayers(handleError))

    /** Drops the local value; returns whether the stored value was still the effective one. */
    async function restore(): Promise<boolean> {
        const current = layer.value
        if (!current || restoring.value) return false

        restoring.value = true
        try {
            const removed = await clearSettingsOverride(current.key, current.effective)
            await loadLayers(handleError)
            return removed
        } catch (error) {
            handleError(error)
            return false
        } finally {
            restoring.value = false
        }
    }

    return { isOverridden, restoring, restore }
}
