<script setup lang="ts">
import {
    ConsolePageLayout,
    createConsoleState,
    defineMessages,
    JLineCommandInput,
    provideConsoleManager,
    useVIntl,
} from '@modrinth/ui'
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'

import { ServerConsoleBuffer } from '@/composables/server-console-buffer'
import { type ServerView, subscribeServerConsoleOutput, useServers } from '@/composables/useServers'
import { type ServerLogCursor, servers } from '@/helpers/servers'

const props = defineProps<{
    server: ServerView
}>()

const { formatMessage } = useVIntl()
const messages = defineMessages({
    forgeCommandPlaceholder: {
        id: 'app.servers.console.forge-command-placeholder',
        defaultMessage: 'Send a command - Tab completion supported',
    },
    notRunning: {
        id: 'app.servers.console.not-running',
        defaultMessage: 'The server is not running',
    },
})

const { sendCommand } = useServers()
const consoleState = createConsoleState()
const loading = ref(true)
const hasLogs = computed(() => consoleState.output.value.length > 0)
const isForge = computed(() => props.server.serverType === 'forge')
const jlineInput = ref<InstanceType<typeof JLineCommandInput> | null>(null)
let disposed = false
let clearing = false
let cursor: ServerLogCursor | undefined
let logGeneration = 0
let hydration: Promise<void> | undefined
let unsubscribeConsoleOutput: (() => void) | null = null
const PENDING_CONSOLE_OUTPUT_CAPACITY = 64 * 1024
let pendingConsoleOutput = new ServerConsoleBuffer(PENDING_CONSOLE_OUTPUT_CAPACITY)

function flushConsoleOutput() {
    for (const data of pendingConsoleOutput.values()) jlineInput.value?.write(data)
    pendingConsoleOutput = new ServerConsoleBuffer(PENDING_CONSOLE_OUTPUT_CAPACITY)
}

watch(
    () => props.server.id,
    (serverId) => {
        unsubscribeConsoleOutput?.()
        unsubscribeConsoleOutput = subscribeServerConsoleOutput(serverId, (data) => {
            if (jlineInput.value) {
                jlineInput.value.write(data)
            } else {
                pendingConsoleOutput.push(data)
            }
        })
    },
    { immediate: true },
)

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
    if (disposed || clearing || !props.server.running) return
    syncTimer = setTimeout(async () => {
        syncTimer = null
        if (disposed || !props.server.running) return
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
    flushConsoleOutput()
    await hydrateAndDisplay()
    if (disposed) return
    loading.value = false
    startSync()
})

onUnmounted(() => {
    disposed = true
    stopSync()
    consoleState.clear()
    unsubscribeConsoleOutput?.()
})

async function handleSendCommand(command: string) {
    // The server echoes the command into its own log (e.g. "> time set 0"),
    // which the console already shows, so we don't echo it a second time here.
    await sendCommand(props.server.id, command)
}

// Starting a server invalidates responses from the previous run.
const consoleLayout = ref<InstanceType<typeof ConsolePageLayout> | null>(null)
watch(
    () => props.server.running,
    async (running, previousRunning) => {
        if (!running) {
            stopSync()
            pendingConsoleOutput = new ServerConsoleBuffer(PENDING_CONSOLE_OUTPUT_CAPACITY)
            await hydration
            await hydrateAndDisplay()
            return
        }
        if (previousRunning) return
        await nextTick()
        if (disposed) return
        flushConsoleOutput()
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
    sendCommand: (command: string) => void handleSendCommand(command),
    showCommandInput: computed(() => props.server.running),
    disableCommandInput: computed(() => !props.server.running),
    disableCommandInputTooltip: computed(() => formatMessage(messages.notRunning)),
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
    <div
        data-onboarding-id="server-console"
        class="flex flex-col pb-3"
        :class="hasLogs ? 'h-[calc(100dvh-80px)] shrink-0' : 'h-full min-h-[240px]'"
    >
        <ConsolePageLayout ref="consoleLayout" :custom-command-input="isForge">
            <template #command-input="{ disabled }">
                <JLineCommandInput
                    ref="jlineInput"
                    :disabled="disabled"
                    :placeholder="formatMessage(messages.forgeCommandPlaceholder)"
                    :send-command="handleSendCommand"
                    :send-input="(data) => servers.sendConsoleInput(server.id, data)"
                    :resize-console="(cols, rows) => servers.resizeConsole(server.id, cols, rows)"
                />
            </template>
        </ConsolePageLayout>
    </div>
</template>
