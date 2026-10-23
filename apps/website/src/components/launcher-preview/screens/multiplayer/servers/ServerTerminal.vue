<script setup lang="ts">
import ContractIcon from '@modrinth/assets/icons/contract.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import ExpandIcon from '@modrinth/assets/icons/expand.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { injectPreviewWindow } from '../../../lib/window'
import DesktopAppLink from '../../../shell/DesktopAppLink.vue'
import { base64ToBytes, type ServerLogCursor, servers, unrecordedCommand } from '../backend.ts'
import { type ServerView, subscribeServerConsoleOutput } from '../composables/useServers.ts'
import { serverPreviewMessages } from '../server-preview-messages.ts'

/**
 * `components/multiplayer/servers/ServerTerminal.vue` without xterm.js, which
 * the site does not ship: the same toolbar, terminal panel and Logs view, with
 * the server's terminal output drawn as text. Typing goes to the server as it
 * does through xterm (`onData`): characters, Enter, Backspace, Ctrl+C, paste.
 */
const props = defineProps<{ server: ServerView; view: 'terminal' | 'logs' }>()
const emit = defineEmits<{ 'update:view': [view: 'terminal' | 'logs'] }>()
const { formatMessage } = useVIntl()
const { layer } = injectPreviewWindow()
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

const SCROLLBACK = 10_000
const host = ref<HTMLElement>()
const screen = ref<HTMLElement>()
const fullscreen = ref(false)
const query = ref('')
const lines = ref<string[]>([''])
const match = ref<{ line: number; start: number; end: number } | null>(null)
const focused = ref(false)
const pending = ref<string | null>(null)
let cursor: ServerLogCursor | undefined
let unsubscribe: (() => void) | undefined
let fetching = false
let dirty = false
let disposed = false
let generation = 0
const encoder = new TextEncoder()
const decoder = new TextDecoder()

const active = computed(() => props.server.running || props.server.starting)

/** Applies terminal output: CR/LF, backspace erase, ANSI sequences ignored. */
function writeText(text: string) {
    const next = [...lines.value]
    let column = next[next.length - 1].length
    // ANSI escape sequences start with the ESC control character.
    // eslint-disable-next-line no-control-regex
    const clean = text.replace(/\x1b\[[0-9;?]*[ -/]*[@-~]/g, '')
    for (const char of clean) {
        const row = next.length - 1
        if (char === '\n') {
            next.push('')
            column = 0
        } else if (char === '\r') {
            column = 0
        } else if (char === '\b') {
            column = Math.max(0, column - 1)
        } else if (char >= ' ' || char === '\t') {
            const line = next[row]
            next[row] = line.slice(0, column) + char + line.slice(column + 1)
            column++
        }
    }
    lines.value = next.length > SCROLLBACK ? next.slice(next.length - SCROLLBACK) : next
}

async function sync() {
    if (disposed) return
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
        if (delta.reset) lines.value = ['']
        const text = decoder.decode(base64ToBytes(delta.data))
        if (text) {
            const stick = isAtBottom()
            writeText(text)
            if (stick) void nextTick(scrollToBottom)
        }
        cursor = delta.cursor
        pending.value = unrecordedCommand(serverId)
    } finally {
        fetching = false
        if (dirty && !disposed) void sync()
    }
}

function isAtBottom() {
    const element = screen.value
    return !element || element.scrollHeight - element.scrollTop - element.clientHeight < 24
}

function scrollToBottom() {
    if (screen.value) screen.value.scrollTop = screen.value.scrollHeight
}

function send(text: string) {
    if (!props.server.running || !text) return
    void servers.sendConsoleInput(props.server.id, encoder.encode(text))
}

function onKeydown(event: KeyboardEvent) {
    if (!props.server.running) return
    const key = event.key
    if ((event.ctrlKey || event.metaKey) && !event.altKey) {
        const lower = key.toLowerCase()
        // Copy with a selection and paste stay native, as in the app's terminal.
        if (lower === 'c' && window.getSelection()?.toString()) return
        if (lower === 'v') return
        if (lower === 'c' && event.ctrlKey) {
            event.preventDefault()
            send('\x03')
        }
        return
    }
    if (key === 'Enter') send('\r')
    else if (key === 'Backspace') send('\x7f')
    else if (key === 'Tab') send('\t')
    else if (key.length === 1) send(key)
    else return
    event.preventDefault()
}

function onPaste(event: ClipboardEvent) {
    const text = event.clipboardData?.getData('text') ?? ''
    if (!text || !props.server.running) return
    event.preventDefault()
    send(text.replace(/\r?\n/g, '\r'))
}

function find(previous = false) {
    const needle = query.value.toLocaleLowerCase()
    if (!needle) {
        match.value = null
        return
    }
    const hits: { line: number; start: number; end: number }[] = []
    lines.value.forEach((line, index) => {
        let from = line.toLocaleLowerCase().indexOf(needle)
        while (from !== -1) {
            hits.push({ line: index, start: from, end: from + needle.length })
            from = line.toLocaleLowerCase().indexOf(needle, from + 1)
        }
    })
    if (!hits.length) {
        match.value = null
        return
    }
    const current = match.value
    const position = current
        ? hits.findIndex((hit) => hit.line === current.line && hit.start === current.start)
        : -1
    const next = previous
        ? hits[(position <= 0 ? hits.length : position) - 1]
        : hits[(position + 1) % hits.length]
    match.value = next
    void nextTick(() =>
        screen.value?.querySelector('[data-search-match]')?.scrollIntoView({ block: 'nearest' }),
    )
}

function download() {
    const text = lines.value.join('\n').trimEnd() + '\n'
    const link = document.createElement('a')
    link.href = URL.createObjectURL(new Blob([text], { type: 'text/plain' }))
    link.download = `${props.server.id}-console.txt`
    link.click()
    URL.revokeObjectURL(link.href)
}

function bindSession() {
    generation++
    cursor = undefined
    unsubscribe?.()
    unsubscribe = subscribeServerConsoleOutput(props.server.id, () => void sync())
    lines.value = ['']
    match.value = null
    void sync()
}

watch(() => props.server.id, bindSession)
watch(active, (now, before) => {
    if (now && !before) bindSession()
    else void sync()
})
watch(query, () => find())

onMounted(bindSession)
onBeforeUnmount(() => {
    disposed = true
    generation++
    unsubscribe?.()
})
</script>

<template>
    <Teleport :to="layer ?? 'body'" :disabled="!fullscreen">
        <div
            class="flex min-h-0 min-w-0 flex-1 flex-col gap-3"
            :class="fullscreen ? 'fixed inset-0 z-[150] bg-surface-1 p-6 pt-12' : ''"
        >
            <div v-if="fullscreen" class="absolute inset-x-0 top-0 h-10" />
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
                    <div
                        ref="screen"
                        role="log"
                        :tabindex="server.running ? 0 : -1"
                        class="custom-focus-indicator min-h-0 min-w-0 flex-1 overflow-y-auto whitespace-pre-wrap break-all font-[Consolas,'Cascadia_Mono','Liberation_Mono',monospace] text-[14px] leading-[1.2] text-[var(--color-text-default)] outline-none selection:bg-brand-highlight"
                        @keydown="onKeydown"
                        @paste="onPaste"
                        @focus="focused = true"
                        @blur="focused = false"
                    >
                        <div v-for="(line, index) in lines" :key="index" class="min-h-[1.2em]">
                            <template v-if="match && match.line === index"
                                >{{ line.slice(0, match.start)
                                }}<mark data-search-match class="bg-brand-highlight text-inherit">{{
                                    line.slice(match.start, match.end)
                                }}</mark
                                >{{ line.slice(match.end) }}</template
                            ><template v-else>{{ line }}</template
                            ><span
                                v-if="index === lines.length - 1 && server.running"
                                aria-hidden="true"
                                class="inline-block h-[1.1em] w-[0.6em] translate-y-[0.15em] bg-[var(--color-text-primary)]"
                                :class="focused ? 'animate-pulse' : 'opacity-60'"
                            />
                        </div>
                    </div>
                </div>
                <span
                    v-if="!server.running && !server.starting"
                    class="text-sm text-[var(--color-text-tertiary)]"
                    >{{ formatMessage(messages.notRunning) }}</span
                >
                <span v-else class="text-sm text-[var(--color-text-tertiary)]">{{
                    formatMessage(serverPreviewMessages.recordedSession)
                }}</span>
                <Admonition
                    v-if="pending"
                    type="info"
                    :body="
                        formatMessage(serverPreviewMessages.unrecordedCommand, { command: pending })
                    "
                >
                    <template #actions><DesktopAppLink /></template>
                </Admonition>
            </div>
            <div v-show="view === 'logs'" class="flex min-h-0 flex-1 flex-col"><slot /></div>
        </div>
    </Teleport>
</template>
