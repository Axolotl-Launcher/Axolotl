import { injectNotificationManager } from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed, type ComputedRef, onUnmounted, type Ref, ref, watch } from 'vue'

import { useMinecraftLaunchError } from '@/composables/useMinecraftLaunchError'
import { trackEvent } from '@/helpers/analytics'
import { process_listener } from '@/helpers/events'
import { install_existing_instance, install_pack_to_existing_instance } from '@/helpers/install'
import { kill, run } from '@/helpers/instance'
import { instanceLaunchStartedAt } from '@/helpers/instance-launch-state'
import { get_by_instance_id } from '@/helpers/process'
import type { GameInstance } from '@/helpers/types'
import { handleSevereError } from '@/store/error'

export { HOME_LAUNCH_KEY, type HomeLaunchContext, useHomeLaunchTarget } from './home-launch-context'

export type HomeLaunchPhase =
    'not-installed' | 'installing' | 'busy' | 'running' | 'ready' | 'error'

export type HomeLaunchState = {
    phase: ComputedRef<HomeLaunchPhase>
    play: () => Promise<void>
    stop: () => Promise<void>
    install: () => Promise<void>
    retry: () => Promise<void>
}

/**
 * Launch, stop and install for one instance, with the process state that
 * decides which of them Home offers. These are the same calls the instance
 * page uses; `source` only labels the analytics event.
 */
export function useHomeLaunchState(
    instance: Ref<GameInstance | null | undefined>,
    source: string,
): HomeLaunchState {
    const { handleError } = injectNotificationManager()
    const handleMinecraftLaunchError = useMinecraftLaunchError()

    const processQuery = useQuery({
        queryKey: computed(() => ['home', 'instance-processes', instance.value?.id]),
        queryFn: () => get_by_instance_id(instance.value!.id),
        enabled: computed(() => !!instance.value),
        refetchOnWindowFocus: true,
    })
    const running = computed(() => (processQuery.data.value?.length ?? 0) > 0)
    const loading = ref(false)
    const currentEvent = ref<string | null>(null)
    const installed = computed(() => instance.value?.install_stage === 'installed')
    const installing = computed(() => instance.value?.install_stage.includes('installing') ?? false)

    const phase = computed<HomeLaunchPhase>(() => {
        if (processQuery.isError.value && !running.value) return 'error'
        if (running.value) return 'running'
        if (installing.value) return 'installing'
        if (
            loading.value ||
            (instance.value != null && instanceLaunchStartedAt(instance.value.id) !== undefined) ||
            processQuery.isFetching.value ||
            currentEvent.value === 'launched'
        )
            return 'busy'
        return installed.value ? 'ready' : 'not-installed'
    })

    async function refresh() {
        if (instance.value) await processQuery.refetch()
    }

    async function play() {
        const target = instance.value
        if (!target) return

        loading.value = true
        try {
            await run(target.id)
            trackEvent('InstanceStart', {
                loader: target.loader,
                game_version: target.game_version,
                source,
            })
        } catch (error) {
            const handled = await handleMinecraftLaunchError(error, {
                instance_id: target.id,
                instance_name: target.name,
            })
            if (!handled) handleSevereError(error, { instanceId: target.id })
        } finally {
            loading.value = false
            await refresh()
        }
    }

    async function stop() {
        const target = instance.value
        if (!target) return

        await kill(target.id).catch(handleError)
        await refresh()
        trackEvent('InstanceStop', {
            loader: target.loader,
            game_version: target.game_version,
            source,
        })
    }

    async function install() {
        const target = instance.value
        if (!target) return

        loading.value = true
        try {
            if (
                target.install_stage !== 'pack_installed' &&
                (target.link?.type === 'modrinth_modpack' ||
                    target.link?.type === 'server_project_modpack')
            ) {
                await install_pack_to_existing_instance(target.id, {
                    type: 'fromVersionId',
                    project_id: target.link.project_id ?? target.link.server_project_id ?? '',
                    version_id: target.link.version_id ?? target.link.content_version_id ?? '',
                    title: target.name,
                })
            } else {
                await install_existing_instance(target.id, false)
            }
        } catch (error) {
            handleError(error)
        } finally {
            loading.value = false
        }
    }

    watch(
        () => instance.value?.id,
        () => {
            currentEvent.value = null
        },
    )

    let disposed = false
    let unlistenProcess: (() => void) | undefined
    void process_listener((event: { instance_id: string; event: string }) => {
        if (disposed || event.instance_id !== instance.value?.id) return
        currentEvent.value = event.event
        void refresh()
    })
        .then((unlisten) => {
            if (disposed) unlisten()
            else unlistenProcess = unlisten
        })
        .catch(handleError)

    onUnmounted(() => {
        disposed = true
        unlistenProcess?.()
    })

    return { phase, play, stop, install, retry: refresh }
}
