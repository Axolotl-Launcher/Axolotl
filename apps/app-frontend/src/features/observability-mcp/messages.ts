import { defineMessages } from '@modrinth/ui'

export const observabilityMcpMessages = defineMessages({
    enabled: {
        id: 'app.observability-mcp.enabled',
        defaultMessage: 'Enable the observability MCP',
    },
    enabledDescription: {
        id: 'app.observability-mcp.enabled-description',
        defaultMessage:
            'Keep the most recent telemetry in memory so a local AI client can read it. Nothing is uploaded, and turning this off drops everything that was kept.',
    },
    requireAuth: {
        id: 'app.observability-mcp.require-auth',
        defaultMessage: 'Require authentication',
    },
    requireAuthDescription: {
        id: 'app.observability-mcp.require-auth-description',
        defaultMessage:
            'Require clients to provide a connection token. Changing this setting restarts the listener.',
    },
    running: {
        id: 'app.observability-mcp.running',
        defaultMessage: 'Listening for connections',
    },
    notRunning: {
        id: 'app.observability-mcp.not-running',
        defaultMessage: 'The observability MCP could not start. Resolve the error below and retry.',
    },
    disabled: {
        id: 'app.observability-mcp.disabled',
        defaultMessage: 'The observability MCP is off and nothing is being kept.',
    },
    token: {
        id: 'app.observability-mcp.token',
        defaultMessage: 'Connection token',
    },
    copyToken: { id: 'app.observability-mcp.copy-token', defaultMessage: 'Copy token' },
    tokenCopied: {
        id: 'app.observability-mcp.token-copied',
        defaultMessage: 'Token copied',
    },
    copyFailed: {
        id: 'app.observability-mcp.copy-failed',
        defaultMessage: 'Could not copy the token. Try again.',
    },
    enableFirst: {
        id: 'app.observability-mcp.enable-first',
        defaultMessage: 'Enable the observability MCP to see the connection address.',
    },
    privacyNotice: {
        id: 'app.observability-mcp.privacy-notice',
        defaultMessage:
            'Captured telemetry stays in this process: recent events, aggregated counters and histogram summaries, and span names with durations. Values the launcher marks sensitive are never recorded, and the lists are bounded.',
    },
    claudeStep: {
        id: 'app.observability-mcp.claude-step',
        defaultMessage:
            'Run this command in a terminal. The tools only read telemetry, so no approval flow follows.',
    },
    codexStep: {
        id: 'app.observability-mcp.codex-step',
        defaultMessage:
            'Merge this into ~/.codex/config.toml, replacing any existing entry with the same name, then restart Codex.',
    },
    debugTitle: {
        id: 'app.observability-mcp.debug-title',
        defaultMessage: 'Read telemetry from an AI client',
    },
    debugDescription: {
        id: 'app.observability-mcp.debug-description',
        defaultMessage:
            'Start with observability_status, then list events, metrics or spans. These tools are read-only.',
    },
})
