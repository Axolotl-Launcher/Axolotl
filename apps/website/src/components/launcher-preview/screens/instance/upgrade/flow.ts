import type { ComputedRef, InjectionKey, MaybeRef, Ref } from 'vue'
import { computed, inject, provide, ref } from 'vue'

import type { GameInstance } from '../../../lib/types'
import type {
    InstallJobSnapshot,
    InstanceUpgradeIssue,
    InstanceUpgradePlan,
    InstanceUpgradeResult,
    InstanceUpgradeSolutionKind,
    InstanceUpgradeTargetEnvironment,
    SharedUpgradeMode,
} from './backend'

export type UpgradeRouteRequirement = 'plan' | 'unblocked-plan' | 'selection' | 'job' | 'result'
export type UpgradeJobRecoveryState = 'idle' | 'loading' | 'ready'

export function upgradeDownloadsLocation(jobId: string) {
    return { path: '/downloads', query: { job: jobId } } as const
}

export function upgradeProgressDestination(
    recoveryState: UpgradeJobRecoveryState,
    jobId: string | null,
    instanceId: string,
) {
    if (recoveryState !== 'ready') return null
    return jobId
        ? upgradeDownloadsLocation(jobId)
        : { path: `/instance/${encodeURIComponent(instanceId)}/upgrade` }
}

export function attachUpgradeJobToFlow(
    flow: Pick<InstanceUpgradeFlow, 'setJob' | 'setResult'>,
    job: InstallJobSnapshot,
) {
    flow.setJob(job.job_id)
    if (job.status === 'succeeded' && job.upgrade_result) {
        flow.setResult(job.upgrade_result)
    }
    return upgradeDownloadsLocation(job.job_id)
}

export interface InstanceUpgradeFlow {
    instance: Readonly<Ref<GameInstance>>
    instanceId: Readonly<Ref<string>>
    targetEnvironment: Ref<InstanceUpgradeTargetEnvironment | null>
    plan: Ref<InstanceUpgradePlan | null>
    selectedSolutionKind: ComputedRef<InstanceUpgradeSolutionKind | null>
    createFullBackup: Ref<boolean>
    directFullBackupPreference: Ref<boolean>
    sharedUpgradeMode: Ref<SharedUpgradeMode | null>
    activeJobId: Ref<string | null>
    jobRecoveryState: Ref<UpgradeJobRecoveryState>
    result: Ref<InstanceUpgradeResult | null>
    initialBlockingPlanId: Ref<string | null>
    initialBlockingIssues: Ref<Record<string, InstanceUpgradeIssue[]>>
    customizeActiveStrategy: Ref<InstanceUpgradeSolutionKind | null>
    busy: Ref<boolean>
    error: Ref<unknown | null>
    reset: () => void
    clearPlan: () => void
    setTargetEnvironment: (environment: InstanceUpgradeTargetEnvironment | null) => void
    setPlan: (plan: InstanceUpgradePlan | null) => void
    setJob: (jobId: string | null) => void
    setJobRecoveryState: (state: UpgradeJobRecoveryState) => void
    setResult: (result: InstanceUpgradeResult | null) => void
    hydrate: (snapshot: UpgradeFlowSnapshot) => void
    controls: Ref<UpgradeStepControls | null>
    registerStepControls: (controls: UpgradeStepControls | null) => void
}

export interface UpgradeStepControls {
    canNext: MaybeRef<boolean>
    busy?: MaybeRef<boolean>
    nextLabel: string
    onNext: () => void | Promise<void>
    onBack: () => void | Promise<void>
}

export interface UpgradeFlowSnapshot {
    instanceId: string
    returnFullPath: string
    targetEnvironment: InstanceUpgradeTargetEnvironment | null
    plan: InstanceUpgradePlan | null
    createFullBackup: boolean
    directFullBackupPreference?: boolean
    sharedUpgradeMode: SharedUpgradeMode | null
    activeJobId: string | null
    result: InstanceUpgradeResult | null
    initialBlockingPlanId?: string | null
    initialBlockingIssues?: Record<string, InstanceUpgradeIssue[]>
    customizeActiveStrategy?: InstanceUpgradeSolutionKind | null
    scrollTop?: number
}

export const INSTANCE_UPGRADE_FLOW_KEY: InjectionKey<InstanceUpgradeFlow> =
    Symbol('instance-upgrade-flow')

export function provideUpgradeFlow(flow: InstanceUpgradeFlow) {
    provide(INSTANCE_UPGRADE_FLOW_KEY, flow)
}

export function provideInstanceUpgradeFlow(
    instance: Readonly<Ref<GameInstance>>,
): InstanceUpgradeFlow {
    const instanceId = computed(() => instance.value.id)
    const targetEnvironment = ref<InstanceUpgradeTargetEnvironment | null>(null)
    const plan = ref<InstanceUpgradePlan | null>(null)
    const createFullBackup = ref(true)
    const directFullBackupPreference = ref(true)
    const sharedUpgradeMode = ref<SharedUpgradeMode | null>(null)
    const activeJobId = ref<string | null>(null)
    const jobRecoveryState = ref<UpgradeJobRecoveryState>('idle')
    const result = ref<InstanceUpgradeResult | null>(null)
    const initialBlockingPlanId = ref<string | null>(null)
    const initialBlockingIssues = ref<Record<string, InstanceUpgradeIssue[]>>({})
    const customizeActiveStrategy = ref<InstanceUpgradeSolutionKind | null>(null)
    const busy = ref(false)
    const error = ref<unknown | null>(null)
    const selectedSolutionKind = computed(() => plan.value?.selectedSolution?.kind ?? null)
    const controls = ref<UpgradeStepControls | null>(null)

    function clearPlan() {
        plan.value = null
        initialBlockingPlanId.value = null
        initialBlockingIssues.value = {}
        customizeActiveStrategy.value = null
        activeJobId.value = null
        result.value = null
    }

    function reset() {
        targetEnvironment.value = null
        clearPlan()
        createFullBackup.value = true
        directFullBackupPreference.value = true
        sharedUpgradeMode.value = null
        busy.value = false
        error.value = null
    }

    function hydrate(snapshot: UpgradeFlowSnapshot) {
        if (snapshot.instanceId !== instance.value.id) return
        targetEnvironment.value = snapshot.targetEnvironment
        plan.value = snapshot.plan
        createFullBackup.value = snapshot.createFullBackup
        directFullBackupPreference.value = snapshot.directFullBackupPreference ?? true
        sharedUpgradeMode.value = snapshot.sharedUpgradeMode
        activeJobId.value = snapshot.activeJobId
        result.value = snapshot.result
        initialBlockingPlanId.value = snapshot.initialBlockingPlanId ?? null
        initialBlockingIssues.value = snapshot.initialBlockingIssues ?? {}
        customizeActiveStrategy.value = snapshot.customizeActiveStrategy ?? null
    }

    const flow: InstanceUpgradeFlow = {
        instance,
        instanceId,
        targetEnvironment,
        plan,
        selectedSolutionKind,
        createFullBackup,
        directFullBackupPreference,
        sharedUpgradeMode,
        activeJobId,
        jobRecoveryState,
        result,
        initialBlockingPlanId,
        initialBlockingIssues,
        customizeActiveStrategy,
        busy,
        error,
        reset,
        clearPlan,
        setTargetEnvironment: (environment) => (targetEnvironment.value = environment),
        setPlan: (nextPlan) => {
            if (nextPlan?.id !== plan.value?.id) {
                initialBlockingPlanId.value = null
                initialBlockingIssues.value = {}
                customizeActiveStrategy.value = null
                sharedUpgradeMode.value = null
                createFullBackup.value = true
                directFullBackupPreference.value = true
            }
            plan.value = nextPlan
        },
        setJob: (jobId) => (activeJobId.value = jobId),
        setJobRecoveryState: (state) => (jobRecoveryState.value = state),
        setResult: (nextResult) => (result.value = nextResult),
        hydrate,
        controls,
        registerStepControls: (next) => (controls.value = next),
    }
    provideUpgradeFlow(flow)
    return flow
}

export function isUpgradeRouteRecoveryPending(
    requirement: UpgradeRouteRequirement | undefined,
    flow: InstanceUpgradeFlow,
): boolean {
    return (
        (requirement === 'job' || requirement === 'result') &&
        flow.jobRecoveryState.value === 'loading'
    )
}

export function useInstanceUpgradeFlow(): InstanceUpgradeFlow {
    const flow = inject(INSTANCE_UPGRADE_FLOW_KEY)
    if (!flow) throw new Error('Instance upgrade flow was not provided')
    return flow
}

export function isUpgradeRouteAvailable(
    requirement: UpgradeRouteRequirement | undefined,
    flow: InstanceUpgradeFlow,
): boolean {
    switch (requirement) {
        case 'plan':
            return flow.plan.value !== null
        case 'unblocked-plan':
            return flow.plan.value !== null && flow.plan.value.blockingIssues.length === 0
        case 'selection':
            return (
                flow.plan.value !== null &&
                flow.plan.value.blockingIssues.length === 0 &&
                flow.plan.value.selectedSolution !== null
            )
        case 'job':
            return flow.activeJobId.value !== null
        case 'result':
            return flow.result.value !== null
        default:
            return true
    }
}
