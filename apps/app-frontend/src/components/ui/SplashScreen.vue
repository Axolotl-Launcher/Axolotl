<template>
    <Transition name="splash-fade" @after-leave="onAfterLeave">
        <div v-if="!doneLoading" class="dark fixed inset-0 z-[10000]">
            <div
                class="absolute z-[9998] flex h-screen w-full flex-col items-center justify-center gap-4"
                data-tauri-drag-region
            >
                <img class="app-logo" src="@/assets/axolotl.png" alt="Axolotl Launcher" />
                <ProgressBar
                    class="max-w-xs"
                    :progress="Math.min(loadingProgress, 100)"
                    :max="100"
                />
                <span v-if="message">{{ message }}</span>
                <span v-if="migrationDetail" class="text-sm">{{ migrationDetail }}</span>
                <span v-if="migrationCount" class="text-sm tabular-nums">{{ migrationCount }}</span>
            </div>
            <div class="gradient-bg" data-tauri-drag-region></div>
            <div class="cube-bg"></div>
            <div class="bg-bg absolute left-0 top-0 z-[9995] h-full w-full"></div>
        </div>
    </Transition>
</template>

<script setup lang="ts">
import { defineMessages, injectLoadingState, ProgressBar, useVIntl } from '@modrinth/ui'
import { computed, onUnmounted, ref, watch } from 'vue'

import type { StartupLoadingProgress } from '@/helpers/startup-progress'

const props = defineProps<{ startupProgress?: StartupLoadingProgress | null }>()
const emit = defineEmits<{ finished: [] }>()

const doneLoading = ref(false)
const loadingProgress = ref(0)
let finishTimer: ReturnType<typeof setTimeout> | undefined

const MIN_DISPLAY_MS = 500
const mountedAt = Date.now()

const loading = injectLoadingState()
const { formatMessage } = useVIntl()

const messages = defineMessages({
    updatingAppDirectory: {
        id: 'app.splash.updating-app-directory',
        defaultMessage: 'Updating app directory...',
    },
    checkingForUpdates: {
        id: 'app.splash.checking-for-updates',
        defaultMessage: 'Checking for updates...',
    },
    migratingData: {
        id: 'app.splash.migrating-data',
        defaultMessage: 'Migrating launcher data...',
    },
    preparingMigration: {
        id: 'app.splash.migration.preparing',
        defaultMessage: 'Preparing migration...',
    },
    migratingCore: {
        id: 'app.splash.migration.core',
        defaultMessage: 'Migrating launcher records...',
    },
    migratingRecovery: {
        id: 'app.splash.migration.recovery',
        defaultMessage: 'Migrating recovery records...',
    },
    finishingMigration: {
        id: 'app.splash.migration.finishing',
        defaultMessage: 'Finishing migration...',
    },
    migrationCount: {
        id: 'app.splash.migration.count',
        defaultMessage: '{completed, number} / {total, number}',
    },
})

const migrating = computed(() => props.startupProgress?.event.type === 'database_migration')
const migrationPending = computed(() => migrating.value && props.startupProgress?.fraction !== null)
const message = computed(() => {
    switch (props.startupProgress?.event.type) {
        case 'database_migration':
            return formatMessage(messages.migratingData)
        case 'directory_move':
            return formatMessage(messages.updatingAppDirectory)
        case 'checking_for_updates':
            return formatMessage(messages.checkingForUpdates)
        default:
            return null
    }
})
const migrationDetail = computed(() => {
    if (!migrating.value) return null
    switch (props.startupProgress?.event.stage) {
        case 'preparing':
            return formatMessage(messages.preparingMigration)
        case 'core':
            return formatMessage(messages.migratingCore)
        case 'recovery':
            return formatMessage(messages.migratingRecovery)
        case 'finishing':
            return formatMessage(messages.finishingMigration)
        default:
            return null
    }
})
const migrationCount = computed(() => {
    const event = props.startupProgress?.event
    if (!migrating.value || !event?.total || event.completed == null) return null
    return formatMessage(messages.migrationCount, {
        completed: event.completed,
        total: event.total,
    })
})

function onAfterLeave() {
    loading.setEnabled(true)
    emit('finished')
}

watch(
    () => props.startupProgress,
    (progress) => {
        if (!progress) return
        loadingProgress.value = Math.max(0, Math.min(100, 100 * (progress.fraction ?? 1)))
    },
    { immediate: true },
)

watch(
    [loading.barEnabled, loading.pending, migrationPending],
    ([barEnabled, pending, migratingData]) => {
        clearTimeout(finishTimer)
        if (barEnabled) {
            return
        }

        if (pending || migratingData) {
            return
        }

        const elapsed = Date.now() - mountedAt
        const delay = Math.max(0, MIN_DISPLAY_MS - elapsed)

        finishTimer = setTimeout(() => {
            if (loading.pending.value || migrationPending.value) {
                return
            }
            loadingProgress.value = 100
            doneLoading.value = true
        }, delay)
    },
    { immediate: true },
)

onUnmounted(() => {
    clearTimeout(finishTimer)
})
</script>

<style scoped lang="scss">
.splash-fade-leave-active {
    transition: opacity 0.3s ease-in-out;
}

.splash-fade-leave-to {
    opacity: 0;
}

.app-logo {
    height: min(18rem, 45vh);
    width: min(18rem, 45vw);
    object-fit: contain;
}

.gradient-bg {
    position: absolute;
    height: 100vh;
    height: 100dvh;
    width: 100vw;
    width: 100dvw;
    background:
        linear-gradient(180deg, rgba(255, 77, 157, 0.24) 0%, rgba(48, 16, 40, 0.56) 97.29%),
        linear-gradient(0deg, rgba(22, 18, 28, 0.68), rgba(22, 18, 28, 0.68));
    z-index: 9997;
}

.cube-bg {
    position: absolute;

    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);

    width: 180vw;
    width: 180dvw;
    height: 180vh;
    height: 180dvh;
    opacity: 0.8;
    background: var(--surface-1) url('@/assets/loading/cube.webp') center no-repeat;
    background-size: contain;

    z-index: 9996;
}
</style>
