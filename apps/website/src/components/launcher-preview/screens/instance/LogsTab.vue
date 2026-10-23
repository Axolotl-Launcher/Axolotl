<template>
    <div class="flex h-full flex-col gap-4">
        <ConsolePageLayout />
    </div>
</template>

<script setup lang="ts">
import { createConsoleState } from '@modrinth/ui/src/composables/console-state.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { provideConsoleManager } from '@modrinth/ui/src/layouts/shared/console/providers/console-manager.ts'
import { computed, ref, shallowRef, triggerRef, watch, watchEffect } from 'vue'

import { usePreview, usePreviewDomain } from '../../lib/state'
import type { GameInstance } from '../../lib/types'
import ConsolePageLayout from '../../shared/console/layout.vue'
import { useInstanceLogs } from './logs/state'
import { logsMessages as messages } from './messages'

/**
 * Port of `apps/app-frontend/src/pages/instance/Logs.vue`. The live log is the
 * instance's game process in the preview; the log files are the instance's
 * `logs/` folder, which gains the output of every session played here.
 */
const props = defineProps<{
    instance: GameInstance
    playing?: boolean
    offline?: boolean
}>()

const { formatMessage } = useVIntl()
const preview = usePreview()
const logFiles = useInstanceLogs()

type ConsoleState = ReturnType<typeof createConsoleState>

/** The app's `useInstanceConsole` store: one live and one historical console per instance. */
const consoles = usePreviewDomain('instance-consoles', () => ({
    value: new Map<
        string,
        { live: ConsoleState; historical: ConsoleState; process: string | null; fed: number }
    >(),
}))

function consoleFor(instanceId: string) {
    let entry = consoles.get(instanceId)
    if (!entry) {
        entry = {
            live: createConsoleState(),
            historical: createConsoleState(),
            process: null,
            fed: 0,
        }
        consoles.set(instanceId, entry)
    }
    return entry
}

const entry = consoleFor(props.instance.id)
const liveConsole = entry.live
const historicalConsole = entry.historical

/** Feeds the live console from the instance's game process, as the app's log events do. */
const process = computed(() => preview.processes.lastByInstance.value[props.instance.id] ?? null)
watch(
    () => [process.value?.uuid, process.value?.logs.length] as const,
    () => {
        const current = process.value
        if (!current) return
        if (entry.process !== current.uuid) {
            entry.process = current.uuid
            entry.fed = 0
            liveConsole.clear()
        }
        const lines = current.logs.slice(entry.fed)
        if (!lines.length) return
        entry.fed = current.logs.length
        void liveConsole.addLegacyLog(lines.join('\n'))
    },
    { immediate: true },
)

const logs = computed(() => [
    { name: formatMessage(messages.liveLog), live: true, filename: '', log_type: '' },
    ...logFiles.filesFor(props.instance.id).map((file) => ({
        name: file.filename || formatMessage(messages.unknownLog),
        live: false,
        filename: file.filename,
        log_type: file.log_type,
    })),
])

const selectedLogIndex = ref(0)
const isLive = computed(() => selectedLogIndex.value === 0)

const filteredLogs = computed(() =>
    props.playing ? logs.value.filter((log) => log.live || log.name !== 'latest.log') : logs.value,
)

const logSources = computed(() =>
    filteredLogs.value.map((log, index) => ({
        id: String(index),
        name: log?.name ?? formatMessage(messages.logName, { index }),
        live: log?.live ?? false,
    })),
)

const activeConsole = computed(() => (isLive.value ? liveConsole : historicalConsole))

const logLines = shallowRef(activeConsole.value.output.value)
watchEffect(() => {
    logLines.value = activeConsole.value.output.value
    triggerRef(logLines)
})

const selectedLog = computed(() => filteredLogs.value[selectedLogIndex.value])

const deleteDisabled = computed(() => {
    const log = selectedLog.value
    if (!log || log.live) return true
    return log.filename === 'latest.log' && !!props.playing
})

async function deleteSelectedLog() {
    const log = selectedLog.value
    if (!log || log.live) return
    logFiles.deleteFile(props.instance.id, log.filename)
    selectedLogIndex.value = 0
}

provideConsoleManager({
    logLines,
    logSources,
    activeLogSourceIndex: selectedLogIndex,
    showCommandInput: false,
    loading: computed(() => false),
    onClear: () => {
        if (!isLive.value) return
        liveConsole.clear()
        entry.fed = process.value?.logs.length ?? 0
    },
    onDelete: deleteSelectedLog,
    deleteDisabled,
    deleteDisabledTooltip: computed(() => formatMessage(messages.cannotDeleteLatest)),
    shareDisabled: computed(() => !!props.offline),
    emptyStateType: 'instance',
    localCrashAnalysis: ref(null),
    crashAnalysisLoading: ref(false),
})

watch(selectedLogIndex, async (newIndex) => {
    if (newIndex === 0) return
    const log = filteredLogs.value[newIndex]
    if (!log) return
    historicalConsole.clear()
    const file = logFiles.filesFor(props.instance.id).find((item) => item.filename === log.filename)
    if (file?.lines.length) await historicalConsole.addLegacyLog(file.lines.join('\n'))
})

// A new session starts on the live log, as the app's `launched` event does.
watch(
    () => process.value?.uuid,
    (uuid, previous) => {
        if (uuid && uuid !== previous) selectedLogIndex.value = 0
    },
)
</script>
