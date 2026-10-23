/** Port of `apps/app-frontend/src/pages/download-focus.ts` (deep links to a job). */
import type { DownloadJob as InstallJobSnapshot, InstallJobStatus } from './state'

export type DownloadsTab = 'active' | 'history'

export interface DownloadFocusState {
    jobId: string | null
    focused: boolean
    autoFollow: boolean
    lastStatus: InstallJobStatus | null
}

export interface DownloadFocusEffect {
    state: DownloadFocusState
    tab: DownloadsTab | null
    expand: boolean
    scroll: boolean
}

const ACTIVE_STATUSES = new Set<InstallJobStatus>([
    'queued',
    'running',
    'canceling',
    'waiting_for_user',
])

export function focusedDownloadJobId(value: unknown): string | null {
    return typeof value === 'string' && value.length > 0 ? value : null
}

export function createDownloadFocusState(jobId: string | null): DownloadFocusState {
    return {
        jobId,
        focused: false,
        autoFollow: jobId !== null,
        lastStatus: null,
    }
}

export function downloadTabForJob(job: InstallJobSnapshot): DownloadsTab {
    return ACTIVE_STATUSES.has(job.status) ? 'active' : 'history'
}

export function reconcileDownloadFocus(
    state: DownloadFocusState,
    job: InstallJobSnapshot | null,
): DownloadFocusEffect {
    if (!state.jobId || !job || job.job_id !== state.jobId) {
        return { state, tab: null, expand: false, scroll: false }
    }

    const targetTab = downloadTabForJob(job)
    if (!state.focused) {
        return {
            state: {
                ...state,
                focused: true,
                autoFollow: targetTab === 'active',
                lastStatus: job.status,
            },
            tab: targetTab,
            expand: true,
            scroll: true,
        }
    }

    const completedWhileFollowing =
        state.autoFollow &&
        state.lastStatus !== null &&
        ACTIVE_STATUSES.has(state.lastStatus) &&
        targetTab === 'history'
    return {
        state: {
            ...state,
            autoFollow: completedWhileFollowing ? false : state.autoFollow,
            lastStatus: job.status,
        },
        tab: completedWhileFollowing ? 'history' : null,
        expand: false,
        scroll: false,
    }
}

export function stopDownloadFocusAutoFollow(state: DownloadFocusState): DownloadFocusState {
    return { ...state, autoFollow: false }
}
