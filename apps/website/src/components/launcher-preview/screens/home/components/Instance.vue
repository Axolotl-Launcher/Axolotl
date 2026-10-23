<script setup lang="ts">
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import GameIcon from '@modrinth/assets/icons/game.svg?component'
import PlayIcon from '@modrinth/assets/icons/play.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import StopCircleIcon from '@modrinth/assets/icons/stop-circle.svg?component'
import TimerIcon from '@modrinth/assets/icons/timer.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useRelativeTime } from '@modrinth/ui/src/composables/how-ago.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'

import type { GameInstance } from '../../../lib/types'
import { homeMessages } from '../messages'
import { useHomeRuntime } from '../runtime'
import InstanceIcon from './InstanceIcon.vue'

/**
 * Port of `components/ui/Instance.vue` in the two forms the Home cards use:
 * the compact row and the standard card (the library tile is not used here).
 */
const props = withDefaults(
    defineProps<{
        instance: GameInstance
        compact?: boolean
        flat?: boolean
        playing?: boolean
        first?: boolean
        disabled?: boolean
    }>(),
    {
        compact: false,
        flat: false,
        playing: undefined,
        first: false,
        disabled: false,
    },
)

const { formatMessage } = useVIntl()
const formatRelativeTime = useRelativeTime()
const router = useRouter()
const runtime = useHomeRuntime()
const messages = homeMessages

const isPlaying = computed(
    () => props.playing ?? runtime.runningInstanceIds.value.includes(props.instance.id),
)
const loading = ref(false)
/** The app's `modLoading`: a launch from this card that has not reached the game yet. */
const modLoading = computed(() => loading.value)
const installing = computed(() => props.instance.install_stage.includes('installing'))
const installed = computed(() => props.instance.install_stage === 'installed')

async function seeInstance() {
    await router.push(`/instance/${encodeURIComponent(props.instance.id)}`)
}

async function play(event: MouseEvent) {
    event?.stopPropagation()
    loading.value = true
    await runtime.run(props.instance).catch(runtime.handleError)
    loading.value = false
}

async function stop(event: MouseEvent) {
    event?.stopPropagation()
    await runtime.kill(props.instance.id).catch(runtime.handleError)
}

async function repair(event: MouseEvent) {
    event?.stopPropagation()
    await runtime.installExistingInstance(props.instance).catch(runtime.handleError)
}
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
                    @click="stop"
                    ><StopCircleIcon />
                </Button>
                <Button
                    v-else-if="modLoading"
                    v-tooltip="formatMessage(messages.instanceLoading)"
                    circular
                    icon-only
                    disabled
                    ><SpinnerIcon class="animate-spin" />
                </Button>
                <Button
                    v-else
                    v-tooltip="formatMessage(commonMessages.playButton)"
                    :type="first ? 'colored' : 'base'"
                    :color="first ? 'brand' : undefined"
                    circular
                    icon-only
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
                            formatMessage(messages.instancePlayed, {
                                time: formatRelativeTime(instance.last_played),
                            })
                        }}
                    </template>
                    <template v-else>{{ formatMessage(messages.instanceNeverPlayed) }}</template>
                </span>
            </div>
        </div>
    </template>
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
                        :class="{ 'scale-100 opacity-100': isPlaying }"
                        class="card-shadow origin-bottom opacity-0 transition-all"
                        @click="stop"
                        ><StopCircleIcon />
                    </Button>
                    <SpinnerIcon
                        v-else-if="modLoading || installing"
                        v-tooltip="
                            modLoading
                                ? formatMessage(messages.instanceLoading)
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
