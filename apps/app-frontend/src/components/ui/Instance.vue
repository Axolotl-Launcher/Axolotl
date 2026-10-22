<script setup>
import {
    DownloadIcon,
    GameIcon,
    PlayIcon,
    SpinnerIcon,
    StopCircleIcon,
    TimerIcon,
} from '@modrinth/assets'
import {
    Avatar,
    Button,
    commonMessages,
    defineMessages,
    injectNotificationManager,
    useRelativeTime,
    useVIntl,
} from '@modrinth/ui'
import dayjs from 'dayjs'
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'

import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import { useMinecraftLaunchError } from '@/composables/useMinecraftLaunchError'
import { useNetworkStatus } from '@/composables/useNetworkStatus'
import { trackEvent } from '@/helpers/analytics'
import { process_listener } from '@/helpers/events'
import { install_existing_instance, install_pack_to_existing_instance } from '@/helpers/install'
import { kill, run } from '@/helpers/instance'
import { getDisplayInstanceIcon } from '@/helpers/instance-icons'
import { get_by_instance_id } from '@/helpers/process'
import { showInstanceInFolder } from '@/helpers/utils.js'
import { handleSevereError } from '@/store/error.js'

const { handleError } = injectNotificationManager()
const formatRelativeTime = useRelativeTime()
const { formatMessage } = useVIntl()
const handleMinecraftLaunchError = useMinecraftLaunchError()
const messages = defineMessages({
    loading: { id: 'app.instance.loading', defaultMessage: 'Instance is loading...' },
    played: { id: 'app.instance.played', defaultMessage: 'Played {time}' },
    neverPlayed: { id: 'app.instance.never-played', defaultMessage: 'Never played' },
    offlineInstalledOnly: {
        id: 'app.instance.offline-installed-only',
        defaultMessage: 'Offline mode can only launch fully downloaded instances.',
    },
})

const { offline } = useNetworkStatus()

const props = defineProps({
    instance: {
        type: Object,
        default() {
            return {}
        },
    },
    compact: {
        type: Boolean,
        default: false,
    },
    flat: {
        type: Boolean,
        default: false,
    },
    variant: {
        type: String,
        default: 'standard',
    },
    playing: {
        type: Boolean,
        default: undefined,
    },
    first: {
        type: Boolean,
        default: false,
    },
    disabled: {
        type: Boolean,
        default: false,
    },
})

const internalPlaying = ref(false)
const isPlaying = computed(() => props.playing ?? internalPlaying.value)
const displayIcon = computed(() =>
    getDisplayInstanceIcon(props.instance.icon_path, props.instance.loader),
)
const loading = ref(false)
const modLoading = computed(
    () =>
        loading.value ||
        currentEvent.value === 'installing' ||
        (currentEvent.value === 'launched' && !isPlaying.value),
)
const installing = computed(() => props.instance.install_stage.includes('installing'))
const installed = computed(() => props.instance.install_stage === 'installed')

const router = useRouter()

const seeInstance = async () => {
    await router.push(`/instance/${encodeURIComponent(props.instance.id)}`)
}

const checkProcess = async () => {
    if (props.playing !== undefined) return
    const runningProcesses = await get_by_instance_id(props.instance.id).catch(handleError)

    internalPlaying.value = runningProcesses.length > 0
}

const play = async (e, context) => {
    e?.stopPropagation()
    loading.value = true
    await run(props.instance.id)
        .catch(async (err) => {
            const handled = await handleMinecraftLaunchError(err, {
                instance_id: props.instance.id,
                instance_name: props.instance.name,
            })
            if (!handled) handleSevereError(err, { instanceId: props.instance.id })
        })
        .finally(() => {
            trackEvent('InstanceStart', {
                loader: props.instance.loader,
                game_version: props.instance.game_version,
                source: context,
            })
        })
    loading.value = false
}

const stop = async (e, context) => {
    e?.stopPropagation()
    internalPlaying.value = false

    await kill(props.instance.id).catch(handleError)

    trackEvent('InstanceStop', {
        loader: props.instance.loader,
        game_version: props.instance.game_version,
        source: context,
    })
}

const repair = async (e) => {
    e?.stopPropagation()

    if (
        props.instance.install_stage !== 'pack_installed' &&
        (props.instance.link?.type === 'modrinth_modpack' ||
            props.instance.link?.type === 'server_project_modpack')
    ) {
        await install_pack_to_existing_instance(props.instance.id, {
            type: 'fromVersionId',
            project_id:
                props.instance.link.project_id ?? props.instance.link.server_project_id ?? '',
            version_id:
                props.instance.link.version_id ?? props.instance.link.content_version_id ?? '',
            title: props.instance.name,
        }).catch(handleError)
    } else {
        await install_existing_instance(props.instance.id, false).catch(handleError)
    }
}

const openFolder = async () => {
    await showInstanceInFolder(props.instance.id)
}

const addContent = async () => {
    await router.push({
        path: `/browse/${props.instance.loader === 'vanilla' ? 'datapack' : 'mod'}`,
        query: { i: props.instance.id },
    })
}

defineExpose({
    play,
    stop,
    seeInstance,
    openFolder,
    addContent,
    instance: props.instance,
})

const currentEvent = ref(null)

const unlisten =
    props.playing === undefined
        ? await process_listener((e) => {
              if (e.instance_id === props.instance.id) {
                  currentEvent.value = e.event
                  if (e.event === 'finished') {
                      internalPlaying.value = false
                  }
              }
          })
        : () => undefined

onMounted(() => checkProcess())
onUnmounted(() => unlisten())
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
            @mouseenter="checkProcess"
        >
            <InstanceIcon
                size="48px"
                :icon-path="instance.icon_path"
                :instance-id="instance.id"
                :loader="instance.loader"
                :alt="instance.name"
            />
            <div
                class="h-full flex items-center font-bold text-[var(--color-text-primary)] leading-normal"
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
                    @mousehover="checkProcess"
                    @click="(e) => stop(e, 'InstanceCard')"
                    ><StopCircleIcon />
                </Button>
                <Button
                    v-else-if="modLoading"
                    v-tooltip="formatMessage(messages.loading)"
                    circular
                    icon-only
                    disabled
                    ><SpinnerIcon class="animate-spin" />
                </Button>
                <Button
                    v-else
                    v-tooltip="
                        offline && !installed
                            ? formatMessage(messages.offlineInstalledOnly)
                            : formatMessage(commonMessages.playButton)
                    "
                    :type="first ? 'colored' : 'base'"
                    :color="first ? 'brand' : undefined"
                    circular
                    icon-only
                    :disabled="offline && !installed"
                    @click="(e) => play(e, 'InstanceCard')"
                    @mousehover="checkProcess"
                >
                    <PlayIcon />
                </Button>
            </div>
            <div
                class="flex items-center col-span-3 gap-1 text-[var(--color-text-tertiary)] font-semibold"
            >
                <TimerIcon />
                <span class="text-sm">
                    <template v-if="instance.last_played">
                        {{
                            formatMessage(messages.played, {
                                time: formatRelativeTime(dayjs(instance.last_played).toISOString()),
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
            @mouseenter="checkProcess"
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
                        @click="(e) => stop(e, 'InstanceCard')"
                        @mousehover="checkProcess"
                        ><StopCircleIcon />
                    </Button>
                    <Button
                        v-else-if="!modLoading && !installing && !installed"
                        v-tooltip="
                            offline
                                ? formatMessage(messages.offlineInstalledOnly)
                                : formatMessage(commonMessages.repairButton)
                        "
                        type="colored"
                        color="brand"
                        size="xl"
                        circular
                        icon-only
                        :disabled="offline"
                        :class="{
                            'pointer-events-none scale-75 opacity-0': disabled,
                            'scale-75 opacity-0 transition-all group-hover:scale-100 group-hover:opacity-100':
                                !disabled,
                        }"
                        @click="(e) => repair(e)"
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
                        :class="{
                            'pointer-events-none scale-75 opacity-0': disabled,
                            'scale-75 opacity-0 transition-all group-hover:scale-100 group-hover:opacity-100':
                                !disabled,
                        }"
                        @click="(e) => play(e, 'InstanceCard')"
                        @mousehover="checkProcess"
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
            class="button-base flex gap-3 group"
            :class="
                flat
                    ? 'rounded-lg bg-transparent px-2 py-2 hover:bg-surface-4'
                    : 'rounded-xl bg-surface-2 p-4'
            "
            @click="seeInstance"
            @mouseenter="checkProcess"
        >
            <div class="relative flex items-center justify-center">
                <InstanceIcon
                    size="48px"
                    :icon-path="instance.icon_path"
                    :instance-id="instance.id"
                    :loader="instance.loader"
                    :alt="instance.name"
                    :class="`transition-all ${modLoading || installing ? `brightness-[0.25] scale-[0.85]` : `group-hover:brightness-75`}`"
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
                        class="transition-all origin-bottom opacity-0 card-shadow"
                        @click="(e) => stop(e, 'InstanceCard')"
                        @mousehover="checkProcess"
                        ><StopCircleIcon />
                    </Button>
                    <SpinnerIcon
                        v-else-if="modLoading || installing"
                        v-tooltip="
                            modLoading
                                ? formatMessage(messages.loading)
                                : formatMessage(commonMessages.installingLabel)
                        "
                        class="animate-spin w-8 h-8"
                        tabindex="-1"
                    />
                    <Button
                        v-else-if="!installed"
                        v-tooltip="
                            offline
                                ? formatMessage(messages.offlineInstalledOnly)
                                : formatMessage(commonMessages.repairButton)
                        "
                        type="colored"
                        color="brand"
                        size="xl"
                        circular
                        icon-only
                        :disabled="offline"
                        :class="`transition-all scale-75 origin-bottom card-shadow ${disabled ? 'opacity-0 scale-75' : 'opacity-0 group-hover:scale-100 group-hover:opacity-100 group-focus-within:scale-100 group-focus-within:opacity-100'}`"
                        @click="(e) => repair(e)"
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
                        :class="`transition-all scale-75 origin-bottom card-shadow ${disabled ? 'opacity-0 scale-75' : 'opacity-0 group-hover:scale-100 group-hover:opacity-100 group-focus-within:scale-100 group-focus-within:opacity-100'}`"
                        @click="(e) => play(e, 'InstanceCard')"
                        @mousehover="checkProcess"
                        ><PlayIcon />
                    </Button>
                </div>
            </div>
            <div class="flex flex-col gap-1">
                <p
                    class="m-0 text-md font-bold text-[var(--color-text-primary)] leading-tight line-clamp-1"
                >
                    {{ instance.name }}
                </p>
                <div
                    class="flex items-center col-span-3 gap-1 text-[var(--color-text-tertiary)] font-semibold mt-auto"
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
