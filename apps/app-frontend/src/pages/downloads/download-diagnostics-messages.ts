import { defineMessages } from '@modrinth/ui'

export const diagnosticMetricHints = defineMessages({
    elapsed: {
        id: 'app.downloads.diagnostics.elapsed-help',
        defaultMessage:
            'Time spent processing this file, including resource waits, existing-file checks, transfer, writing and integrity verification. This is not the entire installation time.',
    },
    networkElapsed: {
        id: 'app.downloads.diagnostics.network-elapsed-help',
        defaultMessage:
            'Time from the first recorded request starting to the last request finishing, including connection setup, response waits and retry gaps. Parallel request times are not added together.',
    },
    received: {
        id: 'app.downloads.diagnostics.received-help',
        defaultMessage:
            'Total response body bytes received, excluding headers and TLS overhead. Retries may count duplicate bytes, so this can differ from the file size; cached files may show zero.',
    },
    activeRequests: {
        id: 'app.downloads.diagnostics.active-requests-help',
        defaultMessage:
            'Requests or HTTP/2 streams currently in the active state, excluding those still connecting. Active requests may be waiting without receiving data.',
    },
    effectiveRequests: {
        id: 'app.downloads.diagnostics.effective-requests-help',
        defaultMessage: 'Active requests that received bytes within the last 2 seconds.',
    },
    physicalConnections: {
        id: 'app.downloads.diagnostics.physical-connections-help',
        defaultMessage:
            'Distinct identifiable physical connections used by this file’s active requests. HTTP/2 streams may share a connection. Idle connections are excluded; — means the count is unknown.',
    },
    effectivePhysical: {
        id: 'app.downloads.diagnostics.effective-physical-help',
        defaultMessage:
            'Distinct identifiable connections with an active request that received bytes within the last 2 seconds. Shared connections count once; — means the count is unknown.',
    },
    configuredLimit: {
        id: 'app.downloads.diagnostics.configured-limit-help',
        defaultMessage:
            'Configured resource permit limit for the global download pool. Shared across files and tasks; this is not a physical connection count or a per-file allocation.',
    },
    effectiveCap: {
        id: 'app.downloads.diagnostics.effective-cap-help',
        defaultMessage:
            'Adaptive concurrency limit for this file, adjusted for remaining data, available resources and failures. Completed files retain the last reported value.',
    },
    target: {
        id: 'app.downloads.diagnostics.target-help',
        defaultMessage:
            'Concurrency the scheduler aims to maintain for this file, subject to available permits and splittable work. This is not the active request count; completed files retain the last value.',
    },
    held: {
        id: 'app.downloads.diagnostics.held-help',
        defaultMessage:
            'Download resource permits currently held by this file’s scheduler. These are not physical connections and are released when the file finishes.',
    },
    pool: {
        id: 'app.downloads.diagnostics.pool-help',
        defaultMessage:
            'Currently available resource permits in the global download pool, shared with other files and tasks. This is not the number of sockets the operating system can open.',
    },
})

export const diagnosticMessages = defineMessages({
    title: {
        id: 'app.downloads.diagnostics.title',
        defaultMessage: 'Advanced download information',
    },
    file: { id: 'app.downloads.diagnostics.file', defaultMessage: 'File' },
    waiting: {
        id: 'app.downloads.diagnostics.waiting',
        defaultMessage:
            'Waiting for download observations. Start a download after enabling this option.',
    },
    loading: {
        id: 'app.downloads.diagnostics.loading',
        defaultMessage: 'Loading download information…',
    },
    unavailable: {
        id: 'app.downloads.diagnostics.unavailable',
        defaultMessage: 'Download information could not be refreshed. Retrying…',
    },
    disabled: {
        id: 'app.downloads.diagnostics.disabled',
        defaultMessage: 'Download observation is disabled.',
    },
    elapsed: { id: 'app.downloads.diagnostics.elapsed', defaultMessage: 'File elapsed time' },
    networkElapsed: {
        id: 'app.downloads.diagnostics.network-elapsed',
        defaultMessage: 'Network time',
    },
    received: { id: 'app.downloads.diagnostics.received', defaultMessage: 'Received bytes' },
    activeRequests: {
        id: 'app.downloads.diagnostics.active-requests',
        defaultMessage: 'Active requests',
    },
    effectiveRequests: {
        id: 'app.downloads.diagnostics.effective-requests',
        defaultMessage: 'Requests receiving data',
    },
    physicalConnections: {
        id: 'app.downloads.diagnostics.physical-connections',
        defaultMessage: 'Physical connections',
    },
    effectivePhysical: {
        id: 'app.downloads.diagnostics.effective-physical',
        defaultMessage: 'Connections receiving data',
    },
    configuredLimit: {
        id: 'app.downloads.diagnostics.configured-limit',
        defaultMessage: 'Configured limit',
    },
    effectiveCap: {
        id: 'app.downloads.diagnostics.effective-cap',
        defaultMessage: 'Current request limit',
    },
    target: { id: 'app.downloads.diagnostics.target', defaultMessage: 'Target requests' },
    held: { id: 'app.downloads.diagnostics.held', defaultMessage: 'Held permits' },
    pool: { id: 'app.downloads.diagnostics.pool', defaultMessage: 'Available pool permits' },
    address: { id: 'app.downloads.diagnostics.address', defaultMessage: 'Remote address' },
    protocol: { id: 'app.downloads.diagnostics.protocol', defaultMessage: 'Protocol / socket' },
    speed: { id: 'app.downloads.diagnostics.speed', defaultMessage: 'Request speed' },
    duration: { id: 'app.downloads.diagnostics.duration', defaultMessage: 'Request time' },
    state: { id: 'app.downloads.diagnostics.state', defaultMessage: 'State' },
    slow: { id: 'app.downloads.diagnostics.slow', defaultMessage: 'Slow' },
    proxy: { id: 'app.downloads.diagnostics.proxy', defaultMessage: 'Proxy' },
    socket: { id: 'app.downloads.diagnostics.socket', defaultMessage: 'Socket #{id}' },
    seconds: { id: 'app.downloads.diagnostics.seconds', defaultMessage: '{seconds} s' },
    rate: { id: 'app.downloads.diagnostics.rate', defaultMessage: '{speed}/s' },
    priority: { id: 'app.downloads.diagnostics.priority', defaultMessage: 'Active / slow first' },
    fileSelectionGuide: {
        id: 'app.downloads.diagnostics.fileSelectionGuide',
        defaultMessage: 'Choose a file, and its download summary will appear below.',
    },
    note: {
        id: 'app.downloads.diagnostics.note',
        defaultMessage:
            'Each row is a request or HTTP/2 stream; streams may share a socket. Unknown physical connection counts are shown as —. Active speeds use recent samples; finished speeds are averages. Slow means below 256 KiB/s after warm-up. Receiving data means bytes arrived within the last 2 seconds.',
    },
    truncated: {
        id: 'app.downloads.diagnostics.truncated',
        defaultMessage:
            'Observation limits reached: {files} earlier files and {transfers} earlier requests are omitted.',
    },
    noRequests: {
        id: 'app.downloads.diagnostics.no-requests',
        defaultMessage: 'No requests observed for this file yet.',
    },
    active: { id: 'app.downloads.diagnostics.active', defaultMessage: 'Active' },
    connecting: { id: 'app.downloads.diagnostics.connecting', defaultMessage: 'Connecting' },
    completed: { id: 'app.downloads.diagnostics.completed', defaultMessage: 'Completed' },
    closed: { id: 'app.downloads.diagnostics.closed', defaultMessage: 'Closed' },
    failed: { id: 'app.downloads.diagnostics.failed', defaultMessage: 'Failed' },
    canceled: { id: 'app.downloads.diagnostics.canceled', defaultMessage: 'Canceled' },
})
