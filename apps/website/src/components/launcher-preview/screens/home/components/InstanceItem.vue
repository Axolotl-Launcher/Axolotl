<script setup lang="ts">
import EyeIcon from '@modrinth/assets/icons/eye.svg?component'
import FolderOpenIcon from '@modrinth/assets/icons/folder-open.svg?component'
import MoreVerticalIcon from '@modrinth/assets/icons/more-vertical.svg?component'
import PlayIcon from '@modrinth/assets/icons/play.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import StopCircleIcon from '@modrinth/assets/icons/stop-circle.svg?component'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import OverflowMenu from '@modrinth/ui/src/components/base/OverflowMenu.vue'
import SmartClickable from '@modrinth/ui/src/components/base/SmartClickable.vue'
import { useFormatDateTime } from '@modrinth/ui/src/composables/format-date-time.ts'
import { useRelativeTime } from '@modrinth/ui/src/composables/how-ago.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import { loadProject, projectIcon } from '../../../data'
import { instanceContentProject } from '../../../lib/instance-project'
import type { GameInstance } from '../../../lib/types'
import { injectPreviewWindow } from '../../../lib/window'
import { homeMessages } from '../messages'
import { useHomeRuntime } from '../runtime'
import InstanceIcon from './InstanceIcon.vue'

/** Port of `components/ui/world/InstanceItem.vue`: a recently played instance row. */
const props = defineProps<{
    instance: GameInstance
    lastPlayed: string | null
    flat?: boolean
    playing?: boolean
    dashboardDensity?: 'compact' | 'comfortable'
}>()

const emit = defineEmits<{
    (e: 'play' | 'stop'): void
}>()

const { formatMessage } = useVIntl()
const formatRelativeTime = useRelativeTime()
const formatDateTime = useFormatDateTime({
    timeStyle: 'short',
    dateStyle: 'long',
})
const { handleError } = injectNotificationManager()
const { layer } = injectPreviewWindow()
const router = useRouter()
const runtime = useHomeRuntime()
const messages = homeMessages

const linkedProject = computed(() => instanceContentProject(props.instance.link))
const modpack = ref<{ title: string; iconUrl: string } | null>(null)
const loadingModpack = ref(false)

/**
 * The `instance-card-project` query: `resourceProjectDocuments` reads the
 * project's metadata from its source. The preview has Modrinth's snapshots;
 * other sources are not part of it, as if the request found no document.
 */
watch(
    () =>
        linkedProject.value
            ? `${linkedProject.value.source}:${linkedProject.value.projectId}`
            : null,
    async (key) => {
        const project = linkedProject.value
        modpack.value = null
        loadingModpack.value = !!project
        if (!project) return
        const metadata =
            project.source === 'modrinth'
                ? await loadProject(project.projectId)
                      .then((detail) => ({ title: detail.title, iconUrl: projectIcon(detail.id) }))
                      .catch(() => null)
                : null
        const current = linkedProject.value
        if (!current || `${current.source}:${current.projectId}` !== key) return
        modpack.value = metadata
        loadingModpack.value = false
    },
    { immediate: true },
)
const modpackRoute = computed(() =>
    linkedProject.value?.source === 'curseforge'
        ? `/project/curseforge/${linkedProject.value.projectId}`
        : `/project/${linkedProject.value?.projectId}`,
)

const loader = computed(() => {
    if (props.instance.loader === 'vanilla') return 'Minecraft'
    if (props.instance.loader === 'neoforge') return 'NeoForge'
    const name: string = props.instance.loader
    return name ? name.charAt(0).toUpperCase() + name.slice(1) : name
})

const loading = ref(false)
const isPlaying = computed(() => props.playing ?? false)

async function play(event: MouseEvent) {
    event?.stopPropagation()
    loading.value = true
    await runtime.run(props.instance).catch(handleError)
    emit('play')
    loading.value = false
}

async function stop(event: MouseEvent) {
    event?.stopPropagation()
    loading.value = true
    await runtime.kill(props.instance.id).catch(handleError)
    emit('stop')
    loading.value = false
}

/*
 * The app reads `instance.lastPlayed` for this tooltip, which an instance does
 * not have, so the row never shows one; kept as it behaves.
 */
const lastPlayedTooltip = computed(() => {
    const value = (props.instance as GameInstance & { lastPlayed?: string }).lastPlayed
    return value ? formatDateTime(value) : null
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
            class="smart-clickable:highlight-on-hover grid grid-cols-[auto_minmax(0,3fr)_minmax(0,4fr)_auto] items-center gap-2 rounded-lg"
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
            <div class="col-span-2 flex h-full flex-col justify-between">
                <div class="flex items-center gap-2">
                    <div
                        class="smart-clickable:underline-on-hover truncate text-lg font-bold text-[var(--color-text-primary)]"
                    >
                        {{ instance.name }}
                    </div>
                </div>
                <div class="flex items-center gap-2 text-sm text-[var(--color-text-tertiary)]">
                    <div
                        v-tooltip="lastPlayedTooltip"
                        class="w-fit shrink-0"
                        :class="{ 'smart-clickable:allow-pointer-events cursor-help': lastPlayed }"
                    >
                        <template v-if="lastPlayed">
                            {{
                                formatMessage(commonMessages.playedLabel, {
                                    ago: formatRelativeTime(lastPlayed),
                                })
                            }}
                        </template>
                        <template v-else>
                            {{ formatMessage(messages.instanceItemNotPlayedYet) }}
                        </template>
                    </div>
                    <span v-if="dashboardDensity !== 'compact'" aria-hidden="true">•</span>
                    <span
                        v-if="modpack"
                        class="flex items-center gap-1 truncate text-[var(--color-text-tertiary)]"
                    >
                        <router-link
                            class="smart-clickable:allow-pointer-events inline-flex items-center gap-1 truncate text-[var(--color-text-tertiary)] hover:underline"
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
                        <SpinnerIcon class="shrink-0 animate-spin" />
                        <span class="truncate">{{
                            formatMessage(messages.instanceItemLoadingModpack)
                        }}</span>
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
            <div class="smart-clickable:allow-pointer-events flex justify-end gap-1">
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
                        v-tooltip="
                            isPlaying ? formatMessage(messages.instanceItemAlreadyOpen) : null
                        "
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
                            action: () => runtime.showInstanceInFolder(instance.id),
                        },
                    ]"
                    :container="layer ?? undefined"
                >
                    <MoreVerticalIcon aria-hidden="true" />
                    <template #open-instance>
                        <EyeIcon aria-hidden="true" />
                        {{ formatMessage(messages.instanceItemViewInstance) }}
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
