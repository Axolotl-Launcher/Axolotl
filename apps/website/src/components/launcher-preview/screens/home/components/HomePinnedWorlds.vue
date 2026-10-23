<script setup lang="ts">
import GameIcon from '@modrinth/assets/icons/game.svg?component'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { GAME_MODES } from '@modrinth/ui/src/utils/game-modes.ts'
import { computed, ref, watch } from 'vue'

import type { GameInstance } from '../../../lib/types'
import { getHomeWidgetCardDensity, type HomeWidgetSize } from '../home-dashboard'
import { getWorldIdentifier, hasWorldQuickPlaySupport, type HomeWorld } from '../home-utils'
import { homeMessages } from '../messages'
import { useHomeRuntime } from '../runtime'
import WorldItem from './WorldItem.vue'

const props = defineProps<{
    instances: GameInstance[]
    dashboard?: boolean
    dashboardSize?: HomeWidgetSize | null
}>()

const { formatMessage } = useVIntl()
const runtime = useHomeRuntime()
const { favoriteWorlds, gameVersions, runningInstanceIds } = runtime
const messages = homeMessages

const startingWorldKey = ref<string | null>(null)
const playingWorldKey = ref<string | null>(null)

const instanceById = computed(
    () => new Map(props.instances.map((instance) => [instance.id, instance])),
)
const favorites = computed(() =>
    favoriteWorlds.value.flatMap((world) => {
        if (world.type !== 'singleplayer') return []
        const instance = instanceById.value.get(world.instance_id)
        return instance ? [{ instance, world }] : []
    }),
)
const worldDensity = computed(() => getHomeWidgetCardDensity(props.dashboardSize))

function favoriteKey(world: HomeWorld): string {
    return `${world.instance_id}:${world.type}:${getWorldIdentifier(world)}`
}

watch(runningInstanceIds, (instanceIds) => {
    if (playingWorldKey.value && !instanceIds.includes(playingWorldKey.value.split(':', 1)[0])) {
        playingWorldKey.value = null
    }
})

async function joinWorld(world: HomeWorld, instance: GameInstance) {
    if (world.type !== 'singleplayer') return
    const key = favoriteKey(world)
    startingWorldKey.value = key

    try {
        await runtime.startJoinSingleplayerWorld(instance, world.path)
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
        class="home-pinned-worlds flex h-full min-h-0 min-w-0 flex-col gap-3"
        :data-size="dashboardSize"
    >
        <div class="home-widget-heading flex h-8 min-w-0 flex-none items-center gap-2">
            <h2>{{ formatMessage(messages.worldsPinned) }}</h2>
        </div>
        <div
            v-if="favorites.length > 0"
            class="home-world-list flex min-h-0 min-w-0 flex-1 flex-col gap-1 overflow-y-auto overflow-x-hidden pr-1"
        >
            <WorldItem
                v-for="favorite in favorites"
                :key="favoriteKey(favorite.world)"
                :world="favorite.world"
                :playing-instance="runningInstanceIds.includes(favorite.instance.id)"
                :playing-world="playingWorldKey === favoriteKey(favorite.world)"
                :starting-instance="startingWorldKey === favoriteKey(favorite.world)"
                :supports-world-quick-play="
                    hasWorldQuickPlaySupport(gameVersions, favorite.instance.game_version)
                "
                :game-mode="
                    favorite.world.type === 'singleplayer'
                        ? GAME_MODES[favorite.world.game_mode]
                        : undefined
                "
                :instance-id="favorite.instance.id"
                :instance-name="favorite.instance.name"
                :instance-icon="favorite.instance.icon_path"
                :instance-loader="favorite.instance.loader"
                :shortcut-instance-id="favorite.instance.id"
                :flat="dashboard"
                :dashboard-density="worldDensity"
                @play="joinWorld(favorite.world, favorite.instance)"
                @play-instance="playInstance(favorite.instance)"
                @stop="stopInstance(favorite.instance)"
                @open-folder="runtime.showInstanceInFolder(favorite.instance.id)"
            />
        </div>
        <div v-else class="home-widget-empty">
            <GameIcon aria-hidden="true" />
            <span>{{ formatMessage(messages.worldsEmpty) }}</span>
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

.home-pinned-worlds[data-size='1x1'] {
    gap: 0.375rem;
}

.home-pinned-worlds[data-size='1x1'] .home-widget-heading {
    height: 1.5rem;
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
