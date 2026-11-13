<script setup lang="ts">
import { GameIcon, GridIcon, HistoryIcon, ServerIcon } from '@modrinth/assets'
import {
    ButtonLink,
    commonMessages,
    defineMessages,
    formatLoaderLabel,
    injectNotificationManager,
    useRelativeTime,
    useVIntl,
} from '@modrinth/ui'
import type { Dayjs } from 'dayjs'
import dayjs from 'dayjs'
import { computed, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'

import {
    HOME_RECENT_DEFAULT_LIMIT,
    type HomeRecentLimit,
    type HomeWidgetSize,
} from '@/components/home/home-dashboard'
import { useHomeDashboardRuntime } from '@/components/home/home-dashboard-runtime'
import { useHomeRowMenu } from '@/components/home/home-row-menu'
import { getHomeWorldRowState } from '@/components/home/home-utils'
import HomeItemRow from '@/components/home/HomeItemRow.vue'
import HomeRowMenu from '@/components/home/HomeRowMenu.vue'
import HomeWidgetFrame from '@/components/home/HomeWidgetFrame.vue'
import { useMinecraftLaunchError } from '@/composables/useMinecraftLaunchError'
import { trackEvent } from '@/helpers/analytics'
import { kill, run } from '@/helpers/instance'
import type { GameInstance } from '@/helpers/types'
import {
    getWorldIdentifier,
    hasServerQuickPlaySupport,
    hasWorldQuickPlaySupport,
    start_join_server,
    start_join_singleplayer_world,
    type WorldWithInstance,
} from '@/helpers/worlds'
import { handleSevereError } from '@/store/error'

const props = withDefaults(
    defineProps<{
        instances: GameInstance[]
        dashboard?: boolean
        dashboardSize?: HomeWidgetSize | null
        limit?: HomeRecentLimit
        /** The instance the launch card already offers; listing it again adds nothing. */
        excludeInstanceId?: string | null
    }>(),
    {
        dashboardSize: null,
        limit: HOME_RECENT_DEFAULT_LIMIT,
        excludeInstanceId: null,
    },
)

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const formatRelativeTime = useRelativeTime()
const handleMinecraftLaunchError = useMinecraftLaunchError()
const runtime = useHomeDashboardRuntime()
const { gameVersions, recentWorlds, recentWorldsLoaded, runningInstanceIds } = runtime
const { instanceMenu, worldMenu } = useHomeRowMenu()

const messages = defineMessages({
    recentTitle: {
        id: 'app.home.recent.title',
        defaultMessage: 'Start from your recent projects',
    },
    emptyRecent: {
        id: 'app.home.recent.empty-title',
        defaultMessage: 'No recent activity yet',
    },
    emptyRecentHint: {
        id: 'app.home.recent.empty-hint',
        defaultMessage: 'Worlds, servers and instances you play show up here.',
    },
    viewAllInstances: {
        id: 'app.home.instances.view-all',
        defaultMessage: 'View all instances',
    },
    noServerQuickPlay: {
        id: 'instance.worlds.no_server_quick_play',
        defaultMessage: 'You can only jump straight into servers on Minecraft Alpha 1.0.5+',
    },
    noWorldQuickPlay: {
        id: 'instance.worlds.no_singleplayer_quick_play',
        defaultMessage: 'You can only jump straight into singleplayer worlds on Minecraft 1.20+',
    },
    gameAlreadyOpen: {
        id: 'instance.worlds.game_already_open',
        defaultMessage: 'Instance is already open',
    },
    worldInUse: { id: 'instance.worlds.world_in_use', defaultMessage: 'World is in use' },
})

type RecentItem =
    | { type: 'world'; last_played: Dayjs; instance: GameInstance; world: WorldWithInstance }
    | { type: 'instance'; last_played: Dayjs; instance: GameInstance }

const startingWorldKey = ref<string | null>(null)
const playingWorldKey = ref<string | null>(null)

const instanceById = computed(
    () => new Map(props.instances.map((instance) => [instance.id, instance])),
)

const recentItems = computed<RecentItem[]>(() => {
    const worldItems: RecentItem[] = recentWorlds.value.flatMap((world) => {
        const instance = instanceById.value.get(world.instance_id)
        if (!instance || !world.last_played) return []
        return [{ type: 'world', last_played: dayjs(world.last_played), instance, world }]
    })
    const coveredInstanceIds = new Set(worldItems.map((item) => item.instance.id))
    const instanceItems: RecentItem[] = props.instances
        .filter(
            (instance) =>
                instance.last_played &&
                instance.id !== props.excludeInstanceId &&
                !coveredInstanceIds.has(instance.id),
        )
        .map((instance) => ({
            type: 'instance',
            last_played: dayjs(instance.last_played),
            instance,
        }))

    return [...worldItems, ...instanceItems]
        .sort((a, b) => b.last_played.diff(a.last_played))
        .slice(0, props.limit)
})

function versionLabel(instance: GameInstance): string {
    const loader = instance.loader === 'vanilla' ? 'Minecraft' : formatLoaderLabel(instance.loader)
    return `${loader} ${instance.game_version}`
}

function itemMeta(item: RecentItem): string {
    const when = formatRelativeTime(item.last_played.toISOString())
    return item.type === 'world'
        ? `${item.instance.name} · ${when}`
        : `${versionLabel(item.instance)} · ${when}`
}

function supportsQuickPlay(item: RecentItem): boolean {
    if (item.type !== 'world') return true
    return item.world.type === 'server'
        ? hasServerQuickPlaySupport(gameVersions.value, item.instance.game_version)
        : hasWorldQuickPlaySupport(gameVersions.value, item.instance.game_version)
}

/** Play or stop for a row; an instance row is simply its instance's state. */
function rowState(item: RecentItem) {
    const instanceRunning = runningInstanceIds.value.includes(item.instance.id)
    if (item.type === 'instance') {
        return { showStop: instanceRunning, playDisabled: false, reason: null }
    }
    const key = worldKey(item.world)
    return getHomeWorldRowState({
        instanceRunning,
        joinedHere: playingWorldKey.value === key,
        starting: startingWorldKey.value === key,
        locked: item.world.type === 'singleplayer' && item.world.locked,
        supportsQuickPlay: supportsQuickPlay(item),
    })
}

function playLabel(item: RecentItem): string {
    switch (rowState(item).reason) {
        case 'no-quick-play':
            return formatMessage(
                item.type === 'world' && item.world.type === 'server'
                    ? messages.noServerQuickPlay
                    : messages.noWorldQuickPlay,
            )
        case 'game-open':
            return formatMessage(messages.gameAlreadyOpen)
        case 'world-in-use':
            return formatMessage(messages.worldInUse)
        default:
            return formatMessage(commonMessages.playButton)
    }
}

function rowMenu(item: RecentItem) {
    if (item.type === 'instance') return instanceMenu(item.instance)
    const { world, instance } = item
    return worldMenu(world, instance, {
        instanceRunning: runningInstanceIds.value.includes(instance.id),
        playInstance: () => playInstance(instance),
        refreshServer:
            world.type === 'server'
                ? () => runtime.refreshServer(instance.id, world.address, true)
                : undefined,
        pinChanged: () => Promise.all([runtime.refreshRecentWorlds(), runtime.refreshFavorites()]),
    })
}

function itemRoute(item: RecentItem): string {
    const instancePath = `/instance/${encodeURIComponent(item.instance.id)}`
    return item.type === 'world'
        ? `${instancePath}/worlds?highlight=${encodeURIComponent(getWorldIdentifier(item.world))}`
        : instancePath
}

function worldIcon(world: WorldWithInstance): string | null {
    if (world.type === 'server') {
        return (
            runtime.getServerData(world.instance_id, world.address).status?.favicon ??
            world.icon ??
            null
        )
    }
    return world.icon ?? null
}

function worldKey(world: WorldWithInstance): string {
    return `${world.instance_id}:${world.type}:${getWorldIdentifier(world)}`
}

watch(runningInstanceIds, (instanceIds) => {
    if (playingWorldKey.value && !instanceIds.includes(playingWorldKey.value.split(':', 1)[0])) {
        playingWorldKey.value = null
    }
})

async function joinWorld(world: WorldWithInstance, instance: GameInstance) {
    const key = worldKey(world)
    startingWorldKey.value = key

    try {
        if (world.type === 'server') {
            await start_join_server(world.instance_id, world.address)
        } else {
            await start_join_singleplayer_world(world.instance_id, world.path)
        }
        playingWorldKey.value = key
        trackEvent('InstanceStart', {
            loader: instance.loader,
            game_version: instance.game_version,
            source: 'HomeRecentWorld',
        })
    } catch (error) {
        const handled = await handleMinecraftLaunchError(error, {
            instance_id: instance.id,
            instance_name: instance.name,
        })
        if (!handled) handleSevereError(error, { instanceId: instance.id })
    } finally {
        startingWorldKey.value = null
    }
}

async function playInstance(instance: GameInstance) {
    try {
        await run(instance.id)
        trackEvent('InstanceStart', {
            loader: instance.loader,
            game_version: instance.game_version,
            source: 'HomeRecentWorld',
        })
    } catch (error) {
        const handled = await handleMinecraftLaunchError(error, {
            instance_id: instance.id,
            instance_name: instance.name,
        })
        if (!handled) handleSevereError(error, { instanceId: instance.id })
    }
}

async function stopInstance(instance: GameInstance) {
    await kill(instance.id).catch(handleError)
    playingWorldKey.value = null
    trackEvent('InstanceStop', {
        loader: instance.loader,
        game_version: instance.game_version,
        source: 'HomeRecentWorld',
    })
}
</script>

<template>
    <HomeWidgetFrame
        class="home-recent-worlds"
        :data-size="dashboardSize"
        :title="formatMessage(messages.recentTitle)"
        :icon="HistoryIcon"
        :dense="dashboardSize?.endsWith('x1')"
        :loading="!recentWorldsLoaded"
        :skeleton-rows="Math.min(limit, 2)"
        :empty-text="recentItems.length === 0 ? formatMessage(messages.emptyRecent) : null"
        :empty-hint="formatMessage(messages.emptyRecentHint)"
        :empty-icon="HistoryIcon"
    >
        <template #empty-action>
            <ButtonLink :as="RouterLink" to="/library" size="sm">
                <GridIcon aria-hidden="true" />
                {{ formatMessage(messages.viewAllInstances) }}
            </ButtonLink>
        </template>
        <ul class="home-pinned-list home-recent-list">
            <li
                v-for="item in recentItems"
                :key="item.type === 'world' ? worldKey(item.world) : `${item.instance.id}:instance`"
                class="min-w-0"
            >
                <HomeItemRow
                    :title="item.type === 'world' ? item.world.name : item.instance.name"
                    :to="itemRoute(item)"
                    :meta="itemMeta(item)"
                    :meta-icon="
                        item.type === 'world'
                            ? item.world.type === 'server'
                                ? ServerIcon
                                : GameIcon
                            : null
                    "
                    :instance="
                        item.type === 'instance' || !worldIcon(item.world)
                            ? {
                                  id: item.instance.id,
                                  iconPath: item.instance.icon_path,
                                  loader: item.instance.loader,
                              }
                            : null
                    "
                    :image-src="item.type === 'world' ? worldIcon(item.world) : null"
                    :running="rowState(item).showStop"
                    :starting="item.type === 'world' && startingWorldKey === worldKey(item.world)"
                    :play-disabled="rowState(item).playDisabled"
                    :play-label="playLabel(item)"
                    :stop-label="formatMessage(commonMessages.stopButton)"
                    @play="
                        item.type === 'world'
                            ? joinWorld(item.world, item.instance)
                            : playInstance(item.instance)
                    "
                    @stop="stopInstance(item.instance)"
                >
                    <template #actions>
                        <HomeRowMenu :options="rowMenu(item)" />
                    </template>
                </HomeItemRow>
            </li>
        </ul>
    </HomeWidgetFrame>
</template>
