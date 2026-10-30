<script setup lang="ts">
import {
    Admonition,
    Avatar,
    Button,
    Combobox,
    defineMessages,
    NewModal,
    useVIntl,
} from '@modrinth/ui'
import { computed, nextTick, onBeforeUnmount, ref, shallowRef, triggerRef } from 'vue'

import type { InstallJobSnapshot } from '@/helpers/install'
import { refresh_content } from '@/helpers/instance'
import { resourceAnalysisRequest } from '@/helpers/resource-analysis-request'
import {
    resourceFailureFeedback,
    resourcePlanIssueFeedback,
} from '@/helpers/resource-operation-feedback'
import { resourceOperationError, resourceSourceName } from '@/helpers/resource-operation-logic'
import {
    type ResourceDependencyNode,
    resourceGet,
    type ResourceOperationIntent,
    type ResourceOperationPhase,
    type ResourceOperationPlan,
    type ResourceOperationProgress,
    type ResourcePlanDecision,
    resourcePrepareEntryOperation,
    resourcePrepareOperation,
    resourceProjectDocuments,
    resourceQueuePlan,
    resourceRevisePlan,
    type ResourceSelection,
    type SourceReleaseRef,
} from '@/helpers/resources'
import { get_instance_worlds } from '@/helpers/worlds'
import type { ContentAnalysisReview } from '@/providers/content-analysis'

const { formatMessage } = useVIntl()
const messages = defineMessages({
    invalidPath: {
        id: 'app.resources.operation.invalid-path',
        defaultMessage: 'The file path is invalid or cannot be used for this installation.',
    },
    identityUnconfirmed: {
        id: 'app.resources.operation.identity-unconfirmed',
        defaultMessage: 'The selected resource could not be identified reliably.',
    },
    world: { id: 'app.resources.operation.world', defaultMessage: 'Target world for data packs' },
    noWorlds: {
        id: 'app.resources.operation.no-worlds',
        defaultMessage:
            'Create a singleplayer world in this instance before installing data packs.',
    },
    header: { id: 'app.resources.operation.header', defaultMessage: 'Review content changes' },
    loading: {
        id: 'app.resources.operation.loading',
        defaultMessage: 'Resolving releases and dependencies…',
    },
    loadingCollectingConstraints: {
        id: 'app.resources.operation.loading-collecting-constraints',
        defaultMessage: 'Collecting the version requirements of installed content…',
    },
    loadingIdentifyingInstalled: {
        id: 'app.resources.operation.loading-identifying-installed',
        defaultMessage: 'Identifying installed content…',
    },
    loadingPrefetching: {
        id: 'app.resources.operation.loading-prefetching',
        defaultMessage: 'Fetching project details…',
    },
    loadingRefreshingContent: {
        id: 'app.resources.operation.loading-refreshing-content',
        defaultMessage: 'Checking installed files…',
    },
    loadingReresolving: {
        id: 'app.resources.operation.loading-reresolving',
        defaultMessage: 'Resolving dependencies again to settle version conflicts…',
    },
    loadingValidating: {
        id: 'app.resources.operation.loading-validating',
        defaultMessage: 'Checking that the files can be written…',
    },
    artifact: {
        id: 'app.resources.operation.artifact',
        defaultMessage: '{source} · release {release} · {file}',
    },
    required: { id: 'app.resources.operation.required', defaultMessage: 'Required dependencies' },
    optional: { id: 'app.resources.operation.optional', defaultMessage: 'Available dependencies' },
    satisfied: { id: 'app.resources.operation.satisfied', defaultMessage: 'Already satisfied' },
    disabledDependency: {
        id: 'app.resources.operation.disabled-dependency',
        defaultMessage:
            'This installed dependency is disabled. Enable it or review its version before continuing.',
    },
    installedDependencyUnavailable: {
        id: 'app.resources.operation.installed-dependency-unavailable',
        defaultMessage:
            'An installed copy is disabled, missing, or does not satisfy the required release. Review that copy before continuing.',
    },
    changes: {
        id: 'app.resources.operation.changes',
        defaultMessage:
            '{count, plural, one {# file will change} other {# files will change}} in {instance}.',
    },
    add: { id: 'app.resources.operation.add', defaultMessage: 'Add {name}' },
    remove: { id: 'app.resources.operation.remove', defaultMessage: 'Remove {name}' },
    requiredBy: {
        id: 'app.resources.operation.required-by',
        defaultMessage: 'Required by {names}',
    },
    optionalFor: {
        id: 'app.resources.operation.optional-for',
        defaultMessage: 'Optional dependency of {names}',
    },
    acceptDimension: {
        id: 'app.resources.operation.accept-dimension',
        defaultMessage:
            '{dimension, select, gameVersion {Accept Minecraft compatibility exception} other {Accept loader compatibility exception}}',
    },
    assessment: {
        id: 'app.resources.operation.assessment',
        defaultMessage: 'Game version: {game}; loader: {loader}',
    },
    state: {
        id: 'app.resources.operation.compatibility',
        defaultMessage:
            '{state, select, compatible {Compatible} incompatible {Incompatible} other {Unknown}}',
    },
    retry: { id: 'app.resources.operation.retry', defaultMessage: 'Retry' },
    refresh: { id: 'app.resources.operation.refresh', defaultMessage: 'Refresh this preview' },
    cancel: { id: 'app.resources.operation.cancel', defaultMessage: 'Cancel' },
    apply: { id: 'app.resources.operation.apply', defaultMessage: 'Queue changes' },
    empty: { id: 'app.resources.operation.empty', defaultMessage: 'No file changes are needed.' },
    sourceUnavailable: {
        id: 'app.resources.operation.source-unavailable',
        defaultMessage: 'The source cannot provide this item right now.',
    },
    unresolvedOptional: {
        id: 'app.resources.operation.unresolved-optional',
        defaultMessage: 'Some optional dependencies could not be resolved.',
    },
    missingDependency: {
        id: 'app.resources.operation.missing-dependency',
        defaultMessage: 'A required dependency could not be resolved.',
    },
    dependencyAnalysisFailed: {
        id: 'app.resources.operation.dependency-analysis-failed',
        defaultMessage: 'Dependency analysis failed. The instance may not work correctly.',
    },
    unselectedDependency: {
        id: 'app.resources.operation.unselected-dependency',
        defaultMessage:
            'Automatic dependency installation is off. This required dependency is not selected.',
    },
    versionConflict: {
        id: 'app.resources.operation.version-conflict',
        defaultMessage: 'A version conflict was detected.',
    },
    conflictDetails: {
        id: 'app.resources.operation.conflict.details',
        defaultMessage: 'Conflict details',
    },
    reviewIssues: {
        id: 'app.resources.operation.review-issues',
        defaultMessage: 'Installation issues',
    },
    issueSubject: {
        id: 'app.resources.operation.issue-subject',
        defaultMessage: '{subject}: {reason}',
    },
    issueSource: {
        id: 'app.resources.operation.issue-source',
        defaultMessage: '{subject} ({source}): {reason}',
    },
    fileIssue: {
        id: 'app.resources.operation.file-issue',
        defaultMessage: '{reason} File: {path}',
    },
    compatibilityDetail: {
        id: 'app.resources.operation.compatibility-detail',
        defaultMessage:
            '{subject}: {assessment}. Choose a matching version, or install the selected content with these compatibility exceptions.',
    },
    compatibilitySummary: {
        id: 'app.resources.operation.compatibility-summary',
        defaultMessage: 'Compatibility is unknown or mismatched.',
    },
    dependencyAnalysisSummary: {
        id: 'app.resources.operation.dependency-analysis-summary',
        defaultMessage: 'Dependency checks could not be completed.',
    },
    fileWriteSummary: {
        id: 'app.resources.operation.file-write-summary',
        defaultMessage: 'Some files cannot be written.',
    },
    previewUnavailable: {
        id: 'app.resources.operation.preview-unavailable',
        defaultMessage: 'Installation checks could not be completed.',
    },
    manualDownloadSummary: {
        id: 'app.resources.operation.manual-download-summary',
        defaultMessage: 'This file requires a manual download.',
    },
    staleSummary: {
        id: 'app.resources.operation.stale-summary',
        defaultMessage: 'The instance content has changed.',
    },
    conflictSelectedContent: {
        id: 'app.resources.operation.conflict.selected-content',
        defaultMessage: 'The selected content',
    },
    conflictDependency: {
        id: 'app.resources.operation.conflict.dependency',
        defaultMessage: 'a required dependency',
    },
    conflictRequiredRelease: {
        id: 'app.resources.operation.conflict.required-release',
        defaultMessage: 'the required release or file',
    },
    conflictUnresolvedVersion: {
        id: 'app.resources.operation.conflict.unresolved-version',
        defaultMessage: 'version unknown',
    },
    conflictAllVersions: {
        id: 'app.resources.operation.conflict.all-versions',
        defaultMessage: 'all versions',
    },
    conflictingReleases: {
        id: 'app.resources.operation.conflict.conflicting-releases',
        defaultMessage:
            '{subject} is required with different files: {required}. The selected file is {actual}.',
    },
    incompatibleResource: {
        id: 'app.resources.operation.conflict.incompatible-resource',
        defaultMessage: '{subject} declares an incompatibility with {related} ({required}).',
    },
    incompatibleInstalledDependent: {
        id: 'app.resources.operation.conflict.incompatible-installed-dependent',
        defaultMessage:
            'Installed {related} declares an incompatibility with {subject} ({required}).',
    },
    dependencyVersion: {
        id: 'app.resources.operation.conflict.dependency-version',
        defaultMessage:
            '{subject} requires {related} to match {required}; the selected version or file is {actual}.',
    },
    versionConstraints: {
        id: 'app.resources.operation.conflict.version-constraints',
        defaultMessage: 'No release of {subject} satisfies all requirements: {required}.',
    },
    retainedVersionConstraints: {
        id: 'app.resources.operation.conflict.retained-version-constraints',
        defaultMessage:
            'No release of {subject} satisfies the requirements of {related}: {required}.',
    },
    installedDependent: {
        id: 'app.resources.operation.conflict.installed-dependent',
        defaultMessage:
            'Installed {related} requires {subject} to match {required}; the selected version is {actual}.',
    },
    installedDependencyDisabled: {
        id: 'app.resources.operation.conflict.installed-dependency-disabled',
        defaultMessage: 'The installed dependency {related} is disabled.',
    },
    installedDependencyMissing: {
        id: 'app.resources.operation.conflict.installed-dependency-missing',
        defaultMessage: 'The installed dependency file {related} is missing.',
    },
    installedDependencyVersion: {
        id: 'app.resources.operation.conflict.installed-dependency-version',
        defaultMessage:
            'Installed {related} does not satisfy {subject}: required {required}, installed {actual}.',
    },
    notNewerRelease: {
        id: 'app.resources.operation.conflict.not-newer-release',
        defaultMessage:
            '{subject} version {actual} is not newer than the installed version {required}. Use Switch version to install it.',
    },
    artifactMismatch: {
        id: 'app.resources.operation.artifact-mismatch',
        defaultMessage: 'The selected installed entry does not match this item.',
    },
    skipUnwritable: {
        id: 'app.resources.operation.skip-unwritable',
        defaultMessage: 'Force install writable files',
    },
    fileWriteSkipped: {
        id: 'app.resources.operation.file-write-skipped',
        defaultMessage: '{path} will be skipped because it cannot be written.',
    },
    noWritableFiles: {
        id: 'app.resources.operation.no-writable-files',
        defaultMessage: 'No writable files remain to install.',
    },
})

type OperationInput = {
    analysis?: ContentAnalysisReview
    importFiles?: Record<string, string>
    locators?: Record<string, string>
    instanceId: string
    instanceName: string
    entryIds: string[]
    selections?: ResourceSelection[]
    intent: ResourceOperationIntent
    release?: SourceReleaseRef | null
    installDependencies: boolean
}
const modal = ref<InstanceType<typeof NewModal> | null>(null)
const plan = ref<ResourceOperationPlan | null>(null)
const progressNodes = shallowRef(new Map<string, ResourceDependencyNode>())
/** Shown plan nodes the running analysis retracted; hidden until its plan arrives. */
const removedNodes = shallowRef(new Set<string>())
/** What the running analysis reports doing; null while it resolves nodes. */
const progressPhase = ref<ResourceOperationPhase | null>(null)
const progressRound = ref(0)
const phaseMessages = {
    collectingConstraints: messages.loadingCollectingConstraints,
    identifyingInstalled: messages.loadingIdentifyingInstalled,
    prefetching: messages.loadingPrefetching,
    refreshingContent: messages.loadingRefreshingContent,
    validating: messages.loadingValidating,
} satisfies Record<ResourceOperationPhase, unknown>
const progressMessage = computed(() =>
    progressPhase.value
        ? phaseMessages[progressPhase.value]
        : progressRound.value > 0
          ? messages.loadingReresolving
          : messages.loading,
)
const displayedNodes = computed(() => {
    const nodes = new Map((plan.value?.nodes ?? []).map((node) => [node.id, node]))
    for (const id of removedNodes.value) nodes.delete(id)
    for (const [id, node] of progressNodes.value) nodes.set(id, node)
    return [...nodes.values()]
})
const input = ref<OperationInput | null>(null)
const busy = ref(false)
const worldOptions = ref<{ value: string; label: string }[]>([])
const targetWorld = ref<string | null>(null)
const needsWorld = computed(
    () =>
        input.value?.selections?.some(
            (selection) =>
                selection.resourceType === 'dataPack' && selection.scope.kind === 'instance',
        ) ?? false,
)
const queueing = ref(false)
/** Only the queue request writes; analysis and plan revisions before it stay cancellable. */
const committing = ref(false)
const closing = ref(false)
const queueFailed = ref(false)
const dependenciesIgnored = ref(false)
const error = ref<string | null>(null)
const errorSummary = ref<string | null>(null)
const names = ref<Record<string, { title: string; iconUrl: string | null; summary?: string }>>({})
let controller: AbortController | null = null
let generation = 0
let resolveShow: ((job: InstallJobSnapshot | null) => void) | null = null
const submittedRoots = ref<string[]>([])
let retryAction: (() => Promise<void>) | null = null

const active = computed(() => {
    return displayedNodes.value.filter((node) => node.state !== 'optionalCandidate')
})
const candidates = computed(() =>
    displayedNodes.value.filter((node) => node.state === 'optionalCandidate'),
)
type PlanIssue = ResourceOperationPlan['issues'][number]
type Notice = { text: string; blocking: boolean }
function isSelectedIssue(issue: PlanIssue, context: ResourceOperationPlan | null) {
    if (issue.context) return !issue.context.dependency
    return context?.activeRoots.includes(issue.selectionId ?? '') ?? false
}
function issueSubject(issue: PlanIssue, context: ResourceOperationPlan | null) {
    const node = context?.nodes.find((node) => node.id === issue.selectionId)
    return (
        (issue.selectionId ? names.value[issue.selectionId]?.title : undefined) ||
        issue.context?.subject ||
        node?.artifact?.fileName ||
        formatMessage(
            isSelectedIssue(issue, context)
                ? messages.conflictSelectedContent
                : messages.conflictDependency,
        )
    )
}
function issueDetail(issue: PlanIssue, context: ResourceOperationPlan | null, reason: string) {
    const selection = context?.selections.find((selection) => selection.id === issue.selectionId)
    const source =
        issue.context?.source ||
        (selection?.resource.kind === 'source' ? selection.resource.project.source : undefined)
    return formatMessage(source ? messages.issueSource : messages.issueSubject, {
        subject: issueSubject(issue, context),
        source: source ? resourceSourceName(source) : '',
        reason,
    })
}
function showFailure(failure: unknown) {
    const feedback = resourceFailureFeedback(failure, formatMessage)
    error.value = feedback.detail
    errorSummary.value = feedback.summary
}
function conflictNotice(issue: PlanIssue, context: ResourceOperationPlan | null): string {
    const detail = issue.conflict
    if (!detail) return formatMessage(messages.versionConflict)
    const node = context?.nodes.find((candidate) => candidate.id === issue.selectionId)
    const related = detail.related
        ? context?.nodes.find((candidate) => candidate.artifact?.fileName === detail.related)
        : undefined
    const subjectName =
        (issue.selectionId ? names.value[issue.selectionId]?.title : undefined) ||
        detail.subject ||
        (node ? name(node) : undefined) ||
        formatMessage(messages.conflictSelectedContent)
    const values = {
        subject: subjectName,
        related:
            (related ? name(related) : detail.related) ||
            formatMessage(messages.conflictDependency),
        required:
            detail.required === '*' ||
            (!detail.required &&
                ['incompatibleResource', 'incompatibleInstalledDependent'].includes(detail.reason))
                ? formatMessage(messages.conflictAllVersions)
                : detail.required || formatMessage(messages.conflictRequiredRelease),
        actual: detail.actual || formatMessage(messages.conflictUnresolvedVersion),
    }
    const message =
        detail.reason === 'versionConstraints' && detail.related
            ? messages.retainedVersionConstraints
            : messages[detail.reason]
    return message ? formatMessage(message, values) : formatMessage(messages.versionConflict)
}
function issueNotice(issue: PlanIssue, context = plan.value): Notice | null {
    const node = context?.nodes.find((candidate) => candidate.id === issue.selectionId)
    if (issue.code === 'compatibilityUnknown' || issue.code === 'compatibilityMismatch') {
        const assessment = node
            ? formatMessage(messages.assessment, {
                  game: formatMessage(messages.state, { state: node.compatibility.gameVersion }),
                  loader: formatMessage(messages.state, { state: node.compatibility.loader }),
              })
            : formatMessage(messages.compatibilitySummary)
        return {
            text: formatMessage(messages.compatibilityDetail, {
                subject: issueSubject(issue, context),
                assessment,
            }),
            blocking: issue.blocking,
        }
    }
    if (!['versionConflict', 'fileWriteSkipped'].includes(issue.code)) {
        const reason =
            issue.code === 'missingDependency' && node?.state === 'missing'
                ? formatMessage(messages.unselectedDependency)
                : resourcePlanIssueFeedback(issue, formatMessage, isSelectedIssue(issue, context))
                      .detail
        const path = issue.message.startsWith('target already exists: ')
            ? issue.message.slice('target already exists: '.length)
            : ['fileNotWritable', 'invalidPath'].includes(issue.code) &&
                !issue.message.includes(': ') &&
                !issue.message.startsWith('invalid') &&
                /[\\/]|\.[a-z0-9]{1,8}$/i.test(issue.message)
              ? issue.message
              : undefined
        return {
            text: issueDetail(
                issue,
                context,
                path ? formatMessage(messages.fileIssue, { path, reason }) : reason,
            ),
            blocking: issue.blocking,
        }
    }
    const text = (() => {
        switch (issue.code) {
            case 'fileWriteSkipped':
                return formatMessage(messages.fileWriteSkipped, { path: issue.message })
            case 'versionConflict':
                if (issue.conflict) return conflictNotice(issue, context)
                if (
                    issue.message ===
                    'installed dependency is disabled, missing or does not satisfy this release; review the installed entry'
                )
                    return formatMessage(messages.installedDependencyUnavailable)
                return issue.blocking
                    ? formatMessage(messages.versionConflict)
                    : formatMessage(messages.unresolvedOptional)
            default:
                return resourceFailureFeedback(issue.message, formatMessage).detail
        }
    })()
    return { text, blocking: issue.blocking }
}
const notices = computed(() => {
    const nodes = plan.value?.nodes ?? []
    const bound = new Map<string, Notice[]>()
    const other: Notice[] = []
    for (const issue of plan.value?.issues ?? []) {
        if (
            issue.code === 'versionConflict' ||
            issue.blocking ||
            issue.code === 'dependencyAnalysisFailed'
        )
            continue
        const notice = issueNotice(issue)
        if (!notice) continue
        const node = nodes.find((candidate) => candidate.id === issue.selectionId)
        if (!node) {
            other.push(notice)
            continue
        }
        const list = bound.get(node.id) ?? []
        if (!list.some((existing) => existing.text === notice.text)) list.push(notice)
        bound.set(node.id, list)
    }
    return { bound, other }
})
const conflictNotices = computed(() => {
    const unique = new Map<string, Notice>()
    for (const issue of plan.value?.issues ?? []) {
        if (
            issue.code !== 'versionConflict' &&
            !issue.blocking &&
            issue.code !== 'dependencyAnalysisFailed'
        )
            continue
        const notice = issueNotice(issue)
        if (notice) unique.set(notice.text, notice)
    }
    return [...unique.values()]
})
const reviewMessages = computed(() => {
    const values = conflictNotices.value.map((notice) => notice.text)
    if (error.value) values.push(...error.value.split('\n').filter(Boolean))
    return [...new Set(values)]
})
function noticesFor(node: ResourceDependencyNode) {
    return notices.value.bound.get(node.id) ?? []
}
function noticeClass(notice: Notice) {
    return notice.blocking ? 'text-[var(--color-red)]' : 'text-[var(--color-text-tertiary)]'
}
const canApply = computed(() => !busy.value && !queueing.value)
const dependencyAnalysisFailed = computed(
    () => plan.value?.issues.some((issue) => issue.code === 'dependencyAnalysisFailed') ?? false,
)
function requiresForceInstall(value: ResourceOperationPlan) {
    return value.issues.some((issue) => issue.blocking || issue.code === 'dependencyAnalysisFailed')
}
const forceInstall = computed(
    () =>
        !!error.value ||
        !plan.value ||
        requiresForceInstall(plan.value) ||
        !plan.value.actions.length,
)
function installationErrors(value: ResourceOperationPlan): string[] {
    return [
        ...new Set(
            value.issues
                .filter(
                    (issue) =>
                        issue.code !== 'versionConflict' &&
                        (issue.blocking || issue.code === 'dependencyAnalysisFailed'),
                )
                .map((issue) => {
                    const node = value.nodes.find((node) => node.id === issue.selectionId)
                    if (
                        node &&
                        ['compatibilityUnknown', 'compatibilityMismatch'].includes(issue.code)
                    )
                        return formatMessage(messages.assessment, {
                            game: formatMessage(messages.state, {
                                state: node.compatibility.gameVersion,
                            }),
                            loader: formatMessage(messages.state, {
                                state: node.compatibility.loader,
                            }),
                        })
                    return (
                        issueNotice(issue, value)?.text ??
                        resourceOperationError(issue.message, formatMessage)
                    )
                }),
        ),
    ]
}
const applyTooltip = computed(() => {
    if (!forceInstall.value) return undefined
    if (errorSummary.value && error.value) return errorSummary.value
    if (!plan.value)
        return formatMessage(
            needsWorld.value && !targetWorld.value
                ? messages.noWorlds
                : messages.previewUnavailable,
        )
    const codes = new Set(
        plan.value.issues
            .filter((issue) => issue.blocking || issue.code === 'dependencyAnalysisFailed')
            .map((issue) => issue.code),
    )
    const summaries = [
        ['invalidPath', messages.invalidPath],
        ['fileNotWritable', messages.fileWriteSummary],
        ['identityUnconfirmed', messages.identityUnconfirmed],
        ['artifactMismatch', messages.artifactMismatch],
        ['staleRevision', messages.staleSummary],
        ['sourceUnavailable', messages.sourceUnavailable],
        ['manualDownloadRequired', messages.manualDownloadSummary],
        ['versionConflict', messages.versionConflict],
        ['compatibilityMismatch', messages.compatibilitySummary],
        ['compatibilityUnknown', messages.compatibilitySummary],
        ['missingDependency', messages.missingDependency],
        ['dependencyAnalysisFailed', messages.dependencyAnalysisSummary],
    ] as const
    const selectedSummary = summaries.find(([code]) => codes.has(code))
    const summary = selectedSummary?.[1]
    const issue = plan.value.issues.find(
        (issue) =>
            issue.code === selectedSummary?.[0] &&
            issue.code !== 'versionConflict' &&
            !issue.code.startsWith('compatibility'),
    )
    if (issue)
        return resourcePlanIssueFeedback(issue, formatMessage, isSelectedIssue(issue, plan.value))
            .summary
    return formatMessage(summary ?? messages.empty)
})
const canSkipUnwritable = computed(
    () =>
        !busy.value &&
        !queueing.value &&
        !dependencyAnalysisFailed.value &&
        plan.value?.issues.some((issue) => issue.code === 'fileNotWritable') &&
        !plan.value.issues.some((issue) => issue.blocking && issue.code !== 'fileNotWritable'),
)
const hasSkippedWrites = computed(
    () => plan.value?.issues.some((issue) => issue.code === 'fileWriteSkipped') ?? false,
)
function name(node: ResourceDependencyNode) {
    return (
        names.value[node.id]?.title ??
        node.artifact?.fileName ??
        (node.resource.kind === 'source' ? node.resource.project.projectId : node.resource.id)
    )
}
function explicitRoots(node: ResourceDependencyNode): string[] {
    return node.rootSelectionIds?.length
        ? node.rootSelectionIds
        : plan.value?.activeRoots.includes(node.id)
          ? [node.id]
          : []
}
function requiredBy(id: string, kind = 'required') {
    return [
        ...new Set(
            (plan.value?.edges ?? [])
                .filter((edge) => edge.child === id && edge.kind === kind)
                .map((edge) => plan.value?.nodes.find((node) => node.id === edge.parent))
                .filter((node): node is ResourceDependencyNode => !!node)
                .map(name),
        ),
    ].join(', ')
}
async function enrich(value: ResourceOperationPlan, signal: AbortSignal, current: number) {
    const unresolved = value.selections.filter(
        (selection) =>
            !value.nodes.some((node) => node.id === selection.id) && !names.value[selection.id],
    )
    void Promise.allSettled(
        unresolved.map(async (selection) => {
            const resource = await resourceGet(selection.resource, undefined, undefined, signal)
            if (current !== generation || signal.aborted) return
            const project = resource.projects[0]
            if (project)
                names.value = {
                    ...names.value,
                    [selection.id]: { title: project.title, iconUrl: project.iconUrl },
                }
        }),
    )
    const refs = value.nodes
        .filter((node) => node.state !== 'satisfied' && !names.value[node.id])
        .flatMap((node) => {
            const project =
                node.artifact?.reference.release.project ??
                (node.resource.kind === 'source' ? node.resource.project : null)
            return project ? [project] : []
        })
    await Promise.allSettled(
        [...new Set(refs.map((project) => project.source))]
            .filter((source) => source === 'modrinth' || source === 'curseforge')
            .map((source) =>
                resourceProjectDocuments(
                    source,
                    [
                        ...new Set(
                            refs
                                .filter((project) => project.source === source)
                                .map((project) => project.projectId),
                        ),
                    ],
                    signal,
                ),
            ),
    )
    if (current !== generation || signal.aborted) return
    await Promise.allSettled(
        value.nodes.map(async (node) => {
            if (names.value[node.id]) return
            const cachedRef =
                node.artifact?.reference.release.project ??
                (node.resource.kind === 'source' ? node.resource.project : null)
            if (cachedRef?.source === 'local') return
            if (node.state === 'satisfied' && cachedRef) {
                const [document] = await resourceProjectDocuments(
                    cachedRef.source,
                    [cachedRef.projectId],
                    signal,
                    'onlyCached',
                )
                if (document && current === generation && !signal.aborted)
                    names.value = {
                        ...names.value,
                        [node.id]: {
                            title: document.metadata.title,
                            iconUrl: document.metadata.iconUrl,
                            summary: document.metadata.summary,
                        },
                    }
                return
            }
            const resource = await resourceGet(node.resource, undefined, undefined, signal)
            if (current !== generation || signal.aborted) return
            const project =
                resource.projects.find(
                    (project) =>
                        project.project.source === node.artifact?.reference.release.project.source,
                ) ?? resource.projects[0]
            if (project)
                names.value = {
                    ...names.value,
                    [node.id]: {
                        title: project.title,
                        iconUrl: project.iconUrl,
                        summary: project.summary,
                    },
                }
        }),
    )
}
async function run(
    action: (
        signal: AbortSignal,
        onProgress: (progress: ResourceOperationProgress) => void,
    ) => Promise<ResourceOperationPlan>,
    ignoreDependencies = dependenciesIgnored.value,
) {
    const current = generation
    controller?.abort()
    controller = new AbortController()
    const request = controller
    const signal = request.signal
    const isCurrent = () => current === generation && controller === request
    busy.value = true
    clearProgress()
    error.value = null
    errorSummary.value = null
    retryAction = () => run(action, ignoreDependencies)
    let acceptingProgress = true
    try {
        const value = await resourceAnalysisRequest(request, (signal) =>
            action(signal, (progress) => {
                if (!acceptingProgress || !isCurrent() || signal.aborted) return
                applyProgress(progress)
            }),
        )
        if (!isCurrent() || signal.aborted) return
        plan.value = value
        dependenciesIgnored.value = ignoreDependencies
        void enrich(value, signal, current)
        return value
    } catch (failure) {
        if (isCurrent() && (!signal.aborted || signal.reason?.name === 'TimeoutError'))
            showFailure(failure)
    } finally {
        acceptingProgress = false
        if (isCurrent()) {
            busy.value = false
            clearProgress()
        }
    }
}
function clearProgress() {
    progressNodes.value = new Map()
    removedNodes.value = new Set()
    progressPhase.value = null
    progressRound.value = 0
}
function applyProgress(progress: ResourceOperationProgress) {
    switch (progress.kind) {
        case 'phase':
            progressPhase.value = progress.phase
            return
        case 'roundStarted':
            progressPhase.value = null
            progressRound.value = progress.round
            return
        case 'nodeRemoved':
            progressNodes.value.delete(progress.nodeId)
            triggerRef(progressNodes)
            removedNodes.value.add(progress.nodeId)
            triggerRef(removedNodes)
            return
        case 'node': {
            const { node, metadata } = progress
            progressPhase.value = null
            if (removedNodes.value.delete(node.id)) triggerRef(removedNodes)
            progressNodes.value.set(node.id, node)
            triggerRef(progressNodes)
            if (metadata)
                names.value[node.id] = {
                    title: metadata.title,
                    iconUrl: metadata.iconUrl,
                    summary: node.state === 'satisfied' ? undefined : metadata.summary,
                }
        }
    }
}
async function revise(decision: ResourcePlanDecision) {
    const value = plan.value
    if (!value || busy.value || queueing.value) return
    return await run(
        (signal, onProgress) =>
            resourceRevisePlan(value.id, value.revision, [decision], signal, onProgress),
        decision.kind === 'ignoreDependencies' || dependenciesIgnored.value,
    )
}
function release() {
    generation++
    input.value?.analysis?.signal.removeEventListener('abort', invalidateAnalysis)
    controller?.abort()
    const resolve = resolveShow
    resolveShow = null
    resolve?.(null)
}
function hide() {
    const analysis =
        !closing.value && busy.value && !committing.value ? input.value?.analysis : null
    release()
    // Leaving during analysis stops the cart's analysis too; the selection itself is kept.
    analysis?.cancel()
    // A queued submission keeps its controls locked until the close animation ends.
    if (!committing.value && !closing.value) {
        busy.value = false
        queueing.value = false
    }
}
function show(value: OperationInput): Promise<InstallJobSnapshot | null> {
    if (queueing.value) return Promise.resolve(null)
    release()
    input.value = value
    plan.value = null
    clearProgress()
    submittedRoots.value = []
    names.value = {}
    error.value = null
    errorSummary.value = null
    queueFailed.value = false
    dependenciesIgnored.value = false
    retryAction = null
    worldOptions.value = []
    targetWorld.value = null
    modal.value?.show()
    value.analysis?.signal.addEventListener('abort', invalidateAnalysis, { once: true })
    const result = new Promise<InstallJobSnapshot | null>((resolve) => {
        resolveShow = resolve
    })
    if (needsWorld.value) {
        const current = generation
        busy.value = true
        const loadWorlds = async () => {
            busy.value = true
            error.value = null
            await get_instance_worlds(value.instanceId)
                .then((worlds) => {
                    if (current !== generation) return
                    worldOptions.value = worlds.flatMap((world) =>
                        world.type === 'singleplayer'
                            ? [
                                  {
                                      value: `saves/${world.path.replace(/^saves\//, '')}`,
                                      label: world.name,
                                  },
                              ]
                            : [],
                    )
                })
                .catch((failure) => {
                    if (current === generation) showFailure(failure)
                })
                .finally(() => {
                    if (current === generation) busy.value = false
                })
        }
        retryAction = loadWorlds
        void loadWorlds()
        return result
    }
    void prepareInput(value)
    return result
}
function invalidateAnalysis() {
    void cancel()
}
async function prepareInput(
    value: OperationInput,
    resolveDependencies = !dependenciesIgnored.value,
) {
    return await run(
        (signal, onProgress) =>
            value.analysis && resolveDependencies
                ? value.analysis.prepare(signal, onProgress, value.selections)
                : value.selections
                  ? resourcePrepareOperation(
                        value.instanceId,
                        'install',
                        value.selections,
                        value.installDependencies,
                        signal,
                        value.importFiles,
                        value.locators,
                        onProgress,
                        resolveDependencies,
                    )
                  : resourcePrepareEntryOperation(
                        value.instanceId,
                        value.intent,
                        value.entryIds,
                        value.release ?? null,
                        value.installDependencies,
                        signal,
                        onProgress,
                        resolveDependencies,
                    ),
        !resolveDependencies,
    )
}
function withTargetWorld(value: OperationInput): OperationInput {
    return {
        ...value,
        selections: value.selections?.map((selection) =>
            selection.resourceType === 'dataPack' && targetWorld.value
                ? {
                      ...selection,
                      scope: { kind: 'world' as const, relativePath: targetWorld.value },
                  }
                : selection,
        ),
    }
}
function selectWorld(path: string) {
    if (!input.value || busy.value || queueing.value) return
    targetWorld.value = path
    plan.value = null
    void prepareInput(withTargetWorld(input.value))
}
async function apply() {
    await submit(true)
}
async function submit(retryStale: boolean) {
    if (!input.value || !canApply.value) return
    if (needsWorld.value && !targetWorld.value) {
        error.value = formatMessage(messages.noWorlds)
        errorSummary.value = formatMessage(messages.noWorlds)
        return
    }
    const current = generation
    const resolve = resolveShow
    queueing.value = true
    queueFailed.value = false
    error.value = null
    errorSummary.value = null
    try {
        let reviewed = plan.value
        if (!reviewed || requiresForceInstall(reviewed)) {
            if (!reviewed) {
                reviewed = (await prepareInput(withTargetWorld(input.value), false)) ?? null
            } else if (!dependenciesIgnored.value) {
                const previous = reviewed
                reviewed =
                    (await run(
                        (signal, onProgress) =>
                            resourceRevisePlan(
                                previous.id,
                                previous.revision,
                                [{ kind: 'ignoreDependencies' }],
                                signal,
                                onProgress,
                            ),
                        true,
                    )) ?? null
            }
            if (!reviewed || current !== generation) {
                if (current === generation) retryAction = apply
                return
            }
            const selectedOnly = reviewed
            const decisions: ResourcePlanDecision[] = selectedOnly.nodes.flatMap((node) => {
                if (!node.artifact || node.state === 'optionalCandidate') return []
                const dimensions = (['gameVersion', 'loader'] as const).filter(
                    (dimension) =>
                        node.compatibility[dimension] === 'incompatible' &&
                        !selectedOnly.exceptions.some(
                            (exception) =>
                                exception.selectionId === node.id &&
                                exception.dimensions.includes(dimension),
                        ),
                )
                return dimensions.length
                    ? [
                          {
                              kind: 'acceptCompatibility' as const,
                              exception: {
                                  selectionId: node.id,
                                  release: node.artifact.reference.release,
                                  dimensions,
                              },
                          },
                      ]
                    : []
            })
            if (decisions.length) {
                const previous = reviewed
                reviewed =
                    (await run(
                        (signal, onProgress) =>
                            resourceRevisePlan(
                                previous.id,
                                previous.revision,
                                decisions,
                                signal,
                                onProgress,
                            ),
                        true,
                    )) ?? null
            }
        }
        if (!reviewed || current !== generation) {
            if (current === generation) retryAction = apply
            return
        }
        if (requiresForceInstall(reviewed) || !reviewed.actions.length) {
            error.value =
                installationErrors(reviewed).join('\n') ||
                (reviewed.issues.some((issue) => issue.code === 'versionConflict')
                    ? null
                    : formatMessage(
                          hasSkippedWrites.value ? messages.noWritableFiles : messages.empty,
                      ))
            retryAction = apply
            return
        }
        busy.value = true
        committing.value = true
        let job: InstallJobSnapshot
        try {
            job = await resourceQueuePlan(reviewed.id, reviewed.revision)
        } finally {
            // Only the queue request itself is locked; the refresh after a stale
            // plan below is analysis again and must stay cancellable.
            committing.value = false
        }
        if (generation !== current) return
        const skippedRoots = new Set(
            reviewed.issues
                .filter((issue) => issue.code === 'fileWriteSkipped')
                .flatMap((issue) => {
                    const node = reviewed.nodes.find((node) => node.id === issue.selectionId)
                    return node?.rootSelectionIds?.length
                        ? node.rootSelectionIds
                        : issue.selectionId
                          ? [issue.selectionId]
                          : []
                }),
        )
        submittedRoots.value = reviewed.activeRoots.filter((root) => !skippedRoots.has(root))
        resolveShow = null
        closing.value = true
        await nextTick()
        await modal.value?.hide()
        resolve?.(job)
    } catch (failure) {
        if (generation !== current) return
        if (
            retryStale &&
            resourceOperationError(failure).includes('instance changed after review')
        ) {
            busy.value = false
            const refreshed = await refresh()
            if (generation === current) {
                if (refreshed) {
                    queueing.value = false
                    await submit(false)
                } else retryAction = apply
            }
            return
        }
        showFailure(failure)
        retryAction = apply
        queueFailed.value = true
    } finally {
        if (generation === current || (closing.value && generation === current + 1)) {
            busy.value = false
            queueing.value = false
            closing.value = false
        }
    }
}
async function refresh() {
    const current = plan.value
    const value = input.value
    if (!current || !value || busy.value) return
    queueFailed.value = false
    return await run(async (signal, onProgress) => {
        await refresh_content(value.instanceId)
        signal.throwIfAborted()
        const prepared = await resourcePrepareOperation(
            value.instanceId,
            current.intent,
            current.selections,
            value.installDependencies,
            signal,
            value.importFiles,
            value.locators,
            onProgress,
            !dependenciesIgnored.value,
        )
        const retainedExceptions = current.exceptions.filter((exception) =>
            prepared.nodes.some(
                (node) =>
                    node.id === exception.selectionId &&
                    node.artifact?.reference.release.project.source ===
                        exception.release.project.source &&
                    node.artifact.reference.release.project.projectId ===
                        exception.release.project.projectId &&
                    node.artifact.reference.release.releaseId === exception.release.releaseId,
            ),
        )
        if (!retainedExceptions.length) return prepared
        return resourceRevisePlan(
            prepared.id,
            prepared.revision,
            retainedExceptions.map((exception) => ({ kind: 'acceptCompatibility', exception })),
            signal,
            onProgress,
        )
    })
}
function accept(node: ResourceDependencyNode, dimension: 'gameVersion' | 'loader') {
    if (!node.artifact) return
    void revise({
        kind: 'acceptCompatibility',
        exception: {
            selectionId: node.id,
            release: node.artifact.reference.release,
            dimensions: [dimension],
        },
    })
}
onBeforeUnmount(hide)
function showInstall(value: {
    analysis?: ContentAnalysisReview
    importFiles?: Record<string, string>
    locators?: Record<string, string>
    instanceId: string
    instanceName: string
    selections: ResourceSelection[]
    installDependencies: boolean
}) {
    return show({ ...value, intent: 'install', entryIds: [] })
}
async function showInstallSelection(value: Parameters<typeof showInstall>[0]) {
    const job = await showInstall(value)
    return job ? { job, activeRoots: [...submittedRoots.value] } : null
}
async function cancel() {
    if (committing.value || closing.value) return
    hide()
    const current = generation
    plan.value = null
    input.value = null
    clearProgress()
    await nextTick()
    if (generation !== current) return
    await modal.value?.hide()
}
defineExpose({ show, showInstall, showInstallSelection, cancel })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.header)"
        :on-hide="hide"
        :disable-close="committing && !closing"
        scrollable
        width="min(42rem, 90vw)"
    >
        <div class="flex flex-col gap-4">
            <Admonition
                v-if="reviewMessages.length && !busy"
                type="warning"
                :header="
                    formatMessage(
                        !error && plan?.issues.every((issue) => issue.code === 'versionConflict')
                            ? messages.conflictDetails
                            : messages.reviewIssues,
                    )
                "
                :role="error ? 'alert' : undefined"
                data-resource-conflicts
            >
                <ul class="m-0 grid list-disc gap-3 pl-5 text-sm leading-relaxed">
                    <li v-for="text in reviewMessages" :key="text" class="[overflow-wrap:anywhere]">
                        {{ text }}
                    </li>
                </ul>
                <template v-if="error" #actions>
                    <Button
                        v-if="retryAction"
                        :disabled="busy || queueing"
                        @click="retryAction?.()"
                        >{{ formatMessage(messages.retry) }}</Button
                    >
                    <Button v-if="queueFailed" :disabled="busy || queueing" @click="refresh">{{
                        formatMessage(messages.refresh)
                    }}</Button>
                </template>
            </Admonition>
            <div v-if="needsWorld" class="flex flex-col gap-2">
                <label>{{ formatMessage(messages.world) }}</label>
                <Combobox
                    :model-value="targetWorld"
                    :options="worldOptions"
                    :disabled="busy"
                    @update:model-value="selectWorld"
                />
                <p v-if="!busy && !worldOptions.length" class="m-0">
                    {{ formatMessage(messages.noWorlds) }}
                </p>
            </div>
            <p v-if="plan && !busy" class="m-0">
                {{
                    formatMessage(messages.changes, {
                        count: plan.actions.length,
                        instance: input?.instanceName ?? '',
                    })
                }}
            </p>
            <div
                v-for="node in active"
                :key="node.id"
                class="flex flex-col gap-2 rounded-lg border border-surface-4 bg-surface-2 p-3"
            >
                <div class="flex items-center gap-2">
                    <Avatar
                        :src="names[node.id]?.iconUrl ?? undefined"
                        :alt="name(node)"
                        size="32px"
                    />
                    <span class="grow font-semibold">{{ name(node) }}</span>
                    <Button
                        v-for="rootId in explicitRoots(node)"
                        :key="rootId"
                        size="sm"
                        :disabled="busy"
                        :aria-label="formatMessage(messages.remove, { name: name(node) })"
                        @click="revise({ kind: 'removeRoot', selectionId: rootId })"
                        >×</Button
                    >
                </div>
                <p
                    v-if="node.state !== 'satisfied' && names[node.id]?.summary"
                    class="m-0 text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ names[node.id].summary }}
                </p>
                <p v-if="node.state !== 'satisfied' && node.artifact" class="m-0 break-all text-sm">
                    {{
                        formatMessage(messages.artifact, {
                            source: resourceSourceName(
                                node.artifact.reference.release.project.source,
                            ),
                            release: node.artifact.reference.release.releaseId,
                            file: node.artifact.fileName ?? '',
                        })
                    }}
                </p>
                <p v-if="node.scope.kind === 'world'" class="m-0 break-all text-sm">
                    {{ node.scope.relativePath }}
                </p>
                <p v-if="node.state === 'satisfied'" class="m-0 text-sm">
                    {{ formatMessage(messages.satisfied) }}
                </p>
                <p v-if="node.state === 'disabled'" class="m-0 text-sm">
                    {{ formatMessage(messages.disabledDependency) }}
                </p>
                <Button
                    v-if="node.state === 'missing'"
                    size="sm"
                    :disabled="busy"
                    :aria-label="formatMessage(messages.add, { name: name(node) })"
                    @click="revise({ kind: 'addRequired', nodeId: node.id })"
                    >＋</Button
                >
                <p v-if="requiredBy(node.id)" class="m-0 text-sm">
                    {{ formatMessage(messages.requiredBy, { names: requiredBy(node.id) }) }}
                </p>
                <p v-else-if="requiredBy(node.id, 'optional')" class="m-0 text-sm">
                    {{
                        formatMessage(messages.optionalFor, {
                            names: requiredBy(node.id, 'optional'),
                        })
                    }}
                </p>
                <p
                    v-for="(notice, index) in noticesFor(node)"
                    :key="index"
                    class="m-0 text-sm"
                    :class="noticeClass(notice)"
                >
                    {{ notice.text }}
                </p>
                <p
                    v-for="action in plan?.actions.filter((action) => action.nodeId === node.id)"
                    :key="action.entryId"
                    class="m-0 break-all text-sm"
                >
                    {{ action.targetRelativePath }}
                </p>
                <template
                    v-if="
                        node.artifact &&
                        (node.compatibility.gameVersion !== 'compatible' ||
                            node.compatibility.loader !== 'compatible')
                    "
                >
                    <p class="m-0 text-sm">
                        {{
                            formatMessage(messages.assessment, {
                                game: formatMessage(messages.state, {
                                    state: node.compatibility.gameVersion,
                                }),
                                loader: formatMessage(messages.state, {
                                    state: node.compatibility.loader,
                                }),
                            })
                        }}
                    </p>
                    <template
                        v-for="dimension in ['gameVersion', 'loader'] as const"
                        :key="dimension"
                    >
                        <Button
                            v-if="
                                node.compatibility[dimension] === 'incompatible' &&
                                !plan?.exceptions.some(
                                    (exception) =>
                                        exception.selectionId === node.id &&
                                        exception.dimensions.includes(dimension),
                                )
                            "
                            :disabled="busy"
                            @click="accept(node, dimension)"
                            >{{ formatMessage(messages.acceptDimension, { dimension }) }}</Button
                        >
                    </template>
                </template>
            </div>
            <template v-if="candidates.length">
                <h3 class="m-0">{{ formatMessage(messages.optional) }}</h3>
                <div
                    v-for="node in candidates"
                    :key="node.id"
                    class="flex items-center gap-2 rounded-lg border border-surface-4 p-3"
                >
                    <Avatar
                        :src="names[node.id]?.iconUrl ?? undefined"
                        :alt="name(node)"
                        size="32px"
                    />
                    <div class="flex min-w-0 grow flex-col gap-1">
                        <span>{{ name(node) }}</span>
                        <span
                            v-if="requiredBy(node.id, 'optional')"
                            class="text-sm text-[var(--color-text-tertiary)]"
                        >
                            {{
                                formatMessage(messages.optionalFor, {
                                    names: requiredBy(node.id, 'optional'),
                                })
                            }}
                        </span>
                    </div>
                    <Button
                        size="sm"
                        :disabled="busy"
                        :aria-label="formatMessage(messages.add, { name: name(node) })"
                        @click="revise({ kind: 'addOptional', nodeId: node.id })"
                        >＋</Button
                    >
                </div>
            </template>
            <div
                v-if="busy && !queueing"
                class="flex flex-col gap-3"
                role="status"
                data-operation-skeleton
            >
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]" data-operation-phase>
                    {{ formatMessage(progressMessage) }}
                </p>
                <div
                    class="flex flex-col gap-2 rounded-lg border border-surface-4 bg-surface-2 p-3"
                    aria-hidden="true"
                >
                    <div class="flex items-center gap-2">
                        <div class="size-8 shrink-0 animate-pulse rounded-full bg-surface-3" />
                        <div class="h-4 w-2/5 animate-pulse rounded bg-surface-3" />
                    </div>
                    <div class="h-3 w-3/5 animate-pulse rounded bg-surface-3" />
                </div>
            </div>
            <p
                v-for="(notice, index) in notices.other"
                :key="index"
                class="m-0 text-sm"
                :class="noticeClass(notice)"
            >
                {{ notice.text }}
            </p>
            <p
                v-if="
                    plan &&
                    !busy &&
                    !plan.actions.length &&
                    !plan.issues.some((issue) => issue.blocking)
                "
                class="m-0"
            >
                {{ formatMessage(hasSkippedWrites ? messages.noWritableFiles : messages.empty) }}
            </p>
        </div>
        <template #actions>
            <div class="flex items-center justify-end gap-2">
                <Button :disabled="committing || closing" @click="cancel">{{
                    formatMessage(messages.cancel)
                }}</Button>
                <Button
                    v-if="canSkipUnwritable"
                    color="orange"
                    @click="revise({ kind: 'skipUnwritableFiles' })"
                >
                    {{ formatMessage(messages.skipUnwritable) }}
                </Button>
                <Button
                    v-tooltip="applyTooltip"
                    type="colored"
                    :color="forceInstall ? 'orange' : 'brand'"
                    :disabled="!canApply"
                    @click="apply"
                    >{{ formatMessage(messages.apply) }}</Button
                >
            </div>
        </template>
    </NewModal>
</template>
