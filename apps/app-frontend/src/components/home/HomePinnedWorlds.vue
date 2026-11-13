<script setup lang="ts">
import { GameIcon } from '@modrinth/assets'
import {
    ButtonLink,
    commonMessages,
    defineMessages,
    GAME_MODES,
    injectNotificationManager,
    useVIntl,
} from '@modrinth/ui'
import { computed, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'

import type { HomeWidgetSize } from '@/components/home/home-dashboard'
import { useHomeDashboardRuntime } from '@/components/home/home-dashboard-runtime'
import { useHomeLaunchTarget } from '@/components/home/home-launch-context'
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
    hasWorldQuickPlaySupport,
    start_join_singleplayer_world,
    type WorldWithInstance,
} from '@/helpers/worlds'
import { handleSevereError } from '@/store/error'

const props = defineProps<{
    instances: GameInstance[]
    dashboard?: boolean
    dashboardSize?: HomeWidgetSize | null
}>()

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const handleMinecraftLaunchError = useMinecraftLaunchError()
const runtime = useHomeDashboardRuntime()
const { favoriteWorlds, favoritesLoaded, gameVersions, runningInstanceIds } = runtime
const { worldMenu } = useHomeRowMenu()
const messages = defineMessages({
    pinnedWorlds: {
        id: 'app.home.worlds.pinned',
        defaultMessage: 'Pinned worlds',
    },
    emptyWorldsTitle: { id: 'app.home.worlds.empty-title', defaultMessage: 'No pinned worlds' },
    emptyWorlds: {
        id: 'app.home.worlds.empty',
        defaultMessage: 'Favorite a world and it will be pinned here.',
    },
    hardcore: { id: 'instance.worlds.hardcore', defaultMessage: 'Hardcore mode' },
    browseWorlds: { id: 'app.home.worlds.browse', defaultMessage: 'Open worlds' },
    gameAlreadyOpen: {
        id: 'instance.worlds.game_already_open',
        defaultMessage: 'Instance is already open',
    },
    worldInUse: { id: 'instance.worlds.world_in_use', defaultMessage: 'World is in use' },
    noWorldQuickPlay: {
        id: 'instance.worlds.no_singleplayer_quick_play',
        defaultMessage: 'You can only jump straight into singleplayer worlds on Minecraft 1.20+',
    },
})
const launchTarget = useHomeLaunchTarget(() => props.instances)

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

function worldMeta(world: WorldWithInstance, instance: GameInstance): string {
    if (world.type !== 'singleplayer') return instance.name
    const mode = world.hardcore
        ? formatMessage(messages.hardcore)
        : formatMessage(GAME_MODES[world.game_mode].message)
    return `${mode} · ${instance.name}`
}

function rowState(world: WorldWithInstance, instance: GameInstance) {
    const key = favoriteKey(world)
    return getHomeWorldRowState({
        instanceRunning: runningInstanceIds.value.includes(instance.id),
        joinedHere: playingWorldKey.value === key,
        starting: startingWorldKey.value === key,
        locked: world.type === 'singleplayer' && world.locked,
        supportsQuickPlay: hasWorldQuickPlaySupport(gameVersions.value, instance.game_version),
    })
}

function playLabel(world: WorldWithInstance, instance: GameInstance): string {
    switch (rowState(world, instance).reason) {
        case 'no-quick-play':
            return formatMessage(messages.noWorldQuickPlay)
        case 'game-open':
            return formatMessage(messages.gameAlreadyOpen)
        case 'world-in-use':
            return formatMessage(messages.worldInUse)
        default:
            return formatMessage(commonMessages.playButton)
    }
}

function rowMenu(world: WorldWithInstance, instance: GameInstance) {
    return worldMenu(world, instance, {
        instanceRunning: runningInstanceIds.value.includes(instance.id),
        playInstance: () => playInstance(instance),
        pinChanged: () => Promise.all([runtime.refreshFavorites(), runtime.refreshRecentWorlds()]),
    })
}

async function playInstance(instance: GameInstance) {
    try {
        await run(instance.id)
        trackEvent('InstanceStart', {
            loader: instance.loader,
            game_version: instance.game_version,
            source: 'HomePinnedWorld',
        })
    } catch (error) {
        const handled = await handleMinecraftLaunchError(error, {
            instance_id: instance.id,
            instance_name: instance.name,
        })
        if (!handled) handleSevereError(error, { instanceId: instance.id })
    }
}

function favoriteKey(world: WorldWithInstance): string {
    return `${world.instance_id}:${world.type}:${getWorldIdentifier(world)}`
}

watch(runningInstanceIds, (instanceIds) => {
    if (playingWorldKey.value && !instanceIds.includes(playingWorldKey.value.split(':', 1)[0])) {
        playingWorldKey.value = null
    }
})

async function joinWorld(world: WorldWithInstance, instance: GameInstance) {
    if (world.type !== 'singleplayer') return
    const key = favoriteKey(world)
    startingWorldKey.value = key

    try {
        await start_join_singleplayer_world(world.instance_id, world.path)
        playingWorldKey.value = key
        trackEvent('InstanceStart', {
            loader: instance.loader,
            game_version: instance.game_version,
            source: 'HomePinnedWorld',
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

async function stopInstance(instance: GameInstance) {
    await kill(instance.id).catch(handleError)
    playingWorldKey.value = null
    trackEvent('InstanceStop', {
        loader: instance.loader,
        game_version: instance.game_version,
        source: 'HomePinnedWorld',
    })
}
</script>

<template>
    <HomeWidgetFrame
        class="home-pinned-worlds"
        :data-size="dashboardSize"
        :title="formatMessage(messages.pinnedWorlds)"
        :icon="GameIcon"
        :badge="favorites.length || null"
        :dense="dashboardSize?.endsWith('x1')"
        :loading="!favoritesLoaded"
        :skeleton-rows="dashboardSize === '1x1' ? 1 : 3"
        :empty-text="favorites.length === 0 ? formatMessage(messages.emptyWorldsTitle) : null"
        :empty-hint="formatMessage(messages.emptyWorlds)"
        :empty-icon="GameIcon"
    >
        <template #empty-action>
            <ButtonLink
                :as="RouterLink"
                :to="
                    launchTarget
                        ? `/instance/${encodeURIComponent(launchTarget.id)}/worlds`
                        : '/library'
                "
                size="sm"
                type="quiet"
                color="brand"
            >
                <GameIcon aria-hidden="true" />
                {{ formatMessage(messages.browseWorlds) }}
            </ButtonLink>
        </template>
        <ul class="home-pinned-list">
            <li v-for="favorite in favorites" :key="favoriteKey(favorite.world)" class="min-w-0">
                <HomeItemRow
                    :title="favorite.world.name"
                    :to="`/instance/${encodeURIComponent(favorite.instance.id)}/worlds?highlight=${encodeURIComponent(getWorldIdentifier(favorite.world))}`"
                    :meta="worldMeta(favorite.world, favorite.instance)"
                    :instance="
                        favorite.world.icon
                            ? null
                            : {
                                  id: favorite.instance.id,
                                  iconPath: favorite.instance.icon_path,
                                  loader: favorite.instance.loader,
                              }
                    "
                    :image-src="favorite.world.icon ?? null"
                    :running="rowState(favorite.world, favorite.instance).showStop"
                    :starting="startingWorldKey === favoriteKey(favorite.world)"
                    :play-disabled="rowState(favorite.world, favorite.instance).playDisabled"
                    :play-label="playLabel(favorite.world, favorite.instance)"
                    :stop-label="formatMessage(commonMessages.stopButton)"
                    @play="joinWorld(favorite.world, favorite.instance)"
                    @stop="stopInstance(favorite.instance)"
                >
                    <template #actions>
                        <HomeRowMenu :options="rowMenu(favorite.world, favorite.instance)" />
                    </template>
                </HomeItemRow>
            </li>
        </ul>
    </HomeWidgetFrame>
</template>
