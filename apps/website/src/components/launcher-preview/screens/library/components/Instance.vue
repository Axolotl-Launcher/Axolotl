<script setup lang="ts">
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import GameIcon from '@modrinth/assets/icons/game.svg?component'
import PlayIcon from '@modrinth/assets/icons/play.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import StopCircleIcon from '@modrinth/assets/icons/stop-circle.svg?component'
import TimerIcon from '@modrinth/assets/icons/timer.svg?component'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useRelativeTime } from '@modrinth/ui/src/composables/how-ago.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed } from 'vue'
import { useRouter } from 'vue-router'

import { getDisplayInstanceIcon } from '../../../lib/instance-icons'
import type { GameInstance } from '../../../lib/types'
import InstanceIcon from '../../../shell/InstanceIcon.vue'
import { useLocalJobs } from '../../downloads/local-jobs'
import { messages } from '../messages'
import { useLibrary } from '../state'

/**
 * Port of `components/ui/Instance.vue` (compact, library and standard
 * variants). `run()` and `kill()` go through the preview's process simulation:
 * the spinner covers the launch preparation and the stop button appears once
 * the game is running, as with the app's `process_listener` events.
 */
const props = withDefaults(
    defineProps<{
        instance: GameInstance
        compact?: boolean
        flat?: boolean
        variant?: string
        playing?: boolean
        first?: boolean
        disabled?: boolean
    }>(),
    {
        compact: false,
        flat: false,
        variant: 'standard',
        playing: undefined,
        first: false,
        disabled: false,
    },
)

const { formatMessage } = useVIntl()
const formatRelativeTime = useRelativeTime()
const router = useRouter()
const library = useLibrary()
const localJobs = useLocalJobs()

const isPlaying = computed(() => props.playing ?? library.isRunning(props.instance.id))
const displayIcon = computed(() =>
    getDisplayInstanceIcon(props.instance.icon_path, props.instance.loader),
)
const modLoading = computed(() => library.isLaunching(props.instance.id))
const installing = computed(() => props.instance.install_stage.includes('installing'))
const installed = computed(() => props.instance.install_stage === 'installed')

function seeInstance() {
    void router.push(`/instance/${encodeURIComponent(props.instance.id)}`)
}

function play(event?: MouseEvent) {
    event?.stopPropagation()
    library.launch(props.instance)
}

function stop(event?: MouseEvent) {
    event?.stopPropagation()
    library.stop(props.instance.id)
}

/** `install_existing_instance`: reinstalls the game files in a download job. */
function repair(event?: MouseEvent) {
    event?.stopPropagation()
    localJobs.repair(props.instance)
}

defineExpose({ play, stop, seeInstance })
</script>

<template>
    <template v-if="compact">
        <div
            class="grid cursor-pointer grid-cols-[auto_1fr_auto] items-center gap-2 rounded-lg transition-colors"
            :class="
                flat
                    ? 'px-2 py-2 hover:bg-surface-4'
                    : 'card-shadow bg-surface-2 p-3 pl-4 hover:brightness-90'
            "
            @click="seeInstance"
        >
            <InstanceIcon
                size="48px"
                :icon-path="instance.icon_path"
                :instance-id="instance.id"
                :loader="instance.loader"
                :alt="instance.name"
            />
            <div
                class="flex h-full items-center font-bold leading-normal text-[var(--color-text-primary)]"
            >
                <span class="line-clamp-2">{{ instance.name }}</span>
            </div>
            <div class="flex items-center">
                <Button
                    v-if="isPlaying"
                    v-tooltip="formatMessage(commonMessages.stopButton)"
                    type="colored"
                    color="red"
                    circular
                    icon-only
                    :label="formatMessage(commonMessages.stopButton)"
                    @click="stop"
                    ><StopCircleIcon />
                </Button>
                <Button
                    v-else-if="modLoading"
                    v-tooltip="formatMessage(messages.loading)"
                    circular
                    icon-only
                    disabled
                    :label="formatMessage(messages.loading)"
                    ><SpinnerIcon class="animate-spin" />
                </Button>
                <Button
                    v-else
                    v-tooltip="formatMessage(commonMessages.playButton)"
                    :type="first ? 'colored' : 'base'"
                    :color="first ? 'brand' : undefined"
                    circular
                    icon-only
                    :label="formatMessage(commonMessages.playButton)"
                    @click="play"
                >
                    <PlayIcon />
                </Button>
            </div>
            <div
                class="col-span-3 flex items-center gap-1 font-semibold text-[var(--color-text-tertiary)]"
            >
                <TimerIcon />
                <span class="text-sm">
                    <template v-if="instance.last_played">
                        {{
                            formatMessage(messages.played, {
                                time: formatRelativeTime(new Date(instance.last_played)),
                            })
                        }}
                    </template>
                    <template v-else>{{ formatMessage(messages.neverPlayed) }}</template>
                </span>
            </div>
        </div>
    </template>
    <div v-else-if="variant === 'library'">
        <div
            class="group relative flex w-full cursor-pointer select-none flex-col items-start justify-end gap-3 overflow-clip rounded-[20px] border border-solid border-surface-4 bg-surface-3 p-3 text-left transition-[border-color,filter,transform] hover:border-surface-5 hover:brightness-110 active:scale-[0.98]"
            @click="seeInstance"
        >
            <div
                class="relative flex aspect-square w-full shrink-0 items-center overflow-clip rounded-2xl"
            >
                <Avatar
                    size="100%"
                    :src="displayIcon.url"
                    :tint-by="instance.id"
                    :class="[
                        'pointer-events-none !rounded-2xl outline-none',
                        { '!border-0 !bg-transparent !shadow-none': displayIcon.frameless },
                    ]"
                    :alt="instance.name"
                />
                <div
                    v-if="modLoading || installing"
                    class="instance-loading-overlay pointer-events-none absolute inset-0 flex items-center justify-center"
                >
                    <SpinnerIcon
                        v-tooltip="
                            modLoading
                                ? formatMessage(messages.loading)
                                : formatMessage(commonMessages.installingLabel)
                        "
                        class="size-[30%] animate-spin text-[var(--color-text-primary)]"
                        tabindex="-1"
                    />
                </div>
                <div class="absolute bottom-1.5 right-1.5 flex size-12 items-center justify-center">
                    <Button
                        v-if="isPlaying"
                        v-tooltip="formatMessage(commonMessages.stopButton)"
                        type="colored"
                        color="red"
                        size="xl"
                        circular
                        icon-only
                        :label="formatMessage(commonMessages.stopButton)"
                        @click="stop"
                        ><StopCircleIcon />
                    </Button>
                    <Button
                        v-else-if="!modLoading && !installing && !installed"
                        v-tooltip="formatMessage(commonMessages.repairButton)"
                        type="colored"
                        color="brand"
                        size="xl"
                        circular
                        icon-only
                        :label="formatMessage(commonMessages.repairButton)"
                        :class="{
                            'pointer-events-none scale-75 opacity-0': disabled,
                            'scale-75 opacity-0 transition-all group-hover:scale-100 group-hover:opacity-100':
                                !disabled,
                        }"
                        @click="repair"
                        ><DownloadIcon />
                    </Button>
                    <Button
                        v-else-if="!modLoading && !installing"
                        v-tooltip="formatMessage(commonMessages.playButton)"
                        type="colored"
                        color="brand"
                        size="xl"
                        circular
                        icon-only
                        :label="formatMessage(commonMessages.playButton)"
                        :class="{
                            'pointer-events-none scale-75 opacity-0': disabled,
                            'scale-75 opacity-0 transition-all group-hover:scale-100 group-hover:opacity-100':
                                !disabled,
                        }"
                        @click="play"
                        ><PlayIcon />
                    </Button>
                </div>
            </div>
            <div class="flex w-full min-w-0 flex-col items-start justify-center gap-1 px-0.5">
                <p
                    class="m-0 w-full truncate text-base font-semibold leading-5 text-[var(--color-text-primary)]"
                >
                    {{ instance.name }}
                </p>
                <p
                    class="m-0 w-full truncate text-sm font-medium capitalize leading-[18px] text-[var(--color-text-default)]"
                >
                    {{ instance.loader }} {{ instance.game_version }}
                </p>
            </div>
        </div>
    </div>
    <div v-else>
        <div
            class="button-base group flex gap-3"
            :class="
                flat
                    ? 'rounded-lg bg-transparent px-2 py-2 hover:bg-surface-4'
                    : 'rounded-xl bg-surface-2 p-4'
            "
            @click="seeInstance"
        >
            <div class="relative flex items-center justify-center">
                <InstanceIcon
                    size="48px"
                    :icon-path="instance.icon_path"
                    :instance-id="instance.id"
                    :loader="instance.loader"
                    :alt="instance.name"
                    :class="`transition-all ${modLoading || installing ? `scale-[0.85] brightness-[0.25]` : `group-hover:brightness-75`}`"
                />
                <div class="absolute inset-0 flex items-center justify-center">
                    <Button
                        v-if="isPlaying"
                        v-tooltip="formatMessage(commonMessages.stopButton)"
                        type="colored"
                        color="red"
                        size="xl"
                        circular
                        icon-only
                        :label="formatMessage(commonMessages.stopButton)"
                        :class="{ 'scale-100 opacity-100': isPlaying }"
                        class="card-shadow origin-bottom opacity-0 transition-all"
                        @click="stop"
                        ><StopCircleIcon />
                    </Button>
                    <SpinnerIcon
                        v-else-if="modLoading || installing"
                        v-tooltip="
                            modLoading
                                ? formatMessage(messages.loading)
                                : formatMessage(commonMessages.installingLabel)
                        "
                        class="h-8 w-8 animate-spin"
                        tabindex="-1"
                    />
                    <Button
                        v-else-if="!installed"
                        v-tooltip="formatMessage(commonMessages.repairButton)"
                        type="colored"
                        color="brand"
                        size="xl"
                        circular
                        icon-only
                        :label="formatMessage(commonMessages.repairButton)"
                        :class="`card-shadow origin-bottom scale-75 transition-all ${disabled ? 'scale-75 opacity-0' : 'opacity-0 group-focus-within:scale-100 group-focus-within:opacity-100 group-hover:scale-100 group-hover:opacity-100'}`"
                        @click="repair"
                        ><DownloadIcon />
                    </Button>
                    <Button
                        v-else
                        v-tooltip="formatMessage(commonMessages.playButton)"
                        type="colored"
                        color="brand"
                        size="xl"
                        circular
                        icon-only
                        :label="formatMessage(commonMessages.playButton)"
                        :class="`card-shadow origin-bottom scale-75 transition-all ${disabled ? 'scale-75 opacity-0' : 'opacity-0 group-focus-within:scale-100 group-focus-within:opacity-100 group-hover:scale-100 group-hover:opacity-100'}`"
                        @click="play"
                        ><PlayIcon />
                    </Button>
                </div>
            </div>
            <div class="flex flex-col gap-1">
                <p
                    class="text-md m-0 line-clamp-1 font-bold leading-tight text-[var(--color-text-primary)]"
                >
                    {{ instance.name }}
                </p>
                <div
                    class="col-span-3 mt-auto flex items-center gap-1 font-semibold text-[var(--color-text-tertiary)]"
                >
                    <GameIcon class="shrink-0" />
                    <span class="text-sm capitalize">
                        {{ instance.loader }} {{ instance.game_version }}
                    </span>
                </div>
            </div>
        </div>
    </div>
</template>

<style scoped>
.instance-loading-overlay {
    background-color: color-mix(
        in srgb,
        var(--surface-1) var(--opacity-ratio-keep-30),
        transparent
    );
}
</style>
