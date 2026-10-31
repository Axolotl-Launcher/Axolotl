import { invoke } from '@tauri-apps/api/core'

export interface DownloadFileSummary {
    id: string
    job_id: string | null
    item_id: string | null
    name: string
    status: 'active' | 'completed' | 'failed' | 'canceled'
    elapsed_ms: number
    network_elapsed_ms: number | null
    received_bytes: number
    configured_limit: number | null
    effective_cap: number | null
    target_connections: number | null
    held_permits: number | null
    active_requests: number
    effective_requests: number
    physical_connections: number | null
    effective_physical_connections: number | null
}

export interface DownloadTransferSnapshot {
    id: string
    identity_kind: 'http_request' | 'physical_h2'
    physical_id: number | null
    protocol: string
    remote_addr: string | null
    proxied: boolean
    state: 'connecting' | 'active' | 'completed' | 'closed'
    received_bytes: number
    speed_bytes_per_second: number | null
    elapsed_ms: number
}

export interface DownloadFileSample {
    elapsed_ms: number
    received_bytes: number
    active_requests: number
    effective_requests: number
    physical_connections: number | null
    effective_physical_connections: number | null
    effective_cap: number | null
    target_connections: number | null
}

export interface DownloadFileDetails {
    summary: DownloadFileSummary
    transfers: DownloadTransferSnapshot[]
    samples: DownloadFileSample[]
}

export interface DownloadDiagnosticsSnapshot {
    enabled: boolean
    pool_available: number | null
    files: DownloadFileSummary[]
    selected_file: DownloadFileDetails | null
    dropped_files: number
    dropped_transfers: number
}

export async function download_diagnostics(jobId: string, fileId?: string) {
    return await invoke<DownloadDiagnosticsSnapshot>('plugin:install|download_diagnostics', {
        jobId,
        fileId,
    })
}
