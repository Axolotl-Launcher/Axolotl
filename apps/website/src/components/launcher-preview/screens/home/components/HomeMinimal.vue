<script setup lang="ts">
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import GameIcon from '@modrinth/assets/icons/game.svg?component'
import ListIcon from '@modrinth/assets/icons/list.svg?component'
import PlayIcon from '@modrinth/assets/icons/play.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import StopCircleIcon from '@modrinth/assets/icons/stop-circle.svg?component'
import TimerIcon from '@modrinth/assets/icons/timer.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Card from '@modrinth/ui/src/components/base/Card.vue'
import { useRelativeTime } from '@modrinth/ui/src/composables/how-ago.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, ref } from 'vue'

import type { GameInstance } from '../../../lib/types'
import { homeMessages } from '../messages'
import { useHomeRuntime } from '../runtime'
import HomeGreeting from './HomeGreeting.vue'
import InstanceIcon from './InstanceIcon.vue'

const props = defineProps<{
    instances: GameInstance[]
    selectedInstanceId?: string | null
    playerName?: string | null
}>()

const emit = defineEmits<{
    choose: []
    create: []
}>()

const { formatMessage } = useVIntl()
const formatRelativeTime = useRelativeTime()
const runtime = useHomeRuntime()
const messages = homeMessages

const selectedInstance = computed(() =>
    props.instances.find((instance) => instance.id === props.selectedInstanceId),
)
/** `get_by_instance_id`: the selected instance has a game process. */
const running = computed(() =>
    selectedInstance.value
        ? runtime.runningInstanceIds.value.includes(selectedInstance.value.id)
        : false,
)
const loading = ref(false)
const installed = computed(() => selectedInstance.value?.install_stage === 'installed')
const installing = computed(
    () => selectedInstance.value?.install_stage.includes('installing') ?? false,
)
const busy = computed(() => loading.value || installing.value)

const lastPlayed = computed(() => {
    if (!selectedInstance.value?.last_played) return formatMessage(messages.instanceNeverPlayed)
    return formatMessage(messages.instancePlayed, {
        time: formatRelativeTime(selectedInstance.value.last_played),
    })
})

async function playInstance() {
    const instance = selectedInstance.value
    if (!instance) return

    loading.value = true
    try {
        await runtime.run(instance)
    } catch (error) {
        runtime.handleError(error)
    } finally {
        loading.value = false
    }
}

async function stopInstance() {
    const instance = selectedInstance.value
    if (!instance) return

    await runtime.kill(instance.id).catch(runtime.handleError)
}

async function installInstance() {
    const instance = selectedInstance.value
    if (!instance) return

    loading.value = true
    try {
        await runtime.installExistingInstance(instance)
    } catch (error) {
        runtime.handleError(error)
    } finally {
        loading.value = false
    }
}
</script>

<template>
    <section
        data-onboarding-id="home-instances"
        class="minimal-home-stage flex min-w-0 items-center justify-center px-6 pb-14 pt-8"
    >
        <div class="flex w-full max-w-3xl flex-col items-center text-center">
            <HomeGreeting :player-name="playerName" variant="minimal" />

            <template v-if="selectedInstance">
                <Card class="mb-0 mt-10 w-full text-left">
                    <div
                        class="grid min-w-0 grid-cols-1 items-center gap-5 sm:grid-cols-[minmax(0,1fr)_auto]"
                    >
                        <router-link
                            :to="`/instance/${encodeURIComponent(selectedInstance.id)}`"
                            class="group flex min-w-0 items-center gap-5 rounded-lg text-inherit no-underline focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-shadow"
                        >
                            <InstanceIcon
                                class="size-20 shrink-0 transition-transform group-hover:scale-[1.03]"
                                :icon-path="selectedInstance.icon_path"
                                :instance-id="selectedInstance.id"
                                :loader="selectedInstance.loader"
                            />
                            <div class="flex min-w-0 flex-1 flex-col gap-1.5">
                                <h2
                                    class="m-0 truncate text-xl font-bold text-[var(--color-text-primary)] group-hover:underline"
                                >
                                    {{ selectedInstance.name }}
                                </h2>
                                <div
                                    class="flex min-w-0 flex-wrap gap-x-4 gap-y-1 text-sm text-[var(--color-text-tertiary)]"
                                >
                                    <span class="flex min-w-0 items-center gap-1.5 capitalize">
                                        <GameIcon class="size-4 shrink-0" aria-hidden="true" />
                                        <span class="truncate">
                                            {{ selectedInstance.loader }}
                                            {{ selectedInstance.game_version }}
                                        </span>
                                    </span>
                                    <span class="flex min-w-0 items-center gap-1.5">
                                        <TimerIcon class="size-4 shrink-0" aria-hidden="true" />
                                        <span class="truncate">{{ lastPlayed }}</span>
                                    </span>
                                </div>
                            </div>
                        </router-link>

                        <div class="flex min-h-11 shrink-0 items-center justify-end gap-2">
                            <Button
                                v-if="running"
                                type="colored"
                                color="red"
                                size="xl"
                                class="w-36 justify-center"
                                @click="stopInstance"
                                ><StopCircleIcon aria-hidden="true" />
                                <span class="truncate">{{
                                    formatMessage(commonMessages.stopButton)
                                }}</span>
                            </Button>
                            <Button v-else-if="busy" size="xl" class="w-36 justify-center" disabled
                                ><SpinnerIcon class="animate-spin" aria-hidden="true" />
                                <span class="truncate">
                                    {{
                                        formatMessage(
                                            installing
                                                ? commonMessages.installingLabel
                                                : messages.instanceLoading,
                                        )
                                    }}
                                </span>
                            </Button>
                            <Button
                                v-else-if="installed"
                                type="colored"
                                color="brand"
                                size="xl"
                                class="w-36 justify-center"
                                @click="playInstance"
                                ><PlayIcon aria-hidden="true" />
                                <span class="truncate">{{
                                    formatMessage(commonMessages.playButton)
                                }}</span>
                            </Button>
                            <Button
                                v-else
                                type="colored"
                                color="brand"
                                size="xl"
                                class="w-36 justify-center"
                                @click="installInstance"
                                ><DownloadIcon aria-hidden="true" />
                                <span class="truncate">{{
                                    formatMessage(commonMessages.installButton)
                                }}</span>
                            </Button>

                            <Button
                                v-tooltip="formatMessage(messages.minimalChangeInstance)"
                                type="quiet"
                                size="xl"
                                circular
                                icon-only
                                :aria-label="formatMessage(messages.minimalChangeInstance)"
                                @click="emit('choose')"
                                ><ListIcon aria-hidden="true" />
                            </Button>
                        </div>
                    </div>
                </Card>
            </template>

            <template v-else>
                <Card class="mb-0 mt-10 w-full text-left">
                    <div class="flex min-w-0 flex-wrap items-center gap-4">
                        <div
                            class="flex size-16 shrink-0 items-center justify-center rounded-lg bg-surface-4 text-[var(--color-text-tertiary)]"
                        >
                            <ListIcon class="size-7" aria-hidden="true" />
                        </div>
                        <div class="min-w-48 flex-1">
                            <h2 class="m-0 text-lg font-bold text-[var(--color-text-primary)]">
                                {{
                                    formatMessage(
                                        instances.length > 0
                                            ? messages.minimalChooseInstance
                                            : messages.instancesEmpty,
                                    )
                                }}
                            </h2>
                        </div>
                        <Button
                            type="colored"
                            color="brand"
                            size="xl"
                            @click="instances.length > 0 ? emit('choose') : emit('create')"
                        >
                            <ListIcon v-if="instances.length > 0" aria-hidden="true" />
                            <PlusIcon v-else aria-hidden="true" />
                            {{
                                formatMessage(
                                    instances.length > 0
                                        ? messages.minimalChooseInstance
                                        : messages.instancesCreate,
                                )
                            }}
                        </Button>
                    </div>
                </Card>
            </template>
        </div>
    </section>
</template>

<style scoped>
/* The app's `calc(100vh - var(--top-bar-height) - 4rem)`, against the 800px window. */
.minimal-home-stage {
    min-height: calc(800px - var(--top-bar-height) - 4rem);
}
</style>
