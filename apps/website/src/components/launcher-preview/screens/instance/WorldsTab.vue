<template>
    <AddServerModal
        ref="addServerModal"
        :instance="instance"
        @submit="
            (server, start) => {
                addServer(server)
                if (start) {
                    joinWorld(server)
                }
            }
        "
    />
    <EditServerModal ref="editServerModal" :instance="instance" @submit="editServer" />
    <ConfirmRemoveWorldModal
        ref="removeWorldModal"
        :world="worldToRemove"
        :symlink-target="instance.symlink_target"
        @confirm="proceedRemoveWorld"
    />
    <ReadyTransition :pending="worldsReadyPending">
        <div v-if="dedupedWorlds.length > 0" class="flex flex-col gap-4">
            <div class="flex flex-wrap items-center gap-2">
                <StyledInput
                    v-model="searchFilter"
                    :icon="SearchIcon"
                    type="text"
                    autocomplete="off"
                    :spellcheck="false"
                    input-class="!h-10"
                    wrapper-class="flex-1 min-w-0"
                    clearable
                    :placeholder="
                        formatMessage(messages.searchWorldsPlaceholder, {
                            count: dedupedWorlds.length,
                        })
                    "
                />
                <div class="flex gap-2">
                    <Button type="outlined" class="!h-10" @click="addServerModal?.show()"
                        ><PlusIcon class="size-5" />
                        {{ formatMessage(messages.addServer) }}
                    </Button>
                    <Button
                        type="colored"
                        color="brand"
                        class="flex !h-10 items-center gap-2"
                        @click="
                            router.push({
                                path: '/browse/server',
                                query: { i: instance.id, from: 'worlds' },
                            })
                        "
                        ><CompassIcon class="size-5" />
                        <span>{{ formatMessage(messages.browseServers) }}</span>
                    </Button>
                    <Button
                        type="outlined"
                        class="flex !h-10 items-center gap-2"
                        @click="
                            router.push({
                                path: '/browse/world',
                                query: { i: instance.id, from: 'world-maps' },
                            })
                        "
                        ><WorldIcon class="size-5" />
                        <span>{{ formatMessage(messages.browseMaps) }}</span>
                    </Button>
                </div>
            </div>
            <div class="flex flex-wrap items-center justify-between gap-2">
                <div class="flex flex-wrap items-center gap-1.5">
                    <FilterIcon class="size-5 text-[var(--color-text-tertiary)]" />
                    <button
                        :class="filterPillClass(selectedFilters.length === 0)"
                        @click="selectedFilters = []"
                    >
                        {{ formatMessage(commonMessages.allProjectType) }}
                    </button>
                    <button
                        v-for="option in filterOptions"
                        :key="option.id"
                        :class="filterPillClass(selectedFilters.includes(option.id))"
                        @click="toggleFilter(option.id)"
                    >
                        {{ option.label }}
                    </button>
                </div>
                <Button
                    type="quiet"
                    interaction="none"
                    :disabled="refreshingAll"
                    @click="refreshAllWorlds"
                >
                    <RefreshCwIcon :class="refreshingAll ? 'animate-spin' : ''" />
                    {{ formatMessage(commonMessages.refreshButton) }}
                </Button>
            </div>
            <div class="flex w-full flex-col gap-2">
                <WorldItem
                    v-for="world in filteredWorlds"
                    :key="`world-${world.type}-${world.type == 'singleplayer' ? world.path : `${world.address}-${world.index}`}`"
                    :world="world"
                    :managed="world.type === 'server' ? isManagedServerWorld(world) : false"
                    :highlighted="highlightedWorld === getWorldIdentifier(world)"
                    :supports-server-quick-play="supportsServerQuickPlay"
                    :supports-world-quick-play="supportsWorldQuickPlay"
                    :current-protocol="protocolVersion"
                    :playing-instance="playing"
                    :playing-world="worldsMatch(world, worldPlaying)"
                    :starting-instance="startingInstance"
                    :refreshing="
                        world.type === 'server' ? serverDataFor(world).refreshing : undefined
                    "
                    :server-status="
                        world.type === 'server' ? serverDataFor(world).status : undefined
                    "
                    :rendered-motd="
                        world.type === 'server' ? serverDataFor(world).renderedMotd : undefined
                    "
                    :game-mode="
                        world.type === 'singleplayer' ? GAME_MODES[world.game_mode] : undefined
                    "
                    :shortcut-instance-id="instance.id"
                    @update="refreshAllWorlds"
                    @play="() => joinWorld(world)"
                    @stop="() => emit('stop')"
                    @refresh="() => refreshServer((world as ServerWorld).address)"
                    @edit="
                        () =>
                            world.type === 'singleplayer'
                                ? router.push(
                                      `/instance/${encodeURIComponent(instance.id)}/worlds/${encodeURIComponent(world.path)}/edit`,
                                  )
                                : isManagedServerWorld(world)
                                  ? undefined
                                  : editServerModal?.show(world)
                    "
                    @delete="() => !isManagedServerWorld(world) && promptToRemoveWorld(world)"
                    @open-folder="(world: HomeWorld) => showWorldInFolder(instance.id, world.path)"
                />
            </div>
        </div>
        <EmptyState
            v-else
            type="empty-inbox"
            :heading="formatMessage(messages.noWorldsHeading)"
            :description="formatMessage(messages.noWorldsDescription)"
        >
            <template #actions>
                <Button type="outlined" class="!h-10" @click="addServerModal?.show()"
                    ><PlusIcon class="size-5" />
                    {{ formatMessage(messages.addServer) }}
                </Button>
                <Button
                    type="colored"
                    color="brand"
                    class="flex !h-10 items-center gap-2"
                    @click="
                        router.push({
                            path: '/browse/server',
                            query: { i: instance.id, from: 'worlds' },
                        })
                    "
                    ><CompassIcon class="size-5" />
                    <span>{{ formatMessage(messages.browseServers) }}</span>
                </Button>
                <Button
                    type="outlined"
                    class="flex !h-10 items-center gap-2"
                    @click="
                        router.push({
                            path: '/browse/world',
                            query: { i: instance.id, from: 'world-maps' },
                        })
                    "
                    ><WorldIcon class="size-5" />
                    <span>{{ formatMessage(messages.browseMaps) }}</span>
                </Button>
            </template>
        </EmptyState>
    </ReadyTransition>
</template>

<script setup lang="ts">
import CompassIcon from '@modrinth/assets/icons/compass.svg?component'
import FilterIcon from '@modrinth/assets/icons/filter.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import RefreshCwIcon from '@modrinth/assets/icons/refresh-cw.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import WorldIcon from '@modrinth/assets/icons/world.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import EmptyState from '@modrinth/ui/src/components/base/EmptyState.vue'
import ReadyTransition from '@modrinth/ui/src/components/base/ReadyTransition.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { GAME_MODES } from '@modrinth/ui/src/utils/game-modes.ts'
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import type { GameInstance } from '../../lib/types'
import WorldItem from '../home/components/WorldItem.vue'
import { provideHomeRuntime } from '../home/runtime'
import type { HomeServerData } from '../home/state'
import { useHomeState } from '../home/state'
import { worldsMessages as messages } from './messages'
import AddServerModal from './worlds/AddServerModal.vue'
import ConfirmRemoveWorldModal from './worlds/ConfirmRemoveWorldModal.vue'
import EditServerModal from './worlds/EditServerModal.vue'
import {
    delete_world,
    getServerDomainKey,
    getWorldIdentifier,
    hasServerQuickPlaySupport,
    hasWorldQuickPlaySupport,
    normalizeServerAddress,
    refreshWorlds,
    remove_server_from_instance,
    resolveManagedServerWorld,
    type ServerWorld,
    showWorldInFolder,
    type SingleplayerWorld,
    type World,
} from './worlds/worlds-api'

type HomeWorld = World

/**
 * Port of `apps/app-frontend/src/pages/instance/Worlds.vue`. The world list is
 * the Home screen's (the app reads both from one backend), server status comes
 * from the Server List Ping snapshot, and joins go through the same launch path
 * as Home (`start_join_*`). Server-project entries remain managed by their link.
 */
defineOptions({ inheritAttrs: false })

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const route = useRoute()
const router = useRouter()
const home = useHomeState()
const runtime = provideHomeRuntime(home)

const addServerModal = ref<InstanceType<typeof AddServerModal>>()
const editServerModal = ref<InstanceType<typeof EditServerModal>>()
const removeWorldModal = ref<InstanceType<typeof ConfirmRemoveWorldModal>>()

const worldToRemove = ref<World | null>(null)

const emit = defineEmits<{
    (event: 'play', world: World): void
    (event: 'stop'): void
}>()

const props = defineProps<{
    instance: GameInstance
    offline: boolean
    playing: boolean
    installed: boolean
    serverProject?: import('../../data').ProjectDetail | null
}>()

const instance = computed(() => props.instance)
const playing = computed(() => props.playing)

function play(world: World) {
    emit('play', world)
}

const selectedFilters = ref<string[]>([])
const searchFilter = ref('')

function filterPillClass(isActive: boolean) {
    return [
        'cursor-pointer rounded-full border border-solid px-3 py-1.5 text-base font-semibold leading-5 transition-all duration-100 active:scale-[0.97]',
        isActive
            ? 'border-brand bg-brand-highlight text-brand'
            : 'border-surface-5 bg-surface-4 text-[var(--color-text-default)] hover:bg-surface-5',
    ]
}

function toggleFilter(id: string) {
    const idx = selectedFilters.value.indexOf(id)
    if (idx >= 0) {
        selectedFilters.value.splice(idx, 1)
    } else {
        selectedFilters.value.push(id)
        if (id === 'singleplayer') {
            selectedFilters.value = selectedFilters.value.filter(
                (f) => f !== 'online' && f !== 'offline',
            )
        } else if (id === 'online' || id === 'offline') {
            selectedFilters.value = selectedFilters.value.filter((f) => f !== 'singleplayer')
        }
    }
}

const refreshingAll = ref(false)
const startingInstance = ref(false)
const worldPlaying = ref<World>()

/** The `worlds` query: the first read shows the loading transition, later ones refresh quietly. */
const worldsLoaded = ref(false)
const worldsReadyPending = computed(() => !worldsLoaded.value)
void refreshWorlds(props.instance.id)
    .catch(handleError)
    .finally(() => {
        worldsLoaded.value = true
    })

const worlds = computed<World[]>(() => {
    const list = home.worlds.value.filter((world) => world.instance_id === props.instance.id)
    return [...list].sort((a, b) => {
        if (!a.last_played) return 1
        if (!b.last_played) return -1
        return Date.parse(b.last_played) - Date.parse(a.last_played)
    })
})

/** `get_instance_protocol_version`: the preview has no protocol table for client versions. */
const protocolVersion = null

const supportsServerQuickPlay = computed(() =>
    hasServerQuickPlaySupport(home.gameVersions.value, instance.value.game_version),
)
const supportsWorldQuickPlay = computed(() =>
    hasWorldQuickPlaySupport(home.gameVersions.value, instance.value.game_version),
)

function serverDataFor(world: ServerWorld): HomeServerData {
    const ping = props.serverProject?.minecraft_java_server?.ping?.data
    if (isManagedServerWorld(world) && ping) {
        return {
            refreshing: false,
            status: {
                players: { online: ping.players_online, max: ping.players_max, sample: [] },
                ping: Math.round(ping.latency.secs * 1000 + ping.latency.nanos / 1e6),
                enforces_secure_chat: false,
            },
        }
    }
    return home.getServerData(props.instance.id, world.address)
}

watch(
    () => worldsLoaded.value && worlds.value.filter((world) => world.type === 'server').length,
    () => {
        if (!worldsLoaded.value) return
        for (const world of worlds.value) {
            if (world.type === 'server') void home.refreshServer(props.instance.id, world.address)
        }
    },
    { immediate: true },
)

const managedServerName = computed(() => props.serverProject?.title ?? null)
const managedServerAddress = computed(
    () => props.serverProject?.minecraft_java_server?.address ?? null,
)

const managedServerWorld = computed(() =>
    resolveManagedServerWorld(worlds.value, managedServerName.value, managedServerAddress.value),
)

function isManagedServerWorld(world: World): world is ServerWorld {
    return world.type === 'server' && managedServerWorld.value?.index === world.index
}

async function refreshServer(address: string) {
    await home.refreshServer(props.instance.id, address, true)
}

async function refreshAllWorlds() {
    if (refreshingAll.value) return

    refreshingAll.value = true
    try {
        await refreshWorlds(props.instance.id)
        await Promise.all(
            worlds.value
                .filter((world): world is ServerWorld => world.type === 'server')
                .map((world) => home.refreshServer(props.instance.id, world.address, true)),
        )
    } catch (error) {
        handleError(error)
    } finally {
        refreshingAll.value = false
    }
}

async function addServer(server: { address: string }) {
    await refreshServer(server.address)
}

async function editServer(server: ServerWorld) {
    await refreshServer(server.address)
}

async function removeServer(server: ServerWorld) {
    await remove_server_from_instance(instance.value.id, server.index).catch(handleError)
}

async function deleteWorld(world: SingleplayerWorld) {
    await delete_world(instance.value.id, world.path).catch(handleError)
}

async function joinWorld(world: Pick<World, 'type'> & Partial<World>) {
    startingInstance.value = true
    worldPlaying.value = world as World
    try {
        if (world.type === 'server' && world.address) {
            await runtime.startJoinServer(instance.value, world.address)
        } else if (world.type === 'singleplayer' && world.path) {
            await runtime.startJoinSingleplayerWorld(instance.value, world.path)
        }
        play(world as World)
    } catch (error) {
        handleError(error)
        worldPlaying.value = undefined
    } finally {
        startingInstance.value = false
    }
}

watch(
    () => playing.value,
    (isPlaying) => {
        if (!isPlaying) worldPlaying.value = undefined
    },
)

function worldsMatch(world: World, other: World | undefined) {
    if (world.type === 'server' && other?.type === 'server') {
        return world.address === other.address
    } else if (world.type === 'singleplayer' && other?.type === 'singleplayer') {
        return world.path === other.path
    }
    return false
}

const dedupedWorlds = computed(() => {
    const visibleWorlds: World[] = []
    const serverIndexByDomain = new Map<string, number>()

    for (const world of worlds.value) {
        if (world.type !== 'server') {
            visibleWorlds.push(world)
            continue
        }

        const domainKey =
            getServerDomainKey(world.address) ||
            normalizeServerAddress(world.address) ||
            `server-${world.index}`
        const existingIndex = serverIndexByDomain.get(domainKey)

        if (existingIndex == null) {
            serverIndexByDomain.set(domainKey, visibleWorlds.length)
            visibleWorlds.push(world)
            continue
        }

        // replace world with managed world if applicable
        const existingWorld = visibleWorlds[existingIndex]
        if (
            existingWorld?.type === 'server' &&
            !isManagedServerWorld(existingWorld) &&
            isManagedServerWorld(world)
        ) {
            visibleWorlds[existingIndex] = world
        }
    }

    return visibleWorlds
})

const filterOptions = computed(() => {
    const options: { id: string; label: string }[] = []
    const hasSingleplayer = dedupedWorlds.value.some((x) => x.type === 'singleplayer')
    const hasServer = dedupedWorlds.value.some((x) => x.type === 'server')

    if (hasSingleplayer && hasServer) {
        options.push({ id: 'singleplayer', label: formatMessage(commonMessages.singleplayerLabel) })
    }

    if (hasServer) {
        const servers = dedupedWorlds.value.filter((x): x is ServerWorld => x.type === 'server')
        const hasVanilla = servers.some((x) => x.content_kind !== 'modpack')
        const hasModded = servers.some((x) => x.content_kind === 'modpack')
        if (hasVanilla && hasModded) {
            options.push({ id: 'vanilla', label: formatMessage(messages.vanillaFilter) })
            options.push({ id: 'modded', label: formatMessage(messages.moddedFilter) })
        }
        const hasOnline = servers.some((x) => !!serverDataFor(x).status)
        const hasOffline = servers.some((x) => !serverDataFor(x).status)
        if (hasOnline && hasOffline) {
            options.push({ id: 'online', label: formatMessage(messages.onlineFilter) })
            options.push({ id: 'offline', label: formatMessage(messages.offlineFilter) })
        }
    }

    return options
})

watch(filterOptions, (options) => {
    const validIds = new Set(options.map((opt) => opt.id))
    const cleaned = selectedFilters.value.filter((f) => validIds.has(f))
    if (cleaned.length !== selectedFilters.value.length) {
        selectedFilters.value = cleaned
    }
})

const filteredWorlds = computed(() =>
    dedupedWorlds.value.filter((x) => {
        if (
            searchFilter.value &&
            !x.name.toLowerCase().includes(searchFilter.value.toLowerCase())
        ) {
            return false
        }

        if (selectedFilters.value.length === 0) return true

        const hasSingleplayerFilter = selectedFilters.value.includes('singleplayer')
        const typeFilters = selectedFilters.value.filter((f) => f === 'vanilla' || f === 'modded')
        const statusFilters = selectedFilters.value.filter((f) => f === 'online' || f === 'offline')

        if (x.type === 'singleplayer') {
            return hasSingleplayerFilter || (typeFilters.length === 0 && statusFilters.length === 0)
        }

        if (hasSingleplayerFilter && typeFilters.length === 0 && statusFilters.length === 0) {
            return false
        }

        let passesType = true
        if (typeFilters.length > 0) {
            const isModded = x.content_kind === 'modpack'
            passesType =
                (typeFilters.includes('modded') && isModded) ||
                (typeFilters.includes('vanilla') && !isModded)
        }

        let passesStatus = true
        if (statusFilters.length > 0) {
            const isOnline = !!serverDataFor(x).status
            passesStatus =
                (statusFilters.includes('online') && isOnline) ||
                (statusFilters.includes('offline') && !isOnline)
        }

        return passesType && passesStatus
    }),
)

const highlightedWorld = ref(route.query.highlight)

function promptToRemoveWorld(world: World): boolean {
    worldToRemove.value = world
    removeWorldModal.value?.show()
    return !!removeWorldModal.value
}

async function proceedRemoveWorld(world: World) {
    if (world.type === 'server') {
        await removeServer(world)
    } else {
        await deleteWorld(world)
    }
    worldToRemove.value = null
}
</script>
