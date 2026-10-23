<script setup lang="ts">
import LinkIcon from '@modrinth/assets/icons/link.svg?component'
import MoreVerticalIcon from '@modrinth/assets/icons/more-vertical.svg?component'
import NoSignalIcon from '@modrinth/assets/icons/no-signal.svg?component'
import PinIcon from '@modrinth/assets/icons/pin.svg?component'
import PlayIcon from '@modrinth/assets/icons/play.svg?component'
import ServerIcon from '@modrinth/assets/icons/server.svg?component'
import SignalIcon from '@modrinth/assets/icons/signal.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import StopCircleIcon from '@modrinth/assets/icons/stop-circle.svg?component'
import UnlinkIcon from '@modrinth/assets/icons/unlink.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import OverflowMenu from '@modrinth/ui/src/components/base/OverflowMenu.vue'
import SmartClickable from '@modrinth/ui/src/components/base/SmartClickable.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, ref } from 'vue'

import type { GameInstance } from '../../../lib/types'
import { injectPreviewWindow } from '../../../lib/window'
import type { HomeWidgetSize } from '../home-dashboard'
import { getWorldIdentifier, isLocalAddress, type ServerWorld } from '../home-utils'
import { homeMessages } from '../messages'
import { useHomeRuntime } from '../runtime'
import type { HomeLocalServer } from '../state'
import HomeLocalServerPickerModal from './HomeLocalServerPickerModal.vue'
import HomeOnlineModeWarningModal from './HomeOnlineModeWarningModal.vue'
import ManagedServerIcon from './ManagedServerIcon.vue'

const props = defineProps<{
    instances: GameInstance[]
    dashboard?: boolean
    dashboardSize?: HomeWidgetSize | null
}>()

const { addNotification, handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const { layer } = injectPreviewWindow()
const runtime = useHomeRuntime()
const { favoriteWorlds, localServers, runningInstanceIds } = runtime
const messages = homeMessages
const onlineModeWarningModal = ref<InstanceType<typeof HomeOnlineModeWarningModal>>()

const startingServerKey = ref<string | null>(null)

const instanceById = computed(
    () => new Map(props.instances.map((instance) => [instance.id, instance])),
)
const servers = computed(() =>
    favoriteWorlds.value.flatMap((world) => {
        if (world.type !== 'server') return []
        const instance = instanceById.value.get(world.instance_id)
        return instance ? [{ instance, world }] : []
    }),
)
const pinnedLocalServers = computed(() => runtime.pinnedLocalServers.value)
const hasServers = computed(() => servers.value.length > 0 || pinnedLocalServers.value.length > 0)

const linkingWorld = ref<ServerWorld | null>(null)
const localServerPicker = ref<InstanceType<typeof HomeLocalServerPickerModal>>()

/** Whether the pinned address points back at this machine (`servers.isLocalAddress`). */
function isLocalEntry(world: ServerWorld) {
    return isLocalAddress(world.address)
}

/** The managed server linked to this entry, if any. */
function linkedServer(world: ServerWorld) {
    return (
        localServers.value.find(
            (server) =>
                server.linkedWorld?.instanceId === world.instance_id &&
                server.linkedWorld?.address === world.address,
        ) ?? null
    )
}

function serverMenuOptions(world: ServerWorld) {
    const options = [{ id: 'unpin', action: () => unpinServer(world) }]
    if (!isLocalEntry(world)) return options

    const linked = linkedServer(world)
    if (linked) {
        options.unshift({ id: 'change-local-server', action: () => openLocalServerPicker(world) })
        options.push({ id: 'unlink-local-server', action: () => unlinkLocalServer(linked.id) })
    } else {
        options.unshift({ id: 'link-local-server', action: () => openLocalServerPicker(world) })
    }
    return options
}

function openLocalServerPicker(world: ServerWorld) {
    linkingWorld.value = world
    localServerPicker.value?.show()
}

function linkLocalServer(server: HomeLocalServer) {
    const world = linkingWorld.value
    if (!world) return
    try {
        runtime.setLinkedWorld(server.id, {
            instanceId: world.instance_id,
            address: world.address,
        })
        addNotification({
            type: 'success',
            title: formatMessage(messages.serversLinkedToast, { name: server.name }),
        })
    } catch (error) {
        handleError(error)
    } finally {
        linkingWorld.value = null
    }
}

function unlinkLocalServer(serverId: string) {
    try {
        runtime.setLinkedWorld(serverId, null)
        addNotification({ type: 'success', title: formatMessage(messages.serversUnlinkedToast) })
    } catch (error) {
        handleError(error)
    }
}

function serverKey(world: ServerWorld): string {
    return `${world.instance_id}:${world.index}:${world.address}`
}

function dataFor(world: ServerWorld) {
    return runtime.getServerData(world.instance_id, world.address)
}

/**
 * `useOnlineModeWarning().confirmOnlineModeLaunch`: an offline account cannot
 * join a server that has `online-mode` on, so the launch asks first.
 */
async function confirmOnlineModeLaunch(server: HomeLocalServer) {
    if (server.onlineMode === false) return true
    const accountName = runtime.activeOfflineAccountName.value
    if (accountName === null) return true
    const modal = onlineModeWarningModal.value
    if (!modal) return true
    return modal.show({ serverName: server.name, accountName })
}

async function joinServer(world: ServerWorld, instance: GameInstance) {
    const key = serverKey(world)
    startingServerKey.value = key

    try {
        const linked = linkedServer(world)
        if (linked) {
            if (linked.stopping) {
                addNotification({
                    type: 'info',
                    title: formatMessage(messages.serverActionStopping),
                })
                return
            }
            if (!(await confirmOnlineModeLaunch(linked))) return
            if (!linked.running && !linked.starting) {
                const started = await runtime.startServer(linked.id)
                if (!started) return
            }
            const ready = await runtime.waitUntilReady(linked.id)
            if (!ready) {
                addNotification({
                    type: 'error',
                    title: formatMessage(messages.serversLocalServerNotReady),
                })
                return
            }
        }
        await runtime.startJoinServer(instance, world.address)
    } catch (error) {
        handleError(error)
    } finally {
        startingServerKey.value = null
    }
}

async function stopInstance(instance: GameInstance) {
    await runtime.kill(instance.id).catch(handleError)
}

function unpinServer(world: ServerWorld) {
    try {
        runtime.setWorldDisplayStatus(world.instance_id, 'server', world.address, 'normal')
    } catch (error) {
        handleError(error)
    }
}

async function startLocalServer(serverId: string) {
    await runtime.startServer(serverId)
}

async function stopLocalServer(serverId: string) {
    await runtime.stopServer(serverId)
}

/** `ServerTerminateButton`: `useServers().killServer` ends the server process. */
const terminatingServerIds = ref(new Set<string>())
async function terminateLocalServer(serverId: string) {
    if (terminatingServerIds.value.has(serverId)) return
    terminatingServerIds.value = new Set([...terminatingServerIds.value, serverId])
    try {
        await runtime.killServer(serverId)
    } catch (error) {
        handleError(error)
    } finally {
        const next = new Set(terminatingServerIds.value)
        next.delete(serverId)
        terminatingServerIds.value = next
    }
}

function unpinLocalServer(serverId: string) {
    runtime.setServerHomePinned(serverId, false)
}
</script>

<template>
    <section
        class="home-pinned-servers flex h-full min-h-0 min-w-0 flex-col gap-3"
        :data-size="dashboardSize"
    >
        <div class="home-widget-heading flex h-8 min-w-0 flex-none items-center gap-2">
            <ServerIcon class="size-5 shrink-0 text-brand" aria-hidden="true" />
            <h2>{{ formatMessage(messages.serversPinned) }}</h2>
        </div>
        <div v-if="!hasServers" class="home-widget-empty">
            <ServerIcon aria-hidden="true" />
            <span>{{ formatMessage(messages.serversEmpty) }}</span>
        </div>
        <ul
            v-else
            class="home-server-list grid-auto-rows-max m-0 grid min-h-0 min-w-0 flex-1 list-none grid-cols-1 content-start gap-1 overflow-y-auto overflow-x-hidden p-0 pr-1"
        >
            <li v-for="server in servers" :key="serverKey(server.world)" class="min-w-0">
                <SmartClickable>
                    <template #clickable>
                        <router-link
                            :aria-label="server.world.name"
                            :to="`/instance/${encodeURIComponent(server.instance.id)}/worlds?highlight=${encodeURIComponent(getWorldIdentifier(server.world))}`"
                        />
                    </template>
                    <div class="home-server-row smart-clickable:highlight-on-hover group">
                        <div class="relative shrink-0">
                            <Avatar
                                :src="
                                    dataFor(server.world).status?.favicon ??
                                    (server.world.icon || undefined)
                                "
                                :tint-by="server.world.address"
                                size="36px"
                            />
                            <span
                                class="absolute -bottom-0.5 -right-0.5 size-2.5 rounded-full border-2 border-solid border-bg-raised"
                                :class="
                                    dataFor(server.world).refreshing
                                        ? 'animate-pulse bg-[var(--color-text-tertiary)]'
                                        : dataFor(server.world).status
                                          ? 'bg-brand-green'
                                          : 'bg-red'
                                "
                                aria-hidden="true"
                            />
                        </div>
                        <div class="flex min-w-0 flex-1 flex-col gap-0.5">
                            <span
                                class="truncate text-sm font-semibold text-[var(--color-text-primary)]"
                            >
                                {{ server.world.name }}
                            </span>
                            <span
                                v-if="dataFor(server.world).status"
                                class="flex min-w-0 items-center gap-1 text-xs text-[var(--color-text-tertiary)]"
                            >
                                <SignalIcon class="size-3 shrink-0" aria-hidden="true" />
                                <span class="truncate">
                                    {{
                                        formatMessage(messages.serversPlayersOnline, {
                                            online:
                                                dataFor(server.world).status?.players?.online ?? 0,
                                            max: dataFor(server.world).status?.players?.max ?? 0,
                                        })
                                    }}
                                </span>
                            </span>
                            <span
                                v-else-if="dataFor(server.world).refreshing"
                                class="truncate text-xs text-[var(--color-text-tertiary)]"
                            >
                                {{ server.world.address }}
                            </span>
                            <span
                                v-else
                                class="flex min-w-0 items-center gap-1 text-xs text-[var(--color-text-tertiary)]"
                            >
                                <NoSignalIcon class="size-3 shrink-0" aria-hidden="true" />
                                <span class="truncate">{{
                                    formatMessage(messages.serversOffline)
                                }}</span>
                            </span>
                        </div>
                        <div
                            class="smart-clickable:allow-pointer-events ml-auto flex shrink-0 items-center gap-0.5"
                        >
                            <Button
                                v-if="runningInstanceIds.includes(server.instance.id)"
                                v-tooltip="formatMessage(messages.serverActionStop)"
                                type="quiet"
                                size="2xs"
                                circular
                                icon-only
                                class="!text-red"
                                @click="stopInstance(server.instance)"
                                ><StopCircleIcon />
                            </Button>
                            <Button
                                v-else
                                v-tooltip="formatMessage(messages.serversJoin)"
                                type="quiet"
                                size="2xs"
                                circular
                                icon-only
                                class="!text-brand opacity-60 transition-opacity group-hover:opacity-100"
                                :disabled="startingServerKey === serverKey(server.world)"
                                @click="joinServer(server.world, server.instance)"
                                ><SpinnerIcon
                                    v-if="startingServerKey === serverKey(server.world)"
                                    class="animate-spin"
                                />
                                <PlayIcon v-else />
                            </Button>
                            <OverflowMenu
                                class="home-server-menu relative inline-flex size-6 min-w-6 shrink-0 cursor-pointer items-center justify-center rounded-full border border-solid border-transparent bg-transparent p-0 text-[var(--color-text-default)] transition-[background-color,color,filter,transform] duration-150 hover:bg-surface-4 hover:brightness-[--hover-brightness] active:scale-[0.97] [&>svg]:size-4"
                                :options="serverMenuOptions(server.world)"
                                :tooltip="formatMessage(messages.serversMoreOptions)"
                                :container="layer ?? undefined"
                            >
                                <MoreVerticalIcon />
                                <template #link-local-server>
                                    <LinkIcon aria-hidden="true" />
                                    {{ formatMessage(messages.serversLinkLocalServer) }}
                                </template>
                                <template #change-local-server>
                                    <LinkIcon aria-hidden="true" />
                                    {{ formatMessage(messages.serversChangeLocalServer) }}
                                </template>
                                <template #unlink-local-server>
                                    <UnlinkIcon aria-hidden="true" />
                                    {{ formatMessage(messages.serversUnlinkLocalServer) }}
                                </template>
                                <template #unpin>
                                    <PinIcon class="rotate-45" aria-hidden="true" />
                                    {{ formatMessage(messages.serversUnpin) }}
                                </template>
                            </OverflowMenu>
                        </div>
                    </div>
                </SmartClickable>
            </li>
            <li v-for="server in pinnedLocalServers" :key="'local-' + server.id" class="min-w-0">
                <SmartClickable>
                    <template #clickable>
                        <router-link
                            :aria-label="server.name"
                            :to="`/multiplayer/servers/${encodeURIComponent(server.id)}`"
                        />
                    </template>
                    <div class="home-server-row smart-clickable:highlight-on-hover group">
                        <div class="relative shrink-0">
                            <ManagedServerIcon
                                :icon-path="server.iconPath"
                                :server-type="server.serverType"
                                :server-id="server.id"
                                size="36px"
                            />
                            <span
                                class="absolute -bottom-0.5 -right-0.5 size-2.5 rounded-full border-2 border-solid border-bg-raised"
                                :class="server.running ? 'bg-brand-green' : 'bg-red'"
                                aria-hidden="true"
                            />
                        </div>
                        <div class="flex min-w-0 flex-1 flex-col gap-0.5">
                            <span
                                class="truncate text-sm font-semibold text-[var(--color-text-primary)]"
                                >{{ server.name }}</span
                            >
                            <span
                                class="flex min-w-0 items-center gap-1 text-xs text-[var(--color-text-tertiary)]"
                            >
                                <span
                                    class="shrink-0 rounded bg-surface-4 px-1 text-[10px] font-semibold"
                                >
                                    {{ formatMessage(messages.serversLocal) }}
                                </span>
                                <span class="truncate">
                                    {{
                                        server.port
                                            ? `localhost:${server.port}`
                                            : `${server.serverType} ${server.gameVersion}`
                                    }}
                                </span>
                            </span>
                        </div>
                        <div
                            class="smart-clickable:allow-pointer-events ml-auto flex shrink-0 items-center gap-0.5"
                        >
                            <Button
                                v-tooltip="
                                    formatMessage(
                                        server.stopping
                                            ? messages.serverActionStopping
                                            : server.running || server.starting
                                              ? messages.serverActionStop
                                              : messages.serverActionStart,
                                    )
                                "
                                type="quiet"
                                size="2xs"
                                circular
                                icon-only
                                :disabled="server.stopping"
                                :class="
                                    server.running || server.starting ? '!text-red' : '!text-brand'
                                "
                                @click="
                                    server.running || server.starting
                                        ? stopLocalServer(server.id)
                                        : startLocalServer(server.id)
                                "
                                ><StopCircleIcon v-if="server.running || server.starting" />
                                <PlayIcon v-else />
                            </Button>
                            <Button
                                v-if="server.stopping"
                                type="outlined"
                                color="red"
                                size="2xs"
                                :loading="terminatingServerIds.has(server.id)"
                                @click="terminateLocalServer(server.id)"
                            >
                                <XIcon />{{ formatMessage(messages.serverActionTerminate) }}
                            </Button>
                            <OverflowMenu
                                class="home-server-menu relative inline-flex size-6 min-w-6 shrink-0 cursor-pointer items-center justify-center rounded-full border border-solid border-transparent bg-transparent p-0 text-[var(--color-text-default)] transition-[background-color,color,filter,transform] duration-150 hover:bg-surface-4 hover:brightness-[--hover-brightness] active:scale-[0.97] [&>svg]:size-4"
                                :options="[
                                    { id: 'unpin', action: () => unpinLocalServer(server.id) },
                                ]"
                                :tooltip="formatMessage(messages.serversMoreOptions)"
                                :container="layer ?? undefined"
                            >
                                <MoreVerticalIcon />
                                <template #unpin>
                                    <PinIcon class="rotate-45" aria-hidden="true" />
                                    {{ formatMessage(messages.serversUnpin) }}
                                </template>
                            </OverflowMenu>
                        </div>
                    </div>
                </SmartClickable>
            </li>
        </ul>
        <HomeLocalServerPickerModal
            ref="localServerPicker"
            :servers="localServers"
            :selected-server-id="linkingWorld ? (linkedServer(linkingWorld)?.id ?? null) : null"
            @select="linkLocalServer"
        />
        <HomeOnlineModeWarningModal ref="onlineModeWarningModal" />
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

.home-server-row {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 0.625rem;
    padding: 0.5rem;
    border-radius: 6px;
    transition: background-color 120ms ease;
}

.home-pinned-servers[data-size='2x1'] .home-server-list,
.home-pinned-servers[data-size='2x2'] .home-server-list {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    column-gap: 0.5rem;
}

.home-pinned-servers[data-size='1x1'] {
    gap: 0.375rem;
}

.home-pinned-servers[data-size='1x1'] .home-widget-heading {
    height: 1.5rem;
}

.home-pinned-servers[data-size='1x1'] .home-server-row {
    gap: 0.5rem;
    padding: 0.375rem;
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
