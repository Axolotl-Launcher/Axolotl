<template>
    <div class="flex flex-col gap-4 h-full">
        <ConsolePageLayout />
    </div>
</template>

<script setup>
import {
    ConsolePageLayout,
    defineMessages,
    injectNotificationManager,
    provideConsoleManager,
    useVIntl,
} from '@modrinth/ui'
import { computed, onUnmounted, ref, shallowRef, triggerRef, watch, watchEffect } from 'vue'
import { useRoute } from 'vue-router'

import { useCrashAnalysis } from '@/composables/useCrashAnalysis'
import { useInstanceConsole } from '@/composables/useInstanceConsole'
import { log_listener, process_listener } from '@/helpers/events.js'
import {
    delete_logs_by_filename,
    export_crash_context,
    get_output_by_filename,
} from '@/helpers/logs.js'
import { createRequestGeneration } from '@/helpers/request-generation'

const { handleError } = injectNotificationManager()
const route = useRoute()
const { formatMessage } = useVIntl()

const messages = defineMessages({
    liveLog: { id: 'instance.logs.source.live', defaultMessage: 'Live Log' },
    unknownLog: { id: 'instance.logs.source.unknown', defaultMessage: 'Unknown' },
    logName: { id: 'instance.logs.source.numbered', defaultMessage: 'Log {index}' },
    cannotDeleteLatest: {
        id: 'instance.logs.delete.latest-running',
        defaultMessage: 'Cannot delete latest.log while the instance is running',
    },
})

const props = defineProps({
    instance: {
        type: Object,
        default() {
            return {}
        },
    },
    options: {
        type: Object,
        default() {
            return {}
        },
    },
    offline: {
        type: Boolean,
        default() {
            return false
        },
    },
    playing: {
        type: Boolean,
        default() {
            return false
        },
    },
    installed: {
        type: Boolean,
        default() {
            return false
        },
    },
})

const instanceId = computed(() => route.params.id)
const {
    analysis: localCrashAnalysis,
    loading: crashAnalysisLoading,
    refresh: refreshCrashAnalysis,
    clear: clearCrashAnalysis,
} = useCrashAnalysis(instanceId.value)
const {
    liveConsole,
    historicalConsole,
    hydrate,
    getHistoricalLogs,
    getHistoricalContent,
    invalidate,
    clearLive,
} = useInstanceConsole(instanceId.value)

await hydrate()

function buildLogList(rawLogs) {
    return [
        { name: formatMessage(messages.liveLog), live: true },
        ...rawLogs
            .filter(
                (log) =>
                    log.filename !== 'latest_stdout.log' &&
                    log.filename !== 'latest_stdout' &&
                    log.filename !== 'launcher_log.txt' &&
                    (log.output == null || log.output !== '') &&
                    (log.filename.includes('.log') || log.filename.endsWith('.txt')),
            )
            .map((log) => ({
                ...log,
                name: log.filename || formatMessage(messages.unknownLog),
            })),
    ]
}

const logs = ref(buildLogList([]))
const logsLoading = ref(true)
const historicalLoading = ref(false)
const historyRequests = createRequestGeneration()

async function loadHistoricalLogs() {
    const request = historyRequests.begin(`${props.instance.id}:list`)
    logsLoading.value = true
    try {
        const allLogs = await getHistoricalLogs()
        if (!historyRequests.isCurrent(request, `${props.instance.id}:list`)) return
        logs.value = buildLogList(allLogs)
    } catch (error) {
        if (historyRequests.isCurrent(request, `${props.instance.id}:list`)) handleError(error)
    } finally {
        if (historyRequests.isCurrent(request, `${props.instance.id}:list`))
            logsLoading.value = false
    }
}

void loadHistoricalLogs()

const selectedLogIndex = ref(0)
const isLive = computed(() => selectedLogIndex.value === 0)

const filteredLogs = computed(() =>
    props.playing ? logs.value.filter((l) => l.live || l.name !== 'latest.log') : logs.value,
)

const logSources = computed(() =>
    filteredLogs.value.map((l, i) => ({
        id: String(i),
        name: l?.name ?? formatMessage(messages.logName, { index: i }),
        live: l?.live ?? false,
    })),
)

const activeConsole = computed(() => (isLive.value ? liveConsole : historicalConsole))

const logLines = shallowRef(activeConsole.value.output.value)
watchEffect(() => {
    logLines.value = activeConsole.value.output.value
    triggerRef(logLines)
})

async function analyseForCrash() {
    await refreshCrashAnalysis().catch((error) => {
        handleError(error)
    })
}

async function exportCrashContext() {
    await export_crash_context(props.instance.id, props.instance.name).catch(handleError)
}

const selectedLog = computed(() => filteredLogs.value[selectedLogIndex.value])

const deleteDisabled = computed(() => {
    const log = selectedLog.value
    if (!log || log.live) return true
    return log.filename === 'latest.log' && props.playing
})

async function deleteSelectedLog() {
    const log = selectedLog.value
    if (!log || log.live) return
    await delete_logs_by_filename(props.instance.id, log.log_type, log.filename)
    invalidate()
    await loadHistoricalLogs()
    selectedLogIndex.value = 0
}

provideConsoleManager({
    logLines,
    logSources,
    activeLogSourceIndex: selectedLogIndex,
    showCommandInput: false,
    loading: computed(() => !isLive.value && (logsLoading.value || historicalLoading.value)),
    onClear: () => {
        if (!isLive.value) return
        void clearLive()
    },
    onDelete: deleteSelectedLog,
    deleteDisabled,
    deleteDisabledTooltip: computed(() => formatMessage(messages.cannotDeleteLatest)),
    shareDisabled: computed(() => props.offline),
    emptyStateType: 'instance',
    localCrashAnalysis,
    crashAnalysisLoading,
    onExportCrashContext: exportCrashContext,
})

watch(selectedLogIndex, async (newIndex) => {
    if (newIndex === 0) return
    const log = filteredLogs.value[newIndex]
    if (!log) return

    const requestKey = `${props.instance.id}:${log.log_type}:${log.filename}`
    const request = historyRequests.begin(requestKey)
    historicalLoading.value = true
    historicalConsole.clear()
    const cached = getHistoricalContent(log.filename)
    try {
        const output =
            cached !== undefined
                ? cached
                : await get_output_by_filename(props.instance.id, log.log_type, log.filename)
        if (!historyRequests.isCurrent(request, requestKey)) return
        if (output) await historicalConsole.addLegacyLog(output)
    } catch (error) {
        if (historyRequests.isCurrent(request, requestKey)) handleError(error)
    } finally {
        if (historyRequests.isCurrent(request, requestKey)) historicalLoading.value = false
    }
})

selectedLogIndex.value = 0

if (!props.playing) {
    void analyseForCrash()
}

const unlistenLog = await log_listener((payload) => {
    if (payload.instance_id !== instanceId.value) return

    if (payload.type === 'log4j') {
        liveConsole.addLog4jEvent(payload)
    } else if (payload.type === 'legacy') {
        void liveConsole.addLegacyLog(payload.message)
    }
})

const unlistenProcesses = await process_listener(async (e) => {
    if (e.instance_id !== instanceId.value) return
    if (e.event === 'launched') {
        liveConsole.clear()
        clearCrashAnalysis()
        invalidate()
        selectedLogIndex.value = 0
    }
    if (e.event === 'finished') {
        invalidate()
        await loadHistoricalLogs()
        void analyseForCrash()
    }
})

onUnmounted(() => {
    unlistenLog()
    unlistenProcesses()
})
</script>
