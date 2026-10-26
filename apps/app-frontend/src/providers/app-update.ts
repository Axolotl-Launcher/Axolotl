import { computed, ref } from 'vue'

import { getUpdateCapability } from '@/helpers/utils.js'

export const APP_UPDATE_POPUP_DELAY_MS = 24 * 60 * 60 * 1000

const UPDATE_PROMPT_STORAGE_KEY = 'modrinth-app-update-prompt-state'

export interface AppUpdate {
    rid: number
    version: string
    currentVersion?: string
    publishedAt?: string
    forceUpdate?: boolean
}

interface UpdatePromptState {
    version: string
    stage: AppUpdatePromptStage
    actionableSince: number
    lastUserActionAt?: number
    popupShownAt?: number
}

export type AppUpdatePromptStage = 'available' | 'downloaded'
export type AppUpdateCheckResult = 'available' | 'up-to-date' | 'disabled' | 'offline' | 'paused'

interface AppUpdateActions {
    check?: () => Promise<AppUpdateCheckResult>
    download?: () => Promise<void> | void
    install?: () => Promise<void> | void
    changelog?: (version?: string) => Promise<void> | void
}

const progress = ref(0)
const metered = ref(true)
const finishedDownloading = ref(false)
const downloading = ref(false)
const restarting = ref(false)
const availableUpdate = ref<AppUpdate | null>(null)
const updateSize = ref<number | null>(null)
const updatesEnabled = ref(true)
const updatesPaused = ref(false)

export interface UpdateCapability {
    source: string
    selfUpdate: boolean
    command: string | null
    /** Claimed package names when several package databases disagree. */
    owners: string[]
}

export const updateCapability = ref<UpdateCapability | null>(null)
let updateCapabilityRequest: Promise<UpdateCapability | null> | null = null

/**
 * Read the update capability from the shell once and share it between the
 * app shell and the settings panel, so both always agree on whether update
 * checks may run. A failed query is not cached: the next caller retries.
 */
export async function resolveUpdateCapability(): Promise<UpdateCapability | null> {
    if (!updateCapabilityRequest) {
        updateCapabilityRequest = getUpdateCapability()
            .then((capability) => {
                updateCapability.value = capability
                return capability
            })
            .catch((error) => {
                console.warn('Failed to read the update capability', error)
                updateCapabilityRequest = null
                return null
            })
    }
    return updateCapabilityRequest
}

/** Whether a package manager owns this installation and must do the update. */
export function isPackageManagedInstall(): boolean {
    return updateCapability.value !== null && !updateCapability.value.selfUpdate
}

let actions: AppUpdateActions = {}

function getCurrentAppUpdatePromptStage(): AppUpdatePromptStage {
    return finishedDownloading.value ? 'downloaded' : 'available'
}

export const appUpdateState = {
    progress,
    metered,
    finishedDownloading,
    downloading,
    restarting,
    availableUpdate,
    updateSize,
    updatesEnabled,
    updatesPaused,
    downloadProgress: computed(() => progress.value),
    downloadPercent: computed(() => Math.trunc(progress.value * 100)),
    isVisible: computed(
        () =>
            !!availableUpdate.value &&
            !restarting.value &&
            updatesEnabled.value &&
            !updatesPaused.value,
    ),
}

function readPromptState(): UpdatePromptState | null {
    try {
        const raw = localStorage.getItem(UPDATE_PROMPT_STORAGE_KEY)
        if (!raw) {
            return null
        }

        const parsed = JSON.parse(raw) as Partial<UpdatePromptState>
        if (!parsed.version || typeof parsed.actionableSince !== 'number') {
            return null
        }

        return {
            ...parsed,
            stage: parsed.stage ?? 'available',
        } as UpdatePromptState
    } catch {
        return null
    }
}

function writePromptState(state: UpdatePromptState): void {
    try {
        localStorage.setItem(UPDATE_PROMPT_STORAGE_KEY, JSON.stringify(state))
    } catch (error) {
        console.warn('Failed to persist update prompt state:', error)
    }
}

export function markAppUpdateActionable(
    version: string,
    stage: AppUpdatePromptStage = 'available',
    now = Date.now(),
): void {
    const existing = readPromptState()
    if (existing?.version === version && existing.stage === stage) {
        return
    }

    writePromptState({
        version,
        stage,
        actionableSince: now,
    })
}

export function recordAppUpdateUserAction(
    version = availableUpdate.value?.version,
    stage: AppUpdatePromptStage = getCurrentAppUpdatePromptStage(),
): void {
    if (!version) {
        return
    }

    const now = Date.now()
    const existing = readPromptState()
    const isSamePrompt = existing?.version === version && existing.stage === stage
    writePromptState({
        version,
        stage,
        actionableSince: isSamePrompt ? existing.actionableSince : now,
        lastUserActionAt: now,
        popupShownAt: isSamePrompt ? existing.popupShownAt : undefined,
    })
}

export function markAppUpdatePopupShown(
    version: string,
    stage: AppUpdatePromptStage = 'available',
    now = Date.now(),
): void {
    const existing = readPromptState()
    const isSamePrompt = existing?.version === version && existing.stage === stage
    writePromptState({
        version,
        stage,
        actionableSince: isSamePrompt ? existing.actionableSince : now,
        lastUserActionAt: isSamePrompt ? existing.lastUserActionAt : undefined,
        popupShownAt: now,
    })
}

export function getNextAppUpdatePopupTime(
    version: string,
    stage: AppUpdatePromptStage = 'available',
): number | null {
    const existing = readPromptState()
    if (existing?.version !== version || existing.stage !== stage || existing.popupShownAt) {
        return null
    }

    return (
        Math.max(existing.actionableSince, existing.lastUserActionAt ?? 0) +
        APP_UPDATE_POPUP_DELAY_MS
    )
}

export function setAppUpdateActions(nextActions: AppUpdateActions): void {
    actions = nextActions
}

export async function checkForAppUpdate(): Promise<AppUpdateCheckResult> {
    return (await actions.check?.()) ?? 'disabled'
}

export async function downloadAvailableAppUpdate(): Promise<void> {
    recordAppUpdateUserAction(undefined, 'available')
    await actions.download?.()
}

export async function installAvailableAppUpdate(): Promise<void> {
    recordAppUpdateUserAction(undefined, 'downloaded')
    await actions.install?.()
}

export async function openAppUpdateChangelog(
    version = availableUpdate.value?.version,
): Promise<void> {
    recordAppUpdateUserAction()
    await actions.changelog?.(version)
}
