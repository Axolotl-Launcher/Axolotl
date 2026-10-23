<script setup lang="ts">
import ChevronLeftIcon from '@modrinth/assets/icons/chevron-left.svg?component'
import ChevronRightIcon from '@modrinth/assets/icons/chevron-right.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import WorldIcon from '@modrinth/assets/icons/world.svg?component'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useRelativeTime } from '@modrinth/ui/src/composables/how-ago.ts'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { ref } from 'vue'

import { useInstanceApi } from '../../../lib/platform/instance.ts'
import {
    isSingleplayerWorld,
    type SingleplayerWorld,
    sortWorlds,
    useWorldsApi,
} from '../../../lib/platform/worlds.ts'
import type { GameInstance } from '../../../lib/types.ts'
import NewModal from '../../../overlays/PreviewModal.vue'
import InstanceIcon from '../../../shell/InstanceIcon.vue'
import { readSeedMapLevelDat } from './core.ts'
import { SeedMapPreviewLimit, type SeedMapPreviewLimitReason } from './preview-data.ts'
import SeedMapPreviewNotice from './SeedMapPreviewNotice.vue'

export type SeedMapWorldImport = {
    seed: string
    version?: string
    instance: GameInstance
    world: SingleplayerWorld
}

const emit = defineEmits<{
    import: [selection: SeedMapWorldImport]
}>()

const { handleError } = injectNotificationManager()
const { list, get_full_path } = useInstanceApi()
const { get_instance_worlds } = useWorldsApi()
const { formatMessage } = useVIntl()
const formatRelativeTime = useRelativeTime()

const modal = ref()
const instances = ref<GameInstance[]>([])
const worlds = ref<SingleplayerWorld[]>([])
const selectedInstance = ref<GameInstance | null>(null)
const loading = ref(false)
const importingWorldPath = ref<string | null>(null)
/** Set when the preview cannot read the chosen world. */
const previewLimit = ref<SeedMapPreviewLimitReason | null>(null)

const messages = defineMessages({
    title: {
        id: 'app.lab.seed-map.import-world.title',
        defaultMessage: 'Load a seed from your instances',
    },
    chooseInstance: {
        id: 'app.lab.seed-map.import-world.choose-instance',
        defaultMessage: 'Choose the instance that contains the world',
    },
    chooseWorld: {
        id: 'app.lab.seed-map.import-world.choose-world',
        defaultMessage: 'Choose a singleplayer world',
    },
    back: { id: 'app.lab.seed-map.import-world.back', defaultMessage: 'Back to instances' },
    noInstances: {
        id: 'app.lab.seed-map.import-world.no-instances',
        defaultMessage: 'No instances found. Install an instance first.',
    },
    noWorlds: {
        id: 'app.lab.seed-map.import-world.no-worlds',
        defaultMessage: 'This instance has no singleplayer worlds yet.',
    },
    lastPlayed: {
        id: 'app.lab.seed-map.import-world.last-played',
        defaultMessage: 'Played {ago}',
    },
    neverPlayed: {
        id: 'app.lab.seed-map.import-world.never-played',
        defaultMessage: 'Not played yet',
    },
})

async function show() {
    selectedInstance.value = null
    worlds.value = []
    modal.value?.show()
    loading.value = true
    try {
        const loaded = await list()
        loaded.sort((a, b) => {
            if (!a.last_played) return 1
            if (!b.last_played) return -1
            return Date.parse(b.last_played) - Date.parse(a.last_played)
        })
        instances.value = loaded
    } catch (error) {
        handleError(error)
        instances.value = []
    } finally {
        loading.value = false
    }
}

async function openInstance(instance: GameInstance) {
    selectedInstance.value = instance
    worlds.value = []
    loading.value = true
    try {
        const instanceWorlds = await get_instance_worlds(instance.id)
        sortWorlds(instanceWorlds)
        worlds.value = instanceWorlds.filter(isSingleplayerWorld)
    } catch (error) {
        handleError(error)
        worlds.value = []
    } finally {
        loading.value = false
    }
}

function backToInstances() {
    previewLimit.value = null
    selectedInstance.value = null
    worlds.value = []
}

async function importWorld(world: SingleplayerWorld) {
    const instance = selectedInstance.value
    if (!instance || importingWorldPath.value) return
    importingWorldPath.value = world.path
    previewLimit.value = null
    try {
        const instancePath = await get_full_path(instance.id)
        const levelDat = await readSeedMapLevelDat(`${instancePath}/saves/${world.path}/level.dat`)
        emit('import', {
            seed: levelDat.seed,
            version: levelDat.version,
            instance,
            world,
        })
        modal.value?.hide()
    } catch (error) {
        if (error instanceof SeedMapPreviewLimit) previewLimit.value = error.reason
        else handleError(error)
    } finally {
        importingWorldPath.value = null
    }
}

defineExpose({ show })
</script>

<template>
    <NewModal ref="modal" :header="formatMessage(messages.title)">
        <div class="seed-import-body">
            <div v-if="!selectedInstance" class="seed-import-step">
                <p class="seed-import-hint">{{ formatMessage(messages.chooseInstance) }}</p>
                <div v-if="loading" class="seed-import-status">
                    <SpinnerIcon class="animate-spin" />
                </div>
                <p v-else-if="instances.length === 0" class="seed-import-status">
                    {{ formatMessage(messages.noInstances) }}
                </p>
                <div v-else class="seed-import-list">
                    <button
                        v-for="instance in instances"
                        :key="instance.id"
                        class="seed-import-row flex cursor-pointer items-center gap-[0.65rem] rounded-[var(--radius-md)] border border-surface-5 bg-surface-2 px-[0.65rem] py-2 text-left text-[var(--color-text-primary)] enabled:hover:bg-surface-4 disabled:cursor-default disabled:opacity-70"
                        @click="openInstance(instance)"
                    >
                        <InstanceIcon
                            class="seed-import-avatar"
                            :icon-path="instance.icon_path"
                            :instance-id="instance.id"
                            :loader="instance.loader"
                        />
                        <span class="seed-import-row-text">
                            <span class="seed-import-row-title">{{ instance.name }}</span>
                            <span class="seed-import-row-subtitle">
                                {{ instance.game_version }} · {{ instance.loader }}
                            </span>
                        </span>
                        <ChevronRightIcon class="seed-import-row-chevron" />
                    </button>
                </div>
            </div>

            <div v-else class="seed-import-step">
                <div class="seed-import-world-heading">
                    <Button type="quiet" size="2xs" @click="backToInstances"
                        ><ChevronLeftIcon />{{ formatMessage(messages.back) }}
                    </Button>
                    <strong>{{ selectedInstance.name }}</strong>
                </div>
                <p class="seed-import-hint">{{ formatMessage(messages.chooseWorld) }}</p>
                <SeedMapPreviewNotice v-if="previewLimit" :reason="previewLimit" />
                <div v-if="loading" class="seed-import-status">
                    <SpinnerIcon class="animate-spin" />
                </div>
                <p v-else-if="worlds.length === 0" class="seed-import-status">
                    {{ formatMessage(messages.noWorlds) }}
                </p>
                <div v-else class="seed-import-list">
                    <button
                        v-for="world in worlds"
                        :key="world.path"
                        class="seed-import-row flex cursor-pointer items-center gap-[0.65rem] rounded-[var(--radius-md)] border border-surface-5 bg-surface-2 px-[0.65rem] py-2 text-left text-[var(--color-text-primary)] enabled:hover:bg-surface-4 disabled:cursor-default disabled:opacity-70"
                        :disabled="importingWorldPath !== null"
                        @click="importWorld(world)"
                    >
                        <Avatar v-if="world.icon" class="seed-import-avatar" :src="world.icon" />
                        <span v-else class="seed-import-avatar seed-import-avatar-fallback">
                            <WorldIcon />
                        </span>
                        <span class="seed-import-row-text">
                            <span class="seed-import-row-title">{{ world.name }}</span>
                            <span class="seed-import-row-subtitle">
                                {{
                                    world.last_played
                                        ? formatMessage(messages.lastPlayed, {
                                              ago: formatRelativeTime(
                                                  new Date(world.last_played).toISOString(),
                                              ),
                                          })
                                        : formatMessage(messages.neverPlayed)
                                }}
                            </span>
                        </span>
                        <SpinnerIcon
                            v-if="importingWorldPath === world.path"
                            class="seed-import-row-chevron animate-spin"
                        />
                        <ChevronRightIcon v-else class="seed-import-row-chevron" />
                    </button>
                </div>
            </div>
        </div>
    </NewModal>
</template>

<style scoped>
.seed-import-body {
    display: flex;
    width: min(28rem, calc(1280px - 3rem));
    min-height: 16rem;
    flex-direction: column;
}

.seed-import-step {
    display: flex;
    min-height: 0;
    flex: 1;
    flex-direction: column;
    gap: 0.5rem;
}

.seed-import-hint {
    margin: 0;
    color: var(--color-text-tertiary);
    font-size: 0.8rem;
}

.seed-import-status {
    display: flex;
    flex: 1;
    align-items: center;
    justify-content: center;
    margin: 0;
    padding: 2rem 0;
    color: var(--color-text-tertiary);
    font-size: 0.85rem;
}

.seed-import-list {
    display: flex;
    max-height: min(22rem, 440px);
    flex-direction: column;
    gap: 0.25rem;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding-right: 0.2rem;
}

.seed-import-avatar {
    width: 2.5rem !important;
    height: 2.5rem !important;
    flex: 0 0 auto;
}

.seed-import-avatar-fallback {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-md);
    background: var(--surface-4);
    color: var(--color-text-tertiary);
}

.seed-import-avatar-fallback svg {
    width: 1.25rem;
    height: 1.25rem;
}

.seed-import-row-text {
    display: flex;
    min-width: 0;
    flex: 1;
    flex-direction: column;
    gap: 0.1rem;
}

.seed-import-row-title {
    overflow: hidden;
    font-size: 0.85rem;
    font-weight: 700;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.seed-import-row-subtitle {
    overflow: hidden;
    color: var(--color-text-tertiary);
    font-size: 0.72rem;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.seed-import-row-chevron {
    width: 1rem;
    height: 1rem;
    flex: 0 0 auto;
    color: var(--color-text-tertiary);
}

.seed-import-world-heading {
    display: flex;
    align-items: center;
    gap: 0.5rem;
}

.seed-import-world-heading strong {
    overflow: hidden;
    font-size: 0.85rem;
    text-overflow: ellipsis;
    white-space: nowrap;
}
</style>
