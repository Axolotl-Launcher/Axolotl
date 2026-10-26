import { defineMessages } from '@modrinth/ui'

export const mcpMessages = defineMessages({
    title: { id: 'app.settings.tabs.mcp', defaultMessage: 'MCP' },
    introduction: {
        id: 'app.mcp.introduction',
        defaultMessage:
            'Connect an AI client to manage your launcher through Model Context Protocol.',
    },
    previewNotice: {
        id: 'app.mcp.preview-notice',
        defaultMessage:
            'MCP connections are not available yet. Explore access settings and client setup below.',
    },
    accessTitle: { id: 'app.mcp.access-title', defaultMessage: 'Access permissions' },
    fullAccess: { id: 'app.mcp.full-access', defaultMessage: 'Full access mode' },
    fullAccessDescription: {
        id: 'app.mcp.full-access-description',
        defaultMessage:
            'Allow connections without approval, deletion tools, and direct control of the launcher interface.',
    },
    standardMode: { id: 'app.mcp.standard-mode', defaultMessage: 'Standard access' },
    fullMode: { id: 'app.mcp.full-mode', defaultMessage: 'Full access' },
    manualApproval: {
        id: 'app.mcp.manual-approval',
        defaultMessage: 'Approve each connection before an agent can act.',
    },
    noDeletion: { id: 'app.mcp.no-deletion', defaultMessage: 'Deletion tools are unavailable.' },
    toolsOnly: {
        id: 'app.mcp.tools-only',
        defaultMessage: 'Agents use launcher tools and cannot directly control the interface.',
    },
    fullModeNotice: {
        id: 'app.mcp.full-mode-notice',
        defaultMessage:
            'Agents can connect without approval, delete data, and directly control the interface. Only connect clients you trust.',
    },
    disconnectHint: {
        id: 'app.mcp.disconnect-hint',
        defaultMessage:
            'While connected, the window edge uses your accent color. Click “AI connected” in the title bar to disconnect at any time.',
    },
    warningTitle: { id: 'app.mcp.warning-title', defaultMessage: 'Enable full access mode?' },
    warningDescription: {
        id: 'app.mcp.warning-description',
        defaultMessage:
            'Connected agents will be able to act without connection approval, use deletion tools, and control the launcher interface. Mistakes or malicious instructions could delete worlds and instances, change files, or expose private data.',
    },
    warningSession: {
        id: 'app.mcp.warning-session',
        defaultMessage:
            'Changing access mode disconnects the current session. The next connection uses the new permissions.',
    },
    acknowledge: {
        id: 'app.mcp.acknowledge',
        defaultMessage: 'I understand the risks and trust the AI clients I connect.',
    },
    enableFullAccess: { id: 'app.mcp.enable-full-access', defaultMessage: 'Enable full access' },
    cancel: { id: 'app.mcp.cancel', defaultMessage: 'Cancel' },
    setupTitle: { id: 'app.mcp.setup-title', defaultMessage: 'Connect your AI client' },
    setupDescription: {
        id: 'app.mcp.setup-description',
        defaultMessage: 'Choose a client to view its setup instructions.',
    },
    endpointNotice: {
        id: 'app.mcp.endpoint-notice',
        defaultMessage:
            'The examples use MCP_URL as a placeholder. Replace it with the launcher’s service URL when MCP becomes available, then keep the launcher running.',
    },
    codexStep: {
        id: 'app.mcp.codex-step',
        defaultMessage:
            'Run this command in a terminal. Use codex mcp list to verify the server configuration.',
    },
    claudeStep: {
        id: 'app.mcp.claude-step',
        defaultMessage:
            'Run this command in a terminal. Open /mcp in Claude Code to check the connection.',
    },
    cursorStep: {
        id: 'app.mcp.cursor-step',
        defaultMessage:
            'Add this entry to ~/.cursor/mcp.json for all projects, or .cursor/mcp.json for the current project. Merge it with any existing mcpServers entries.',
    },
    vscodeStep: {
        id: 'app.mcp.vscode-step',
        defaultMessage:
            'Open the Command Palette and run “MCP: Add Server”. Choose HTTP, enter the launcher’s service URL, name it axolotl, and select a configuration location.',
    },
    afterSetup: {
        id: 'app.mcp.after-setup',
        defaultMessage:
            'Ask the agent to connect to the launcher before starting work. In standard access mode, approve the connection in the launcher.',
    },
    officialGuide: { id: 'app.mcp.official-guide', defaultMessage: 'Official setup guide' },
    clientLabel: { id: 'app.mcp.client-label', defaultMessage: 'AI client' },
    connectionTitle: { id: 'app.mcp.connection-title', defaultMessage: 'Allow AI to connect?' },
    connectionDescription: {
        id: 'app.mcp.connection-description',
        defaultMessage: '{client} is requesting access to your launcher.',
    },
    allowConnection: { id: 'app.mcp.allow-connection', defaultMessage: 'Allow connection' },
    denyConnection: { id: 'app.mcp.deny-connection', defaultMessage: 'Decline' },
    connected: { id: 'app.mcp.connected', defaultMessage: 'AI connected' },
    disconnect: { id: 'app.mcp.disconnect', defaultMessage: 'Disconnect {client}' },
    debugTitle: { id: 'app.mcp.debug-title', defaultMessage: 'MCP connection preview' },
    debugDescription: {
        id: 'app.mcp.debug-description',
        defaultMessage:
            'Simulate a connection request, task completion, or an interrupted connection. No external client is connected.',
    },
    debugClient: { id: 'app.mcp.debug-client', defaultMessage: 'Preview agent' },
    debugConnect: { id: 'app.mcp.debug-connect', defaultMessage: 'Simulate connection' },
    debugFinish: { id: 'app.mcp.debug-finish', defaultMessage: 'Simulate task completion' },
    debugInterrupt: { id: 'app.mcp.debug-interrupt', defaultMessage: 'Simulate connection loss' },
    debugIdle: { id: 'app.mcp.debug-idle', defaultMessage: 'Disconnected' },
    debugPending: { id: 'app.mcp.debug-pending', defaultMessage: 'Waiting for approval' },
    debugActive: { id: 'app.mcp.debug-active', defaultMessage: 'Connected · preview' },
})
