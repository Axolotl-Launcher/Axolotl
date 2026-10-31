import { defineMessages } from '@modrinth/ui'

export const agentWindowMessages = defineMessages({
    permissionsTitle: {
        id: 'app.agent-window.chat.permissions-title',
        defaultMessage: 'Permissions',
    },
    fullAccess: { id: 'app.agent-window.chat.full-access', defaultMessage: 'Full access' },
    fullAccessHint: {
        id: 'app.agent-window.chat.full-access-hint',
        defaultMessage: 'Allow high-impact actions such as deleting content.',
    },
    computerUse: { id: 'app.agent-window.chat.computer-use', defaultMessage: 'Interface control' },
    computerUseHint: {
        id: 'app.agent-window.chat.computer-use-hint',
        defaultMessage: 'Observe and operate the launcher only when you explicitly request it.',
    },
    permissionPending: {
        id: 'app.agent-window.chat.permission-pending',
        defaultMessage: 'Loading permissions…',
    },
    permissionFuture: {
        id: 'app.agent-window.chat.permission-future',
        defaultMessage: 'Permissions reset when you restart the launcher.',
    },
    permissionUnavailable: {
        id: 'app.agent-window.chat.permission-unavailable',
        defaultMessage:
            'The execution service is not connected. Permissions cannot be changed yet.',
    },
    permissionError: {
        id: 'app.agent-window.chat.permission-error',
        defaultMessage: 'Could not load permissions. Reopen settings to retry.',
    },
    permissionSaveError: {
        id: 'app.agent-window.chat.permission-save-error',
        defaultMessage: 'Could not save permissions. Your previous settings are kept.',
    },
    fullAccessWarningTitle: {
        id: 'app.agent-window.chat.full-access-warning-title',
        defaultMessage: 'Enable full access?',
    },
    fullAccessWarning: {
        id: 'app.agent-window.chat.full-access-warning',
        defaultMessage:
            'The agent will be able to delete content and modify launcher data when execution is available. Files may be lost. Enable this only for tasks you trust.',
    },
    enableFullAccess: {
        id: 'app.agent-window.chat.enable-full-access',
        defaultMessage: 'Enable full access',
    },
    agent: { id: 'app.agent-window.chat.agent', defaultMessage: 'Agent' },
    newChat: { id: 'app.agent-window.chat.new-chat', defaultMessage: 'New task' },
    search: { id: 'app.agent-window.chat.search', defaultMessage: 'Search tasks' },
    conversations: { id: 'app.agent-window.chat.conversations', defaultMessage: 'Task history' },
    noChats: { id: 'app.agent-window.chat.no-chats', defaultMessage: 'No tasks yet' },
    noResults: { id: 'app.agent-window.chat.no-results', defaultMessage: 'No matching tasks' },
    today: { id: 'app.agent-window.chat.today', defaultMessage: 'Today' },
    earlier: { id: 'app.agent-window.chat.earlier', defaultMessage: 'Earlier' },
    toggleSidebar: {
        id: 'app.agent-window.chat.toggle-sidebar',
        defaultMessage: 'Toggle conversations',
    },
    settings: { id: 'app.agent-window.chat.settings', defaultMessage: 'Settings' },
    guide: { id: 'app.agent-window.chat.guide', defaultMessage: 'Agent guide' },
    guideTaskTitle: {
        id: 'app.agent-window.chat.guide-task-title',
        defaultMessage: 'Start with a task',
    },
    guideTaskBody: {
        id: 'app.agent-window.chat.guide-task-body',
        defaultMessage:
            'Create a task or choose a suggestion, then describe the result you want. Choose a text model configured in launcher AI settings before sending.',
    },
    guideInstanceTitle: {
        id: 'app.agent-window.chat.guide-instance-title',
        defaultMessage: 'Choose the scope',
    },
    guideInstanceBody: {
        id: 'app.agent-window.chat.guide-instance-body',
        defaultMessage:
            'Select an instance when your request concerns its mods, logs or worlds. Your instance and model choices are saved with each request.',
    },
    guideQueueTitle: {
        id: 'app.agent-window.chat.guide-queue-title',
        defaultMessage: 'Understand the queue',
    },
    guideQueueBody: {
        id: 'app.agent-window.chat.guide-queue-body',
        defaultMessage:
            'The task panel shows requests waiting for the execution service. Queued requests have not run; you can remove them. Interrupted requests are kept in history and do not restart automatically.',
    },
    guidePermissionsTitle: {
        id: 'app.agent-window.chat.guide-permissions-title',
        defaultMessage: 'Review permissions',
    },
    guidePermissionsBody: {
        id: 'app.agent-window.chat.guide-permissions-body',
        defaultMessage:
            'Open Settings to review Full access and Interface control. Full access permits high-impact actions such as deletion. Enable Interface control only when you want the agent to interact with the launcher interface.',
    },
    taskPanel: { id: 'app.agent-window.chat.task-panel', defaultMessage: 'Task panel' },
    closePanel: { id: 'app.agent-window.chat.close-panel', defaultMessage: 'Close task panel' },
    untitled: { id: 'app.agent-window.chat.untitled', defaultMessage: 'New task' },
    welcome: { id: 'app.agent-window.chat.welcome', defaultMessage: 'What would you like to do?' },
    suggestCheck: { id: 'app.agent-window.chat.suggest-check', defaultMessage: 'Check updates' },
    suggestCrash: { id: 'app.agent-window.chat.suggest-crash', defaultMessage: 'Diagnose a crash' },
    suggestBackup: { id: 'app.agent-window.chat.suggest-backup', defaultMessage: 'Back up worlds' },
    promptCheck: {
        id: 'app.agent-window.chat.prompt-check',
        defaultMessage:
            'Check the current instance for mod updates and show me a change plan first.',
    },
    promptCrash: {
        id: 'app.agent-window.chat.prompt-crash',
        defaultMessage:
            'Analyze the current instance’s latest crash, identify the cause and suggest a fix.',
    },
    promptBackup: {
        id: 'app.agent-window.chat.prompt-backup',
        defaultMessage: 'Help me back up the current instance’s game worlds.',
    },
    prompt: {
        id: 'app.agent-window.chat.prompt',
        defaultMessage: 'Describe the task you want to complete…',
    },
    messageLabel: { id: 'app.agent-window.chat.message-label', defaultMessage: 'Message' },
    send: { id: 'app.agent-window.chat.send', defaultMessage: 'Send message' },
    stop: { id: 'app.agent-window.chat.stop', defaultMessage: 'Cancel queued request' },
    model: { id: 'app.agent-window.chat.model', defaultMessage: 'Select model' },
    noModel: { id: 'app.agent-window.chat.no-model', defaultMessage: 'Select model' },
    modelSearch: {
        id: 'app.agent-window.chat.model-search',
        defaultMessage: 'Search models or providers',
    },
    noModels: { id: 'app.agent-window.chat.no-models', defaultMessage: 'No models available' },
    configureModel: {
        id: 'app.agent-window.chat.configure-model',
        defaultMessage: 'Enable a provider and models in the launcher’s AI settings.',
    },
    noModelMatches: {
        id: 'app.agent-window.chat.no-model-matches',
        defaultMessage: 'No matching models',
    },
    instance: { id: 'app.agent-window.chat.instance', defaultMessage: 'Select instance' },
    allLauncher: { id: 'app.agent-window.chat.all-launcher', defaultMessage: 'Entire launcher' },
    allLauncherHint: {
        id: 'app.agent-window.chat.all-launcher-hint',
        defaultMessage: 'Not limited to one instance',
    },
    noInstances: {
        id: 'app.agent-window.chat.no-instances',
        defaultMessage: 'No instances available',
    },
    instanceSearch: {
        id: 'app.agent-window.chat.instance-search',
        defaultMessage: 'Search instances',
    },
    noInstanceMatches: {
        id: 'app.agent-window.chat.no-instance-matches',
        defaultMessage: 'No matching instances',
    },
    waiting: { id: 'app.agent-window.chat.waiting', defaultMessage: 'Added to the local queue' },
    executionDisconnected: {
        id: 'app.agent-window.chat.execution-disconnected',
        defaultMessage: 'Execution service is not connected',
    },
    waitingHint: {
        id: 'app.agent-window.chat.waiting-hint',
        defaultMessage: 'Your request is saved. Execution has not started.',
    },
    stopped: { id: 'app.agent-window.chat.stopped', defaultMessage: 'Queued request cancelled' },
    interrupted: {
        id: 'app.agent-window.chat.interrupted',
        defaultMessage: 'Previous queue interrupted',
    },
    queued: { id: 'app.agent-window.chat.queued', defaultMessage: 'Pending requests' },
    queuedEmpty: {
        id: 'app.agent-window.chat.queued-empty',
        defaultMessage: 'No pending requests',
    },
    plan: { id: 'app.agent-window.chat.plan', defaultMessage: 'Execution plan' },
    noPlan: {
        id: 'app.agent-window.chat.no-plan',
        defaultMessage: 'The plan will appear when the task starts',
    },
    tools: { id: 'app.agent-window.chat.tools', defaultMessage: 'Tool calls' },
    noTools: { id: 'app.agent-window.chat.no-tools', defaultMessage: 'No tool calls yet' },
    context: { id: 'app.agent-window.chat.context', defaultMessage: 'Task context' },
    provider: { id: 'app.agent-window.chat.provider', defaultMessage: 'Provider' },
    modelLabel: { id: 'app.agent-window.chat.model-label', defaultMessage: 'Model' },
    unselected: { id: 'app.agent-window.chat.unselected', defaultMessage: 'Not selected' },
    contextError: {
        id: 'app.agent-window.chat.context-error',
        defaultMessage: 'Could not load launcher configuration',
    },
    retry: { id: 'app.agent-window.chat.retry', defaultMessage: 'Retry' },
    close: { id: 'app.agent-window.chat.close', defaultMessage: 'Close' },
    storageError: {
        id: 'app.agent-window.chat.storage-error',
        defaultMessage:
            'Could not save the conversation. Check available storage. Your input is kept.',
    },
    storageLoadError: {
        id: 'app.agent-window.chat.storage-load-error',
        defaultMessage: 'Could not read saved conversations. New messages may not be saved.',
    },
    selectModelFirst: {
        id: 'app.agent-window.chat.select-model-first',
        defaultMessage: 'Select an available model first',
    },
    aiDisabled: {
        id: 'app.agent-window.chat.ai-disabled',
        defaultMessage: 'AI is disabled in the launcher',
    },
    aiDisabledHint: {
        id: 'app.agent-window.chat.ai-disabled-hint',
        defaultMessage: 'Enable AI in launcher settings to select a model.',
    },
    shortcut: {
        id: 'app.agent-window.chat.shortcut',
        defaultMessage: 'Enter to send · Shift + Enter for a new line',
    },
    localHistory: {
        id: 'app.agent-window.chat.local-history',
        defaultMessage: 'Conversations stay on this device',
    },
    rename: { id: 'app.agent-window.chat.rename', defaultMessage: 'Rename conversation' },
    deleteChat: { id: 'app.agent-window.chat.delete-chat', defaultMessage: 'Delete conversation' },
    deleteTitle: {
        id: 'app.agent-window.chat.delete-title',
        defaultMessage: 'Delete this conversation?',
    },
    deleteBody: {
        id: 'app.agent-window.chat.delete-body',
        defaultMessage: 'This conversation’s records will be removed from this device.',
    },
    cancel: { id: 'app.agent-window.chat.cancel', defaultMessage: 'Cancel' },
    delete: { id: 'app.agent-window.chat.delete', defaultMessage: 'Delete' },
    name: { id: 'app.agent-window.chat.name', defaultMessage: 'Name' },
    save: { id: 'app.agent-window.chat.save', defaultMessage: 'Save' },
    more: { id: 'app.agent-window.chat.more', defaultMessage: 'More actions' },
    noContext: { id: 'app.agent-window.chat.no-context', defaultMessage: 'No instance selected' },
    byok: { id: 'app.agent-window.chat.byok', defaultMessage: 'Use your own model credentials' },
    state: { id: 'app.agent-window.chat.state', defaultMessage: 'Status' },
    details: { id: 'app.agent-window.chat.details', defaultMessage: 'View task' },
    source: { id: 'app.agent-window.chat.source', defaultMessage: 'Launcher configuration' },
    copied: { id: 'app.agent-window.chat.copied', defaultMessage: 'Message copied' },
    copy: { id: 'app.agent-window.chat.copy', defaultMessage: 'Copy message' },
    copyError: { id: 'app.agent-window.chat.copy-error', defaultMessage: 'Could not copy message' },
    contextRemoved: {
        id: 'app.agent-window.chat.context-removed',
        defaultMessage: 'This instance is currently unavailable',
    },
    modelRemoved: {
        id: 'app.agent-window.chat.model-removed',
        defaultMessage: 'This model is currently unavailable',
    },
    storageRecovery: {
        id: 'app.agent-window.chat.storage-recovery',
        defaultMessage: 'Saved history could not be read. Export it before starting a new history.',
    },
    exportHistory: {
        id: 'app.agent-window.chat.export-history',
        defaultMessage: 'Export saved history',
    },
    resetHistory: {
        id: 'app.agent-window.chat.reset-history',
        defaultMessage: 'Start new history',
    },
    exportDraft: { id: 'app.agent-window.chat.export-draft', defaultMessage: 'Export draft' },
    storageExportError: {
        id: 'app.agent-window.chat.storage-export-error',
        defaultMessage: 'Could not export saved history. Your original data has been kept.',
    },
    resetHistoryWarning: {
        id: 'app.agent-window.chat.reset-history-warning',
        defaultMessage:
            'The saved history will be replaced. Export a backup first. Your current draft will be kept.',
    },
    historyRetention: {
        id: 'app.agent-window.chat.history-retention',
        defaultMessage:
            'Older completed requests may be removed to make room. Drafts and waiting requests are kept.',
    },
    windowRevealError: {
        id: 'app.agent-window.chat.window-reveal-error',
        defaultMessage: 'Could not show the window. Reopen it from the launcher.',
    },
    windowActionError: {
        id: 'app.agent-window.chat.window-action-error',
        defaultMessage: 'Window action failed',
    },
})
