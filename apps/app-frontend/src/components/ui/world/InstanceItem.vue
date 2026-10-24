<script setup lang="ts">
import {
    EyeIcon,
    FolderOpenIcon,
    MoreVerticalIcon,
    PlayIcon,
    SpinnerIcon,
    StopCircleIcon,
} from '@modrinth/assets'
import {
    Avatar,
    Button,
    commonMessages,
    defineMessages,
    injectNotificationManager,
    OverflowMenu,
    SmartClickable,
    useFormatDateTime,
    useRelativeTime,
    useVIntl,
} from '@modrinth/ui'
import { capitalizeString } from '@modrinth/utils'
import { useQuery } from '@tanstack/vue-query'
import type { Dayjs } from 'dayjs'
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'

import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import { useMinecraftLaunchError } from '@/composables/useMinecraftLaunchError'
import { trackEvent } from '@/helpers/analytics'
import { process_listener } from '@/helpers/events'
import { kill, run } from '@/helpers/instance'
import { instanceContentProject } from '@/helpers/instance-project'
import { get_by_instance_id } from '@/helpers/process'
import { resourceProjectDocuments } from '@/helpers/resources'
import type { GameInstance } from '@/helpers/types'
import { showInstanceInFolder } from '@/helpers/utils'
import { handleSevereError } from '@/store/error'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const handleMinecraftLaunchError = useMinecraftLaunchError()
const formatRelativeTime = useRelativeTime()

const messages = defineMessages({
    notPlayedYet: { id: 'app.instance-item.not-played-yet', defaultMessage: 'Not played yet' },
    loadingModpack: {
        id: 'app.instance-item.loading-modpack',
        defaultMessage: 'Loading modpack...',
    },
    viewInstance: { id: 'app.instance-item.view-instance', defaultMessage: 'View instance' },
    alreadyOpen: {
        id: 'app.instance-item.already-open',
        defaultMessage: 'Instance is already open',
    },
})
const formatDateTime = useFormatDateTime({
    timeStyle: 'short',
    dateStyle: 'long',
})

const router = useRouter()

const emit = defineEmits<{
    (e: 'play' | 'stop'): void
}>()

const props = defineProps<{
    instance: GameInstance
    lastPlayed: Dayjs
    flat?: boolean
    playing?: boolean
    dashboardDensity?: 'compact' | 'comfortable'
}>()

const linkedProject = computed(() => instanceContentProject(props.instance.link))
const { data: modpack, isLoading: loadingModpack } = useQuery({
    queryKey: computed(() => ['instance-card-project', linkedProject.value]),
    enabled: computed(() => !!linkedProject.value),
    queryFn: async ({ signal }) => {
        const project = linkedProject.value!
        const [document] = await resourceProjectDocuments(
            project.source,
            [project.projectId],
            signal,
        )
        return document?.metadata ?? null
    },
})
const modpackRoute = computed(() =>
    linkedProject.value?.source === 'curseforge'
        ? `/project/curseforge/${linkedProject.value.projectId}`
        : `/project/${linkedProject.value?.projectId}`,
)

const loader = computed(() => {
    if (props.instance.loader === 'vanilla') {
        return 'Minecraft'
    } else if (props.instance.loader === 'neoforge') {
        return 'NeoForge'
    } else {
        return capitalizeString(props.instance.loader)
    }
})

const loading = ref(false)
const internalPlaying = ref(false)
const isPlaying = computed(() => props.playing ?? internalPlaying.value)

const play = async (event: MouseEvent) => {
    event?.stopPropagation()
    loading.value = true
    await run(props.instance.id)
        .catch(async (err) => {
            const handled = await handleMinecraftLaunchError(err, {
                instance_id: props.instance.id,
                instance_name: props.instance.name,
            })
            if (!handled) handleSevereError(err, { instanceId: props.instance.id })
        })
        .finally(() => {
            trackEvent('InstanceStart', {
                loader: props.instance.loader,
                game_version: props.instance.game_version,
                source: 'InstanceItem',
            })
        })
    emit('play')
    loading.value = false
}

const stop = async (event: MouseEvent) => {
    event?.stopPropagation()
    loading.value = true
    await kill(props.instance.id).catch(handleError)
    trackEvent('InstanceStop', {
        loader: props.instance.loader,
        game_version: props.instance.game_version,
        source: 'InstanceItem',
    })
    emit('stop')
    loading.value = false
}

const unlistenProcesses =
    props.playing === undefined
        ? await process_listener(async () => {
              await checkProcess()
          })
        : () => undefined

const checkProcess = async () => {
    if (props.playing !== undefined) return
    const runningProcesses = await get_by_instance_id(props.instance.id).catch(handleError)

    internalPlaying.value = runningProcesses.length > 0
}

onMounted(() => {
    checkProcess()
})

onUnmounted(() => {
    unlistenProcesses()
})
</script>
<template>
    <SmartClickable>
        <template #clickable>
            <router-link
                class="no-click-animation"
                :to="`/instance/${encodeURIComponent(instance.id)}`"
            />
        </template>
        <div
            class="grid grid-cols-[auto_minmax(0,3fr)_minmax(0,4fr)_auto] items-center gap-2 rounded-lg smart-clickable:highlight-on-hover"
            :class="[
                flat ? 'px-2 py-2 hover:bg-surface-4' : 'card-shadow bg-surface-2 p-3',
                {
                    'instance-item-dashboard-compact grid grid-cols-[auto_minmax(0,1fr)_auto] gap-2 p-1.5':
                        dashboardDensity === 'compact',
                    'instance-item-dashboard-comfortable p-2': dashboardDensity === 'comfortable',
                },
            ]"
        >
            <InstanceIcon
                :icon-path="instance.icon_path"
                :instance-id="instance.id"
                :loader="instance.loader"
                :size="dashboardDensity === 'compact' ? '40px' : '48px'"
            />
            <div class="flex flex-col col-span-2 justify-between h-full">
                <div class="flex items-center gap-2">
                    <div
                        class="text-lg text-[var(--color-text-primary)] font-bold truncate smart-clickable:underline-on-hover"
                    >
                        {{ instance.name }}
                    </div>
                </div>
                <div class="flex items-center gap-2 text-sm text-[var(--color-text-tertiary)]">
                    <div
                        v-tooltip="instance.lastPlayed ? formatDateTime(instance.lastPlayed) : null"
                        class="w-fit shrink-0"
                        :class="{ 'cursor-help smart-clickable:allow-pointer-events': lastPlayed }"
                    >
                        <template v-if="lastPlayed">
                            {{
                                formatMessage(commonMessages.playedLabel, {
                                    ago: formatRelativeTime(lastPlayed.toISOString?.()),
                                })
                            }}
                        </template>
                        <template v-else> {{ formatMessage(messages.notPlayedYet) }} </template>
                    </div>
                    <span v-if="dashboardDensity !== 'compact'" aria-hidden="true">•</span>
                    <span
                        v-if="modpack"
                        class="flex items-center gap-1 truncate text-[var(--color-text-tertiary)]"
                    >
                        <router-link
                            class="inline-flex items-center gap-1 truncate hover:underline text-[var(--color-text-tertiary)] smart-clickable:allow-pointer-events"
                            :to="modpackRoute"
                        >
                            <Avatar :src="modpack.iconUrl" size="16px" class="shrink-0" />
                            <span class="truncate">{{ modpack.title }}</span>
                        </router-link>
                        ({{ loader }} {{ instance.game_version }})
                    </span>
                    <span
                        v-else-if="loadingModpack"
                        class="flex items-center gap-1 truncate text-[var(--color-text-tertiary)]"
                    >
                        <SpinnerIcon class="animate-spin shrink-0" />
                        <span class="truncate">{{ formatMessage(messages.loadingModpack) }}</span>
                    </span>
                    <span
                        v-else
                        class="flex items-center gap-1 truncate text-[var(--color-text-tertiary)]"
                    >
                        {{ loader }}
                        {{ instance.game_version }}
                    </span>
                </div>
            </div>
            <div class="flex gap-1 justify-end smart-clickable:allow-pointer-events">
                <Button
                    v-if="isPlaying && !loading"
                    color="red"
                    :circular="dashboardDensity === 'compact'"
                >
                    <button @click="stop">
                        <StopCircleIcon aria-hidden="true" />
                        <span v-if="dashboardDensity !== 'compact'">
                            {{ formatMessage(commonMessages.stopButton) }}
                        </span>
                    </button>
                </Button>
                <Button v-else :circular="dashboardDensity === 'compact'">
                    <button
                        v-tooltip="isPlaying ? formatMessage(messages.alreadyOpen) : null"
                        :disabled="isPlaying || loading"
                        @click="play"
                    >
                        <SpinnerIcon v-if="loading" class="animate-spin" />
                        <PlayIcon v-else aria-hidden="true" />
                        <span v-if="dashboardDensity !== 'compact'">
                            {{ formatMessage(commonMessages.playButton) }}
                        </span>
                    </button>
                </Button>
                <OverflowMenu
                    type="button"
                    class="relative inline-flex size-9 min-w-9 shrink-0 cursor-pointer items-center justify-center rounded-full border border-solid border-transparent bg-transparent p-0 text-[var(--color-text-default)] transition-[background-color,color,filter,transform] duration-150 hover:bg-surface-4 hover:brightness-[--hover-brightness] active:scale-[0.97] [&>svg]:size-5"
                    :options="[
                        {
                            id: 'open-instance',
                            shown: !!instance.id,
                            action: () => router.push(encodeURI(`/instance/${instance.id}`)),
                        },
                        {
                            id: 'open-folder',
                            action: () => showInstanceInFolder(instance.id),
                        },
                    ]"
                >
                    <MoreVerticalIcon aria-hidden="true" />
                    <template #open-instance>
                        <EyeIcon aria-hidden="true" />
                        {{ formatMessage(messages.viewInstance) }}
                    </template>
                    <template #open-folder>
                        <FolderOpenIcon aria-hidden="true" />
                        {{ formatMessage(commonMessages.openFolderButton) }}
                    </template>
                </OverflowMenu>
            </div>
        </div>
    </SmartClickable>
</template>

<style scoped>
.instance-item-dashboard-compact > :nth-child(2) {
    grid-column: auto;
}

.instance-item-dashboard-compact > :nth-child(2) > :first-child > :first-child {
    font-size: 0.875rem;
}

.instance-item-dashboard-compact > :nth-child(2) > :last-child {
    font-size: 0.7rem;
}

.instance-item-dashboard-compact > :last-child {
    gap: 0.125rem;
}
</style>
