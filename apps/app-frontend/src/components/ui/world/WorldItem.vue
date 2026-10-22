<script setup lang="ts">
import {
    ClipboardCopyIcon,
    EditIcon,
    ExternalIcon,
    EyeIcon,
    FolderOpenIcon,
    IssuesIcon,
    MoreVerticalIcon,
    NoSignalIcon,
    PinIcon,
    PlayIcon,
    SignalIcon,
    SkullIcon,
    SpinnerIcon,
    StopCircleIcon,
    TrashIcon,
    UpdatedIcon,
    UserIcon,
} from '@modrinth/assets'
import type { MessageDescriptor } from '@modrinth/ui'
import {
    Avatar,
    Button,
    commonMessages,
    defineMessages,
    injectNotificationManager,
    OverflowMenu,
    SmartClickable,
    TagItem,
    useFormatDateTime,
    useFormatNumber,
    useRelativeTime,
    useVIntl,
} from '@modrinth/ui'
import { getPingLevel } from '@modrinth/utils'
import dayjs from 'dayjs'
import type { Component } from 'vue'
import { computed } from 'vue'
import { useRouter } from 'vue-router'

import HeadlessTooltip from '@/components/ui/headless/HeadlessTooltip.vue'
import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import { copyToClipboard, createInstanceShortcut } from '@/helpers/utils'
import type {
    ProtocolVersion,
    ServerStatus,
    ServerWorld,
    SingleplayerWorld,
    World,
} from '@/helpers/worlds.ts'
import { getWorldIdentifier, set_world_display_status } from '@/helpers/worlds.ts'

import { LockIcon } from '../../../../../../packages/assets/generated-icons'

const { formatMessage } = useVIntl()
const formatRelativeTime = useRelativeTime()
const formatNumber = useFormatNumber()
const formatDateTime = useFormatDateTime({
    timeStyle: 'short',
    dateStyle: 'long',
})

const router = useRouter()
const { addNotification, handleError } = injectNotificationManager()

const emit = defineEmits<{
    (e: 'play' | 'play-instance' | 'update' | 'stop' | 'refresh' | 'edit' | 'delete'): void
    (e: 'open-folder', world: SingleplayerWorld): void
}>()

const props = withDefaults(
    defineProps<{
        world: World
        playingInstance?: boolean
        playingWorld?: boolean
        startingInstance?: boolean
        supportsServerQuickPlay?: boolean
        supportsWorldQuickPlay?: boolean
        currentProtocol?: ProtocolVersion | null
        highlighted?: boolean

        // Server only
        refreshing?: boolean
        serverStatus?: ServerStatus
        renderedMotd?: string

        // Singleplayer only
        gameMode?: {
            icon: Component
            message: MessageDescriptor
        }

        managed?: boolean

        // Instance
        instanceId?: string
        instanceName?: string
        instanceIcon?: string
        instanceLoader?: string
        shortcutInstanceId?: string
        flat?: boolean
        dashboardDensity?: 'compact' | 'comfortable'
    }>(),
    {
        playingInstance: false,
        playingWorld: false,
        startingInstance: false,
        supportsServerQuickPlay: true,
        supportsWorldQuickPlay: false,
        currentProtocol: null,

        refreshing: false,
        serverStatus: undefined,
        renderedMotd: undefined,

        gameMode: undefined,
        managed: false,

        instanceId: undefined,
        instanceName: undefined,
        instanceIcon: undefined,
        instanceLoader: undefined,
        shortcutInstanceId: undefined,
        flat: false,
        dashboardDensity: undefined,
    },
)

const playingOtherWorld = computed(() => props.playingInstance && !props.playingWorld)
const hasPlayersTooltip = computed(
    () => !!props.serverStatus?.players?.sample && props.serverStatus.players?.sample?.length > 0,
)
const serverIncompatible = computed(
    () =>
        !!props.serverStatus &&
        !!props.serverStatus.version?.protocol &&
        !!props.currentProtocol &&
        (props.serverStatus.version.protocol !== props.currentProtocol.version ||
            props.serverStatus.version.legacy !== props.currentProtocol.legacy),
)

const locked = computed(() => props.world.type === 'singleplayer' && props.world.locked)
const managed = computed(() => props.managed)
const shortcutInstanceId = computed(() => props.shortcutInstanceId ?? props.instanceId)
const homePinTarget = computed(() => props.shortcutInstanceId ?? props.instanceId)
const pinnedToHome = computed(() => props.world.display_status === 'favorite')

async function updateHomePin() {
    if (!homePinTarget.value) return

    try {
        await set_world_display_status(
            homePinTarget.value,
            props.world.type,
            getWorldIdentifier(props.world),
            pinnedToHome.value ? 'normal' : 'favorite',
        )
        emit('update')
    } catch (error) {
        handleError(error)
    }
}

async function createShortcut() {
    if (!shortcutInstanceId.value) return

    try {
        const shortcutPath = await createInstanceShortcut(
            props.world.name,
            shortcutInstanceId.value,
            props.world.type === 'server'
                ? { server: (props.world as ServerWorld).address }
                : { singleplayerWorld: (props.world as SingleplayerWorld).path },
        )
        if (!shortcutPath) return

        addNotification({
            type: 'success',
            title: 'Shortcut created',
        })
    } catch (error) {
        addNotification({
            type: 'error',
            title: 'Failed to create shortcut',
            text: error instanceof Error ? error.message : '',
        })
    }
}

const messages = defineMessages({
    hardcore: {
        id: 'instance.worlds.hardcore',
        defaultMessage: 'Hardcore mode',
    },
    cantConnect: {
        id: 'instance.worlds.cant_connect',
        defaultMessage: "Can't connect to server",
    },
    aMinecraftServer: {
        id: 'instance.worlds.a_minecraft_server',
        defaultMessage: 'A Minecraft Server',
    },
    noServerQuickPlay: {
        id: 'instance.worlds.no_server_quick_play',
        defaultMessage: 'You can only jump straight into servers on Minecraft Alpha 1.0.5+',
    },
    noSingleplayerQuickPlay: {
        id: 'instance.worlds.no_singleplayer_quick_play',
        defaultMessage: 'You can only jump straight into singleplayer worlds on Minecraft 1.20+',
    },
    gameAlreadyOpen: {
        id: 'instance.worlds.game_already_open',
        defaultMessage: 'Instance is already open',
    },
    noContact: {
        id: 'instance.worlds.no_contact',
        defaultMessage: "Server couldn't be contacted",
    },
    incompatibleServer: {
        id: 'instance.worlds.incompatible_server',
        defaultMessage: 'Server is incompatible',
    },
    copyAddress: {
        id: 'instance.worlds.copy_address',
        defaultMessage: 'Copy address',
    },
    viewInstance: {
        id: 'instance.worlds.view_instance',
        defaultMessage: 'View instance',
    },
    playInstance: {
        id: 'instance.worlds.play_instance',
        defaultMessage: 'Play instance',
    },
    pinToHome: { id: 'app.instances.pin-to-home', defaultMessage: 'Pin to Home' },
    unpinFromHome: { id: 'app.instances.unpin-from-home', defaultMessage: 'Unpin from Home' },
    worldInUse: {
        id: 'instance.worlds.world_in_use',
        defaultMessage: 'World is in use',
    },
    createShortcut: {
        id: 'instance.worlds.create_shortcut',
        defaultMessage: 'Create shortcut',
    },
    linkedServer: {
        id: 'instance.worlds.linked_server',
        defaultMessage: 'Managed by server project',
    },
    incompatibleVersion: {
        id: 'app.world.world-item.incompatible-version',
        defaultMessage: 'Incompatible version {version}',
    },
    playersOnline: {
        id: 'app.world.world-item.players-online',
        defaultMessage: '{count} online',
    },
    offline: {
        id: 'app.world.world-item.offline',
        defaultMessage: 'Offline',
    },
    notPlayedYet: {
        id: 'app.world.world-item.not-played-yet',
        defaultMessage: 'Not played yet',
    },
})
</script>
<template>
    <SmartClickable>
        <template v-if="instanceId" #clickable>
            <router-link
                class="no-click-animation"
                :to="`/instance/${encodeURIComponent(instanceId)}/worlds?highlight=${encodeURIComponent(getWorldIdentifier(world))}`"
            />
        </template>
        <div
            class="grid grid-cols-[auto_minmax(0,3fr)_minmax(0,4fr)_auto] items-center gap-2 rounded-lg smart-clickable:highlight-on-hover"
            :class="[
                {
                    'world-item-highlighted': highlighted,
                    'world-item-dashboard-compact grid grid-cols-[auto_minmax(0,1fr)_auto] gap-2 p-1.5':
                        dashboardDensity === 'compact',
                    'world-item-dashboard-comfortable p-2': dashboardDensity === 'comfortable',
                },
                flat ? 'px-2 py-2 hover:bg-surface-4' : 'card-shadow bg-surface-2 p-3',
            ]"
        >
            <Avatar
                :src="
                    world.type === 'server' && serverStatus
                        ? (serverStatus.favicon ?? world.icon)
                        : world.icon
                "
                :size="dashboardDensity === 'compact' ? '40px' : '48px'"
            />
            <div class="flex flex-col justify-between h-full">
                <div class="flex items-center gap-2">
                    <div
                        class="text-lg text-[var(--color-text-primary)] font-bold truncate smart-clickable:underline-on-hover"
                    >
                        {{ world.name }}
                    </div>
                    <TagItem
                        v-if="managed"
                        v-tooltip="formatMessage(messages.linkedServer)"
                        class="border !border-solid border-blue bg-highlight-blue text-xs"
                        :style="`--_color: var(--color-blue)`"
                    >
                        <LockIcon aria-hidden="true" class="h-5 w-5" />
                    </TagItem>
                    <div
                        v-if="world.type === 'singleplayer'"
                        class="text-sm text-[var(--color-text-tertiary)] flex items-center gap-1 font-semibold flex-nowrap whitespace-nowrap"
                    >
                        <UserIcon
                            aria-hidden="true"
                            class="h-4 w-4 text-[var(--color-text-tertiary)] shrink-0"
                            stroke-width="3px"
                        />
                        {{ formatMessage(commonMessages.singleplayerLabel) }}
                    </div>
                    <div
                        v-else-if="world.type === 'server'"
                        class="text-sm text-[var(--color-text-tertiary)] flex items-center gap-1 font-semibold flex-nowrap whitespace-nowrap"
                    >
                        <template v-if="refreshing">
                            <SpinnerIcon aria-hidden="true" class="animate-spin shrink-0" />
                            {{ formatMessage(commonMessages.loadingLabel) }}
                        </template>
                        <template v-else-if="serverStatus">
                            <template v-if="serverIncompatible">
                                <IssuesIcon class="shrink-0 text-orange" aria-hidden="true" />
                                <span class="text-orange">
                                    {{
                                        formatMessage(messages.incompatibleVersion, {
                                            version: serverStatus.version?.name,
                                        })
                                    }}
                                </span>
                            </template>
                            <template v-else>
                                <SignalIcon
                                    v-tooltip="serverStatus ? `${serverStatus.ping}ms` : null"
                                    aria-hidden="true"
                                    :style="`--_signal-${getPingLevel(serverStatus.ping || 0)}: var(--color-green)`"
                                    stroke-width="3px"
                                    class="shrink-0"
                                    :class="{
                                        'smart-clickable:allow-pointer-events': serverStatus,
                                    }"
                                />
                            </template>
                            <HeadlessTooltip
                                :disabled="!hasPlayersTooltip"
                                :delay-ms="200"
                                content-class="tooltip-surface"
                                arrow-class="tooltip-arrow"
                            >
                                <span :class="{ 'cursor-help': hasPlayersTooltip }">
                                    {{
                                        formatMessage(messages.playersOnline, {
                                            count: formatNumber(serverStatus.players?.online ?? 0),
                                        })
                                    }}
                                </span>
                                <template #content>
                                    <div class="flex flex-col gap-1">
                                        <span
                                            v-for="player in serverStatus.players?.sample"
                                            :key="player.name"
                                        >
                                            {{ player.name }}
                                        </span>
                                    </div>
                                </template>
                            </HeadlessTooltip>
                        </template>
                        <template v-else>
                            <NoSignalIcon aria-hidden="true" stroke-width="3px" class="shrink-0" />
                            {{ formatMessage(messages.offline) }}
                        </template>
                    </div>
                </div>
                <div class="flex items-center gap-2 text-sm text-[var(--color-text-tertiary)]">
                    <div
                        v-tooltip="world.last_played ? formatDateTime(world.last_played) : null"
                        class="w-fit shrink-0"
                        :class="{
                            'cursor-help smart-clickable:allow-pointer-events': world.last_played,
                        }"
                    >
                        <template v-if="world.last_played">
                            {{
                                formatMessage(commonMessages.playedLabel, {
                                    ago: formatRelativeTime(dayjs(world.last_played).toISOString()),
                                })
                            }}
                        </template>
                        <template v-else> {{ formatMessage(messages.notPlayedYet) }} </template>
                    </div>
                    <template v-if="instanceId">
                        <span v-if="dashboardDensity !== 'compact'" aria-hidden="true">•</span>
                        <router-link
                            class="flex items-center gap-1 truncate hover:underline text-[var(--color-text-tertiary)] smart-clickable:allow-pointer-events"
                            :to="`/instance/${instanceId}`"
                        >
                            <InstanceIcon
                                :icon-path="instanceIcon"
                                :instance-id="instanceId"
                                :loader="instanceLoader"
                                size="16px"
                                class="shrink-0"
                            />
                            <span class="truncate">{{ instanceName }}</span>
                        </router-link>
                    </template>
                </div>
            </div>
            <div
                class="font-semibold flex items-center gap-1 justify-center text-center"
                :class="
                    world.type === 'singleplayer' && world.hardcore
                        ? `text-red`
                        : 'text-[var(--color-text-tertiary)]'
                "
            >
                <template v-if="world.type === 'server'">
                    <template v-if="refreshing">
                        <SpinnerIcon aria-hidden="true" class="animate-spin" />
                        {{ formatMessage(commonMessages.loadingLabel) }}
                    </template>
                    <div
                        v-else-if="renderedMotd"
                        class="motd-renderer font-normal font-minecraft line-clamp-2 text-[var(--color-text-tertiary)] leading-5"
                        v-html="renderedMotd"
                    />
                    <div
                        v-else-if="!serverStatus"
                        class="font-normal font-minecraft text-red leading-5"
                    >
                        {{ formatMessage(messages.cantConnect) }}
                    </div>
                    <div
                        v-else
                        class="font-normal font-minecraft text-[var(--color-text-tertiary)] leading-5"
                    >
                        {{ formatMessage(messages.aMinecraftServer) }}
                    </div>
                </template>
                <template v-else-if="world.type === 'singleplayer' && gameMode">
                    <template v-if="world.hardcore">
                        <SkullIcon aria-hidden="true" class="h-4 w-4 shrink-0" />
                        {{ formatMessage(messages.hardcore) }}
                    </template>
                    <template v-else>
                        <component
                            :is="gameMode.icon"
                            aria-hidden="true"
                            class="h-4 w-4 shrink-0"
                        />
                        {{ formatMessage(gameMode.message) }}
                    </template>
                </template>
            </div>
            <div class="flex gap-1 justify-end smart-clickable:allow-pointer-events">
                <Button
                    v-if="(playingWorld || (locked && playingInstance)) && !startingInstance"
                    color="red"
                    :circular="dashboardDensity === 'compact'"
                >
                    <button @click="emit('stop')">
                        <StopCircleIcon aria-hidden="true" />
                        <span v-if="dashboardDensity !== 'compact'">
                            {{ formatMessage(commonMessages.stopButton) }}
                        </span>
                    </button>
                </Button>
                <Button v-else :circular="dashboardDensity === 'compact'">
                    <button
                        v-tooltip="
                            world.type === 'server'
                                ? !supportsServerQuickPlay
                                    ? formatMessage(messages.noServerQuickPlay)
                                    : playingOtherWorld
                                      ? formatMessage(messages.gameAlreadyOpen)
                                      : !serverStatus
                                        ? formatMessage(messages.noContact)
                                        : serverIncompatible
                                          ? formatMessage(messages.incompatibleServer)
                                          : null
                                : !supportsWorldQuickPlay
                                  ? formatMessage(messages.noSingleplayerQuickPlay)
                                  : playingOtherWorld || locked
                                    ? formatMessage(messages.gameAlreadyOpen)
                                    : null
                        "
                        :disabled="
                            playingOtherWorld ||
                            startingInstance ||
                            (world.type == 'server' && !supportsServerQuickPlay) ||
                            (world.type == 'singleplayer' && !supportsWorldQuickPlay)
                        "
                        @click="emit('play')"
                    >
                        <SpinnerIcon v-if="startingInstance && playingWorld" class="animate-spin" />
                        <PlayIcon v-else aria-hidden="true" />
                        <span v-if="dashboardDensity !== 'compact'">
                            {{ formatMessage(commonMessages.playButton) }}
                        </span>
                    </button>
                </Button>
                <OverflowMenu
                    type="button"
                    class="relative inline-flex size-9 min-w-9 shrink-0 cursor-pointer items-center justify-center rounded-full border border-solid border-transparent bg-transparent p-0 text-[var(--color-text-default)] transition-[background-color,color,filter,transform] duration-150 hover:bg-surface-4 hover:brightness-[--hover-brightness] active:scale-[0.97] [&>svg]:size-5"
                    :options="[
                        {
                            id: 'play-instance',
                            section: 'primary',
                            shown: !!instanceId,
                            disabled: playingInstance,
                            action: () => emit('play-instance'),
                        },
                        {
                            id: 'open-instance',
                            section: 'navigate',
                            shown: !!instanceId,
                            action: () =>
                                router.push(`/instance/${encodeURIComponent(instanceId)}`),
                        },
                        {
                            id: 'refresh',
                            section: 'manage',
                            shown: world.type === 'server',
                            action: () => emit('refresh'),
                        },
                        {
                            id: 'copy-address',
                            section: 'navigate',
                            shown: world.type === 'server',
                            action: () => copyToClipboard((world as ServerWorld).address),
                        },
                        {
                            id: 'edit',
                            section: 'manage',
                            action: () => emit('edit'),
                            shown: !instanceId,
                            disabled: locked || managed,
                            tooltip: locked
                                ? formatMessage(messages.worldInUse)
                                : managed
                                  ? formatMessage(messages.linkedServer)
                                  : undefined,
                        },
                        {
                            id: 'open-folder',
                            section: 'navigate',
                            shown: world.type === 'singleplayer',
                            action: () =>
                                world.type === 'singleplayer' ? emit('open-folder', world) : {},
                        },
                        {
                            divider: true,
                            shown: !!instanceId,
                        },
                        {
                            section: 'pin',
                            id: pinnedToHome ? 'unpin-home' : 'pin-home',
                            shown: !!homePinTarget,
                            action: updateHomePin,
                        },
                        {
                            id: 'create-shortcut',
                            section: 'navigate',
                            shown: !!shortcutInstanceId,
                            action: () => createShortcut(),
                        },
                        {
                            divider: true,
                            shown: !instanceId,
                        },
                        {
                            id: 'delete',
                            section: 'danger',
                            color: 'red',
                            hoverFilled: true,
                            action: () => emit('delete'),
                            shown: !instanceId,
                            disabled: locked || managed,
                            tooltip: locked
                                ? formatMessage(messages.worldInUse)
                                : managed
                                  ? formatMessage(messages.linkedServer)
                                  : undefined,
                        },
                    ]"
                >
                    <MoreVerticalIcon aria-hidden="true" />
                    <template #play-instance>
                        <PlayIcon aria-hidden="true" />
                        {{ formatMessage(messages.playInstance) }}
                    </template>
                    <template #open-instance>
                        <EyeIcon aria-hidden="true" />
                        {{ formatMessage(messages.viewInstance) }}
                    </template>
                    <template #edit>
                        <EditIcon aria-hidden="true" />
                        {{ formatMessage(commonMessages.editButton) }}
                    </template>
                    <template #open-folder>
                        <FolderOpenIcon aria-hidden="true" />
                        {{ formatMessage(commonMessages.openFolderButton) }}
                    </template>
                    <template #copy-address>
                        <ClipboardCopyIcon aria-hidden="true" />
                        {{ formatMessage(messages.copyAddress) }}
                    </template>
                    <template #refresh>
                        <UpdatedIcon aria-hidden="true" />
                        {{ formatMessage(commonMessages.refreshButton) }}
                    </template>
                    <template #create-shortcut>
                        <ExternalIcon aria-hidden="true" />
                        {{ formatMessage(messages.createShortcut) }}
                    </template>
                    <template #pin-home>
                        <PinIcon aria-hidden="true" />
                        {{ formatMessage(messages.pinToHome) }}
                    </template>
                    <template #unpin-home>
                        <PinIcon class="rotate-45" aria-hidden="true" />
                        {{ formatMessage(messages.unpinFromHome) }}
                    </template>
                    <template #delete>
                        <TrashIcon aria-hidden="true" />
                        {{
                            formatMessage(
                                world.type === 'server'
                                    ? commonMessages.removeButton
                                    : commonMessages.deleteLabel,
                            )
                        }}
                    </template>
                </OverflowMenu>
            </div>
        </div>
    </SmartClickable>
</template>
<style scoped lang="scss">
.world-item-dashboard-compact {
    > :nth-child(2) > :first-child > :not(:first-child),
    > :nth-child(3) {
        display: none;
    }

    > :nth-child(2) > :first-child > :first-child {
        font-size: 0.875rem;
    }

    > :nth-child(2) > :last-child {
        font-size: 0.7rem;
    }

    > :last-child {
        gap: 0.125rem;
    }
}

.world-item-highlighted {
    position: relative;
    animation: fade-highlight 4s ease-out;
    filter: brightness(1);

    &::before {
        @apply rounded-xl inset-0 absolute;

        animation: fade-opacity 4s ease-out;

        content: '';
        box-shadow: 0 0 8px 2px var(--color-brand);
        border: 1.5px solid var(--color-brand);
        opacity: 0;
        pointer-events: none;
    }
}

@keyframes fade-highlight {
    0% {
        filter: brightness(1.25);
    }
    75% {
        filter: brightness(1.25);
    }
    100% {
        filter: brightness(1);
    }
}

@keyframes fade-opacity {
    0% {
        opacity: 0.5;
    }
    75% {
        opacity: 0.5;
    }
    100% {
        opacity: 0;
    }
}

.light-mode .motd-renderer {
    filter: brightness(0.75);
}
</style>
