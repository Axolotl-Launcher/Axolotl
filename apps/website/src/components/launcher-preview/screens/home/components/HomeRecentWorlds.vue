<script setup lang="ts">
import HistoryIcon from '@modrinth/assets/icons/history.svg?component'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { GAME_MODES } from '@modrinth/ui/src/utils/game-modes.ts'
import { computed, ref, watch } from 'vue'

import type { GameInstance } from '../../../lib/types'
import {
    getHomeWidgetCardDensity,
    HOME_RECENT_DEFAULT_LIMIT,
    type HomeRecentLimit,
    type HomeWidgetSize,
} from '../home-dashboard'
import {
    getWorldIdentifier,
    hasServerQuickPlaySupport,
    hasWorldQuickPlaySupport,
    type HomeWorld,
} from '../home-utils'
import { homeMessages } from '../messages'
import { useHomeRuntime } from '../runtime'
import InstanceItem from './InstanceItem.vue'
import WorldItem from './WorldItem.vue'

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

const { formatMessage } = useVIntl()
const runtime = useHomeRuntime()
const { gameVersions, recentWorlds, runningInstanceIds } = runtime
const messages = homeMessages

type RecentItem =
    | { type: 'world'; last_played: number; instance: GameInstance; world: HomeWorld }
    | { type: 'instance'; last_played: number; instance: GameInstance }

const startingWorldKey = ref<string | null>(null)
const playingWorldKey = ref<string | null>(null)

const instanceById = computed(
    () => new Map(props.instances.map((instance) => [instance.id, instance])),
)

const recentItems = computed<RecentItem[]>(() => {
    const worldItems: RecentItem[] = recentWorlds.value.flatMap((world) => {
        const instance = instanceById.value.get(world.instance_id)
        if (!instance || !world.last_played) return []
        return [{ type: 'world', last_played: Date.parse(world.last_played), instance, world }]
    })
    const coveredInstanceIds = new Set(worldItems.map((item) => item.instance.id))
    const instanceItems: RecentItem[] = props.instances
        .filter((instance) => instance.last_played && !coveredInstanceIds.has(instance.id))
        .map((instance) => ({
            type: 'instance',
            last_played: Date.parse(instance.last_played ?? ''),
            instance,
        }))

    return [...worldItems, ...instanceItems]
        .sort((a, b) => b.last_played - a.last_played)
        .slice(0, props.limit)
})
const itemDensity = computed(() => getHomeWidgetCardDensity(props.dashboardSize))

function worldKey(world: HomeWorld): string {
    return `${world.instance_id}:${world.type}:${getWorldIdentifier(world)}`
}

function serverDataFor(world: HomeWorld) {
    return world.type === 'server'
        ? runtime.getServerData(world.instance_id, world.address)
        : undefined
}

watch(runningInstanceIds, (instanceIds) => {
    if (playingWorldKey.value && !instanceIds.includes(playingWorldKey.value.split(':', 1)[0])) {
        playingWorldKey.value = null
    }
})

async function joinWorld(world: HomeWorld, instance: GameInstance) {
    const key = worldKey(world)
    startingWorldKey.value = key

    try {
        if (world.type === 'server') {
            await runtime.startJoinServer(instance, world.address)
        } else {
            await runtime.startJoinSingleplayerWorld(instance, world.path)
        }
        playingWorldKey.value = key
    } catch (error) {
        runtime.handleError(error)
    } finally {
        startingWorldKey.value = null
    }
}

async function playInstance(instance: GameInstance) {
    try {
        await runtime.run(instance)
    } catch (error) {
        runtime.handleError(error)
    }
}

async function stopInstance(instance: GameInstance) {
    await runtime.kill(instance.id).catch(runtime.handleError)
    playingWorldKey.value = null
}
</script>

<template>
    <section
        class="home-recent-worlds flex h-full min-h-0 min-w-0 flex-col gap-3"
        :data-size="dashboardSize"
    >
        <div class="home-widget-heading flex h-8 min-w-0 flex-none items-center gap-2">
            <h2>{{ formatMessage(messages.recentTitle) }}</h2>
        </div>
        <div
            v-if="recentItems.length > 0"
            class="home-recent-list flex min-h-0 min-w-0 flex-1 flex-col gap-1 overflow-y-auto overflow-x-hidden pr-1"
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
                    :current-protocol="null"
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
                            ? runtime.refreshServer(item.instance.id, item.world.address, true)
                            : undefined
                    "
                    @open-folder="runtime.showInstanceInFolder(item.instance.id)"
                />
                <InstanceItem
                    v-else
                    :instance="item.instance"
                    :last-played="item.instance.last_played ?? null"
                    :flat="dashboard"
                    :playing="runningInstanceIds.includes(item.instance.id)"
                    :dashboard-density="itemDensity"
                />
            </template>
        </div>
        <div v-else class="home-widget-empty">
            <HistoryIcon aria-hidden="true" />
            <span>{{ formatMessage(messages.recentEmpty) }}</span>
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
