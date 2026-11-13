<script setup lang="ts">
import { HistoryIcon } from '@modrinth/assets'
import { defineMessages, GAME_MODES, injectNotificationManager, useVIntl } from '@modrinth/ui'
import type { Dayjs } from 'dayjs'
import dayjs from 'dayjs'
import { computed, ref, watch } from 'vue'

import {
    getHomeWidgetCardDensity,
    HOME_RECENT_DEFAULT_LIMIT,
    type HomeRecentLimit,
    type HomeWidgetSize,
} from '@/components/home/home-dashboard'
import { useHomeDashboardRuntime } from '@/components/home/home-dashboard-runtime'
import InstanceItem from '@/components/ui/world/InstanceItem.vue'
import WorldItem from '@/components/ui/world/WorldItem.vue'
import { useMinecraftLaunchError } from '@/composables/useMinecraftLaunchError'
import { trackEvent } from '@/helpers/analytics'
import { kill, run } from '@/helpers/instance'
import type { GameInstance } from '@/helpers/types'
import {
    getWorldIdentifier,
    hasServerQuickPlaySupport,
    hasWorldQuickPlaySupport,
    type ServerWorld,
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
    }>(),
    {
        dashboardSize: null,
        limit: HOME_RECENT_DEFAULT_LIMIT,
    },
)

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const handleMinecraftLaunchError = useMinecraftLaunchError()
const runtime = useHomeDashboardRuntime()
const { gameVersions, recentWorlds, runningInstanceIds } = runtime

const messages = defineMessages({
    recentTitle: {
        id: 'app.home.recent.title',
        defaultMessage: 'Start from your recent projects',
    },
    emptyRecent: {
        id: 'app.home.recent.empty',
        defaultMessage: 'No recent activity yet.',
    },
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
        .filter((instance) => instance.last_played && !coveredInstanceIds.has(instance.id))
        .map((instance) => ({
            type: 'instance',
            last_played: dayjs(instance.last_played),
            instance,
        }))

    return [...worldItems, ...instanceItems]
        .sort((a, b) => b.last_played.diff(a.last_played))
        .slice(0, props.limit)
})
const itemDensity = computed(() => getHomeWidgetCardDensity(props.dashboardSize))

function worldKey(world: WorldWithInstance): string {
    return `${world.instance_id}:${world.type}:${getWorldIdentifier(world)}`
}

function serverDataFor(world: WorldWithInstance) {
    return world.type === 'server'
        ? runtime.getServerData(world.instance_id, world.address)
        : undefined
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
    <section
        class="home-recent-worlds flex min-w-0 min-h-0 h-full flex-col gap-3"
        :data-size="dashboardSize"
    >
        <div class="home-widget-heading flex min-w-0 h-8 flex-none items-center gap-2">
            <h2>{{ formatMessage(messages.recentTitle) }}</h2>
        </div>
        <div
            v-if="recentItems.length > 0"
            class="home-recent-list flex min-w-0 min-h-0 flex-1 flex-col gap-1 overflow-x-hidden overflow-y-auto pr-1"
        >
            <template
                v-for="item in recentItems"
                :key="item.type === 'world' ? worldKey(item.world) : `${item.instance.id}:instance`"
            >
                <WorldItem
                    v-if="item.type === 'world'"
                    :world="item.world"
                    :playing-instance="runningInstanceIds.includes(item.instance.id)"
                    :playing-world="playingWorldKey === worldKey(item.world)"
                    :starting-instance="startingWorldKey === worldKey(item.world)"
                    :supports-server-quick-play="
                        item.world.type === 'server' &&
                        hasServerQuickPlaySupport(gameVersions, item.instance.game_version)
                    "
                    :supports-world-quick-play="
                        item.world.type === 'singleplayer' &&
                        hasWorldQuickPlaySupport(gameVersions, item.instance.game_version)
                    "
                    :current-protocol="runtime.getProtocolVersion(item.instance.id)"
                    :refreshing="
                        item.world.type === 'server'
                            ? serverDataFor(item.world)?.refreshing
                            : undefined
                    "
                    :server-status="
                        item.world.type === 'server' ? serverDataFor(item.world)?.status : undefined
                    "
                    :rendered-motd="
                        item.world.type === 'server'
                            ? serverDataFor(item.world)?.renderedMotd
                            : undefined
                    "
                    :game-mode="
                        item.world.type === 'singleplayer'
                            ? GAME_MODES[item.world.game_mode]
                            : undefined
                    "
                    :instance-id="item.instance.id"
                    :instance-name="item.instance.name"
                    :instance-icon="item.instance.icon_path"
                    :instance-loader="item.instance.loader"
                    :shortcut-instance-id="item.instance.id"
                    :flat="dashboard"
                    :dashboard-density="itemDensity"
                    @play="joinWorld(item.world, item.instance)"
                    @play-instance="playInstance(item.instance)"
                    @stop="stopInstance(item.instance)"
                    @refresh="
                        item.world.type === 'server'
                            ? runtime.refreshServer(
                                  item.instance.id,
                                  (item.world as ServerWorld).address,
                                  true,
                              )
                            : undefined
                    "
                    @update="runtime.refreshRecentWorlds"
                />
                <InstanceItem
                    v-else
                    :instance="item.instance"
                    :last-played="item.last_played"
                    :flat="dashboard"
                    :playing="runningInstanceIds.includes(item.instance.id)"
                    :dashboard-density="itemDensity"
                />
            </template>
        </div>
        <div v-else class="home-widget-empty">
            <HistoryIcon aria-hidden="true" />
            <span>{{ formatMessage(messages.emptyRecent) }}</span>
        </div>
    </section>
</template>

<style scoped>
.home-widget-heading h2 {
    min-width: 0;
    overflow: hidden;
    margin: 0;
    color: var(--color-text-primary);
    font-size: 1rem;
    font-weight: 700;
    letter-spacing: 0;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.home-recent-worlds[data-size='2x1'] {
    gap: 0.5rem;
}

.home-recent-worlds[data-size='2x1'] .home-recent-list,
.home-recent-worlds[data-size='2x2'] .home-recent-list {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    align-content: start;
    column-gap: 0.5rem;
    grid-auto-rows: max-content;
}

.home-widget-empty {
    display: flex;
    max-width: 20rem;
    margin: auto;
    flex-direction: column;
    align-items: center;
    gap: 0.5rem;
    color: var(--color-text-tertiary);
    font-size: 0.8125rem;
    line-height: 1.4;
    text-align: center;
}

.home-widget-empty svg {
    width: 1.5rem;
    height: 1.5rem;
    opacity: 0.7;
}
</style>
