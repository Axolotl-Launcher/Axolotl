import { defineMessages, type MessageDescriptor } from '@modrinth/ui'

export const agentWindowMessages = defineMessages({
    toggleSidebar: {
        id: 'app.agent-window.chat.toggle-sidebar',
        defaultMessage: 'Show or hide the task list',
    },
    help: { id: 'app.agent-window.chat.help', defaultMessage: 'How it works' },
    settings: { id: 'app.agent-window.chat.settings', defaultMessage: 'Settings' },
    previewBadge: { id: 'app.agent-window.chat.preview-badge', defaultMessage: 'Preview' },
    previewTitle: {
        id: 'app.agent-window.chat.preview-title',
        defaultMessage: 'This is a preview',
    },
    previewBody: {
        id: 'app.agent-window.chat.preview-body',
        defaultMessage:
            'The agent isn’t connected to the launcher yet. Its steps and results are simulated to show how tasks work. Apart from your instance and model lists, nothing in the launcher is read or changed.',
    },
    windowRevealError: {
        id: 'app.agent-window.chat.window-reveal-error',
        defaultMessage: 'Could not show the window. Reopen it from the launcher.',
    },
    windowActionError: {
        id: 'app.agent-window.chat.window-action-error',
        defaultMessage: 'Window action failed',
    },
    newChat: { id: 'app.agent-window.chat.new-chat', defaultMessage: 'New task' },
    search: { id: 'app.agent-window.chat.search', defaultMessage: 'Search tasks' },
    conversations: { id: 'app.agent-window.chat.conversations', defaultMessage: 'Task history' },
    noChats: {
        id: 'app.agent-window.chat.no-chats',
        defaultMessage: 'Your tasks will appear here',
    },
    noResults: { id: 'app.agent-window.chat.no-results', defaultMessage: 'No matching tasks' },
    today: { id: 'app.agent-window.chat.today', defaultMessage: 'Today' },
    yesterday: { id: 'app.agent-window.chat.yesterday', defaultMessage: 'Yesterday' },
    earlier: { id: 'app.agent-window.chat.earlier', defaultMessage: 'Earlier' },
    statusActive: { id: 'app.agent-window.chat.status-active', defaultMessage: 'Working' },
    statusAwaiting: {
        id: 'app.agent-window.chat.status-awaiting',
        defaultMessage: 'Waiting for your approval',
    },
    statusAttention: {
        id: 'app.agent-window.chat.status-attention',
        defaultMessage: 'Needs attention',
    },
    more: { id: 'app.agent-window.chat.more', defaultMessage: 'More actions' },
    rename: { id: 'app.agent-window.chat.rename', defaultMessage: 'Rename' },
    deleteChat: { id: 'app.agent-window.chat.delete-chat', defaultMessage: 'Delete' },
    untitled: { id: 'app.agent-window.chat.untitled', defaultMessage: 'New task' },
    taskPanel: { id: 'app.agent-window.chat.task-panel', defaultMessage: 'Progress' },
    closePanel: { id: 'app.agent-window.chat.close-panel', defaultMessage: 'Close progress' },
    panelEmpty: {
        id: 'app.agent-window.chat.panel-empty',
        defaultMessage: 'Progress for this task appears here once it starts.',
    },
    panelSteps: { id: 'app.agent-window.chat.panel-steps', defaultMessage: 'Steps' },
    panelChanges: { id: 'app.agent-window.chat.panel-changes', defaultMessage: 'Changes' },
    panelLog: { id: 'app.agent-window.chat.panel-log', defaultMessage: 'Activity' },
    context: { id: 'app.agent-window.chat.context', defaultMessage: 'Runs with' },
    instanceLabel: { id: 'app.agent-window.chat.instance-label', defaultMessage: 'Instance' },
    modelLabel: { id: 'app.agent-window.chat.model-label', defaultMessage: 'Model' },
    permissionLabel: {
        id: 'app.agent-window.chat.permission-label',
        defaultMessage: 'Permissions',
    },
    askFirst: { id: 'app.agent-window.chat.ask-first', defaultMessage: 'Ask before changes' },
    unselected: { id: 'app.agent-window.chat.unselected', defaultMessage: 'Not selected' },
    stateWorking: { id: 'app.agent-window.chat.state-working', defaultMessage: 'Working' },
    stateApplying: {
        id: 'app.agent-window.chat.state-applying',
        defaultMessage: 'Applying changes',
    },
    stateAwaiting: {
        id: 'app.agent-window.chat.state-awaiting',
        defaultMessage: 'Needs your approval',
    },
    stateDone: { id: 'app.agent-window.chat.state-done', defaultMessage: 'Done' },
    stateFailed: { id: 'app.agent-window.chat.state-failed', defaultMessage: 'Failed' },
    stateStopped: { id: 'app.agent-window.chat.state-stopped', defaultMessage: 'Stopped' },
    stateDetached: { id: 'app.agent-window.chat.state-detached', defaultMessage: 'Result unknown' },
    stateDeclined: { id: 'app.agent-window.chat.state-declined', defaultMessage: 'Declined' },
    stateSuperseded: { id: 'app.agent-window.chat.state-superseded', defaultMessage: 'Replaced' },
    stateInterrupted: {
        id: 'app.agent-window.chat.state-interrupted',
        defaultMessage: 'Interrupted',
    },
    welcome: { id: 'app.agent-window.chat.welcome', defaultMessage: 'What can I help you with?' },
    suggestCrash: { id: 'app.agent-window.chat.suggest-crash', defaultMessage: 'Fix a crash' },
    suggestSmooth: {
        id: 'app.agent-window.chat.suggest-smooth',
        defaultMessage: 'Make the game smoother',
    },
    suggestBackup: {
        id: 'app.agent-window.chat.suggest-backup',
        defaultMessage: 'Back up a world',
    },
    suggestPack: { id: 'app.agent-window.chat.suggest-pack', defaultMessage: 'Set up a modpack' },
    promptCrash: {
        id: 'app.agent-window.chat.prompt-crash',
        defaultMessage: 'My game crashes on launch. Find out why and suggest a fix.',
    },
    promptSmooth: {
        id: 'app.agent-window.chat.prompt-smooth',
        defaultMessage: 'My game stutters. Check my mods and suggest how to make it run smoother.',
    },
    promptBackup: {
        id: 'app.agent-window.chat.prompt-backup',
        defaultMessage: 'Back up my survival world before I change anything.',
    },
    promptPack: {
        id: 'app.agent-window.chat.prompt-pack',
        defaultMessage: 'Set up a lightweight survival modpack that runs well with 6 GB of memory.',
    },
    introTitle: { id: 'app.agent-window.chat.intro-title', defaultMessage: 'How it works' },
    introDismiss: { id: 'app.agent-window.chat.intro-dismiss', defaultMessage: 'Got it' },
    introDescribeTitle: {
        id: 'app.agent-window.chat.intro-describe-title',
        defaultMessage: 'Describe it',
    },
    introDescribeBody: {
        id: 'app.agent-window.chat.intro-describe-body',
        defaultMessage: 'Say what you want in your own words.',
    },
    introReviewTitle: {
        id: 'app.agent-window.chat.intro-review-title',
        defaultMessage: 'Review the plan',
    },
    introReviewBody: {
        id: 'app.agent-window.chat.intro-review-body',
        defaultMessage: 'The agent checks your setup and lists exactly what it would change.',
    },
    introApproveTitle: {
        id: 'app.agent-window.chat.intro-approve-title',
        defaultMessage: 'You decide',
    },
    introApproveBody: {
        id: 'app.agent-window.chat.intro-approve-body',
        defaultMessage: 'Nothing changes until you approve. You can stop it at any time.',
    },
    previewNote: {
        id: 'app.agent-window.chat.preview-note',
        defaultMessage:
            'Preview: steps and results are simulated. Nothing in your launcher changes.',
    },
    messageLabel: { id: 'app.agent-window.chat.message-label', defaultMessage: 'Message' },
    prompt: {
        id: 'app.agent-window.chat.prompt',
        defaultMessage: 'Describe what you want, like “my game crashes on launch”',
    },
    promptFollowup: {
        id: 'app.agent-window.chat.prompt-followup',
        defaultMessage: 'Ask a follow-up…',
    },
    promptQueued: {
        id: 'app.agent-window.chat.prompt-queued',
        defaultMessage: 'Add a follow-up. It’s sent when the agent finishes.',
    },
    promptAwaiting: {
        id: 'app.agent-window.chat.prompt-awaiting',
        defaultMessage: 'Ask about the plan or request changes…',
    },
    attach: { id: 'app.agent-window.chat.attach', defaultMessage: 'Attach files' },
    attachmentLimit: {
        id: 'app.agent-window.chat.attachment-limit',
        defaultMessage: 'You can attach up to {count} files',
    },
    removeAttachment: {
        id: 'app.agent-window.chat.remove-attachment',
        defaultMessage: 'Remove {name}',
    },
    instance: { id: 'app.agent-window.chat.instance', defaultMessage: 'Choose an instance' },
    allLauncher: { id: 'app.agent-window.chat.all-launcher', defaultMessage: 'Entire launcher' },
    allLauncherHint: {
        id: 'app.agent-window.chat.all-launcher-hint',
        defaultMessage: 'Not limited to one instance',
    },
    instances: { id: 'app.agent-window.chat.instances', defaultMessage: 'Instances' },
    instanceSearch: {
        id: 'app.agent-window.chat.instance-search',
        defaultMessage: 'Search instances',
    },
    noInstances: { id: 'app.agent-window.chat.no-instances', defaultMessage: 'No instances yet' },
    noInstanceMatches: {
        id: 'app.agent-window.chat.no-instance-matches',
        defaultMessage: 'No matching instances',
    },
    contextRemoved: {
        id: 'app.agent-window.chat.context-removed',
        defaultMessage: 'Instance unavailable',
    },
    model: { id: 'app.agent-window.chat.model', defaultMessage: 'Choose a model' },
    noModel: { id: 'app.agent-window.chat.no-model', defaultMessage: 'Choose a model' },
    setupModel: { id: 'app.agent-window.chat.setup-model', defaultMessage: 'Set up a model' },
    modelRemoved: {
        id: 'app.agent-window.chat.model-removed',
        defaultMessage: 'Model unavailable',
    },
    modelSearch: {
        id: 'app.agent-window.chat.model-search',
        defaultMessage: 'Search models or providers',
    },
    noModels: { id: 'app.agent-window.chat.no-models', defaultMessage: 'No models enabled' },
    aiDisabled: { id: 'app.agent-window.chat.ai-disabled', defaultMessage: 'AI is turned off' },
    setupModelHint: {
        id: 'app.agent-window.chat.setup-model-hint',
        defaultMessage:
            'The agent uses your own AI provider. In Axolotl, open Settings → AI, turn AI on, add a provider and enable a model, then check again.',
    },
    checkAgain: { id: 'app.agent-window.chat.check-again', defaultMessage: 'Check again' },
    noModelMatches: {
        id: 'app.agent-window.chat.no-model-matches',
        defaultMessage: 'No matching models',
    },
    byok: {
        id: 'app.agent-window.chat.byok',
        defaultMessage: 'Models and keys come from the launcher’s AI settings',
    },
    send: { id: 'app.agent-window.chat.send', defaultMessage: 'Send' },
    stop: { id: 'app.agent-window.chat.stop', defaultMessage: 'Stop' },
    queueMessage: {
        id: 'app.agent-window.chat.queue-message',
        defaultMessage: 'Send when the agent finishes',
    },
    queuedTag: { id: 'app.agent-window.chat.queued-tag', defaultMessage: 'Queued' },
    sendNow: { id: 'app.agent-window.chat.send-now', defaultMessage: 'Send now' },
    removeQueued: {
        id: 'app.agent-window.chat.remove-queued',
        defaultMessage: 'Remove from queue',
    },
    jumpLatest: { id: 'app.agent-window.chat.jump-latest', defaultMessage: 'Jump to latest' },
    copy: { id: 'app.agent-window.chat.copy', defaultMessage: 'Copy message' },
    copyAnswer: { id: 'app.agent-window.chat.copy-answer', defaultMessage: 'Copy answer' },
    copied: { id: 'app.agent-window.chat.copied', defaultMessage: 'Copied' },
    copyError: { id: 'app.agent-window.chat.copy-error', defaultMessage: 'Couldn’t copy' },
    thinking: { id: 'app.agent-window.chat.thinking', defaultMessage: 'Thinking…' },
    working: { id: 'app.agent-window.chat.working', defaultMessage: 'Working…' },
    applying: { id: 'app.agent-window.chat.applying', defaultMessage: 'Applying changes…' },
    escToStop: { id: 'app.agent-window.chat.esc-to-stop', defaultMessage: 'to stop' },
    stepsDone: { id: 'app.agent-window.chat.steps-done', defaultMessage: '{count} steps' },
    planKicker: { id: 'app.agent-window.chat.plan-kicker', defaultMessage: 'Proposed changes' },
    manualChange: { id: 'app.agent-window.chat.manual-change', defaultMessage: 'Manual' },
    approvalPromise: {
        id: 'app.agent-window.chat.approval-promise',
        defaultMessage: 'Nothing changes until you approve',
    },
    approvalWaiting: {
        id: 'app.agent-window.chat.approval-waiting',
        defaultMessage: 'Waiting for your approval',
    },
    approve: { id: 'app.agent-window.chat.approve', defaultMessage: 'Approve and apply' },
    decline: { id: 'app.agent-window.chat.decline', defaultMessage: 'Decline' },
    applyingProgress: {
        id: 'app.agent-window.chat.applying-progress',
        defaultMessage: 'Applying changes · {done}/{total}',
    },
    planApplied: { id: 'app.agent-window.chat.plan-applied', defaultMessage: 'Applied' },
    planDeclined: {
        id: 'app.agent-window.chat.plan-declined',
        defaultMessage: 'Declined. Nothing was changed.',
    },
    planSuperseded: {
        id: 'app.agent-window.chat.plan-superseded',
        defaultMessage: 'Replaced by your newer message',
    },
    planUnknown: {
        id: 'app.agent-window.chat.plan-unknown',
        defaultMessage: 'Interrupted while applying. Check the status to see what was written.',
    },
    previewResult: {
        id: 'app.agent-window.chat.preview-result',
        defaultMessage: 'Simulated in preview · nothing was changed',
    },
    noticeStopped: {
        id: 'app.agent-window.chat.notice-stopped',
        defaultMessage: 'Stopped. Nothing was changed.',
    },
    noticeInterrupted: {
        id: 'app.agent-window.chat.notice-interrupted',
        defaultMessage: 'Interrupted when the window closed.',
    },
    noticeDetached: {
        id: 'app.agent-window.chat.notice-detached',
        defaultMessage: 'Stopped while applying. Some changes may already be written.',
    },
    retry: { id: 'app.agent-window.chat.retry', defaultMessage: 'Retry' },
    checkStatus: { id: 'app.agent-window.chat.check-status', defaultMessage: 'Check status' },
    runtimeError: {
        id: 'app.agent-window.chat.runtime-error',
        defaultMessage: 'The agent couldn’t start',
    },
    toastAwaiting: {
        id: 'app.agent-window.chat.toast-awaiting',
        defaultMessage: '“{title}” needs your approval',
    },
    toastDone: { id: 'app.agent-window.chat.toast-done', defaultMessage: '“{title}” is done' },
    toastFailed: {
        id: 'app.agent-window.chat.toast-failed',
        defaultMessage: '“{title}” ran into a problem',
    },
    view: { id: 'app.agent-window.chat.view', defaultMessage: 'View' },
    permissionsTitle: {
        id: 'app.agent-window.chat.permissions-title',
        defaultMessage: 'Permissions',
    },
    permissionsLead: {
        id: 'app.agent-window.chat.permissions-lead',
        defaultMessage: 'By default the agent asks before every change.',
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
    historyTitle: { id: 'app.agent-window.chat.history-title', defaultMessage: 'History' },
    localHistory: {
        id: 'app.agent-window.chat.local-history',
        defaultMessage: 'Tasks are saved on this device only.',
    },
    exportHistory: { id: 'app.agent-window.chat.export-history', defaultMessage: 'Export history' },
    clearHistory: { id: 'app.agent-window.chat.clear-history', defaultMessage: 'Clear history' },
    clearHistoryTitle: {
        id: 'app.agent-window.chat.clear-history-title',
        defaultMessage: 'Clear all tasks?',
    },
    clearHistoryBody: {
        id: 'app.agent-window.chat.clear-history-body',
        defaultMessage:
            'Every task on this device will be removed. Export first if you want a copy. Your current draft is kept.',
    },
    aboutTitle: { id: 'app.agent-window.chat.about-title', defaultMessage: 'Getting started' },
    aboutModels: {
        id: 'app.agent-window.chat.about-models',
        defaultMessage:
            'The agent uses the AI providers and models you set up in the launcher. Your API keys stay there.',
    },
    showIntro: { id: 'app.agent-window.chat.show-intro', defaultMessage: 'Show the intro again' },
    storageSaved: { id: 'app.agent-window.chat.storage-saved', defaultMessage: 'Saved' },
    helpTitle: { id: 'app.agent-window.chat.help-title', defaultMessage: 'How the agent works' },
    helpLead: {
        id: 'app.agent-window.chat.help-lead',
        defaultMessage:
            'The agent can look into your launcher and make changes for you, one approved plan at a time.',
    },
    helpFactsTitle: {
        id: 'app.agent-window.chat.help-facts-title',
        defaultMessage: 'Good to know',
    },
    helpFactModel: {
        id: 'app.agent-window.chat.help-fact-model',
        defaultMessage:
            'It uses your own AI provider from the launcher settings. There’s no account or subscription.',
    },
    helpFactScope: {
        id: 'app.agent-window.chat.help-fact-scope',
        defaultMessage:
            'Pick an instance below the message box to focus on its mods, logs and worlds.',
    },
    helpFactScreen: {
        id: 'app.agent-window.chat.help-fact-screen',
        defaultMessage:
            'It never controls the launcher’s interface unless you turn on Interface control and ask it to.',
    },
    shortcutsTitle: {
        id: 'app.agent-window.chat.shortcuts-title',
        defaultMessage: 'Keyboard shortcuts',
    },
    shortcutSend: { id: 'app.agent-window.chat.shortcut-send', defaultMessage: 'Send' },
    shortcutNewline: { id: 'app.agent-window.chat.shortcut-newline', defaultMessage: 'New line' },
    shortcutStop: {
        id: 'app.agent-window.chat.shortcut-stop',
        defaultMessage: 'Stop the running task',
    },
    shortcutNew: { id: 'app.agent-window.chat.shortcut-new', defaultMessage: 'New task' },
    shortcutSearch: { id: 'app.agent-window.chat.shortcut-search', defaultMessage: 'Search tasks' },
    shortcutSidebar: {
        id: 'app.agent-window.chat.shortcut-sidebar',
        defaultMessage: 'Show or hide the task list',
    },
    deleteTitle: { id: 'app.agent-window.chat.delete-title', defaultMessage: 'Delete this task?' },
    deleteBody: {
        id: 'app.agent-window.chat.delete-body',
        defaultMessage: 'Its history will be removed from this device.',
    },
    delete: { id: 'app.agent-window.chat.delete', defaultMessage: 'Delete' },
    cancel: { id: 'app.agent-window.chat.cancel', defaultMessage: 'Cancel' },
    save: { id: 'app.agent-window.chat.save', defaultMessage: 'Save' },
    name: { id: 'app.agent-window.chat.name', defaultMessage: 'Name' },
    close: { id: 'app.agent-window.chat.close', defaultMessage: 'Close' },
    contextError: {
        id: 'app.agent-window.chat.context-error',
        defaultMessage: 'Could not load launcher configuration',
    },
    storageError: {
        id: 'app.agent-window.chat.storage-error',
        defaultMessage:
            'Could not save the conversation. Check available storage. Your input is kept.',
    },
    storageLoadError: {
        id: 'app.agent-window.chat.storage-load-error',
        defaultMessage: 'Could not read saved conversations. New messages may not be saved.',
    },
    storageRecovery: {
        id: 'app.agent-window.chat.storage-recovery',
        defaultMessage: 'Saved history could not be read. Export it before starting a new history.',
    },
    storageExportError: {
        id: 'app.agent-window.chat.storage-export-error',
        defaultMessage: 'Could not export saved history. Your original data has been kept.',
    },
    resetHistory: {
        id: 'app.agent-window.chat.reset-history',
        defaultMessage: 'Start new history',
    },
    resetHistoryWarning: {
        id: 'app.agent-window.chat.reset-history-warning',
        defaultMessage:
            'The saved history will be replaced. Export a backup first. Your current draft will be kept.',
    },
    exportDraft: { id: 'app.agent-window.chat.export-draft', defaultMessage: 'Export draft' },
})

// The window fills simple `{name}` arguments itself, so each one is formatted
// here as a literal placeholder that survives translation.
export function formatAgentWindowMessages(
    format: (message: MessageDescriptor, values?: Record<string, unknown>) => string,
) {
    return Object.fromEntries(
        Object.entries(agentWindowMessages).map(([key, message]) => {
            const names = [...String(message.defaultMessage).matchAll(/\{(\w+)\}/g)].map(
                (match) => match[1],
            )
            return [
                key,
                format(message, Object.fromEntries(names.map((name) => [name, `{${name}}`]))),
            ]
        }),
    )
}
