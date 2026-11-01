import { onScopeDispose, type Ref, ref, shallowRef, watch } from 'vue'

import {
    download_diagnostics,
    type DownloadDiagnosticsSnapshot,
    type DownloadFileSummary,
    type DownloadTransferSnapshot,
} from '@/helpers/download-diagnostics'

export const SLOW_DOWNLOAD_BYTES_PER_SECOND = 256 * 1024

export function isSlowTransfer(transfer: DownloadTransferSnapshot): boolean {
    return (
        transfer.state === 'active' &&
        transfer.elapsed_ms >= 1000 &&
        transfer.speed_bytes_per_second != null &&
        transfer.speed_bytes_per_second < SLOW_DOWNLOAD_BYTES_PER_SECOND
    )
}

function newestId(first: { id: string }, second: { id: string }): number {
    return second.id.localeCompare(first.id, undefined, { numeric: true })
}

export function orderDiagnosticFiles(files: DownloadFileSummary[]): DownloadFileSummary[] {
    return [...files].sort(
        (a, b) =>
            Number(b.status === 'active') - Number(a.status === 'active') ||
            b.active_requests - a.active_requests ||
            b.received_bytes - a.received_bytes ||
            newestId(a, b),
    )
}

export function chooseDiagnosticFile(files: DownloadFileSummary[], current?: string): string {
    if (current && files.some((file) => file.id === current)) return current
    return orderDiagnosticFiles(files)[0]?.id ?? ''
}

export function orderDiagnosticTransfers(
    transfers: DownloadTransferSnapshot[],
    column?: string,
    direction: 'asc' | 'desc' = 'asc',
): DownloadTransferSnapshot[] {
    return [...transfers].sort((a, b) => {
        if (column === 'speed_bytes_per_second' || column === 'elapsed_ms') {
            const first = a[column]
            const second = b[column]
            // Unknown rates stay last in either direction, rather than looking like zero.
            if (first == null || second == null) {
                return Number(first == null) - Number(second == null) || newestId(a, b)
            }
            return (first - second) * (direction === 'asc' ? 1 : -1) || newestId(a, b)
        }
        const rank = { active: 0, connecting: 1, completed: 2, closed: 2 }
        return (
            rank[a.state] - rank[b.state] ||
            Number(isSlowTransfer(b)) - Number(isSlowTransfer(a)) ||
            newestId(a, b)
        )
    })
}

/** One bounded request at a time; stale responses cannot replace another file's details. */
export function useDownloadDiagnostics(jobId: Readonly<Ref<string>>) {
    const snapshot = shallowRef<DownloadDiagnosticsSnapshot>()
    const selectedFileId = ref('')
    const unavailable = ref(false)
    let generation = 0
    let timer: ReturnType<typeof setTimeout> | undefined
    let pending = false
    let disposed = false

    async function refresh() {
        if (disposed || pending) return
        const requestedGeneration = generation
        const requestedJobId = jobId.value
        pending = true
        try {
            const result = await download_diagnostics(
                requestedJobId,
                selectedFileId.value || undefined,
            )
            if (disposed || requestedGeneration !== generation) return
            snapshot.value = result
            unavailable.value = false
            const next = chooseDiagnosticFile(result.files, selectedFileId.value)
            if (next !== selectedFileId.value) selectedFileId.value = next
        } catch {
            if (!disposed && requestedGeneration === generation) unavailable.value = true
        } finally {
            pending = false
            if (!disposed) {
                clearTimeout(timer)
                timer = setTimeout(
                    () => void refresh(),
                    requestedGeneration === generation ? 1000 : 0,
                )
            }
        }
    }

    watch(
        [jobId, selectedFileId],
        ([job], previous) => {
            generation++
            clearTimeout(timer)
            if (previous && job !== previous[0]) {
                snapshot.value = undefined
                selectedFileId.value = ''
            }
            void refresh()
        },
        { immediate: true, flush: 'sync' },
    )
    onScopeDispose(() => {
        disposed = true
        generation++
        clearTimeout(timer)
    })
    return { snapshot, selectedFileId, unavailable }
}
