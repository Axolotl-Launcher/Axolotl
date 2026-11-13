<script setup lang="ts">
import { LinkIcon, NoSignalIcon, ServerIcon, SignalIcon, UnlinkIcon } from '@modrinth/assets'
import {
    ButtonLink,
    defineMessages,
    injectNotificationManager,
    type MenuAction,
    useVIntl,
} from '@modrinth/ui'
import { computed, reactive, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'

import type { HomeWidgetSize } from '@/components/home/home-dashboard'
import { useHomeDashboardRuntime } from '@/components/home/home-dashboard-runtime'
import { useHomeLaunchTarget } from '@/components/home/home-launch-context'
import { useHomeRowMenu } from '@/components/home/home-row-menu'
import {
    getHomeItemVariant,
    getHomeWorldRowState,
    getServerFill,
} from '@/components/home/home-utils'
import HomeFitList from '@/components/home/HomeFitList.vue'
import HomeItemRow from '@/components/home/HomeItemRow.vue'
import HomeLocalServerPickerModal from '@/components/home/HomeLocalServerPickerModal.vue'
import HomeRowMenu from '@/components/home/HomeRowMenu.vue'
import HomeWidgetFrame from '@/components/home/HomeWidgetFrame.vue'
import OnlineModeWarningModal from '@/components/multiplayer/servers/OnlineModeWarningModal.vue'
import ManagedServerIcon from '@/components/multiplayer/servers/ServerIcon.vue'
import ServerTerminateButton from '@/components/multiplayer/servers/ServerTerminateButton.vue'
import { useMinecraftLaunchError } from '@/composables/useMinecraftLaunchError'
import { useOnlineModeWarning } from '@/composables/useOnlineModeWarning'
import { useServers } from '@/composables/useServers'
import { trackEvent } from '@/helpers/analytics'
import { kill, run } from '@/helpers/instance'
import { servers as serversApi } from '@/helpers/servers'
import type { GameInstance } from '@/helpers/types'
import {
    getWorldIdentifier,
    hasServerQuickPlaySupport,
    type ServerWorld,
    start_join_server,
    type WorldWithInstance,
} from '@/helpers/worlds'
import { handleSevereError } from '@/store/error'

const props = defineProps<{
    instances: GameInstance[]
    dashboard?: boolean
    dashboardSize?: HomeWidgetSize | null
}>()

const { addNotification, handleError } = injectNotificationManager()
const { formatMessage, locale } = useVIntl()
const handleMinecraftLaunchError = useMinecraftLaunchError()
const { onlineModeWarningModal, confirmOnlineModeLaunch } = useOnlineModeWarning()
const runtime = useHomeDashboardRuntime()
const { favoriteWorlds, favoritesLoaded, gameVersions, localServers, runningInstanceIds } = runtime
const { homePin, worldMenu } = useHomeRowMenu()
const { startServer, stopServer } = useServers()

const messages = defineMessages({
    stopping: { id: 'app.servers.action.stopping', defaultMessage: 'Stopping...' },
    pinnedServers: {
        id: 'app.home.servers.pinned',
        defaultMessage: 'Pinned servers',
    },
    emptyServersTitle: {
        id: 'app.home.servers.empty-title',
        defaultMessage: 'No pinned servers',
    },
    emptyServers: {
        id: 'app.home.servers.empty',
        defaultMessage: 'Favorite a server and it will be pinned here.',
    },
    playersOnline: {
        id: 'app.home.servers.players-online',
        defaultMessage: '{online}/{max} online',
    },
    offline: {
        id: 'app.home.servers.offline',
        defaultMessage: 'Offline',
    },
    join: {
        id: 'app.home.servers.join',
        defaultMessage: 'Join server',
    },
    gameAlreadyOpen: {
        id: 'instance.worlds.game_already_open',
        defaultMessage: 'Instance is already open',
    },
    noServerQuickPlay: {
        id: 'instance.worlds.no_server_quick_play',
        defaultMessage: 'You can only jump straight into servers on Minecraft Alpha 1.0.5+',
    },
    localServer: {
        id: 'app.home.servers.local',
        defaultMessage: 'Local',
    },
    linkLocalServer: {
        id: 'app.home.servers.link-local-server',
        defaultMessage: 'Link to a local server',
    },
    changeLocalServer: {
        id: 'app.home.servers.change-local-server',
        defaultMessage: 'Change linked local server',
    },
    unlinkLocalServer: {
        id: 'app.home.servers.unlink-local-server',
        defaultMessage: 'Unlink local server',
    },
    linkedToast: {
        id: 'app.home.servers.linked-toast',
        defaultMessage: 'Linked to the local server “{name}”',
    },
    unlinkedToast: {
        id: 'app.home.servers.unlinked-toast',
        defaultMessage: 'Unlinked the local server',
    },
    localServerNotReady: {
        id: 'app.home.servers.local-server-not-ready',
        defaultMessage:
            'The linked local server did not start in time. Check its console and try again.',
    },
    start: { id: 'app.servers.action.start', defaultMessage: 'Start' },
    stop: { id: 'app.servers.action.stop', defaultMessage: 'Stop' },
    addServer: { id: 'app.home.servers.add', defaultMessage: 'Add a server' },
})
const launchTarget = useHomeLaunchTarget(() => props.instances)
const variant = computed(() => getHomeItemVariant(props.dashboardSize))

/** How full the server is, for the bar under its status; null when it does not say. */
function serverFill(world: ServerWorld & WorldWithInstance) {
    const players = dataFor(world).status?.players
    return getServerFill(players?.online, players?.max)
}

function serverMeta(world: ServerWorld & WorldWithInstance): string {
    const data = dataFor(world)
    if (data.status) {
        // Compact counts ("18K/200K") keep big servers from cutting the line short.
        const count = new Intl.NumberFormat(locale.value, {
            notation: 'compact',
            maximumFractionDigits: 1,
        })
        return formatMessage(messages.playersOnline, {
            online: count.format(data.status.players?.online ?? 0),
            max: count.format(data.status.players?.max ?? 0),
        })
    }
    return data.refreshing ? world.address : formatMessage(messages.offline)
}

function serverStatus(world: ServerWorld & WorldWithInstance) {
    const data = dataFor(world)
    if (data.refreshing) return 'pending' as const
    return data.status ? ('online' as const) : ('offline' as const)
}

const startingServerKey = ref<string | null>(null)
/** The server the running game was joined through from this widget. */
const playingServerKey = ref<string | null>(null)

watch(runningInstanceIds, (instanceIds) => {
    if (playingServerKey.value && !instanceIds.includes(playingServerKey.value.split(':', 1)[0])) {
        playingServerKey.value = null
    }
})

function rowState(world: ServerWorld & WorldWithInstance, instance: GameInstance) {
    const key = serverKey(world)
    return getHomeWorldRowState({
        instanceRunning: runningInstanceIds.value.includes(instance.id),
        joinedHere: playingServerKey.value === key,
        starting: startingServerKey.value === key,
        supportsQuickPlay: hasServerQuickPlaySupport(gameVersions.value, instance.game_version),
    })
}

function playLabel(world: ServerWorld & WorldWithInstance, instance: GameInstance): string {
    switch (rowState(world, instance).reason) {
        case 'no-quick-play':
            return formatMessage(messages.noServerQuickPlay)
        case 'game-open':
            return formatMessage(messages.gameAlreadyOpen)
        default:
            return formatMessage(messages.join)
    }
}

const instanceById = computed(
    () => new Map(props.instances.map((instance) => [instance.id, instance])),
)
const servers = computed(() =>
    favoriteWorlds.value.flatMap((world) => {
        if (world.type !== 'server') return []
        const instance = instanceById.value.get(world.instance_id)
        return instance ? [{ instance, world: world as ServerWorld & WorldWithInstance }] : []
    }),
)
const pinnedLocalServers = computed(() => runtime.pinnedLocalServers.value)
/** Saved servers first, then the pinned local ones, as one list. */
const entries = computed(() => [
    ...servers.value.map((server) => ({
        key: serverKey(server.world),
        type: 'remote' as const,
        server,
    })),
    ...pinnedLocalServers.value.map((server) => ({
        key: `local-${server.id}`,
        type: 'local' as const,
        server,
    })),
])
const hasServers = computed(() => servers.value.length > 0 || pinnedLocalServers.value.length > 0)

/** Whether each pinned address points back at this machine. */
const localAddresses = reactive<Record<string, boolean>>({})
const pinnedAddresses = computed(() => [
    ...new Set(servers.value.map((server) => server.world.address)),
])

watch(
    pinnedAddresses,
    async (addresses) => {
        for (const address of addresses) {
            if (address in localAddresses) continue
            try {
                localAddresses[address] = await serversApi.isLocalAddress(address)
            } catch {
                localAddresses[address] = false
            }
        }
    },
    { immediate: true },
)

const linkingWorld = ref<(ServerWorld & WorldWithInstance) | null>(null)
const localServerPicker = ref<InstanceType<typeof HomeLocalServerPickerModal>>()

function isLocalEntry(world: ServerWorld & WorldWithInstance) {
    return localAddresses[world.address] === true
}

/** The managed server linked to this entry, if any. */
function linkedServer(world: ServerWorld & WorldWithInstance) {
    return (
        localServers.value.find(
            (server) =>
                server.linkedWorld?.instanceId === world.instance_id &&
                server.linkedWorld?.address === world.address,
        ) ?? null
    )
}

function serverMenuOptions(world: ServerWorld & WorldWithInstance, instance: GameInstance) {
    const options = worldMenu(world, instance, {
        instanceRunning: runningInstanceIds.value.includes(instance.id),
        playInstance: () => playInstance(instance),
        refreshServer: () => runtime.refreshServer(instance.id, world.address, true),
        pinChanged: () => Promise.all([runtime.refreshFavorites(), runtime.refreshRecentWorlds()]),
    })
    if (!isLocalEntry(world)) return options

    const linked = linkedServer(world)
    const local: MenuAction[] = linked
        ? [
              {
                  id: 'change-local-server',
                  section: 'manage',
                  icon: LinkIcon,
                  label: formatMessage(messages.changeLocalServer),
                  action: () => openLocalServerPicker(world),
              },
              {
                  id: 'unlink-local-server',
                  section: 'manage',
                  icon: UnlinkIcon,
                  label: formatMessage(messages.unlinkLocalServer),
                  action: () => void unlinkLocalServer(linked.id),
              },
          ]
        : [
              {
                  id: 'link-local-server',
                  section: 'manage',
                  icon: LinkIcon,
                  label: formatMessage(messages.linkLocalServer),
                  action: () => openLocalServerPicker(world),
              },
          ]
    return [...local, ...options]
}

async function playInstance(instance: GameInstance) {
    try {
        await run(instance.id)
        trackEvent('InstanceStart', {
            loader: instance.loader,
            game_version: instance.game_version,
            source: 'HomePinnedServer',
        })
    } catch (error) {
        const handled = await handleMinecraftLaunchError(error, {
            instance_id: instance.id,
            instance_name: instance.name,
        })
        if (!handled) handleSevereError(error, { instanceId: instance.id })
    }
}

function openLocalServerPicker(world: ServerWorld & WorldWithInstance) {
    linkingWorld.value = world
    localServerPicker.value?.show()
}

async function linkLocalServer(server: { id: string; name: string }) {
    const world = linkingWorld.value
    if (!world) return
    try {
        await serversApi.setLinkedWorld(server.id, {
            instanceId: world.instance_id,
            address: world.address,
        })
        addNotification({
            type: 'success',
            title: formatMessage(messages.linkedToast, { name: server.name }),
        })
    } catch (error) {
        handleError(error)
    } finally {
        linkingWorld.value = null
        await runtime.refreshPinnedLocalServers()
    }
}

async function unlinkLocalServer(serverId: string) {
    try {
        await serversApi.setLinkedWorld(serverId, null)
        addNotification({ type: 'success', title: formatMessage(messages.unlinkedToast) })
    } catch (error) {
        handleError(error)
    } finally {
        await runtime.refreshPinnedLocalServers()
    }
}

function serverKey(world: ServerWorld & WorldWithInstance): string {
    return `${world.instance_id}:${world.index}:${world.address}`
}

function dataFor(world: ServerWorld & WorldWithInstance) {
    return runtime.getServerData(world.instance_id, world.address)
}

async function joinServer(world: ServerWorld & WorldWithInstance, instance: GameInstance) {
    const key = serverKey(world)
    startingServerKey.value = key

    try {
        const linked = linkedServer(world)
        if (linked) {
            if (linked.stopping) {
                addNotification({ type: 'info', title: formatMessage(messages.stopping) })
                return
            }
            if (!(await confirmOnlineModeLaunch(linked))) return
            if (!linked.running && !linked.starting) {
                const started = await startServer(linked.id)
                if (!started) return
            }
            const ready = await serversApi.waitUntilReady(linked.id)
            await runtime.refreshPinnedLocalServers()
            if (!ready) {
                addNotification({
                    type: 'error',
                    title: formatMessage(messages.localServerNotReady),
                })
                return
            }
        }
        await start_join_server(world.instance_id, world.address)
        playingServerKey.value = key
        trackEvent('InstanceStart', {
            loader: instance.loader,
            game_version: instance.game_version,
            source: 'HomePinnedServer',
        })
    } catch (error) {
        const handled = await handleMinecraftLaunchError(error, {
            instance_id: instance.id,
            instance_name: instance.name,
        })
        if (!handled) handleSevereError(error, { instanceId: instance.id })
    } finally {
        startingServerKey.value = null
    }
}

async function stopInstance(instance: GameInstance) {
    await kill(instance.id).catch(handleError)
    playingServerKey.value = null
    trackEvent('InstanceStop', {
        loader: instance.loader,
        game_version: instance.game_version,
        source: 'HomePinnedServer',
    })
}

async function startLocalServer(serverId: string) {
    await startServer(serverId)
    await runtime.refreshPinnedLocalServers()
}

async function stopLocalServer(serverId: string) {
    await stopServer(serverId)
    await runtime.refreshPinnedLocalServers()
}

function localServerMenu(serverId: string): MenuAction[] {
    return [
        homePin(true, async () => {
            await serversApi.updateSettings(serverId, { homePinned: false })
            await runtime.refreshPinnedLocalServers()
        }),
    ]
}
</script>

<template>
    <HomeWidgetFrame
        class="home-pinned-servers"
        :data-size="dashboardSize"
        :title="formatMessage(messages.pinnedServers)"
        :icon="ServerIcon"
        :badge="servers.length + pinnedLocalServers.length || null"
        :dense="dashboardSize?.endsWith('x1')"
        :loading="!favoritesLoaded"
        :skeleton-rows="dashboardSize === '1x1' ? 1 : 3"
        :empty-text="hasServers ? null : formatMessage(messages.emptyServersTitle)"
        :empty-hint="formatMessage(messages.emptyServers)"
        :empty-icon="ServerIcon"
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
                <ServerIcon aria-hidden="true" />
                {{ formatMessage(messages.addServer) }}
            </ButtonLink>
        </template>
        <HomeFitList
            v-slot="{ item: entry, variant: itemVariant }"
            :items="entries"
            :item-key="(entry) => entry.key"
            :variant="variant"
        >
            <template v-if="entry.type === 'remote'">
                <HomeItemRow
                    :title="entry.server.world.name"
                    :to="`/instance/${encodeURIComponent(entry.server.instance.id)}/worlds?highlight=${encodeURIComponent(getWorldIdentifier(entry.server.world))}`"
                    :tag="
                        isLocalEntry(entry.server.world)
                            ? formatMessage(messages.localServer)
                            : null
                    "
                    :image-src="
                        dataFor(entry.server.world).status?.favicon ??
                        (entry.server.world.icon || null)
                    "
                    :image-tint="entry.server.world.address"
                    :status="serverStatus(entry.server.world)"
                    :dimmed="serverStatus(entry.server.world) === 'offline'"
                    :variant="itemVariant"
                    :running="rowState(entry.server.world, entry.server.instance).showStop"
                    :starting="startingServerKey === serverKey(entry.server.world)"
                    :play-disabled="
                        rowState(entry.server.world, entry.server.instance).playDisabled
                    "
                    :play-label="playLabel(entry.server.world, entry.server.instance)"
                    :stop-label="formatMessage(messages.stop)"
                    @play="joinServer(entry.server.world, entry.server.instance)"
                    @stop="stopInstance(entry.server.instance)"
                >
                    <template #extra>
                        <span class="home-server-status">
                            <span
                                class="home-server-state"
                                :data-state="serverStatus(entry.server.world)"
                            >
                                <component
                                    :is="
                                        dataFor(entry.server.world).status
                                            ? SignalIcon
                                            : NoSignalIcon
                                    "
                                    aria-hidden="true"
                                />
                                <span class="truncate">{{ serverMeta(entry.server.world) }}</span>
                            </span>
                            <span
                                v-if="serverFill(entry.server.world) !== null"
                                class="home-server-meter"
                                role="presentation"
                            >
                                <span
                                    :style="{
                                        width: `${Math.max(3, (serverFill(entry.server.world) ?? 0) * 100)}%`,
                                    }"
                                />
                            </span>
                        </span>
                    </template>
                    <template #actions>
                        <HomeRowMenu
                            class="home-server-menu"
                            :options="serverMenuOptions(entry.server.world, entry.server.instance)"
                        />
                    </template>
                </HomeItemRow>
            </template>
            <template v-else>
                <HomeItemRow
                    :title="entry.server.name"
                    :to="`/multiplayer/servers/${encodeURIComponent(entry.server.id)}`"
                    :meta="
                        entry.server.port
                            ? `localhost:${entry.server.port}`
                            : `${entry.server.serverType} ${entry.server.gameVersion}`
                    "
                    :tag="formatMessage(messages.localServer)"
                    :status="entry.server.running ? 'online' : 'offline'"
                    :dimmed="!entry.server.running && !entry.server.starting"
                    :variant="itemVariant"
                    :running="entry.server.running || entry.server.starting"
                    :play-disabled="entry.server.stopping"
                    :play-label="formatMessage(messages.start)"
                    :stop-label="
                        formatMessage(entry.server.stopping ? messages.stopping : messages.stop)
                    "
                    @play="startLocalServer(entry.server.id)"
                    @stop="stopLocalServer(entry.server.id)"
                >
                    <template #icon="{ size }">
                        <ManagedServerIcon
                            :icon-path="entry.server.iconPath"
                            :server-type="entry.server.serverType"
                            :server-id="entry.server.id"
                            :size="size"
                        />
                    </template>
                    <template #actions>
                        <ServerTerminateButton
                            v-if="entry.server.stopping"
                            :server-id="entry.server.id"
                            size="2xs"
                        />
                        <HomeRowMenu
                            class="home-server-menu"
                            :options="localServerMenu(entry.server.id)"
                        />
                    </template>
                </HomeItemRow>
            </template>
        </HomeFitList>
        <HomeLocalServerPickerModal
            ref="localServerPicker"
            :servers="localServers"
            :selected-server-id="linkingWorld ? (linkedServer(linkingWorld)?.id ?? null) : null"
            @select="linkLocalServer"
        />
        <OnlineModeWarningModal ref="onlineModeWarningModal" />
    </HomeWidgetFrame>
</template>

<style scoped>
/*
 * A server's state is a solid badge with its player count, and a thin bar
 * under it shows how full it is. An offline server says so in red and its
 * picture goes grey, so the two read apart at a glance.
 */
.home-server-status {
    @apply mt-0.5 flex min-w-0 flex-col items-start gap-1;
}

.home-server-state {
    @apply inline-flex min-w-0 shrink items-center gap-1 rounded-full px-1.5 text-[0.6875rem] font-bold leading-[1.125rem];
    background: var(--home-widget-tint);
    color: var(--color-text-default);
}

.home-server-state svg {
    @apply size-3 shrink-0;
}

.home-server-state[data-state='online'] {
    @apply bg-highlight-green text-brand-green;
}

.home-server-state[data-state='offline'] {
    @apply bg-highlight-red text-brand-red;
}

.home-server-meter {
    @apply block h-1 w-full max-w-[9rem] overflow-hidden rounded-full;
    background: var(--home-widget-tint-strong);
}

.home-server-meter > span {
    @apply block h-full rounded-full bg-brand-green;
}

/* A tile has the width for a bar as long as the card. */
:deep(.home-item-row.is-tile) .home-server-status {
    @apply gap-1.5;
}

:deep(.home-item-row.is-tile) .home-server-meter {
    @apply max-w-none;
}
</style>
