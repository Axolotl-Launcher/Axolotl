<script setup lang="ts">
import ClipboardCopyIcon from '@modrinth/assets/icons/clipboard-copy.svg?component'
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import EyeIcon from '@modrinth/assets/icons/eye.svg?component'
import FolderOpenIcon from '@modrinth/assets/icons/folder-open.svg?component'
import IssuesIcon from '@modrinth/assets/icons/issues.svg?component'
import MoreVerticalIcon from '@modrinth/assets/icons/more-vertical.svg?component'
import NoSignalIcon from '@modrinth/assets/icons/no-signal.svg?component'
import PinIcon from '@modrinth/assets/icons/pin.svg?component'
import PlayIcon from '@modrinth/assets/icons/play.svg?component'
import SignalIcon from '@modrinth/assets/icons/signal.svg?component'
import SkullIcon from '@modrinth/assets/icons/skull.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import StopCircleIcon from '@modrinth/assets/icons/stop-circle.svg?component'
import UpdatedIcon from '@modrinth/assets/icons/updated.svg?component'
import UserIcon from '@modrinth/assets/icons/user.svg?component'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import OverflowMenu from '@modrinth/ui/src/components/base/OverflowMenu.vue'
import SmartClickable from '@modrinth/ui/src/components/base/SmartClickable.vue'
import { useFormatDateTime } from '@modrinth/ui/src/composables/format-date-time.ts'
import { useFormatNumber } from '@modrinth/ui/src/composables/format-number.ts'
import { useRelativeTime } from '@modrinth/ui/src/composables/how-ago.ts'
import { type MessageDescriptor, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { type Component, computed } from 'vue'
import { useRouter } from 'vue-router'

import { injectPreviewWindow } from '../../../lib/window'
import { getPingLevel, getWorldIdentifier, type HomeWorld } from '../home-utils'
import { homeMessages } from '../messages'
import { useHomeRuntime } from '../runtime'
import type { HomeServerStatus } from '../state'
import InstanceIcon from './InstanceIcon.vue'

/** Port of `components/ui/world/WorldItem.vue` as the Home widgets render it. */
const props = withDefaults(
    defineProps<{
        world: HomeWorld
        playingInstance?: boolean
        playingWorld?: boolean
        startingInstance?: boolean
        supportsServerQuickPlay?: boolean
        supportsWorldQuickPlay?: boolean
        currentProtocol?: { version: number; legacy: boolean } | null
        highlighted?: boolean

        // Server only
        refreshing?: boolean
        serverStatus?: HomeServerStatus
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
        highlighted: false,

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

const emit = defineEmits<{
    (e: 'play' | 'play-instance' | 'update' | 'stop' | 'refresh' | 'edit' | 'delete'): void
    (e: 'open-folder', world: HomeWorld): void
}>()

const { formatMessage } = useVIntl()
const formatRelativeTime = useRelativeTime()
const formatNumber = useFormatNumber()
const formatDateTime = useFormatDateTime({
    timeStyle: 'short',
    dateStyle: 'long',
})
const { addNotification, handleError } = injectNotificationManager()
const { layer } = injectPreviewWindow()
const runtime = useHomeRuntime()
const router = useRouter()
const messages = homeMessages

const playingOtherWorld = computed(() => props.playingInstance && !props.playingWorld)
const hasPlayersTooltip = computed(
    () => !!props.serverStatus?.players?.sample && props.serverStatus.players.sample.length > 0,
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
const shortcutInstanceId = computed(() => props.shortcutInstanceId ?? props.instanceId)
const homePinTarget = computed(() => props.shortcutInstanceId ?? props.instanceId)
const pinnedToHome = computed(() => props.world.display_status === 'favorite')
const playersTooltip = computed(() =>
    hasPlayersTooltip.value
        ? (props.serverStatus?.players?.sample ?? []).map((player) => player.name).join('\n')
        : null,
)

function updateHomePin() {
    if (!homePinTarget.value) return
    try {
        runtime.setWorldDisplayStatus(
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
        const shortcutPath = await runtime.createInstanceShortcut(
            props.world.name,
            shortcutInstanceId.value,
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

const playTooltip = computed(() => {
    if (props.world.type === 'server') {
        if (!props.supportsServerQuickPlay) return formatMessage(messages.worldNoServerQuickPlay)
        if (playingOtherWorld.value) return formatMessage(messages.worldGameAlreadyOpen)
        if (!props.serverStatus) return formatMessage(messages.worldNoContact)
        if (serverIncompatible.value) return formatMessage(messages.worldIncompatibleServer)
        return null
    }
    if (!props.supportsWorldQuickPlay) return formatMessage(messages.worldNoSingleplayerQuickPlay)
    if (playingOtherWorld.value || locked.value) return formatMessage(messages.worldGameAlreadyOpen)
    return null
})

const menuOptions = computed(() => [
    {
        id: 'play-instance',
        section: 'primary' as const,
        shown: !!props.instanceId,
        disabled: props.playingInstance,
        action: () => emit('play-instance'),
    },
    {
        id: 'open-instance',
        section: 'navigate' as const,
        shown: !!props.instanceId,
        action: () => router.push(`/instance/${encodeURIComponent(props.instanceId ?? '')}`),
    },
    {
        id: 'refresh',
        section: 'manage' as const,
        shown: props.world.type === 'server',
        action: () => emit('refresh'),
    },
    {
        id: 'copy-address',
        section: 'navigate' as const,
        shown: props.world.type === 'server',
        action: () => {
            if (props.world.type === 'server') void runtime.copyToClipboard(props.world.address)
        },
    },
    {
        id: 'edit',
        section: 'manage' as const,
        action: () => emit('edit'),
        shown: !props.instanceId,
        disabled: locked.value || props.managed,
    },
    {
        id: 'open-folder',
        section: 'navigate' as const,
        shown: props.world.type === 'singleplayer',
        action: () => {
            if (props.world.type === 'singleplayer') emit('open-folder', props.world)
        },
    },
    {
        divider: true,
        shown: !!props.instanceId,
    },
    {
        section: 'pin' as const,
        id: pinnedToHome.value ? 'unpin-home' : 'pin-home',
        shown: !!homePinTarget.value,
        action: updateHomePin,
    },
    {
        id: 'create-shortcut',
        section: 'navigate' as const,
        shown: !!shortcutInstanceId.value,
        action: () => createShortcut(),
    },
    {
        divider: true,
        shown: !props.instanceId,
    },
    {
        id: 'delete',
        section: 'danger' as const,
        color: 'red' as const,
        hoverFilled: true,
        action: () => emit('delete'),
        shown: !props.instanceId,
        disabled: locked.value || props.managed,
    },
])
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
            class="smart-clickable:highlight-on-hover grid grid-cols-[auto_minmax(0,3fr)_minmax(0,4fr)_auto] items-center gap-2 rounded-lg"
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
            <div class="flex h-full flex-col justify-between">
                <div class="flex items-center gap-2">
                    <div
                        class="smart-clickable:underline-on-hover truncate text-lg font-bold text-[var(--color-text-primary)]"
                    >
                        {{ world.name }}
                    </div>
                    <div
                        v-if="world.type === 'singleplayer'"
                        class="flex flex-nowrap items-center gap-1 whitespace-nowrap text-sm font-semibold text-[var(--color-text-tertiary)]"
                    >
                        <UserIcon
                            aria-hidden="true"
                            class="h-4 w-4 shrink-0 text-[var(--color-text-tertiary)]"
                            stroke-width="3px"
                        />
                        {{ formatMessage(commonMessages.singleplayerLabel) }}
                    </div>
                    <div
                        v-else-if="world.type === 'server'"
                        class="flex flex-nowrap items-center gap-1 whitespace-nowrap text-sm font-semibold text-[var(--color-text-tertiary)]"
                    >
                        <template v-if="refreshing">
                            <SpinnerIcon aria-hidden="true" class="shrink-0 animate-spin" />
                            {{ formatMessage(commonMessages.loadingLabel) }}
                        </template>
                        <template v-else-if="serverStatus">
                            <template v-if="serverIncompatible">
                                <IssuesIcon class="shrink-0 text-orange" aria-hidden="true" />
                                <span class="text-orange">
                                    {{
                                        formatMessage(messages.worldItemIncompatibleVersion, {
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
                            <span
                                v-tooltip="playersTooltip"
                                :class="{
                                    'smart-clickable:allow-pointer-events cursor-help':
                                        hasPlayersTooltip,
                                }"
                            >
                                {{
                                    formatMessage(messages.worldItemPlayersOnline, {
                                        count: formatNumber(serverStatus.players?.online ?? 0),
                                    })
                                }}
                            </span>
                        </template>
                        <template v-else>
                            <NoSignalIcon aria-hidden="true" stroke-width="3px" class="shrink-0" />
                            {{ formatMessage(messages.worldItemOffline) }}
                        </template>
                    </div>
                </div>
                <div class="flex items-center gap-2 text-sm text-[var(--color-text-tertiary)]">
                    <div
                        v-tooltip="world.last_played ? formatDateTime(world.last_played) : null"
                        class="w-fit shrink-0"
                        :class="{
                            'smart-clickable:allow-pointer-events cursor-help': world.last_played,
                        }"
                    >
                        <template v-if="world.last_played">
                            {{
                                formatMessage(commonMessages.playedLabel, {
                                    ago: formatRelativeTime(world.last_played),
                                })
                            }}
                        </template>
                        <template v-else>
                            {{ formatMessage(messages.worldItemNotPlayedYet) }}
                        </template>
                    </div>
                    <template v-if="instanceId">
                        <span v-if="dashboardDensity !== 'compact'" aria-hidden="true">•</span>
                        <router-link
                            class="smart-clickable:allow-pointer-events flex items-center gap-1 truncate text-[var(--color-text-tertiary)] hover:underline"
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
                class="flex items-center justify-center gap-1 text-center font-semibold"
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
                    <!-- eslint-disable-next-line vue/no-v-html -- the app renders the MOTD the same way -->
                    <div
                        v-else-if="renderedMotd"
                        class="motd-renderer font-minecraft line-clamp-2 font-normal leading-5 text-[var(--color-text-tertiary)]"
                        v-html="renderedMotd"
                    />
                    <div
                        v-else-if="!serverStatus"
                        class="font-minecraft font-normal leading-5 text-red"
                    >
                        {{ formatMessage(messages.worldCantConnect) }}
                    </div>
                    <div
                        v-else
                        class="font-minecraft font-normal leading-5 text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.worldAMinecraftServer) }}
                    </div>
                </template>
                <template v-else-if="world.type === 'singleplayer' && gameMode">
                    <template v-if="world.hardcore">
                        <SkullIcon aria-hidden="true" class="h-4 w-4 shrink-0" />
                        {{ formatMessage(messages.worldHardcore) }}
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
            <div class="smart-clickable:allow-pointer-events flex justify-end gap-1">
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
                        v-tooltip="playTooltip"
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
                    :options="menuOptions"
                    :container="layer ?? undefined"
                >
                    <MoreVerticalIcon aria-hidden="true" />
                    <template #play-instance>
                        <PlayIcon aria-hidden="true" />
                        {{ formatMessage(messages.worldPlayInstance) }}
                    </template>
                    <template #open-instance>
                        <EyeIcon aria-hidden="true" />
                        {{ formatMessage(messages.worldViewInstance) }}
                    </template>
                    <template #open-folder>
                        <FolderOpenIcon aria-hidden="true" />
                        {{ formatMessage(commonMessages.openFolderButton) }}
                    </template>
                    <template #copy-address>
                        <ClipboardCopyIcon aria-hidden="true" />
                        {{ formatMessage(messages.worldCopyAddress) }}
                    </template>
                    <template #refresh>
                        <UpdatedIcon aria-hidden="true" />
                        {{ formatMessage(commonMessages.refreshButton) }}
                    </template>
                    <template #create-shortcut>
                        <ExternalIcon aria-hidden="true" />
                        {{ formatMessage(messages.worldCreateShortcut) }}
                    </template>
                    <template #pin-home>
                        <PinIcon aria-hidden="true" />
                        {{ formatMessage(messages.pinToHome) }}
                    </template>
                    <template #unpin-home>
                        <PinIcon class="rotate-45" aria-hidden="true" />
                        {{ formatMessage(messages.unpinFromHome) }}
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
        position: absolute;
        inset: 0;
        border-radius: 0.75rem;
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

.font-minecraft {
    font-family: 'bundled-minecraft-font-mrapp', monospace;
}

.launcher-preview-window.light-mode .motd-renderer {
    filter: brightness(0.75);
}
</style>
