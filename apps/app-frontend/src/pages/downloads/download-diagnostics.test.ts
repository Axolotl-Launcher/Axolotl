import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { effectScope, ref } from 'vue'

import {
    download_diagnostics,
    type DownloadDiagnosticsSnapshot,
    type DownloadFileSummary,
    type DownloadTransferSnapshot,
} from '@/helpers/download-diagnostics'

import {
    chooseDiagnosticFile,
    isSlowTransfer,
    orderDiagnosticTransfers,
    useDownloadDiagnostics,
} from './download-diagnostics'

vi.mock('@/helpers/download-diagnostics', () => ({ download_diagnostics: vi.fn() }))

function file(id: string, active_requests = 0): DownloadFileSummary {
    return {
        id,
        job_id: 'job',
        item_id: id,
        name: `${id}.jar`,
        status: active_requests ? 'active' : 'completed',
        elapsed_ms: 5000,
        network_elapsed_ms: 4000,
        received_bytes: 1000,
        configured_limit: 64,
        effective_cap: 64,
        target_connections: 32,
        held_permits: 32,
        active_requests,
        effective_requests: active_requests,
        physical_connections: null,
        effective_physical_connections: null,
    }
}
function transfer(
    id: string,
    speed: number | null,
    state: DownloadTransferSnapshot['state'] = 'active',
): DownloadTransferSnapshot {
    return {
        id,
        identity_kind: 'physical_h2',
        physical_id: 1,
        protocol: 'h2',
        remote_addr: '192.0.2.1:443',
        proxied: false,
        state,
        received_bytes: 10000,
        speed_bytes_per_second: speed,
        elapsed_ms: 3000,
    }
}
function snapshot(fileId?: string): DownloadDiagnosticsSnapshot {
    const files = [file('1', 20), file('2', 2)]
    return {
        enabled: true,
        pool_available: 42,
        files,
        selected_file: fileId
            ? { summary: files.find((file) => file.id === fileId)!, transfers: [], samples: [] }
            : null,
        dropped_files: 0,
        dropped_transfers: 0,
    }
}

beforeEach(() => {
    vi.useFakeTimers()
    vi.mocked(download_diagnostics).mockReset()
})
afterEach(() => vi.useRealTimers())

it('initially selects the busiest active file and preserves an existing user selection', () => {
    const files = [file('99'), file('1', 32), file('2', 1)]
    expect(chooseDiagnosticFile(files)).toBe('1')
    expect(chooseDiagnosticFile(files, '2')).toBe('2')
    expect(chooseDiagnosticFile(files, '99')).toBe('99')
    expect(chooseDiagnosticFile(files, 'missing')).toBe('1')
})

it('marks only sampled active requests after warm-up as slow, including zero throughput', () => {
    expect(isSlowTransfer(transfer('1', 0))).toBe(true)
    expect(isSlowTransfer(transfer('1', null))).toBe(false)
    expect(isSlowTransfer(transfer('1', 256 * 1024))).toBe(false)
    expect(isSlowTransfer(transfer('1', 10, 'completed'))).toBe(false)
    expect(isSlowTransfer({ ...transfer('1', 10), elapsed_ms: 999 })).toBe(false)
})

it('sorts numeric speeds without treating unknown as zero or merging shared sockets', () => {
    const rows = [
        transfer('1', 1000),
        transfer('2', null),
        transfer('3', 20),
        transfer('4', 5, 'completed'),
    ]
    expect(orderDiagnosticTransfers(rows).map((row) => row.id)).toEqual(['3', '1', '2', '4'])
    expect(orderDiagnosticTransfers(rows, 'speed_bytes_per_second').map((row) => row.id)).toEqual([
        '4',
        '3',
        '1',
        '2',
    ])
    expect(
        orderDiagnosticTransfers(rows, 'speed_bytes_per_second', 'desc').map((row) => row.id),
    ).toEqual(['1', '3', '4', '2'])
    expect(rows).toHaveLength(4)
})

it('polls only summaries plus the selected file and stops when its scope closes', async () => {
    vi.mocked(download_diagnostics).mockImplementation(async (_, selected) => snapshot(selected))
    const scope = effectScope()
    const state = scope.run(() => useDownloadDiagnostics(ref('job')))!
    await vi.advanceTimersByTimeAsync(0)
    expect(state.selectedFileId.value).toBe('1')
    expect(download_diagnostics).toHaveBeenLastCalledWith('job', '1')
    state.selectedFileId.value = '2'
    await vi.advanceTimersByTimeAsync(0)
    expect(download_diagnostics).toHaveBeenLastCalledWith('job', '2')
    scope.stop()
    const calls = vi.mocked(download_diagnostics).mock.calls.length
    await vi.advanceTimersByTimeAsync(5000)
    expect(download_diagnostics).toHaveBeenCalledTimes(calls)
})

it('discards an old selection response and never overlaps polling requests', async () => {
    let resolvePending!: (value: DownloadDiagnosticsSnapshot) => void
    vi.mocked(download_diagnostics).mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                resolvePending = resolve
            }),
    )
    vi.mocked(download_diagnostics).mockImplementation(async (_, selected) => snapshot(selected))
    const scope = effectScope()
    const state = scope.run(() => useDownloadDiagnostics(ref('job')))!
    state.selectedFileId.value = '2'
    await vi.advanceTimersByTimeAsync(3000)
    expect(download_diagnostics).toHaveBeenCalledTimes(1)
    resolvePending(snapshot('1'))
    await vi.advanceTimersByTimeAsync(0)
    expect(state.snapshot.value?.selected_file?.summary.id).toBe('2')
    expect(download_diagnostics).toHaveBeenCalledTimes(2)
    scope.stop()
})

it('ignores a response after unmount rather than restarting its timer', async () => {
    let resolvePending!: (value: DownloadDiagnosticsSnapshot) => void
    vi.mocked(download_diagnostics).mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                resolvePending = resolve
            }),
    )
    const scope = effectScope()
    const state = scope.run(() => useDownloadDiagnostics(ref('job')))!
    scope.stop()
    resolvePending(snapshot())
    await vi.advanceTimersByTimeAsync(5000)
    expect(state.snapshot.value).toBeUndefined()
    expect(download_diagnostics).toHaveBeenCalledTimes(1)
})
