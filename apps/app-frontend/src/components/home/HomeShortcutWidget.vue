<script setup lang="ts">
import {
    BoxIcon,
    GameIcon,
    IssuesIcon,
    NoSignalIcon,
    PlayIcon,
    RefreshCwIcon,
    ServerIcon,
    SignalIcon,
    SpinnerIcon,
    StopCircleIcon,
    TimerIcon,
    TrashIcon,
} from '@modrinth/assets'
import {
    Avatar,
    Button,
    commonMessages,
    defineMessages,
    GAME_MODES,
    injectNotificationManager,
    useRelativeTime,
    useVIntl,
} from '@modrinth/ui'
import dayjs from 'dayjs'
import { computed, ref, watch } from 'vue'

import type { HomeWidgetPlacement, HomeWidgetSize } from '@/components/home/home-dashboard'
import { useHomeDashboardRuntime } from '@/components/home/home-dashboard-runtime'
import HomeWidgetFrame from '@/components/home/HomeWidgetFrame.vue'
import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import { useMinecraftLaunchError } from '@/composables/useMinecraftLaunchError'
import { trackEvent } from '@/helpers/analytics'
import { kill, run } from '@/helpers/instance'
import { instanceLaunchStartedAt } from '@/helpers/instance-launch-state'
import type { GameInstance } from '@/helpers/types'
import {
    getWorldIdentifier,
    hasServerQuickPlaySupport,
    hasWorldQuickPlaySupport,
    normalizeServerAddress,
    start_join_server,
    start_join_singleplayer_world,
    type World,
} from '@/helpers/worlds'
import { handleSevereError } from '@/store/error'

const props = defineProps<{
    placement: HomeWidgetPlacement
    instances: GameInstance[]
    dashboardSize: HomeWidgetSize
}>()

const emit = defineEmits<{
    /** The target is gone: pick another one for this widget, or remove it. */
    replace: []
    remove: []
}>()

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const formatRelativeTime = useRelativeTime()
const handleMinecraftLaunchError = useMinecraftLaunchError()
const runtime = useHomeDashboardRuntime()
const { gameVersions, runningInstanceIds } = runtime
const world = ref<World | null>(null)
const starting = ref(false)
const loadingTarget = ref(false)
let targetRequestId = 0

const messages = defineMessages({
    unavailable: {
        id: 'app.home.widgets.unavailable',
        defaultMessage: 'Content unavailable',
    },
    missingHint: {
        id: 'app.home.shortcut.missing-hint',
        defaultMessage: 'It was deleted or moved. Pick another one or remove this widget.',
    },
    replace: { id: 'app.home.widgets.replace', defaultMessage: 'Replace target' },
    remove: { id: 'app.home.widgets.remove', defaultMessage: 'Remove widget' },
    played: { id: 'app.instance.played', defaultMessage: 'Played {time}' },
    neverPlayed: { id: 'app.instance.never-played', defaultMessage: 'Never played' },
    instance: { id: 'app.home.shortcut.kind.instance', defaultMessage: 'Instance' },
    world: { id: 'app.home.shortcut.kind.world', defaultMessage: 'World' },
    server: { id: 'app.home.shortcut.kind.server', defaultMessage: 'Server' },
    offline: { id: 'app.home.shortcut.server.offline', defaultMessage: 'Server offline' },
    playersOnline: {
        id: 'app.home.shortcut.server.players-online',
        defaultMessage: '{count} online',
    },
    hardcore: { id: 'instance.worlds.hardcore', defaultMessage: 'Hardcore mode' },
    noServerQuickPlay: {
        id: 'instance.worlds.no_server_quick_play',
        defaultMessage: 'You can only jump straight into servers on Minecraft Alpha 1.0.5+',
    },
    noWorldQuickPlay: {
        id: 'instance.worlds.no_singleplayer_quick_play',
        defaultMessage: 'You can only jump straight into singleplayer worlds on Minecraft 1.20+',
    },
})

const instance = computed(() =>
    props.instances.find((candidate) => candidate.id === props.placement.target?.instanceId),
)
const missing = computed(
    () => !instance.value || (props.placement.kind !== 'instance' && !world.value),
)
const serverData = computed(() =>
    instance.value && world.value?.type === 'server'
        ? runtime.getServerData(instance.value.id, world.value.address)
        : undefined,
)
const isRunning = computed(() =>
    instance.value ? runningInstanceIds.value.includes(instance.value.id) : false,
)
const versionLabel = computed(() => {
    if (!instance.value) return ''
    const loader = instance.value.loader === 'vanilla' ? 'Minecraft' : instance.value.loader
    return `${loader} ${instance.value.game_version}`
})
const lastPlayedLabel = computed(() => {
    const lastPlayed = world.value?.last_played ?? instance.value?.last_played
    return lastPlayed
        ? formatMessage(messages.played, {
              time: formatRelativeTime(dayjs(lastPlayed).toISOString()),
          })
        : formatMessage(messages.neverPlayed)
})
const shortcutTitle = computed(
    () =>
        (props.placement.kind === 'instance' ? instance.value?.name : world.value?.name) ??
        props.placement.target?.fallbackLabel ??
        '',
)
const shortcutRoute = computed(() => {
    if (!instance.value) return '/'
    if (!world.value) return `/instance/${encodeURIComponent(instance.value.id)}`
    return `/instance/${encodeURIComponent(instance.value.id)}/worlds?highlight=${encodeURIComponent(getWorldIdentifier(world.value))}`
})
const shortcutIcon = computed(() => {
    if (!world.value) return undefined
    return world.value.type === 'server'
        ? (serverData.value?.status?.favicon ?? world.value.icon)
        : world.value.icon
})
const iconSize = computed(() => (props.dashboardSize === '2x1' ? '104px' : '64px'))
const serverOffline = computed(
    () =>
        world.value?.type === 'server' &&
        !serverData.value?.refreshing &&
        !serverData.value?.status,
)
const playDisabled = computed(
    () =>
        starting.value ||
        (instance.value !== undefined &&
            instanceLaunchStartedAt(instance.value.id) !== undefined) ||
        !supportsQuickPlay.value,
)
const launching = computed(
    () =>
        starting.value ||
        (instance.value !== undefined && instanceLaunchStartedAt(instance.value.id) !== undefined),
)
const kindLabel = computed(() =>
    formatMessage(
        props.placement.kind === 'instance'
            ? messages.instance
            : props.placement.kind === 'world'
              ? messages.world
              : messages.server,
    ),
)
const kindIcon = computed(() =>
    props.placement.kind === 'instance'
        ? BoxIcon
        : props.placement.kind === 'world'
          ? GameIcon
          : ServerIcon,
)
const primaryLabel = computed(() => {
    if (!world.value) return versionLabel.value
    if (world.value.type === 'singleplayer') {
        return world.value.hardcore
            ? formatMessage(messages.hardcore)
            : formatMessage(GAME_MODES[world.value.game_mode].message)
    }
    if (serverData.value?.refreshing) return formatMessage(commonMessages.loadingLabel)
    if (!serverData.value?.status) return formatMessage(messages.offline)
    return formatMessage(messages.playersOnline, {
        count: serverData.value.status.players?.online ?? 0,
    })
})
const secondaryLabel = computed(() => {
    if (world.value?.type === 'server') return world.value.address
    if (world.value) return `${instance.value?.name ?? ''} · ${lastPlayedLabel.value}`
    return lastPlayedLabel.value
})
const statusIcon = computed(() => {
    if (world.value?.type !== 'server') return world.value ? GameIcon : TimerIcon
    return serverData.value?.status ? SignalIcon : NoSignalIcon
})
const supportsQuickPlay = computed(() => {
    if (!world.value || !instance.value) return true
    return world.value.type === 'server'
        ? hasServerQuickPlaySupport(gameVersions.value, instance.value.game_version)
        : hasWorldQuickPlaySupport(gameVersions.value, instance.value.game_version)
})
const playTooltip = computed(() => {
    if (supportsQuickPlay.value) return formatMessage(commonMessages.playButton)
    return formatMessage(
        world.value?.type === 'server' ? messages.noServerQuickPlay : messages.noWorldQuickPlay,
    )
})

async function refreshTarget(force = false) {
    const requestId = ++targetRequestId
    world.value = null
    loadingTarget.value = false
    const target = props.placement.target
    const kind = props.placement.kind
    if (!target || kind === 'instance' || !instance.value) return

    loadingTarget.value = true
    try {
        const available = await runtime.getInstanceWorlds(target.instanceId, force)
        if (requestId !== targetRequestId) return

        const normalizedTargetAddress = target.address
            ? normalizeServerAddress(target.address)
            : undefined
        world.value =
            available.find((candidate) =>
                candidate.type === 'server'
                    ? kind === 'server' &&
                      normalizeServerAddress(candidate.address) === normalizedTargetAddress
                    : kind === 'world' && candidate.path === target.path,
            ) ?? null

        if (world.value?.type === 'server') {
            await runtime.refreshServer(target.instanceId, world.value.address, force)
        }
    } finally {
        if (requestId === targetRequestId) loadingTarget.value = false
    }
}

async function playInstance(targetInstance: GameInstance) {
    starting.value = true
    try {
        await run(targetInstance.id)
        trackEvent('InstanceStart', {
            loader: targetInstance.loader,
            game_version: targetInstance.game_version,
            source: 'HomeInstanceWidget',
        })
    } catch (error) {
        const handled = await handleMinecraftLaunchError(error, {
            instance_id: targetInstance.id,
            instance_name: targetInstance.name,
        })
        if (!handled) handleSevereError(error, { instanceId: targetInstance.id })
    } finally {
        starting.value = false
    }
}

async function playWorld() {
    if (!instance.value || !world.value) return
    starting.value = true
    try {
        if (world.value.type === 'server') {
            await start_join_server(instance.value.id, world.value.address)
        } else {
            await start_join_singleplayer_world(instance.value.id, world.value.path)
        }
        trackEvent('InstanceStart', {
            loader: instance.value.loader,
            game_version: instance.value.game_version,
            source: 'HomeShortcutWidget',
        })
    } catch (error) {
        const handled = await handleMinecraftLaunchError(error, {
            instance_id: instance.value.id,
            instance_name: instance.value.name,
        })
        if (!handled) handleSevereError(error, { instanceId: instance.value.id })
    } finally {
        starting.value = false
    }
}

async function playShortcut() {
    if (!instance.value) return
    if (world.value) await playWorld()
    else await playInstance(instance.value)
}

async function stopInstance() {
    if (!instance.value) return
    await kill(instance.value.id).catch(handleError)
}

watch(
    () => [props.placement, props.instances, runtime.instanceRevision.value] as const,
    (_, previous) => refreshTarget(previous !== undefined),
    {
        immediate: true,
        deep: true,
    },
)
</script>

<template>
    <div
        class="home-shortcut-widget min-w-0 min-h-0 h-full"
        :data-size="dashboardSize"
        :data-kind="placement.kind"
    >
        <div
            v-if="loadingTarget"
            class="box-border flex h-full min-h-0 min-w-0 items-center gap-3 p-4"
            role="status"
            :aria-label="formatMessage(commonMessages.loadingLabel)"
        >
            <div class="home-skeleton size-11 shrink-0 !rounded-[var(--radius-md)]" />
            <div class="flex min-w-0 flex-1 flex-col gap-2">
                <div class="home-skeleton h-3.5 w-3/4" />
                <div class="home-skeleton h-2.5 w-1/2" />
            </div>
        </div>
        <HomeWidgetFrame
            v-else-if="missing"
            class="home-shortcut-missing box-border h-full p-4"
            :title="placement.target?.fallbackLabel || kindLabel"
            :icon="kindIcon"
            dense
            :empty-text="formatMessage(messages.unavailable)"
            :empty-hint="formatMessage(messages.missingHint)"
            empty-tone="warning"
            :empty-icon="IssuesIcon"
        >
            <template #empty-action>
                <Button size="sm" type="quiet" color="brand" @click="emit('replace')">
                    <RefreshCwIcon aria-hidden="true" />
                    {{ formatMessage(messages.replace) }}
                </Button>
                <!-- A 1x1 tile keeps one action; removing stays in the edit menu. -->
                <Button
                    v-tooltip="formatMessage(messages.remove)"
                    class="home-shortcut-remove"
                    size="sm"
                    circular
                    icon-only
                    type="quiet"
                    :aria-label="formatMessage(messages.remove)"
                    @click="emit('remove')"
                >
                    <TrashIcon aria-hidden="true" />
                </Button>
            </template>
        </HomeWidgetFrame>
        <div v-else class="home-shortcut-card" :class="{ 'is-offline': serverOffline }">
            <router-link
                class="home-shortcut-cover"
                :to="shortcutRoute"
                tabindex="-1"
                aria-hidden="true"
            >
                <Avatar v-if="shortcutIcon" :src="shortcutIcon" :size="iconSize" />
                <InstanceIcon
                    v-else-if="instance"
                    :icon-path="instance.icon_path"
                    :instance-id="instance.id"
                    :loader="instance.loader"
                    :size="iconSize"
                />
            </router-link>
            <span class="home-shortcut-kind">
                <component :is="kindIcon" aria-hidden="true" />
                {{ kindLabel }}
            </span>
            <span class="home-shortcut-copy">
                <router-link class="home-shortcut-title" :to="shortcutRoute">{{
                    shortcutTitle
                }}</router-link>
                <span class="home-shortcut-meta" :data-state="serverOffline ? 'offline' : null">
                    <SpinnerIcon
                        v-if="world?.type === 'server' && serverData?.refreshing"
                        class="animate-spin"
                        aria-hidden="true"
                    />
                    <component :is="statusIcon" v-else aria-hidden="true" />
                    <span class="truncate">{{ primaryLabel }}</span>
                </span>
                <span class="home-shortcut-meta home-shortcut-secondary">
                    <TimerIcon v-if="world?.type !== 'server'" aria-hidden="true" />
                    <ServerIcon v-else aria-hidden="true" />
                    <span class="truncate">{{ secondaryLabel }}</span>
                </span>
            </span>

            <div class="home-shortcut-action">
                <Button
                    v-if="isRunning"
                    v-tooltip="formatMessage(commonMessages.stopButton)"
                    type="colored"
                    color="red"
                    size="lg"
                    :circular="dashboardSize !== '2x1'"
                    :icon-only="dashboardSize !== '2x1'"
                    :aria-label="formatMessage(commonMessages.stopButton)"
                    @click="stopInstance"
                    ><StopCircleIcon />
                    <span v-if="dashboardSize === '2x1'">{{
                        formatMessage(commonMessages.stopButton)
                    }}</span>
                </Button>
                <Button
                    v-else
                    v-tooltip="playTooltip"
                    type="colored"
                    color="brand"
                    size="lg"
                    :circular="dashboardSize !== '2x1'"
                    :icon-only="dashboardSize !== '2x1'"
                    :aria-label="playTooltip"
                    :disabled="playDisabled"
                    @click="playShortcut"
                    ><SpinnerIcon v-if="launching" class="animate-spin" />
                    <PlayIcon v-else />
                    <span v-if="dashboardSize === '2x1'">{{
                        formatMessage(commonMessages.playButton)
                    }}</span>
                </Button>
            </div>
        </div>
    </div>
</template>

<style scoped>
@container (max-width: 24rem) {
    .home-shortcut-remove {
        display: none;
    }
}

/*
 * The target's icon is the card: it sits large in the corner on the plain
 * widget surface, in its own colours, and the one button to start it has a
 * fixed place. Large washes of colour are kept for the launch card.
 */
.home-shortcut-card {
    @apply relative grid h-full min-h-0 min-w-0 gap-x-3 gap-y-2 overflow-hidden p-4;
    grid-template-columns: auto minmax(0, 1fr) auto;
    grid-template-rows: minmax(0, 1fr) auto;
    grid-template-areas:
        'cover . kind'
        'copy copy action';
}

.home-shortcut-cover {
    grid-area: cover;
    @apply flex self-start;
    border-radius: var(--radius-lg);
}

.home-shortcut-cover :deep(img),
.home-shortcut-cover :deep(.avatar) {
    border-radius: var(--radius-lg);
    box-shadow: 0 8px 22px color-mix(in srgb, var(--surface-1-opaque) 45%, transparent);
    transition: transform 180ms ease;
}

.home-shortcut-card:hover .home-shortcut-cover :deep(img),
.home-shortcut-card:hover .home-shortcut-cover :deep(.avatar) {
    transform: translateY(-2px) rotate(-2deg);
}

.home-shortcut-card.is-offline .home-shortcut-cover :deep(img),
.home-shortcut-card.is-offline .home-shortcut-cover :deep(.avatar) {
    filter: grayscale(1);
    opacity: 0.6;
}

.home-shortcut-kind {
    grid-area: kind;
    @apply flex min-w-0 items-center gap-1 self-start text-[0.6875rem] font-extrabold uppercase leading-none tracking-[0.08em] text-[var(--color-text-tertiary)];
}

.home-shortcut-kind svg,
.home-shortcut-meta svg {
    @apply size-3.5 shrink-0;
}

.home-shortcut-copy {
    grid-area: copy;
    @apply flex min-w-0 flex-col gap-0.5 self-end;
}

.home-shortcut-title {
    @apply min-w-0 truncate text-base font-extrabold leading-tight text-[var(--color-text-primary)] no-underline;
}

.home-shortcut-title:hover {
    @apply underline;
}

.home-shortcut-title:focus-visible {
    border-radius: var(--radius-sm);
    outline: none;
    box-shadow: 0 0 0 3px var(--color-brand-shadow);
}

.home-shortcut-meta {
    @apply flex min-w-0 items-center gap-1 text-xs font-semibold leading-tight text-[var(--color-text-tertiary)];
}

.home-shortcut-meta[data-state='offline'] {
    color: var(--home-status-red-text);
}

.home-shortcut-action {
    grid-area: action;
    @apply self-end;
}

/* 1x1 keeps one meta line: the status says the most. */
.home-shortcut-widget[data-size='1x1'] .home-shortcut-secondary {
    display: none;
}

/* 2x1: the icon fills the height on the left, the button stands on the right. */
.home-shortcut-widget[data-size='2x1'] .home-shortcut-card {
    @apply gap-x-5 p-5;
    grid-template-columns: auto minmax(0, 1fr) auto;
    grid-template-rows: auto minmax(0, 1fr);
    grid-template-areas:
        'cover kind action'
        'cover copy action';
}

.home-shortcut-widget[data-size='2x1'] .home-shortcut-cover {
    @apply self-center;
}

.home-shortcut-widget[data-size='2x1'] .home-shortcut-copy {
    @apply gap-1 self-center;
}

.home-shortcut-widget[data-size='2x1'] .home-shortcut-title {
    @apply text-xl;
}

.home-shortcut-widget[data-size='2x1'] .home-shortcut-action {
    @apply self-center;
}

@media (prefers-reduced-motion: reduce) {
    .home-shortcut-cover :deep(img),
    .home-shortcut-cover :deep(.avatar) {
        transition: none;
    }

    .home-shortcut-card:hover .home-shortcut-cover :deep(img),
    .home-shortcut-card:hover .home-shortcut-cover :deep(.avatar) {
        transform: none;
    }
}
</style>
