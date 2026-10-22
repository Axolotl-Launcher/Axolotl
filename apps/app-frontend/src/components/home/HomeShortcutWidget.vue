<script setup lang="ts">
import {
    BoxIcon,
    GameIcon,
    IssuesIcon,
    NoSignalIcon,
    PlayIcon,
    ServerIcon,
    SignalIcon,
    SpinnerIcon,
    StopCircleIcon,
    TimerIcon,
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
import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import { useMinecraftLaunchError } from '@/composables/useMinecraftLaunchError'
import { trackEvent } from '@/helpers/analytics'
import { kill, run } from '@/helpers/instance'
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
            class="flex min-w-0 min-h-0 h-full flex-col items-center justify-center gap-2 p-4 box-border text-center"
        >
            <SpinnerIcon
                class="size-6 animate-spin text-[var(--color-text-tertiary)]"
                aria-hidden="true"
            />
            <span class="text-sm text-[var(--color-text-tertiary)]">{{
                formatMessage(commonMessages.loadingLabel)
            }}</span>
        </div>
        <div
            v-else-if="missing"
            class="flex min-w-0 min-h-0 h-full flex-col items-center justify-center gap-2 p-4 box-border text-center"
        >
            <IssuesIcon class="size-6 text-[var(--color-text-tertiary)]" aria-hidden="true" />
            <strong class="max-w-full truncate text-[var(--color-text-primary)]">{{
                placement.target?.fallbackLabel
            }}</strong>
            <span class="text-sm text-[var(--color-text-tertiary)]">{{
                formatMessage(messages.unavailable)
            }}</span>
        </div>
        <div v-else class="home-shortcut-card grid min-w-0 min-h-0 h-full overflow-hidden">
            <router-link
                class="home-shortcut-visual relative flex min-w-0 min-h-0 items-center justify-center overflow-hidden bg-surface-4 text-[var(--color-text-tertiary)] no-underline"
                :to="shortcutRoute"
                tabindex="-1"
            >
                <component
                    :is="kindIcon"
                    class="home-shortcut-watermark absolute -bottom-3 right-3 size-[4.5rem] opacity-[0.08]"
                    aria-hidden="true"
                />
                <Avatar
                    v-if="shortcutIcon"
                    class="home-shortcut-icon relative z-10 flex-none shadow-[var(--shadow-card)]"
                    :src="shortcutIcon"
                    :size="dashboardSize === '2x1' ? '72px' : '44px'"
                />
                <InstanceIcon
                    v-else-if="instance"
                    class="home-shortcut-icon relative z-10 flex-none shadow-[var(--shadow-card)]"
                    :icon-path="instance.icon_path"
                    :instance-id="instance.id"
                    :loader="instance.loader"
                    :size="dashboardSize === '2x1' ? '72px' : '44px'"
                />
            </router-link>

            <div class="home-shortcut-body relative flex min-w-0 min-h-0 items-stretch">
                <router-link
                    class="home-shortcut-copy flex min-w-0 flex-1 flex-col text-inherit no-underline"
                    :to="shortcutRoute"
                >
                    <span
                        class="home-shortcut-kind flex min-w-0 items-center gap-[0.3rem] text-[var(--color-text-tertiary)] text-[0.6875rem] font-bold leading-none"
                    >
                        <component :is="kindIcon" aria-hidden="true" />
                        {{ kindLabel }}
                    </span>
                    <strong
                        class="home-shortcut-title min-w-0 truncate text-[var(--color-text-primary)] font-[750]"
                        >{{ shortcutTitle }}</strong
                    >
                    <span
                        class="home-shortcut-meta home-shortcut-primary flex min-w-0 items-center gap-[0.35rem] truncate text-xs font-semibold leading-[1.2] text-[var(--color-text-tertiary)]"
                    >
                        <SpinnerIcon
                            v-if="world?.type === 'server' && serverData?.refreshing"
                            class="animate-spin"
                            aria-hidden="true"
                        />
                        <component :is="statusIcon" v-else aria-hidden="true" />
                        {{ primaryLabel }}
                    </span>
                    <span
                        class="home-shortcut-meta home-shortcut-secondary flex min-w-0 items-center gap-[0.35rem] truncate text-xs font-semibold leading-[1.2] text-[var(--color-text-tertiary)]"
                    >
                        <TimerIcon v-if="world?.type !== 'server'" aria-hidden="true" />
                        <ServerIcon v-else aria-hidden="true" />
                        {{ secondaryLabel }}
                    </span>
                </router-link>

                <div class="absolute bottom-3 right-3 z-[2]">
                    <Button
                        v-if="isRunning"
                        v-tooltip="formatMessage(commonMessages.stopButton)"
                        type="colored"
                        color="red"
                        size="2xs"
                        circular
                        icon-only
                        @click="stopInstance"
                        ><StopCircleIcon />
                    </Button>
                    <Button
                        v-else
                        v-tooltip="playTooltip"
                        type="colored"
                        color="brand"
                        size="2xs"
                        circular
                        icon-only
                        :disabled="starting || !supportsQuickPlay"
                        @click="playShortcut"
                        ><SpinnerIcon v-if="starting" class="animate-spin" />
                        <PlayIcon v-else />
                    </Button>
                </div>
            </div>
        </div>
    </div>
</template>

<style scoped>
.home-shortcut-copy:focus-visible {
    border-radius: 6px;
    outline: 4px solid var(--color-brand-shadow);
    outline-offset: 2px;
}

.home-shortcut-kind svg,
.home-shortcut-meta svg {
    width: 0.8rem;
    height: 0.8rem;
    flex: 0 0 auto;
}

.home-shortcut-copy:hover .home-shortcut-title {
    text-decoration: underline;
}

.home-shortcut-widget[data-size='1x1'] .home-shortcut-card {
    grid-template-rows: 3.75rem minmax(0, 1fr);
}

.home-shortcut-widget[data-size='1x1'] .home-shortcut-visual {
    justify-content: flex-start;
    padding: 0 0.875rem;
}

.home-shortcut-widget[data-size='1x1'] .home-shortcut-watermark {
    right: 0.5rem;
    bottom: -1.25rem;
    width: 4rem;
    height: 4rem;
}

.home-shortcut-widget[data-size='1x1'] .home-shortcut-body {
    padding: 0.625rem 0.75rem 0.75rem;
}

.home-shortcut-widget[data-size='1x1'] .home-shortcut-copy {
    padding-right: 2.5rem;
}

.home-shortcut-widget[data-size='1x1'] .home-shortcut-kind {
    display: none;
}

.home-shortcut-widget[data-size='1x1'] .home-shortcut-title {
    font-size: 0.9375rem;
    line-height: 1.2;
}

.home-shortcut-widget[data-size='1x1'] .home-shortcut-primary {
    margin-top: 0.3rem;
}

.home-shortcut-widget[data-size='1x1'] .home-shortcut-secondary {
    display: none;
}

.home-shortcut-widget[data-size='2x1'] .home-shortcut-card {
    grid-template-columns: minmax(8.5rem, 0.8fr) minmax(0, 1.65fr);
}

.home-shortcut-widget[data-size='2x1'] .home-shortcut-body {
    padding: 1rem;
}

.home-shortcut-widget[data-size='2x1'] .home-shortcut-copy {
    justify-content: center;
    padding-right: 3rem;
}

.home-shortcut-widget[data-size='2x1'] .home-shortcut-title {
    margin-top: 0.4rem;
    font-size: 1.125rem;
    line-height: 1.25;
}

.home-shortcut-widget[data-size='2x1'] .home-shortcut-primary {
    margin-top: 0.65rem;
}

.home-shortcut-widget[data-size='2x1'] .home-shortcut-secondary {
    margin-top: 0.3rem;
}
</style>
