<script setup lang="ts">
import '@xterm/xterm/css/xterm.css'

import { ContractIcon, DownloadIcon, ExpandIcon, SearchIcon } from '@modrinth/assets'
import {
    Button,
    commonMessages,
    defineMessages,
    injectNotificationManager,
    StyledInput,
    useVIntl,
} from '@modrinth/ui'
import { save } from '@tauri-apps/plugin-dialog'
import { writeTextFile } from '@tauri-apps/plugin-fs'
import { FitAddon } from '@xterm/addon-fit'
import { SearchAddon } from '@xterm/addon-search'
import { Unicode11Addon } from '@xterm/addon-unicode11'
import { type IDisposable, Terminal } from '@xterm/xterm'
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { type ServerView, subscribeServerConsoleOutput } from '@/composables/useServers'
import { base64ToBytes, type ServerLogCursor, servers } from '@/helpers/servers'

const props = defineProps<{ server: ServerView; view: 'terminal' | 'logs' }>()
const emit = defineEmits<{ 'update:view': [view: 'terminal' | 'logs'] }>()
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const messages = defineMessages({
    terminal: { id: 'app.servers.console.terminal', defaultMessage: 'Terminal' },
    logs: { id: 'app.servers.console.logs', defaultMessage: 'Logs' },
    notRunning: {
        id: 'app.servers.console.not-running',
        defaultMessage: 'The server is not running',
    },
    expand: { id: 'console.action.expand', defaultMessage: 'Expand' },
    collapse: { id: 'console.action.collapse', defaultMessage: 'Collapse' },
})

const host = ref<HTMLElement>()
const terminalHost = ref<HTMLElement>()
const exporting = ref(false)
const fullscreen = ref(false)
const query = ref('')
let terminal: Terminal | undefined
let fit: FitAddon | undefined
let search: SearchAddon | undefined
let resizeObserver: ResizeObserver | undefined
let themeObserver: MutationObserver | undefined
let unsubscribe: (() => void) | undefined
const disposables: IDisposable[] = []
let disposed = false
let generation = 0
let cursor: ServerLogCursor | undefined
let timer: ReturnType<typeof setTimeout> | undefined
let syncDue = 0
let frame: number | undefined
let fetching = false
let dirty = false
let inputQueue = Promise.resolve()
let resizeQueue = Promise.resolve()
let lastSize = ''
const encoder = new TextEncoder()

function report(error: unknown, session: number) {
    if (!disposed && generation === session && props.server.running) handleError(error)
}

function sendBytes(bytes: Uint8Array) {
    if (disposed || !props.server.running) return
    const serverId = props.server.id
    const session = generation
    inputQueue = inputQueue
        .then(async () => {
            if (disposed || session !== generation || !props.server.running) return
            // Keep paste and terminal replies ordered, including pastes above the IPC limit.
            for (let offset = 0; offset < bytes.length; offset += 64 * 1024) {
                if (disposed || session !== generation || !props.server.running) return
                await servers.sendConsoleInput(serverId, bytes.slice(offset, offset + 64 * 1024))
            }
        })
        .catch((error) => report(error, session))
}

function scheduleSync(delay = 1000) {
    if (disposed) return
    const due = performance.now() + delay
    if (timer && syncDue <= due) return
    if (timer) clearTimeout(timer)
    syncDue = due
    timer = setTimeout(() => {
        timer = undefined
        void sync()
    }, delay)
}

async function sync() {
    if (disposed || !terminal) return
    if (fetching) {
        dirty = true
        return
    }
    fetching = true
    dirty = false
    const session = generation
    const serverId = props.server.id
    try {
        const delta = await servers.getConsoleDelta(serverId, cursor)
        if (disposed || generation !== session) return
        const bytes = base64ToBytes(delta.data)
        if (delta.reset) terminal.reset()
        if (bytes.length) {
            await new Promise<void>((resolve) => terminal!.write(bytes, resolve))
        }
        if (disposed || generation !== session) return
        cursor = delta.cursor
    } catch (error) {
        report(error, session)
    } finally {
        fetching = false
        if (!disposed && (dirty || props.server.running || props.server.starting))
            scheduleSync(dirty ? 0 : 1000)
    }
}

function requestSync() {
    if (fetching) dirty = true
    else scheduleSync(16)
}

function resize() {
    if (!terminal || !fit || !host.value?.clientWidth || !host.value.clientHeight) return
    fit.fit()
    if (!props.server.running) return
    const cols = Math.min(500, Math.max(2, terminal.cols))
    const rows = Math.min(8192, Math.max(1, terminal.rows))
    if (terminal.cols !== cols || terminal.rows !== rows) terminal.resize(cols, rows)
    const size = `${cols}:${rows}`
    if (size === lastSize) return
    lastSize = size
    const serverId = props.server.id
    const session = generation
    resizeQueue = resizeQueue
        .then(async () => {
            if (disposed || generation !== session || !props.server.running) return
            await servers.resizeConsole(serverId, cols, rows)
        })
        .catch((error) => {
            lastSize = ''
            report(error, session)
        })
}

function scheduleResize() {
    if (frame) cancelAnimationFrame(frame)
    frame = requestAnimationFrame(() => {
        frame = undefined
        resize()
    })
}

function updateTheme() {
    if (!terminal || !host.value) return
    const style = getComputedStyle(host.value)
    const color = (name: string) => style.getPropertyValue(name).trim()
    terminal.options.theme = {
        background: color('--surface-2'),
        foreground: color('--color-text-default'),
        cursor: color('--color-text-primary'),
        selectionBackground: color('--color-brand-highlight'),
    }
}

function find(previous = false) {
    if (!query.value) {
        search?.clearDecorations()
        return
    }
    if (previous) search?.findPrevious(query.value)
    else search?.findNext(query.value)
}

async function download() {
    if (!terminal || exporting.value) return
    const buffer = terminal.buffer.active
    const lines: string[] = []
    for (let index = 0; index < buffer.length; index++) {
        const line = buffer.getLine(index)
        if (!line) continue
        const text = line.translateToString(!buffer.getLine(index + 1)?.isWrapped)
        if (line.isWrapped && lines.length) lines[lines.length - 1] += text
        else lines.push(text)
    }
    const text = lines.join('\n').trimEnd() + '\n'
    exporting.value = true
    try {
        const path = await save({
            defaultPath: `${props.server.id}-console.txt`,
        })
        if (path && !disposed) await writeTextFile(path, text)
    } catch (error) {
        if (!disposed) handleError(error)
    } finally {
        exporting.value = false
    }
}

function bindSession() {
    generation++
    cursor = undefined
    lastSize = ''
    dirty = true
    unsubscribe?.()
    unsubscribe = subscribeServerConsoleOutput(props.server.id, requestSync)
    terminal?.reset()
    scheduleResize()
    void sync()
}

watch(() => props.server.id, bindSession)
watch(
    () => props.server.running || props.server.starting,
    (active, previous) => {
        if (active && !previous) bindSession()
        else requestSync()
    },
)
watch(
    () => props.server.running,
    (running) => {
        if (terminal) terminal.options.disableStdin = !running
        if (running) {
            lastSize = ''
            scheduleResize()
        }
    },
)
watch(
    () => [fullscreen.value, props.view],
    async () => {
        await nextTick()
        scheduleResize()
    },
)
watch(query, () => find())

onMounted(() => {
    terminal = new Terminal({
        cols: 80,
        rows: 24,
        scrollback: 10000,
        fontFamily: 'Consolas, "Cascadia Mono", "Liberation Mono", monospace',
        fontSize: 14,
        cursorBlink: true,
        disableStdin: !props.server.running,
        allowProposedApi: true,
        scrollOnUserInput: true,
    })
    fit = new FitAddon()
    search = new SearchAddon()
    terminal.loadAddon(fit)
    terminal.loadAddon(search)
    terminal.loadAddon(new Unicode11Addon())
    terminal.unicode.activeVersion = '11'
    terminal.open(terminalHost.value!)
    updateTheme()
    disposables.push(terminal.onData((data) => sendBytes(encoder.encode(data))))
    disposables.push(
        terminal.onBinary((data) => sendBytes(Uint8Array.from(data, (char) => char.charCodeAt(0)))),
    )
    terminal.attachCustomKeyEventHandler((event) => {
        // Preserve native copy/paste shortcuts and let Ctrl+C reach the server when no text is selected.
        if ((event.ctrlKey || event.metaKey) && !event.altKey) {
            const key = event.key.toLowerCase()
            if (key === 'c' && terminal?.hasSelection()) return false
            if (key === 'v') return false
        }
        return true
    })
    resizeObserver = new ResizeObserver(scheduleResize)
    resizeObserver.observe(host.value!)
    themeObserver = new MutationObserver(updateTheme)
    themeObserver.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ['class', 'style'],
    })
    themeObserver.observe(document.body, { attributes: true, attributeFilter: ['class', 'style'] })
    bindSession()
    void document.fonts.ready.then(() => {
        if (!disposed) scheduleResize()
    })
})

onBeforeUnmount(() => {
    disposed = true
    generation++
    unsubscribe?.()
    if (timer) clearTimeout(timer)
    if (frame) cancelAnimationFrame(frame)
    resizeObserver?.disconnect()
    themeObserver?.disconnect()
    disposables.forEach((disposable) => disposable.dispose())
    terminal?.dispose()
})
</script>

<template>
    <Teleport to="body" :disabled="!fullscreen">
        <div
            class="flex min-h-0 min-w-0 flex-1 flex-col gap-3"
            :class="fullscreen ? 'fixed inset-0 z-[150] bg-surface-1 p-6 pt-12' : ''"
        >
            <div v-if="fullscreen" data-tauri-drag-region class="absolute inset-x-0 top-0 h-10" />
            <div class="flex shrink-0 flex-wrap items-center gap-3">
                <div class="flex shrink-0 items-center gap-1 rounded-xl bg-surface-2 p-1">
                    <Button
                        :type="view === 'terminal' ? 'chip' : 'quiet'"
                        :aria-pressed="view === 'terminal'"
                        @click="emit('update:view', 'terminal')"
                    >
                        {{ formatMessage(messages.terminal) }}
                    </Button>
                    <Button
                        :type="view === 'logs' ? 'chip' : 'quiet'"
                        :aria-pressed="view === 'logs'"
                        @click="emit('update:view', 'logs')"
                    >
                        {{ formatMessage(messages.logs) }}
                    </Button>
                </div>
                <template v-if="view === 'terminal'">
                    <StyledInput
                        v-model="query"
                        :icon="SearchIcon"
                        :placeholder="formatMessage(commonMessages.searchPlaceholder)"
                        wrapper-class="min-w-0 flex-1 sm:max-w-sm sm:ml-auto"
                        clearable
                        @keydown.enter="find($event.shiftKey)"
                    />
                    <Button
                        v-tooltip="formatMessage(commonMessages.downloadButton)"
                        type="quiet"
                        icon-only
                        :label="formatMessage(commonMessages.downloadButton)"
                        :loading="exporting"
                        @click="download"
                        ><DownloadIcon
                    /></Button>
                </template>
                <Button
                    v-tooltip="formatMessage(fullscreen ? messages.collapse : messages.expand)"
                    type="quiet"
                    icon-only
                    :label="formatMessage(fullscreen ? messages.collapse : messages.expand)"
                    @click="fullscreen = !fullscreen"
                >
                    <ContractIcon v-if="fullscreen" /><ExpandIcon v-else />
                </Button>
            </div>
            <div v-show="view === 'terminal'" class="flex min-h-0 flex-1 flex-col gap-2">
                <div
                    ref="host"
                    class="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-xl bg-surface-2 p-3"
                    :aria-label="formatMessage(messages.terminal)"
                >
                    <div ref="terminalHost" class="min-h-0 min-w-0 flex-1" />
                </div>
                <span
                    v-if="!server.running && !server.starting"
                    class="text-sm text-[var(--color-text-tertiary)]"
                    >{{ formatMessage(messages.notRunning) }}</span
                >
            </div>
            <div v-show="view === 'logs'" class="flex min-h-0 flex-1 flex-col"><slot /></div>
        </div>
    </Teleport>
</template>
