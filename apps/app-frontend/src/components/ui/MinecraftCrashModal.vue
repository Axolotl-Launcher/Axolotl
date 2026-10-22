<script setup lang="ts">
import {
    ClipboardCopyIcon,
    ExternalIcon,
    LinkIcon,
    ListOrderedIcon,
    ScanEyeIcon,
    ShareIcon,
    XIcon,
} from '@modrinth/assets'
import {
    Button,
    Card,
    commonMessages,
    defineMessages,
    injectModrinthClient,
    injectNotificationManager,
    type LogLine,
    LogViewport,
    NewModal,
    ProgressBar,
    shareLogs,
    useVIntl,
} from '@modrinth/ui'
import { renderHighlightedString } from '@modrinth/utils/highlightjs'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import CrashModChangesModal from '@/components/ui/CrashModChangesModal.vue'
import LogAgentActivity from '@/components/ui/LogAgentActivity.vue'
import LogAgentSourcePicker from '@/components/ui/LogAgentSourcePicker.vue'
import LogShareSettings from '@/components/ui/settings/LogShareSettings.vue'
import {
    clearCrashAnalysis,
    type CrashAnalysisResult,
    refreshCrashAnalysis,
} from '@/composables/useCrashAnalysis'
import { useLogAgent } from '@/composables/useLogAgent'
import type { MinecraftLaunchErrorPayload } from '@/composables/useMinecraftLaunchError'
import { getAIState } from '@/helpers/ai'
import { process_listener } from '@/helpers/events.js'
import { get as getInstance } from '@/helpers/instance'
import {
    get_crash_analysis_ai_settings,
    get_log_share_settings,
    get_logs,
    get_output_by_filename,
    logshare_get_insights,
    logshare_upload_crash,
    record_shared_log,
} from '@/helpers/logs'
import { shouldShowMinecraftCrash } from '@/helpers/process.js'
import { visibleDraft } from '@/logagent/progress'
import { parseLogAgentInsight } from '@/logagent/result'
import type { AnalysisSource } from '@/logagent/types'

import { createCrashModalSession } from './crash-modal-session'
import {
    combineCrashLogs,
    type CrashLogFile,
    crashLogKey,
    crashLogLabel,
    preferredCrashLogKey,
    selectCrashLogFiles,
    shouldUseLogShareAutoAnalysis,
} from './minecraft-crash-logs'

interface CrashModalPayload extends MinecraftLaunchErrorPayload {
    title?: string
    summary?: string
    body?: string
    hint?: string
}

interface ProcessEvent {
    instance_id: string
    uuid: string
    event: 'launched' | 'finished'
    crashed?: boolean
}

interface CrashWarningPayload extends MinecraftLaunchErrorPayload {
    kind: 'minecraft_crash'
}

interface LogShareTicket {
    id: string
    url: string
    raw: string
    token: string
}

interface LogShareSettings {
    share_provider: 'logshare' | 'mclogs'
    ai_source: 'logshare' | 'custom'
    auto_upload: boolean
    multi_file: boolean
    no_storage: boolean
    show_progress: boolean
}

type Unlisten = () => void

const { formatMessage } = useVIntl()
const client = injectModrinthClient()
const router = useRouter()
const route = useRoute()
const localSetupModal = ref<InstanceType<typeof NewModal>>()
const localSetupSaving = ref(false)
let resumeAfterProviderSettings = false
const { addNotification } = injectNotificationManager()
const modal = ref<InstanceType<typeof NewModal>>()
const modChangesModal = ref<InstanceType<typeof CrashModChangesModal>>()
const payload = ref<Partial<CrashModalPayload>>({})
let lastAnalysis: CrashAnalysisResult | null = null
const modChangesAvailable = ref(false)
const activeRuns = new Map<string, string>()
const visibleSession = createCrashModalSession()
const lastShownAt = new Map<string, number>()
let unlistenProcess: Unlisten | undefined
let mounted = false
let analysisVersion = 0
let summaryGeneration = 0
let crashLogsPromise: Promise<void> | null = null
let uploadTicketPromise: Promise<LogShareTicket | null> | null = null
const defaultSource = ref<AnalysisSource>('logshare')
const selectedSource = ref<AnalysisSource>('logshare')
const localAvailable = ref(false)
const configuredModel = ref('')
const configuredProvider = ref('')
const choosingSource = ref(false)
const aiAvailable = computed(() => selectedSource.value === 'logshare' || localAvailable.value)
const agent = useLogAgent()
const aiOutput = agent.output
const aiLoading = agent.busy
const aiStatus = computed(() => {
    if (agent.status.value === 'cancelled') return formatMessage(messages.aiCancelled)
    if (agent.status.value === 'error')
        return formatMessage(messages.aiFailed, { message: agent.error.value })
    if (agent.status.value === 'preparing') return formatMessage(messages.aiPreparing)
    if (!aiLoading.value) return ''
    const event = agent.event.value
    if (event?.event_type === 'queued') {
        const position = Number(event.data.position ?? 0)
        return position > 0
            ? formatMessage(messages.aiQueued, { position })
            : formatMessage(messages.aiQueuedFront)
    }
    if (!logShareSettings.value.show_progress) return formatMessage(messages.aiWorking)
    if (event?.event_type === 'tool') return formatMessage(messages.aiCheckingLogs)
    if (event?.event_type === 'model_start') return formatMessage(messages.aiReviewing)
    if (event?.event_type === 'thinking') return formatMessage(messages.aiThinking)
    if (event?.event_type === 'tool_result') return formatMessage(messages.aiToolResult)
    if (event?.event_type === 'limit') return formatMessage(messages.aiReachedLimit)
    return formatMessage(messages.aiWorking)
})
const logShareSettingsLoaded = ref(false)

const logShareSettings = ref<LogShareSettings>({
    share_provider: 'logshare',
    ai_source: 'logshare',
    auto_upload: true,
    multi_file: true,
    no_storage: false,
    show_progress: true,
})
const uploadTicket = ref<LogShareTicket | null>(null)
const shareUrl = ref('')
const sharing = ref(false)
const logShareSummary = ref('')
const logShareSummaryLoading = ref(false)
const logShareSummaryState = ref<'idle' | 'loading' | 'ready' | 'empty' | 'unavailable' | 'error'>(
    'idle',
)
const logShareSummaryError = ref('')
const crashLogFiles = ref<CrashLogFile[]>([])
const crashLogsLoading = ref(false)
const crashLogsError = ref('')
const crashLogContents = ref<Record<string, string>>({})
const crashLogContentErrors = ref<Record<string, string>>({})
const selectedLogKey = ref('')
const selectedLogLoading = ref(false)
const activeTab = ref('')
const aiTabVisible = ref(false)
const AI_TAB = 'logagent'

const messages = defineMessages({
    returnToAnalysis: {
        id: 'app.logagent.controls.return-to-analysis',
        defaultMessage: 'Return to analysis',
    },
    changeSource: { id: 'app.logagent.controls.change-source', defaultMessage: 'Change' },
    sourceDone: { id: 'app.logagent.controls.source-done', defaultMessage: 'Done' },
    configureModel: {
        id: 'app.logagent.controls.configure-model',
        defaultMessage: 'Configure analysis',
    },
    startAnalysis: { id: 'app.logagent.controls.start', defaultMessage: 'Start analysis' },
    retryAnalysis: { id: 'app.logagent.controls.retry', defaultMessage: 'Analyze again' },
    localSending: {
        id: 'app.logagent.controls.local-sending',
        defaultMessage: 'Relevant log excerpts will be sent to {provider}.',
    },
    modelMissing: {
        id: 'app.logagent.controls.model-missing',
        defaultMessage: 'Choose and enable a model to use local analysis.',
    },
    analysisSection: { id: 'app.logagent.controls.section', defaultMessage: 'Analyze this crash' },
    aiCheckingLogs: {
        id: 'app.logagent.checking-logs',
        defaultMessage: 'Checking the relevant log evidence…',
    },
    aiReviewing: {
        id: 'app.logagent.reviewing',
        defaultMessage: 'Reviewing the evidence and preparing the next step…',
    },
    sourceLocal: { id: 'app.logagent.source-local', defaultMessage: 'Local LogAgent' },
    sourceCloud: { id: 'app.logagent.source-cloud', defaultMessage: 'Cloud LogShare' },
    sourceCloudHint: {
        id: 'app.logagent.source-cloud-hint',
        defaultMessage: 'Crash logs are sent to LogShare for cloud analysis.',
    },
    aiPreparing: {
        id: 'app.logagent.preparing',
        defaultMessage: 'Preparing the diagnostic snapshot…',
    },
    aiCancelled: {
        id: 'app.logagent.cancelled',
        defaultMessage:
            'Analysis cancelled in the launcher. A cloud task may continue on the server.',
    },
    aiCancel: { id: 'app.logagent.cancel', defaultMessage: 'Cancel analysis' },
    title: {
        id: 'app.minecraft-crash.title',
        defaultMessage: '{instanceName} crashed',
    },
    body: {
        id: 'app.minecraft-crash.body',
        defaultMessage:
            'Export the error report or share the diagnostic link when asking for help. Do not send only a screenshot of this window.',
    },
    summary: {
        id: 'app.minecraft-crash.summary',
        defaultMessage: 'Minecraft stopped unexpectedly.',
    },
    supportHint: {
        id: 'app.minecraft-crash.support-hint',
        defaultMessage:
            'Share the diagnostic link or exported package instead of only a screenshot.',
    },
    previewInstance: {
        id: 'app.minecraft-crash.preview-instance',
        defaultMessage: 'Minecraft test instance',
    },
    launchFailedTitle: {
        id: 'app.minecraft-crash.launch-failed-title',
        defaultMessage: '{instanceName} could not start',
    },
    launchFailedSummary: {
        id: 'app.minecraft-crash.launch-failed-summary',
        defaultMessage: 'Minecraft failed during launch preparation.',
    },
    exitedBeforeInitialization: {
        id: 'app.minecraft-crash.exited-before-initialization',
        defaultMessage:
            'The Java process exited before it could connect to the launcher. The selected Java version is probably incompatible with this Minecraft or Mod loader version. Select the Java version required by the instance, then try again.',
    },
    initializationTimedOut: {
        id: 'app.minecraft-crash.initialization-timed-out',
        defaultMessage:
            'The Java process started but did not connect to the launcher within 15 seconds. Check the selected Java version and any wrapper command, then try again.',
    },
    preparationTimedOut: {
        id: 'app.minecraft-crash.preparation-timed-out',
        defaultMessage:
            'Launch preparation did not finish within 60 seconds and was cancelled. Check the Java path, launch hooks, wrapper command, and network connection, then try again.',
    },
    launchFailureHint: {
        id: 'app.minecraft-crash.launch-failure-hint',
        defaultMessage:
            'Open the Minecraft logs to view the captured Java output. When asking for help, export and send the complete Minecraft diagnostic package.',
    },
    analyzing: {
        id: 'app.minecraft-crash.analyzing',
        defaultMessage: 'Analyzing the logs from this launch...',
    },
    evidence: {
        id: 'app.minecraft-crash.evidence',
        defaultMessage: 'Reference evidence: {evidence}',
    },
    viewModChanges: {
        id: 'app.minecraft-crash.view-mod-changes',
        defaultMessage: 'View Mod changes',
    },
    modChangesTitle: {
        id: 'app.minecraft-crash.mod-changes-title',
        defaultMessage: 'Possible issue: Mod files changed since the last successful launch',
    },
    modChangesAction: {
        id: 'app.minecraft-crash.mod-changes-action',
        defaultMessage:
            'Review the changed Mod files and restore the previous setup manually if the crash started after those changes.',
    },
    jvmArgumentsTitle: {
        id: 'app.minecraft-crash.diagnosis.jvm-arguments.title',
        defaultMessage: 'Possible issue: the JVM arguments are invalid',
    },
    jvmArgumentsAction: {
        id: 'app.minecraft-crash.diagnosis.jvm-arguments.action',
        defaultMessage:
            'You can try removing the JVM argument shown below from the instance settings, then launch again.',
    },
    javaTooNewTitle: {
        id: 'app.minecraft-crash.diagnosis.java-too-new.title',
        defaultMessage: 'Possible issue: the selected Java version is too new',
    },
    javaTooNewAction: {
        id: 'app.minecraft-crash.diagnosis.java-too-new.action',
        defaultMessage:
            'You can try selecting the Java major version required by this Minecraft and Mod loader version, then launch again.',
    },
    javaIncompatibleTitle: {
        id: 'app.minecraft-crash.diagnosis.java-incompatible.title',
        defaultMessage: 'Possible issue: the Java version is incompatible',
    },
    javaIncompatibleAction: {
        id: 'app.minecraft-crash.diagnosis.java-incompatible.action',
        defaultMessage:
            'You can try selecting the Java version requested in the error below, or using a compatible build of the affected Mod.',
    },
    java32BitTitle: {
        id: 'app.minecraft-crash.diagnosis.java-32bit.title',
        defaultMessage: 'Possible issue: 32-bit Java cannot allocate enough memory',
    },
    java32BitAction: {
        id: 'app.minecraft-crash.diagnosis.java-32bit.action',
        defaultMessage:
            'You can try installing and selecting a 64-bit Java runtime, then launch again.',
    },
    java11RequiredTitle: {
        id: 'app.minecraft-crash.diagnosis.java-11-required.title',
        defaultMessage: 'Possible issue: a Mod requires Java 11',
    },
    java11RequiredAction: {
        id: 'app.minecraft-crash.diagnosis.java-11-required.action',
        defaultMessage:
            'You can try selecting Java 11, or installing a build of the affected Mod that supports the current Java version.',
    },
    openJ9Title: {
        id: 'app.minecraft-crash.diagnosis.openj9.title',
        defaultMessage: 'Possible issue: OpenJ9 is not compatible with this instance',
    },
    openJ9Action: {
        id: 'app.minecraft-crash.diagnosis.openj9.action',
        defaultMessage:
            'You can try selecting a HotSpot-based Java runtime, such as the bundled Minecraft runtime or Eclipse Temurin.',
    },
    jdkRuntimeTitle: {
        id: 'app.minecraft-crash.diagnosis.jdk-runtime.title',
        defaultMessage: 'Possible issue: the selected JDK is not compatible',
    },
    jdkRuntimeAction: {
        id: 'app.minecraft-crash.diagnosis.jdk-runtime.action',
        defaultMessage:
            'You can try selecting a standard HotSpot Java runtime for this Minecraft version.',
    },
    forgeJavaTitle: {
        id: 'app.minecraft-crash.diagnosis.forge-java.title',
        defaultMessage: 'Possible issue: Forge is not compatible with the selected Java version',
    },
    forgeJavaAction: {
        id: 'app.minecraft-crash.diagnosis.forge-java.action',
        defaultMessage:
            'You can try using the Java version expected by this Forge release, or updating Forge.',
    },
    outOfMemoryTitle: {
        id: 'app.minecraft-crash.diagnosis.out-of-memory.title',
        defaultMessage: 'Possible issue: Minecraft ran out of memory',
    },
    outOfMemoryAction: {
        id: 'app.minecraft-crash.diagnosis.out-of-memory.action',
        defaultMessage:
            'You can try increasing the instance memory allocation, or removing memory-heavy Mods and resource packs.',
    },
    diskSpaceTitle: {
        id: 'app.minecraft-crash.diagnosis.disk-space.title',
        defaultMessage: 'Possible issue: the disk ran out of free space',
    },
    diskSpaceAction: {
        id: 'app.minecraft-crash.diagnosis.disk-space.action',
        defaultMessage:
            'Free space on the drive containing this instance, then launch Minecraft again.',
    },
    fileInUseTitle: {
        id: 'app.minecraft-crash.diagnosis.file-in-use.title',
        defaultMessage: 'Possible issue: another process is using a required file',
    },
    fileInUseAction: {
        id: 'app.minecraft-crash.diagnosis.file-in-use.action',
        defaultMessage:
            'Close the program named in the log, including other launchers, backup tools, or antivirus scans, then launch again.',
    },
    knownFailureTitle: {
        id: 'app.minecraft-crash.diagnosis.known-failure.title',
        defaultMessage: 'Possible issue: a specific launch problem was detected',
    },
    knownFailureAction: {
        id: 'app.minecraft-crash.diagnosis.known-failure.action',
        defaultMessage:
            'This is an automatic guess, not a guaranteed diagnosis. Open the log analysis for the full context before applying the suggested fix.',
    },
    shareDiagnostic: {
        id: 'app.minecraft-crash.share-diagnostic',
        defaultMessage: 'Share diagnostic',
    },
    sharingDiagnostic: {
        id: 'app.minecraft-crash.sharing-diagnostic',
        defaultMessage: 'Sharing diagnostic...',
    },
    shareFailed: {
        id: 'app.minecraft-crash.share-failed',
        defaultMessage: 'Failed to share the diagnostic',
    },
    shareTruncated: {
        id: 'app.minecraft-crash.share-truncated',
        defaultMessage: 'The diagnostic log is too large, so only the last 9 MB was uploaded.',
    },
    shareCopied: {
        id: 'app.minecraft-crash.share-copied',
        defaultMessage: 'Diagnostic link copied to your clipboard',
    },
    shareReady: {
        id: 'app.minecraft-crash.share-ready',
        defaultMessage: 'Diagnostic link is ready to share',
    },
    shareFallback: {
        id: 'app.minecraft-crash.share-fallback',
        defaultMessage: 'LogShare is unavailable, so the diagnostic was shared via mclo.gs.',
    },
    copyLink: {
        id: 'app.minecraft-crash.copy-link',
        defaultMessage: 'Copy link',
    },
    noLogContent: {
        id: 'app.minecraft-crash.no-log-content',
        defaultMessage:
            'No log content was found to share or analyze. Make sure the instance has logs generated in the last few minutes.',
    },
    logShareSummaryTitle: {
        id: 'app.log-share.summary.title',
        defaultMessage: 'LogShare analysis',
    },
    logShareSummaryLoading: {
        id: 'app.log-share.summary.loading',
        defaultMessage: 'Loading structured summary...',
    },
    logShareSummaryEmpty: {
        id: 'app.log-share.summary.empty',
        defaultMessage: 'LogShare did not return a structured summary for these logs.',
    },
    logShareSummaryUnavailable: {
        id: 'app.log-share.summary.no-storage',
        defaultMessage: 'Structured summary is unavailable while no-storage mode is enabled.',
    },
    logShareSummaryFailed: {
        id: 'app.log-share.summary.failed',
        defaultMessage: 'Could not load the LogShare summary: {message}',
    },
    logShareProblems: {
        id: 'app.log-share.summary.problems',
        defaultMessage: 'Problems',
    },
    logShareInformation: {
        id: 'app.log-share.summary.information',
        defaultMessage: 'Information',
    },
    logFilesLoading: {
        id: 'app.minecraft-crash.logs.loading',
        defaultMessage: 'Loading crash logs...',
    },
    logFilesEmpty: {
        id: 'app.minecraft-crash.logs.empty',
        defaultMessage: 'No logs from this launch were found.',
    },
    logFilesFailed: {
        id: 'app.minecraft-crash.logs.failed',
        defaultMessage: 'Could not load the crash logs: {message}',
    },
    logFileLoading: {
        id: 'app.minecraft-crash.log-file.loading',
        defaultMessage: 'Loading log file...',
    },
    logFileFailed: {
        id: 'app.minecraft-crash.log-file.failed',
        defaultMessage: 'Could not load this log file: {message}',
    },
    logAgentTab: {
        id: 'app.log-share.ai.tab',
        defaultMessage: 'LogAgent detailed analysis',
    },
    logAgentRootCause: {
        id: 'app.log-share.ai.root-cause',
        defaultMessage: 'Core root cause',
    },
    logAgentConfidence: {
        id: 'app.log-share.ai.confidence',
        defaultMessage: 'Diagnostic confidence',
    },
    logAgentSteps: {
        id: 'app.log-share.ai.steps',
        defaultMessage: 'Recommended troubleshooting steps',
    },
    logAgentEvidence: {
        id: 'app.log-share.ai.evidence',
        defaultMessage: 'Supporting evidence',
    },
    aiThinking: {
        id: 'app.log-share.ai.thinking',
        defaultMessage: 'Thinking…',
    },
    aiQueued: {
        id: 'app.log-share.ai.queued',
        defaultMessage: 'Queued, about {position} ahead',
    },
    aiQueuedFront: {
        id: 'app.log-share.ai.queued-front',
        defaultMessage: 'Queued, waiting for a free slot',
    },
    aiToolResult: {
        id: 'app.log-share.ai.tool-result',
        defaultMessage: 'Tool result received',
    },
    aiReachedLimit: {
        id: 'app.log-share.ai.limit',
        defaultMessage: 'Reached tool loop limit',
    },
    aiWorking: {
        id: 'app.log-share.ai.working',
        defaultMessage: 'Analyzing with LogAgent…',
    },
    aiFailed: {
        id: 'app.log-share.ai.failed',
        defaultMessage: 'LogAgent analysis failed: {message}',
    },
})

const diagnosisMessages = {
    jvm_arguments: [messages.jvmArgumentsTitle, messages.jvmArgumentsAction],
    java_too_new: [messages.javaTooNewTitle, messages.javaTooNewAction],
    java_incompatible: [messages.javaIncompatibleTitle, messages.javaIncompatibleAction],
    java_32bit: [messages.java32BitTitle, messages.java32BitAction],
    java_11_required: [messages.java11RequiredTitle, messages.java11RequiredAction],
    openj9: [messages.openJ9Title, messages.openJ9Action],
    jdk_runtime: [messages.jdkRuntimeTitle, messages.jdkRuntimeAction],
    forge_java_incompatible: [messages.forgeJavaTitle, messages.forgeJavaAction],
    out_of_memory: [messages.outOfMemoryTitle, messages.outOfMemoryAction],
    disk_space: [messages.diskSpaceTitle, messages.diskSpaceAction],
    file_in_use: [messages.fileInUseTitle, messages.fileInUseAction],
} as const

const title = computed(
    () =>
        payload.value.title ||
        formatMessage(messages.title, {
            instanceName: payload.value.instance_name || 'Minecraft',
        }),
)
const summary = computed(() => payload.value.summary || formatMessage(messages.summary))
const body = computed(() => payload.value.body || formatMessage(messages.body))
const hint = computed(() => payload.value.hint || formatMessage(messages.supportHint))
const showSupportHint = computed(
    () => Boolean(payload.value.body) && hint.value !== formatMessage(messages.supportHint),
)
const isLogShareAutoAnalysis = computed(
    () =>
        logShareSettingsLoaded.value &&
        shouldUseLogShareAutoAnalysis({
            ...logShareSettings.value,
            ai_source: selectedSource.value,
        }),
)
const selectedCrashLog = computed(
    () => crashLogFiles.value.find((file) => crashLogKey(file) === selectedLogKey.value) ?? null,
)
const selectedLogContent = computed(() =>
    selectedCrashLog.value
        ? (crashLogContents.value[crashLogKey(selectedCrashLog.value)] ?? '')
        : '',
)
const selectedLogError = computed(() =>
    selectedCrashLog.value
        ? (crashLogContentErrors.value[crashLogKey(selectedCrashLog.value)] ?? '')
        : '',
)
const selectedLogLines = computed(() => {
    if (!selectedLogContent.value) return []
    return selectedLogContent.value.split(/\r?\n/).map((text, originalIndex) => ({
        line: { text, level: null } satisfies LogLine,
        originalIndex,
    }))
})

const logAgentInsight = computed(() =>
    parseLogAgentInsight(
        agent.status.value === 'ready' ? aiOutput.value : visibleDraft(aiOutput.value),
    ),
)
const renderedAiOutput = computed(() => renderHighlightedString(logAgentInsight.value.markdown))
const renderedLogShareSummary = computed(() => renderHighlightedString(logShareSummary.value))

function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error)
}

async function loadCrashLogContent(file: CrashLogFile): Promise<string> {
    const key = crashLogKey(file)
    if (key in crashLogContents.value) return crashLogContents.value[key] ?? ''
    const version = analysisVersion
    if (selectedLogKey.value === key) selectedLogLoading.value = true
    try {
        const output = await get_output_by_filename(
            payload.value.instance_id!,
            file.log_type,
            file.filename,
        )
        if (version !== analysisVersion) return ''
        crashLogContents.value = { ...crashLogContents.value, [key]: output || '' }
        const { [key]: _ignored, ...remainingErrors } = crashLogContentErrors.value
        crashLogContentErrors.value = remainingErrors
        return output || ''
    } catch (error) {
        if (version === analysisVersion) {
            crashLogContentErrors.value = {
                ...crashLogContentErrors.value,
                [key]: errorMessage(error),
            }
        }
        return ''
    } finally {
        if (version === analysisVersion && selectedLogKey.value === key) {
            selectedLogLoading.value = false
        }
    }
}

async function selectCrashLog(file: CrashLogFile): Promise<void> {
    const key = crashLogKey(file)
    selectedLogKey.value = key
    activeTab.value = key
    selectedLogLoading.value = false
    await loadCrashLogContent(file)
}

async function loadCrashLogs(instanceId: string): Promise<void> {
    const version = analysisVersion
    crashLogsLoading.value = true
    crashLogsError.value = ''
    try {
        const logs = (await get_logs(instanceId, true)) as CrashLogFile[]
        if (version !== analysisVersion) return
        crashLogFiles.value = selectCrashLogFiles(logs)
        selectedLogKey.value = preferredCrashLogKey(crashLogFiles.value)
        if (!activeTab.value) activeTab.value = selectedLogKey.value
        const selected = selectedCrashLog.value
        if (selected) await loadCrashLogContent(selected)
    } catch (error) {
        if (version === analysisVersion) crashLogsError.value = errorMessage(error)
    } finally {
        if (version === analysisVersion) crashLogsLoading.value = false
    }
}

async function combinedCrashLogContent(): Promise<string> {
    const files = await Promise.all(
        crashLogFiles.value.map(async (file) => ({
            ...file,
            output: await loadCrashLogContent(file),
        })),
    )
    return combineCrashLogs(files)
}

function applyAnalysis(
    modalPayload: CrashModalPayload,
    analysis: CrashAnalysisResult | null,
): CrashModalPayload {
    const finding = analysis?.findings[0]
    const modChanges = analysis?.mod_changes ?? []
    if (!finding && modChanges.length === 0) return modalPayload

    const diagnosis = finding
        ? diagnosisMessages[finding.id as keyof typeof diagnosisMessages]
        : undefined
    const [titleMessage, actionMessage] = diagnosis ?? [
        messages.knownFailureTitle,
        messages.knownFailureAction,
    ]
    const resolvedTitleMessage = finding ? titleMessage : messages.modChangesTitle
    const resolvedActionMessage = finding ? actionMessage : messages.modChangesAction
    const evidence = finding?.evidence[0]
    return {
        ...modalPayload,
        summary: formatMessage(resolvedTitleMessage),
        body: formatMessage(resolvedActionMessage),
        hint: evidence
            ? formatMessage(messages.evidence, {
                  evidence: `${evidence.filename}:${evidence.line} - ${evidence.text}`,
              })
            : modalPayload.hint,
    }
}

function show(modalPayload: CrashModalPayload, isPreview = false): boolean {
    if (!isPreview) {
        const now = Date.now()
        const lastShown = lastShownAt.get(modalPayload.instance_id) ?? 0
        if (now - lastShown < 5000) return false
        if (
            !visibleSession.open(modalPayload.instance_id, activeRuns.get(modalPayload.instance_id))
        )
            return false
        lastShownAt.set(modalPayload.instance_id, now)
    }
    analysisVersion += 1
    summaryGeneration += 1
    resumeAfterProviderSettings = false
    agent.reset()
    choosingSource.value = false
    selectedSource.value = defaultSource.value
    payload.value = modalPayload
    lastAnalysis = null
    uploadTicket.value = null
    uploadTicketPromise = null
    shareUrl.value = ''
    sharing.value = false
    logShareSummary.value = ''
    logShareSummaryLoading.value = false
    logShareSummaryState.value = 'idle'
    logShareSummaryError.value = ''
    modChangesAvailable.value = false
    crashLogFiles.value = []
    crashLogsLoading.value = false
    crashLogsError.value = ''
    crashLogContents.value = {}
    crashLogContentErrors.value = {}
    selectedLogKey.value = ''
    selectedLogLoading.value = false
    activeTab.value = ''
    aiTabVisible.value = false
    modal.value?.show()
    crashLogsPromise = isPreview ? null : loadCrashLogs(modalPayload.instance_id)
    return true
}

function openModChanges(): void {
    if (lastAnalysis?.mod_changes.length) modChangesModal.value?.show(lastAnalysis)
}

function launchErrorText(error: unknown): string {
    if (typeof error === 'string') return error
    if (error && typeof error === 'object') {
        const record = error as Record<string, unknown>
        const values = [record.message, record.error, record.cause]
            .filter((value): value is string => typeof value === 'string')
            .join('\n')
        if (values) return values
        try {
            return JSON.stringify(error)
        } catch {
            return ''
        }
    }
    return String(error)
}

function launchFailureBody(error: unknown): string | null {
    const errorText = launchErrorText(error)
    if (errorText.includes('Minecraft exited before launcher initialization completed')) {
        return formatMessage(messages.exitedBeforeInitialization)
    }
    if (errorText.includes('Minecraft launcher initialization did not respond')) {
        return formatMessage(messages.initializationTimedOut)
    }
    if (errorText.includes('Minecraft launch preparation timed out')) {
        return formatMessage(messages.preparationTimedOut)
    }
    return null
}

function isLaunchFailure(error: unknown): boolean {
    return launchFailureBody(error) !== null
}

function useLogShareAi(): boolean {
    return selectedSource.value === 'logshare'
}

function formatInsights(value: unknown): string {
    const record = (value ?? {}) as Record<string, unknown>
    const lines: string[] = []
    const name = typeof record.name === 'string' ? record.name : ''
    const version = typeof record.version === 'string' ? record.version : ''
    if (name) lines.push(`### ${name}${version ? ` ${version}` : ''}`)
    const type = typeof record.type === 'string' ? record.type : ''
    if (type) lines.push(`**${type}**`)
    const analysis = (record.analysis ?? {}) as Record<string, unknown>
    const problems = Array.isArray(analysis.problems) ? analysis.problems : []
    const information = Array.isArray(analysis.information) ? analysis.information : []
    if (problems.length) {
        lines.push(`**${formatMessage(messages.logShareProblems)}**`)
        for (const problem of problems as Record<string, unknown>[]) {
            const message =
                typeof problem.message === 'string'
                    ? problem.message
                    : JSON.stringify(problem.message)
            lines.push(`- ${message}`)
            const solutions = Array.isArray(problem.solutions) ? problem.solutions : []
            for (const solution of solutions as string[]) {
                lines.push(`  - ${solution}`)
            }
        }
    }
    if (information.length) {
        lines.push(`**${formatMessage(messages.logShareInformation)}**`)
        for (const item of information as Record<string, unknown>[]) {
            if (typeof item.label === 'string' && typeof item.value === 'string') {
                lines.push(`- ${item.label}: ${item.value}`)
            }
        }
    }
    return lines.join('\n').trim()
}

async function loadLogShareSummary(instanceId: string): Promise<void> {
    if (!isLogShareAutoAnalysis.value || logShareSummaryLoading.value) return
    if (logShareSettings.value.no_storage) {
        logShareSummaryState.value = 'unavailable'
        if (payload.value.hint === formatMessage(messages.analyzing)) payload.value.hint = ''
        return
    }
    const version = analysisVersion
    const generation = summaryGeneration
    const stale = () =>
        version !== analysisVersion ||
        generation !== summaryGeneration ||
        instanceId !== payload.value.instance_id ||
        !useLogShareAi()
    logShareSummaryLoading.value = true
    logShareSummaryState.value = 'loading'
    logShareSummaryError.value = ''
    logShareSummary.value = ''
    try {
        const ticket = await uploadTicketForInstance(instanceId)
        if (!ticket) throw new Error('Could not upload the crash logs to LogShare')
        if (stale()) return
        const insights = await logshare_get_insights(ticket.id)
        if (stale()) return
        logShareSummary.value = formatInsights(insights)
        logShareSummaryState.value = logShareSummary.value ? 'ready' : 'empty'
    } catch (error) {
        console.error('Failed to gather LogShare summary', error)
        if (!stale()) {
            logShareSummary.value = ''
            logShareSummaryError.value = errorMessage(error)
            logShareSummaryState.value = 'error'
        }
    } finally {
        if (!stale()) {
            logShareSummaryLoading.value = false
            if (payload.value.hint === formatMessage(messages.analyzing)) payload.value.hint = ''
        }
    }
}

async function analyzeAndUpdate(
    modalPayload: CrashModalPayload,
    fallbackHint?: string,
): Promise<CrashAnalysisResult | null> {
    const version = analysisVersion
    const analysis = await refreshCrashAnalysis(modalPayload.instance_id).catch((error) => {
        console.error('Failed to analyze Minecraft crash', error)
        return null
    })
    if (!mounted || version !== analysisVersion) return null
    lastAnalysis = analysis
    modChangesAvailable.value = !!analysis?.mod_changes.length
    if (mounted && version === analysisVersion) {
        const updatedPayload = applyAnalysis(modalPayload, analysis)
        if (!analysis?.findings.length && fallbackHint) updatedPayload.hint = fallbackHint
        if (updatedPayload.hint === formatMessage(messages.analyzing)) updatedPayload.hint = ''
        payload.value = updatedPayload
    }
    void loadLogShareSummary(modalPayload.instance_id)
    return analysis
}

async function handleLaunchError(
    error: unknown,
    launchPayload: MinecraftLaunchErrorPayload,
): Promise<boolean> {
    const failureBody = launchFailureBody(error)
    if (!failureBody) return false

    const instanceName = launchPayload.instance_name || 'Minecraft'
    const modalPayload: CrashModalPayload = {
        ...launchPayload,
        title: formatMessage(messages.launchFailedTitle, { instanceName }),
        summary: formatMessage(messages.launchFailedSummary),
        body: failureBody,
        hint: formatMessage(messages.analyzing),
    }
    await refreshAIAvailability()
    if (!show(modalPayload)) return true
    if (isLogShareAutoAnalysis.value) {
        void loadLogShareSummary(modalPayload.instance_id)
    } else {
        await analyzeAndUpdate(modalPayload, formatMessage(messages.launchFailureHint))
    }
    return true
}

async function handleWarning(warning: CrashWarningPayload): Promise<void> {
    const modalPayload = { ...warning, hint: formatMessage(messages.analyzing) }
    await refreshAIAvailability()
    if (!show(modalPayload)) return
    if (isLogShareAutoAnalysis.value) {
        void loadLogShareSummary(modalPayload.instance_id)
    } else {
        await analyzeAndUpdate(modalPayload)
    }
}

function showPreview(): void {
    show(
        {
            instance_id: 'preview',
            instance_name: formatMessage(messages.previewInstance),
        },
        true,
    )
}

function notifyNoLogContent(): void {
    addNotification({
        title: formatMessage(messages.noLogContent),
        type: 'warning',
    })
}

async function uploadTicketForInstance(instanceId: string): Promise<LogShareTicket | null> {
    if (uploadTicket.value) return uploadTicket.value
    if (uploadTicketPromise) return uploadTicketPromise

    const version = analysisVersion
    const request = logshare_upload_crash(instanceId)
        .then((ticket) => {
            if (version === analysisVersion) {
                if (uploadTicket.value?.id !== ticket.id)
                    void recordShared({ ...ticket, provider: 'logshare', truncated: false })
                uploadTicket.value = ticket
            }
            return ticket
        })
        .catch((error) => {
            console.error('Failed to upload crash diagnostic to LogShare', error)
            return null
        })
    uploadTicketPromise = request
    try {
        return await request
    } finally {
        if (uploadTicketPromise === request) uploadTicketPromise = null
    }
}

async function recordShared(
    entry: Pick<SharedLogInput, 'id' | 'url' | 'raw' | 'token' | 'provider' | 'truncated'>,
): Promise<void> {
    try {
        await record_shared_log({
            id: entry.id,
            url: entry.url,
            raw: entry.raw,
            token: entry.token,
            provider: entry.provider,
            instance_id: payload.value.instance_id ?? null,
            instance_name: payload.value.instance_name ?? null,
            truncated: entry.truncated,
            created_at: Math.floor(Date.now() / 1000),
        })
    } catch (error) {
        console.error('Failed to record shared diagnostic', error)
    }
}

interface SharedLogInput {
    id: string
    url: string
    raw: string
    token: string
    provider: string
    truncated: boolean
}

async function copyToClipboard(url: string): Promise<void> {
    try {
        await navigator.clipboard.writeText(url)
        addNotification({
            title: formatMessage(messages.shareCopied),
            type: 'success',
        })
    } catch (error) {
        console.error('Failed to copy shared diagnostic URL', error)
        addNotification({
            title: formatMessage(messages.shareReady),
            type: 'success',
        })
    }
}

async function shareDiagnostic(): Promise<void> {
    if (sharing.value) return
    const version = analysisVersion
    const stale = () => version !== analysisVersion
    sharing.value = true
    shareUrl.value = ''
    const instanceId = payload.value.instance_id!
    try {
        if (logShareSettings.value.share_provider === 'logshare') {
            const ticket = await uploadTicketForInstance(instanceId)
            if (stale()) return
            if (ticket?.url) {
                shareUrl.value = ticket.url
                await recordShared({
                    id: ticket.id,
                    url: ticket.url,
                    raw: ticket.raw,
                    token: ticket.token,
                    provider: 'logshare',
                    truncated: false,
                })
                await copyToClipboard(ticket.url)
                return
            }
            addNotification({
                title: formatMessage(messages.shareFallback),
                type: 'warning',
            })
        }

        await crashLogsPromise
        if (stale()) return
        const shareContent = lastAnalysis?.combined_log || (await combinedCrashLogContent())
        if (!shareContent) {
            notifyNoLogContent()
            return
        }
        const result = await shareLogs(client, shareContent)
        if (stale()) return
        if (result.truncated) {
            addNotification({
                title: formatMessage(messages.shareTruncated),
                type: 'warning',
            })
        }
        if (!stale()) shareUrl.value = result.url
        try {
            await navigator.clipboard.writeText(result.url)
            addNotification({
                title: formatMessage(messages.shareCopied),
                type: 'success',
            })
        } catch (error) {
            console.error('Failed to copy shared diagnostic URL', error)
            addNotification({
                title: formatMessage(messages.shareReady),
                type: 'success',
            })
        }
        await recordShared({
            id: result.url,
            url: result.url,
            raw: '',
            token: '',
            provider: 'mclogs',
            truncated: result.truncated,
        })
    } catch (error) {
        console.error('Failed to share crash diagnostic', error)
        addNotification({
            title: formatMessage(messages.shareFailed),
            type: 'error',
        })
    } finally {
        if (!stale()) sharing.value = false
    }
}

async function copyShareUrl(): Promise<void> {
    if (!shareUrl.value) return
    try {
        await navigator.clipboard.writeText(shareUrl.value)
        addNotification({
            title: formatMessage(messages.shareCopied),
            type: 'success',
        })
    } catch (error) {
        console.error('Failed to copy share URL', error)
    }
}

function selectAnalysisSource(value: unknown): void {
    if (aiLoading.value) return
    const next = value === 'custom' ? 'custom' : 'logshare'
    choosingSource.value = false
    if (next === selectedSource.value) return
    selectedSource.value = next
    agent.reset()
    summaryGeneration += 1
    logShareSummaryLoading.value = false
    logShareSummaryState.value = 'idle'
    logShareSummary.value = ''
    logShareSummaryError.value = ''
    if (next === 'logshare' && payload.value.instance_id)
        void loadLogShareSummary(payload.value.instance_id)
}

function openAnalysisSettings(): void {
    localSetupModal.value?.show()
}

async function openProviderSettings(): Promise<void> {
    resumeAfterProviderSettings = true
    localSetupModal.value?.hide()
    modal.value?.hide()
    await router.push('/settings#ai')
}

watch(
    () => route.fullPath,
    async (path, previous) => {
        if (
            !resumeAfterProviderSettings ||
            !previous.startsWith('/settings') ||
            path.startsWith('/settings#ai')
        )
            return
        await refreshAIAvailability()
        if (!mounted) return
        resumeAfterProviderSettings = false
        modal.value?.show()
    },
)

async function openAIAnalysis(): Promise<void> {
    if (aiLoading.value || !aiAvailable.value || !payload.value.instance_id) return
    aiTabVisible.value = true
    activeTab.value = AI_TAB
    const version = analysisVersion
    const source = selectedSource.value
    const instanceId = payload.value.instance_id
    const ticket = source === 'logshare' ? (uploadTicketPromise ?? uploadTicket.value) : null
    if (version !== analysisVersion || source !== selectedSource.value) return
    await agent.start(instanceId, source, ticket, (ticket) => {
        if (version === analysisVersion) {
            if (uploadTicket.value?.id !== ticket.id)
                void recordShared({ ...ticket, provider: 'logshare', truncated: false })
            uploadTicket.value = ticket
        }
    })
}

async function refreshAIAvailability(): Promise<void> {
    try {
        const [settings, aiSettings] = await Promise.all([
            get_log_share_settings(),
            get_crash_analysis_ai_settings(),
        ])
        logShareSettings.value = settings
        logShareSettingsLoaded.value = true
        defaultSource.value = aiSettings.ai_source
        const state = await getAIState().catch(() => null)
        const provider = state?.providers.find(
            (item) => item.provider_id === aiSettings.provider_id,
        )
        configuredProvider.value = provider?.custom_name || aiSettings.provider_id
        configuredModel.value = [
            configuredProvider.value,
            provider?.models.find((model) => model.id === aiSettings.model_id)?.name ||
                aiSettings.model_id,
        ]
            .filter(Boolean)
            .join(' / ')
        localAvailable.value =
            aiSettings.enabled &&
            !!state?.settings.enabled &&
            !!provider?.enabled &&
            provider.models.some((model) => model.id === aiSettings.model_id && model.enabled)
    } catch {
        logShareSettingsLoaded.value = false
        localAvailable.value = false
    }
}

async function handleProcessEvent(event: ProcessEvent): Promise<void> {
    if (event.event === 'launched') {
        visibleSession.launched(event.instance_id, event.uuid)
        activeRuns.set(event.instance_id, event.uuid)
        clearCrashAnalysis(event.instance_id)
        lastAnalysis = null
        modChangesAvailable.value = false
        return
    }
    if (event.event !== 'finished' || activeRuns.get(event.instance_id) !== event.uuid) return
    if (!shouldShowMinecraftCrash(event.crashed)) {
        activeRuns.delete(event.instance_id)
        return
    }

    await new Promise((resolve) => setTimeout(resolve, 2000))
    if (!mounted || activeRuns.get(event.instance_id) !== event.uuid) return

    try {
        await refreshAIAvailability()
        if (!mounted || activeRuns.get(event.instance_id) !== event.uuid) return
        const instance = await getInstance(event.instance_id).catch(() => null)
        if (!mounted || activeRuns.get(event.instance_id) !== event.uuid) return

        if (
            shouldUseLogShareAutoAnalysis({
                ...logShareSettings.value,
                ai_source: defaultSource.value,
            })
        ) {
            const shown = show({
                instance_id: event.instance_id,
                instance_name: instance?.name || 'Minecraft',
            })
            if (shown) void loadLogShareSummary(event.instance_id)
            return
        }

        const analysis = await refreshCrashAnalysis(event.instance_id).catch((error) => {
            console.error('Failed to analyze finished Minecraft process', error)
            return null
        })
        if (!mounted || activeRuns.get(event.instance_id) !== event.uuid) return

        const shown = show(
            applyAnalysis(
                {
                    instance_id: event.instance_id,
                    instance_name: instance?.name || 'Minecraft',
                },
                analysis,
            ),
        )
        if (!shown) return
        lastAnalysis = analysis
        modChangesAvailable.value = !!analysis?.mod_changes.length
        void loadLogShareSummary(event.instance_id)
    } finally {
        if (activeRuns.get(event.instance_id) === event.uuid) activeRuns.delete(event.instance_id)
    }
}

onMounted(async () => {
    mounted = true
    void refreshAIAvailability()
    const unlisten = await process_listener((event: ProcessEvent) => void handleProcessEvent(event))
    if (!mounted) {
        unlisten()
        return
    }
    unlistenProcess = unlisten
})

onUnmounted(() => {
    mounted = false
    analysisVersion += 1
    activeRuns.clear()
    unlistenProcess?.()
    agent.cancel()
})

defineExpose({
    handleLaunchError,
    handleWarning,
    isLaunchFailure,
    showPreview,
    openAIAnalysis,
})
</script>

<template>
    <NewModal
        ref="modal"
        :on-hide="
            () => {
                if (resumeAfterProviderSettings) return
                analysisVersion += 1
                summaryGeneration += 1
                visibleSession.close()
                agent.cancel()
            }
        "
        fade="danger"
        hide-header
        merge-header
        :closable="false"
        no-padding
        width="80vw"
        max-width="80vw"
    >
        <div class="crash-modal-shell">
            <section class="crash-modal-sidebar flex min-h-0 flex-col gap-4 overflow-y-auto p-6">
                <div class="flex items-start justify-between gap-3">
                    <div class="flex min-w-0 flex-col gap-2">
                        <h2 class="m-0 text-xl font-semibold text-[var(--color-text-primary)]">
                            {{ title }}
                        </h2>
                        <p class="m-0 font-semibold text-red">{{ summary }}</p>
                        <p class="m-0 text-sm text-[var(--color-text-tertiary)]">{{ body }}</p>
                        <p class="m-0 text-sm text-[var(--color-text-tertiary)]">{{ hint }}</p>
                        <p
                            v-if="showSupportHint"
                            class="m-0 text-sm text-[var(--color-text-tertiary)]"
                        >
                            {{ formatMessage(messages.supportHint) }}
                        </p>
                    </div>
                    <!--
						The modal has no header of its own, so the close control lives with the
						title, in the flow: positioned against the modal body it would be laid
						out before that body has its measured width and start off at the corner.
					-->
                    <Button
                        v-tooltip="formatMessage(commonMessages.closeButton)"
                        circular
                        icon-only
                        :label="formatMessage(commonMessages.closeButton)"
                        @click="modal?.hide()"
                        ><XIcon aria-hidden="true"
                    /></Button>
                </div>

                <div
                    v-if="isLogShareAutoAnalysis"
                    class="flex min-h-32 flex-col gap-2 rounded-lg bg-surface-2 p-3"
                >
                    <span class="text-sm font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.logShareSummaryTitle) }}
                    </span>
                    <p
                        v-if="logShareSummaryState === 'loading'"
                        class="m-0 text-sm text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.logShareSummaryLoading) }}
                    </p>
                    <div
                        v-else-if="logShareSummaryState === 'ready'"
                        class="markdown-body text-sm"
                        v-html="renderedLogShareSummary"
                    />
                    <p
                        v-else-if="logShareSummaryState === 'empty'"
                        class="m-0 text-sm text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.logShareSummaryEmpty) }}
                    </p>
                    <p
                        v-else-if="logShareSummaryState === 'unavailable'"
                        class="m-0 text-sm text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.logShareSummaryUnavailable) }}
                    </p>
                    <p v-else-if="logShareSummaryState === 'error'" class="m-0 text-sm text-red">
                        {{
                            formatMessage(messages.logShareSummaryFailed, {
                                message: logShareSummaryError,
                            })
                        }}
                    </p>
                </div>

                <div
                    class="mt-auto flex flex-col gap-3 border-0 border-t border-solid border-surface-4 pt-4"
                    data-testid="crash-analysis-controls"
                >
                    <div class="flex items-center justify-between gap-2">
                        <h3 class="m-0 text-sm font-semibold">
                            {{ formatMessage(messages.analysisSection) }}
                        </h3>
                        <Button
                            v-if="!aiLoading"
                            type="quiet"
                            @click="choosingSource = !choosingSource"
                            >{{
                                formatMessage(
                                    choosingSource ? messages.sourceDone : messages.changeSource,
                                )
                            }}</Button
                        >
                    </div>
                    <LogAgentSourcePicker
                        v-if="choosingSource"
                        :model-value="selectedSource"
                        :local-model="configuredModel"
                        :disabled="aiLoading"
                        @update:model-value="selectAnalysisSource"
                    />
                    <div v-else class="flex min-w-0 flex-col gap-1">
                        <span class="text-sm font-medium">{{
                            formatMessage(
                                selectedSource === 'custom'
                                    ? messages.sourceLocal
                                    : messages.sourceCloud,
                            )
                        }}</span>
                        <span
                            v-if="selectedSource === 'custom' && configuredModel"
                            class="break-words text-xs text-[var(--color-text-tertiary)]"
                            >{{ configuredModel }}</span
                        >
                    </div>
                    <p class="m-0 text-xs leading-relaxed text-[var(--color-text-tertiary)]">
                        {{
                            selectedSource === 'custom'
                                ? configuredProvider
                                    ? formatMessage(messages.localSending, {
                                          provider: configuredProvider,
                                      })
                                    : formatMessage(messages.modelMissing)
                                : formatMessage(messages.sourceCloudHint)
                        }}
                    </p>
                    <Button v-if="aiLoading" type="outlined" @click="agent.cancel">{{
                        formatMessage(messages.aiCancel)
                    }}</Button>
                    <Button
                        v-else-if="aiAvailable"
                        type="colored"
                        color="brand"
                        @click="openAIAnalysis"
                    >
                        <ScanEyeIcon aria-hidden="true" />{{
                            formatMessage(
                                agent.status.value === 'idle'
                                    ? messages.startAnalysis
                                    : messages.retryAnalysis,
                            )
                        }}
                    </Button>
                    <Button v-else type="colored" color="brand" @click="openAnalysisSettings">{{
                        formatMessage(messages.configureModel)
                    }}</Button>
                </div>
                <div
                    class="flex flex-col gap-2 border-0 border-t border-solid border-surface-4 pt-3"
                >
                    <div class="flex flex-wrap gap-2">
                        <Button type="quiet" :disabled="sharing" @click="shareDiagnostic"
                            ><ShareIcon aria-hidden="true" />{{
                                formatMessage(
                                    sharing ? messages.sharingDiagnostic : messages.shareDiagnostic,
                                )
                            }}</Button
                        >
                        <Button v-if="modChangesAvailable" type="quiet" @click="openModChanges">{{
                            formatMessage(messages.viewModChanges)
                        }}</Button>
                    </div>
                    <div
                        v-if="shareUrl"
                        class="flex items-center gap-2 rounded-lg bg-surface-2 p-3"
                    >
                        <ExternalIcon
                            class="size-4 shrink-0 text-[var(--color-text-tertiary)]"
                            aria-hidden="true"
                        />
                        <a
                            :href="shareUrl"
                            target="_blank"
                            rel="noopener noreferrer"
                            class="min-w-0 flex-1 truncate text-sm text-[var(--color-text-default)] underline"
                        >
                            {{ shareUrl }}
                        </a>
                        <Button
                            type="outlined"
                            circular
                            icon-only
                            :label="formatMessage(messages.copyLink)"
                            @click="copyShareUrl"
                            ><ClipboardCopyIcon aria-hidden="true" />
                        </Button>
                    </div>
                </div>
            </section>

            <section class="crash-modal-workspace flex min-h-0 min-w-0 flex-col bg-surface-2">
                <div
                    class="flex min-h-14 shrink-0 items-end gap-1 overflow-x-auto border-0 border-b border-solid border-surface-5 px-5 pt-3 pr-16"
                >
                    <button
                        v-for="file in crashLogFiles"
                        :key="crashLogKey(file)"
                        class="crash-modal-tab"
                        :class="{ 'crash-modal-tab-active': activeTab === crashLogKey(file) }"
                        @click="selectCrashLog(file)"
                    >
                        {{ crashLogLabel(file) }}
                    </button>
                    <button
                        v-if="aiTabVisible"
                        class="crash-modal-tab"
                        :class="{ 'crash-modal-tab-active': activeTab === AI_TAB }"
                        @click="activeTab = AI_TAB"
                    >
                        {{ formatMessage(messages.logAgentTab) }}
                    </button>
                </div>

                <div
                    v-if="activeTab === AI_TAB"
                    class="crash-modal-ai-output min-h-0 flex-1 overflow-y-auto p-5"
                >
                    <p
                        v-if="agent.model.value"
                        class="m-0 mb-2 text-xs text-[var(--color-text-tertiary)]"
                    >
                        {{ agent.model.value }}
                    </p>
                    <p v-if="aiStatus" class="m-0 mb-3 text-sm text-[var(--color-text-tertiary)]">
                        {{ aiStatus }}
                    </p>
                    <LogAgentActivity
                        v-if="logShareSettings.show_progress"
                        :entries="agent.activity.value"
                        :busy="aiLoading"
                        :elapsed="agent.elapsed.value"
                    />
                    <Card v-if="logAgentInsight.insight" class="flex flex-col gap-4 text-sm">
                        <section class="flex flex-col gap-2">
                            <div
                                class="flex items-center justify-between gap-2 text-xs font-semibold uppercase"
                            >
                                <span
                                    class="flex items-center gap-1.5 text-[var(--color-text-tertiary)]"
                                >
                                    <ScanEyeIcon class="size-3.5" aria-hidden="true" />
                                    {{ formatMessage(messages.logAgentRootCause) }}
                                </span>
                                <div
                                    v-if="logAgentInsight.insight.confidence !== null"
                                    class="flex shrink-0 items-center gap-2 font-mono text-xs text-[var(--color-text-tertiary)]"
                                >
                                    <span>{{ formatMessage(messages.logAgentConfidence) }}</span>
                                    <span class="font-semibold text-[var(--color-text-primary)]">
                                        {{
                                            Math.round(
                                                Math.max(
                                                    0,
                                                    Math.min(1, logAgentInsight.insight.confidence),
                                                ) * 100,
                                            )
                                        }}%
                                    </span>
                                    <span class="w-16 shrink-0">
                                        <ProgressBar
                                            full-width
                                            :progress="
                                                Math.max(
                                                    0,
                                                    Math.min(1, logAgentInsight.insight.confidence),
                                                )
                                            "
                                            :gradient-border="false"
                                        />
                                    </span>
                                </div>
                            </div>
                            <p
                                class="m-0 text-base font-medium leading-snug text-[var(--color-text-primary)]"
                            >
                                {{ logAgentInsight.insight.rootCause }}
                            </p>
                        </section>

                        <section
                            v-if="logAgentInsight.insight.steps.length"
                            class="flex flex-col gap-2 rounded-xl border border-solid border-surface-4 bg-surface-2 p-3"
                        >
                            <h3
                                class="m-0 flex items-center gap-1.5 text-xs font-semibold text-[var(--color-text-tertiary)]"
                            >
                                <ListOrderedIcon class="size-3.5" aria-hidden="true" />
                                {{ formatMessage(messages.logAgentSteps) }}
                            </h3>
                            <ol
                                class="m-0 list-decimal space-y-1.5 pl-5 text-[var(--color-text-tertiary)]"
                            >
                                <li
                                    v-for="step in logAgentInsight.insight.steps"
                                    :key="step"
                                    class="leading-relaxed"
                                >
                                    {{ step }}
                                </li>
                            </ol>
                        </section>

                        <section
                            v-if="logAgentInsight.insight.evidence.length"
                            class="flex flex-col gap-2 rounded-xl border border-solid border-surface-4 bg-surface-2 p-3"
                        >
                            <h3
                                class="m-0 flex items-center gap-1.5 text-xs font-semibold text-[var(--color-text-tertiary)]"
                            >
                                <LinkIcon class="size-3.5" aria-hidden="true" />
                                {{ formatMessage(messages.logAgentEvidence) }}
                            </h3>
                            <div class="flex flex-wrap gap-1.5">
                                <span
                                    v-for="evidence in logAgentInsight.insight.evidence"
                                    :key="evidence"
                                    class="break-all rounded border border-surface-5 bg-surface-3 px-2 py-1 font-mono text-xs text-[var(--color-text-tertiary)]"
                                >
                                    {{ evidence }}
                                </span>
                            </div>
                        </section>
                    </Card>
                    <div
                        v-if="logAgentInsight.markdown"
                        class="markdown-body text-sm"
                        v-html="renderedAiOutput"
                    />
                </div>
                <div
                    v-else-if="crashLogsLoading"
                    class="flex min-h-0 flex-1 items-start p-5 text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.logFilesLoading) }}
                </div>
                <div
                    v-else-if="crashLogsError"
                    class="flex min-h-0 flex-1 items-start p-5 text-sm text-red"
                >
                    {{ formatMessage(messages.logFilesFailed, { message: crashLogsError }) }}
                </div>
                <div
                    v-else-if="crashLogFiles.length === 0"
                    class="flex min-h-0 flex-1 items-start p-5 text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.logFilesEmpty) }}
                </div>
                <div
                    v-else-if="selectedLogLoading"
                    class="flex min-h-0 flex-1 items-start p-5 text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.logFileLoading) }}
                </div>
                <div
                    v-else-if="selectedLogError"
                    class="flex min-h-0 flex-1 items-start p-5 text-sm text-red"
                >
                    {{ formatMessage(messages.logFileFailed, { message: selectedLogError }) }}
                </div>
                <div
                    v-else-if="!selectedLogContent"
                    class="flex min-h-0 flex-1 items-start p-5 text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.logFilesEmpty) }}
                </div>
                <LogViewport v-else class="min-h-0 flex-1 p-5" :lines="selectedLogLines" />
            </section>
        </div>
    </NewModal>
    <NewModal
        ref="localSetupModal"
        :header="formatMessage(messages.configureModel)"
        :disable-close="localSetupSaving"
        :on-hide="
            () => {
                void refreshAIAvailability()
            }
        "
        max-width="min(42rem, 90vw)"
    >
        <LogShareSettings
            local-setup
            @open-providers="openProviderSettings"
            @saving-change="localSetupSaving = $event"
        />
        <template #actions
            ><Button
                type="outlined"
                :disabled="localSetupSaving"
                @click="localSetupModal?.hide()"
                >{{ formatMessage(messages.returnToAnalysis) }}</Button
            ></template
        >
    </NewModal>
    <CrashModChangesModal ref="modChangesModal" />
</template>

<style scoped>
.crash-modal-shell {
    display: grid;
    grid-template-columns: minmax(270px, 330px) minmax(0, 1fr);
    height: 80vh;
    min-height: min(520px, 80vh);
}

.crash-modal-sidebar {
    border-right: 1px solid var(--surface-5);
}

.crash-modal-ai-output,
.crash-modal-ai-output * {
    -webkit-user-select: text;
    -moz-user-select: text;
    -ms-user-select: text;
    user-select: text;
}

.crash-modal-tab {
    flex: 0 0 auto;
    min-height: 36px;
    padding: 0 0.75rem;
    border: 0;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--color-text-tertiary);
    font: inherit;
    cursor: pointer;
}

.crash-modal-tab:hover {
    color: var(--color-text-primary);
}

.crash-modal-tab:focus-visible {
    outline: 2px solid var(--color-brand);
    outline-offset: -2px;
}

.crash-modal-tab-active {
    border-bottom-color: var(--color-brand);
    color: var(--color-text-primary);
}

@media screen and (max-width: 760px) {
    .crash-modal-shell {
        display: flex;
        flex-direction: column;
        min-height: 0;
        overflow-y: auto;
    }

    .crash-modal-sidebar {
        flex: 0 0 auto;
        max-height: none;
        overflow: visible;
        border-right: 0;
        border-bottom: 1px solid var(--surface-5);
    }

    .crash-modal-workspace {
        min-height: 360px;
        flex: 1 0 360px;
    }
}
</style>
