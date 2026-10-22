<script setup lang="ts">
import {
    DownloadIcon,
    GameIcon,
    ListIcon,
    PlayIcon,
    PlusIcon,
    SpinnerIcon,
    StopCircleIcon,
    TimerIcon,
} from '@modrinth/assets'
import {
    Button,
    Card,
    commonMessages,
    defineMessages,
    injectNotificationManager,
    useRelativeTime,
    useVIntl,
} from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import dayjs from 'dayjs'
import { computed, onUnmounted, ref, watch } from 'vue'

import HomeGreeting from '@/components/home/HomeGreeting.vue'
import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import { useMinecraftLaunchError } from '@/composables/useMinecraftLaunchError'
import { useNetworkStatus } from '@/composables/useNetworkStatus'
import { trackEvent } from '@/helpers/analytics'
import { process_listener } from '@/helpers/events'
import { install_existing_instance, install_pack_to_existing_instance } from '@/helpers/install'
import { kill, run } from '@/helpers/instance'
import { get_by_instance_id } from '@/helpers/process'
import type { GameInstance } from '@/helpers/types'
import { handleSevereError } from '@/store/error'

const props = defineProps<{
    instances: GameInstance[]
    selectedInstanceId?: string | null
    playerName?: string | null
}>()

const emit = defineEmits<{
    choose: []
    create: []
}>()

const { formatMessage } = useVIntl()
const formatRelativeTime = useRelativeTime()
const { handleError } = injectNotificationManager()
const handleMinecraftLaunchError = useMinecraftLaunchError()
const { offline } = useNetworkStatus()

const messages = defineMessages({
    chooseInstance: {
        id: 'app.home.minimal.choose-instance',
        defaultMessage: 'Choose instance',
    },
    changeInstance: {
        id: 'app.home.minimal.change-instance',
        defaultMessage: 'Change Home instance',
    },
    createInstance: {
        id: 'app.home.instances.create',
        defaultMessage: 'Create instance',
    },
    noInstances: {
        id: 'app.home.instances.empty',
        defaultMessage: 'No instances yet',
    },
    loading: {
        id: 'app.instance.loading',
        defaultMessage: 'Instance is loading...',
    },
    played: {
        id: 'app.instance.played',
        defaultMessage: 'Played {time}',
    },
    neverPlayed: {
        id: 'app.instance.never-played',
        defaultMessage: 'Never played',
    },
    offlineInstalledOnly: {
        id: 'app.instance.offline-installed-only',
        defaultMessage: 'Offline mode can only launch fully downloaded instances.',
    },
})

const selectedInstance = computed(() =>
    props.instances.find((instance) => instance.id === props.selectedInstanceId),
)
const processQuery = useQuery({
    queryKey: computed(() => ['home', 'instance-processes', selectedInstance.value?.id]),
    queryFn: () => get_by_instance_id(selectedInstance.value!.id),
    enabled: computed(() => !!selectedInstance.value),
    refetchOnWindowFocus: true,
})
const running = computed(() => (processQuery.data.value?.length ?? 0) > 0)
const loading = ref(false)
const currentEvent = ref<string | null>(null)
const installed = computed(() => selectedInstance.value?.install_stage === 'installed')
const installing = computed(
    () => selectedInstance.value?.install_stage.includes('installing') ?? false,
)
const busy = computed(
    () =>
        loading.value ||
        installing.value ||
        processQuery.isFetching.value ||
        (currentEvent.value === 'launched' && !running.value),
)

const lastPlayed = computed(() => {
    if (!selectedInstance.value?.last_played) return formatMessage(messages.neverPlayed)
    return formatMessage(messages.played, {
        time: formatRelativeTime(dayjs(selectedInstance.value.last_played).toISOString()),
    })
})

async function refreshProcessState() {
    if (selectedInstance.value) await processQuery.refetch()
}

async function playInstance() {
    const instance = selectedInstance.value
    if (!instance) return

    loading.value = true
    try {
        await run(instance.id)
        trackEvent('InstanceStart', {
            loader: instance.loader,
            game_version: instance.game_version,
            source: 'HomeMinimal',
        })
    } catch (error) {
        const handled = await handleMinecraftLaunchError(error, {
            instance_id: instance.id,
            instance_name: instance.name,
        })
        if (!handled) handleSevereError(error, { instanceId: instance.id })
    } finally {
        loading.value = false
        await refreshProcessState()
    }
}

async function stopInstance() {
    const instance = selectedInstance.value
    if (!instance) return

    await kill(instance.id).catch(handleError)
    await refreshProcessState()
    trackEvent('InstanceStop', {
        loader: instance.loader,
        game_version: instance.game_version,
        source: 'HomeMinimal',
    })
}

async function installInstance() {
    const instance = selectedInstance.value
    if (!instance) return

    loading.value = true
    try {
        if (
            instance.install_stage !== 'pack_installed' &&
            (instance.link?.type === 'modrinth_modpack' ||
                instance.link?.type === 'server_project_modpack')
        ) {
            await install_pack_to_existing_instance(instance.id, {
                type: 'fromVersionId',
                project_id: instance.link.project_id ?? instance.link.server_project_id ?? '',
                version_id: instance.link.version_id ?? instance.link.content_version_id ?? '',
                title: instance.name,
            })
        } else {
            await install_existing_instance(instance.id, false)
        }
    } catch (error) {
        handleError(error)
    } finally {
        loading.value = false
    }
}

watch(
    () => props.selectedInstanceId,
    () => {
        currentEvent.value = null
    },
)

let disposed = false
let unlistenProcess: (() => void) | undefined
void process_listener((event: { instance_id: string; event: string }) => {
    if (disposed || event.instance_id !== selectedInstance.value?.id) return
    currentEvent.value = event.event
    void refreshProcessState()
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
</script>

<template>
    <section
        data-onboarding-id="home-instances"
        class="minimal-home-stage flex min-w-0 items-center justify-center px-6 pb-14 pt-8"
    >
        <div class="flex w-full max-w-3xl flex-col items-center text-center">
            <HomeGreeting :player-name="playerName" variant="minimal" />

            <template v-if="selectedInstance">
                <Card class="mb-0 mt-10 w-full text-left">
                    <div
                        class="grid min-w-0 grid-cols-1 items-center gap-5 sm:grid-cols-[minmax(0,1fr)_auto]"
                    >
                        <router-link
                            :to="`/instance/${encodeURIComponent(selectedInstance.id)}`"
                            class="group flex min-w-0 items-center gap-5 rounded-lg text-inherit no-underline focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-shadow"
                        >
                            <InstanceIcon
                                class="size-20 shrink-0 transition-transform group-hover:scale-[1.03]"
                                :icon-path="selectedInstance.icon_path"
                                :instance-id="selectedInstance.id"
                                :loader="selectedInstance.loader"
                            />
                            <div class="flex min-w-0 flex-1 flex-col gap-1.5">
                                <h2
                                    class="m-0 truncate text-xl font-bold text-[var(--color-text-primary)] group-hover:underline"
                                >
                                    {{ selectedInstance.name }}
                                </h2>
                                <div
                                    class="flex min-w-0 flex-wrap gap-x-4 gap-y-1 text-sm text-[var(--color-text-tertiary)]"
                                >
                                    <span class="flex min-w-0 items-center gap-1.5 capitalize">
                                        <GameIcon class="size-4 shrink-0" aria-hidden="true" />
                                        <span class="truncate">
                                            {{ selectedInstance.loader }}
                                            {{ selectedInstance.game_version }}
                                        </span>
                                    </span>
                                    <span class="flex min-w-0 items-center gap-1.5">
                                        <TimerIcon class="size-4 shrink-0" aria-hidden="true" />
                                        <span class="truncate">{{ lastPlayed }}</span>
                                    </span>
                                </div>
                            </div>
                        </router-link>

                        <div class="flex min-h-11 shrink-0 items-center justify-end gap-2">
                            <Button
                                v-if="processQuery.isError.value && !running"
                                size="xl"
                                class="w-36 justify-center"
                                @click="refreshProcessState()"
                                >{{ formatMessage(commonMessages.retryButton) }}</Button
                            >
                            <Button
                                v-else-if="running"
                                type="colored"
                                color="red"
                                size="xl"
                                class="w-36 justify-center"
                                @click="stopInstance"
                                ><StopCircleIcon aria-hidden="true" />
                                <span class="truncate">{{
                                    formatMessage(commonMessages.stopButton)
                                }}</span>
                            </Button>
                            <Button v-else-if="busy" size="xl" class="w-36 justify-center" disabled
                                ><SpinnerIcon class="animate-spin" aria-hidden="true" />
                                <span class="truncate">
                                    {{
                                        formatMessage(
                                            installing
                                                ? commonMessages.installingLabel
                                                : messages.loading,
                                        )
                                    }}
                                </span>
                            </Button>
                            <Button
                                v-else-if="installed"
                                type="colored"
                                color="brand"
                                size="xl"
                                class="w-36 justify-center"
                                @click="playInstance"
                                ><PlayIcon aria-hidden="true" />
                                <span class="truncate">{{
                                    formatMessage(commonMessages.playButton)
                                }}</span>
                            </Button>
                            <Button
                                v-else
                                v-tooltip="
                                    offline
                                        ? formatMessage(messages.offlineInstalledOnly)
                                        : undefined
                                "
                                type="colored"
                                color="brand"
                                size="xl"
                                class="w-36 justify-center"
                                :disabled="offline"
                                @click="installInstance"
                                ><DownloadIcon aria-hidden="true" />
                                <span class="truncate">{{
                                    formatMessage(commonMessages.installButton)
                                }}</span>
                            </Button>

                            <Button
                                v-tooltip="formatMessage(messages.changeInstance)"
                                type="quiet"
                                size="xl"
                                circular
                                icon-only
                                :aria-label="formatMessage(messages.changeInstance)"
                                @click="emit('choose')"
                                ><ListIcon aria-hidden="true" />
                            </Button>
                        </div>
                    </div>
                </Card>
            </template>

            <template v-else>
                <Card class="mb-0 mt-10 w-full text-left">
                    <div class="flex min-w-0 flex-wrap items-center gap-4">
                        <div
                            class="flex size-16 shrink-0 items-center justify-center rounded-lg bg-surface-4 text-[var(--color-text-tertiary)]"
                        >
                            <ListIcon class="size-7" aria-hidden="true" />
                        </div>
                        <div class="min-w-48 flex-1">
                            <h2 class="m-0 text-lg font-bold text-[var(--color-text-primary)]">
                                {{
                                    formatMessage(
                                        instances.length > 0
                                            ? messages.chooseInstance
                                            : messages.noInstances,
                                    )
                                }}
                            </h2>
                        </div>
                        <Button
                            type="colored"
                            color="brand"
                            size="xl"
                            @click="instances.length > 0 ? emit('choose') : emit('create')"
                        >
                            <ListIcon v-if="instances.length > 0" aria-hidden="true" />
                            <PlusIcon v-else aria-hidden="true" />
                            {{
                                formatMessage(
                                    instances.length > 0
                                        ? messages.chooseInstance
                                        : messages.createInstance,
                                )
                            }}
                        </Button>
                    </div>
                </Card>
            </template>
        </div>
    </section>
</template>

<style scoped>
.minimal-home-stage {
    min-height: calc(100vh - var(--top-bar-height) - 4rem);
}
</style>
