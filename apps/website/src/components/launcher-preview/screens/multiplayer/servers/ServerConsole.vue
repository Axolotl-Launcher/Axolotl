<script setup lang="ts">
import { createConsoleState } from '@modrinth/ui/src/composables/console-state.ts'
import ConsolePageLayout from '@modrinth/ui/src/layouts/shared/console/layout.vue'
import { provideConsoleManager } from '@modrinth/ui/src/layouts/shared/console/providers/console-manager.ts'
import { nextTick, onMounted, onUnmounted, ref, watch } from 'vue'

import { type ServerLogCursor, servers } from '../backend.ts'
import type { ServerView } from '../composables/useServers.ts'
import ServerTerminal from './ServerTerminal.vue'

const props = defineProps<{
    server: ServerView
}>()

const consoleState = createConsoleState()
const loading = ref(true)
const view = ref<'terminal' | 'logs'>('terminal')
let disposed = false
let clearing = false
let cursor: ServerLogCursor | undefined
let logGeneration = 0
let hydration: Promise<void> | undefined
function hydrateAndDisplay(): Promise<void> {
    if (clearing || disposed) return Promise.resolve()
    if (hydration) return hydration
    const serverId = props.server.id
    const generation = logGeneration
    hydration = (async () => {
        try {
            const delta = await servers.getLogDelta(serverId, cursor)
            if (disposed || props.server.id !== serverId || generation !== logGeneration) return
            if (delta.reset) consoleState.clear()
            if (delta.lines.length) await consoleState.addLegacyLog(delta.lines.join('\n'))
            cursor = delta.cursor
        } catch {
            // Preserve the cursor so the next reconciliation retries the missing lines.
        } finally {
            hydration = undefined
        }
    })()
    return hydration
}

// The per-line `server` events can drop during heavy bursts, so we
// periodically reconcile the displayed log against the lossless backend
// buffer. A cursor transfers only new lines and detects rotation and restart.
let syncTimer: ReturnType<typeof setTimeout> | null = null
function startSync() {
    stopSync()
    if (disposed || clearing || !(props.server.running || props.server.starting)) return
    syncTimer = setTimeout(async () => {
        syncTimer = null
        if (disposed || !(props.server.running || props.server.starting)) return
        try {
            await hydrateAndDisplay()
        } finally {
            startSync()
        }
    }, 1000)
}
function stopSync() {
    if (syncTimer) {
        clearTimeout(syncTimer)
        syncTimer = null
    }
}

onMounted(async () => {
    await hydrateAndDisplay()
    if (disposed) return
    loading.value = false
    startSync()
})

onUnmounted(() => {
    disposed = true
    stopSync()
    consoleState.clear()
})

// Starting a server invalidates responses from the previous run.
const consoleLayout = ref<InstanceType<typeof ConsolePageLayout> | null>(null)
watch(
    () => props.server.running || props.server.starting,
    async (running, previousRunning) => {
        if (!running) {
            stopSync()
            await hydration
            await hydrateAndDisplay()
            return
        }
        if (previousRunning) return
        await nextTick()
        if (disposed) return
        consoleState.clear()
        logGeneration++
        cursor = undefined
        await hydration
        await hydrateAndDisplay()
        if (disposed) return
        startSync()
        consoleLayout.value?.scrollToBottom()
    },
)

provideConsoleManager({
    logLines: consoleState.output,
    loading,
    emptyStateType: 'server',
    onClear: async () => {
        clearing = true
        stopSync()
        logGeneration++
        consoleState.clear()
        try {
            await hydration
            await servers.clearLog(props.server.id).catch(() => {})
            cursor = undefined
        } finally {
            clearing = false
            startSync()
        }
    },
})
</script>

<template>
    <div data-onboarding-id="server-console" class="flex h-full min-h-0 min-w-0 flex-col pb-3">
        <ServerTerminal :server="server" :view="view" @update:view="view = $event">
            <ConsolePageLayout ref="consoleLayout" />
        </ServerTerminal>
    </div>
</template>
