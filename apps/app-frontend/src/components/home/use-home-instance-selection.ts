import { useIsMutating, useMutation, useQueryClient } from '@tanstack/vue-query'
import { computed, type Ref, watch } from 'vue'

import { type AppSettings, appSettingsKeys, get, update } from '@/helpers/settings'
import type { GameInstance } from '@/helpers/types'
import { useTheming } from '@/store/state'

const mutationKey = [...appSettingsKeys.update, 'home-instance']

type SelectionChange = { instanceId: string } | { missingInstanceId: string }

export function useHomeInstanceSelection(
    instances: Readonly<Ref<readonly GameInstance[] | undefined>>,
    onError: (error: unknown) => void,
) {
    const themeStore = useTheming()
    const queryClient = useQueryClient()
    const selectedInstanceId = computed(() => themeStore.minimalHomeInstanceId)
    const activeMutations = useIsMutating({ mutationKey, exact: true })
    const selecting = computed(() => activeMutations.value > 0)

    function applySelection(settings: AppSettings) {
        const instanceId = settings.minimal_home_instance_id
        themeStore.minimalHomeInstanceId = instanceId
        queryClient.setQueryData<AppSettings>(appSettingsKeys.all, (cached) =>
            cached ? { ...cached, minimal_home_instance_id: instanceId } : settings,
        )
    }

    const { mutateAsync } = useMutation({
        mutationKey,
        scope: { id: 'home-instance-selection' },
        networkMode: 'always',
        mutationFn: async (change: SelectionChange) => {
            if ('instanceId' in change) {
                if (change.instanceId === selectedInstanceId.value) return
                return update({ minimal_home_instance_id: change.instanceId })
            }

            if (
                selectedInstanceId.value !== change.missingInstanceId ||
                !instances.value ||
                instances.value.some((instance) => instance.id === change.missingInstanceId)
            )
                return
            return update({ minimal_home_instance_id: null })
        },
        onSuccess: (settings) => {
            if (settings) applySelection(settings)
        },
        onError: async (error) => {
            // The settings command can fail after persisting the patch.
            const settings = await get().catch(() => undefined)
            if (settings) applySelection(settings)
            onError(error)
        },
    })

    async function select(instance: GameInstance) {
        try {
            await mutateAsync({ instanceId: instance.id })
            return selectedInstanceId.value === instance.id
        } catch {
            return false
        }
    }

    watch(
        [instances, selectedInstanceId],
        ([available, instanceId]) => {
            if (
                instanceId &&
                available &&
                !available.some((instance) => instance.id === instanceId)
            )
                void mutateAsync({ missingInstanceId: instanceId }).catch(() => undefined)
        },
        { immediate: true },
    )

    return { selectedInstanceId, selecting, select }
}
