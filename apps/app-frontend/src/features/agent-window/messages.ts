import { defineMessages, type MessageDescriptor } from '@modrinth/ui'

export const agentWindowMessages = defineMessages({
    planIssueUnknown: {
        id: 'app.agent-window.chat.plan-issue-unknown',
        defaultMessage: 'Plan warning',
    },
    planWorldScope: {
        id: 'app.agent-window.chat.plan-world-scope',
        defaultMessage: 'World: {path}',
    },
    installResourceCount: {
        id: 'app.agent-window.chat.install-resource-count',
        defaultMessage: 'Installation plan · {count} items',
    },
    updateResourceCount: {
        id: 'app.agent-window.chat.update-resource-count',
        defaultMessage: 'Update plan · {count} items',
    },
    installModpack: {
        id: 'app.agent-window.chat.install-modpack',
        defaultMessage: 'Install {name}',
    },
    planGameVersions: {
        id: 'app.agent-window.chat.plan-game-versions',
        defaultMessage: 'Supported Minecraft versions',
    },
    packReplacementEffect: {
        id: 'app.agent-window.chat.pack-replacement-effect',
        defaultMessage:
            'Replaces managed pack content and may overwrite overrides or remove files.',
    },
    packNewInstanceEffect: {
        id: 'app.agent-window.chat.pack-new-instance-effect',
        defaultMessage: 'Creates a new instance and installs the selected pack release.',
    },
    planReplace: { id: 'app.agent-window.chat.plan-replace', defaultMessage: 'Replace' },
    planEnable: { id: 'app.agent-window.chat.plan-enable', defaultMessage: 'Enable' },
    planKeep: { id: 'app.agent-window.chat.plan-keep', defaultMessage: 'Keep' },
    planDependency: { id: 'app.agent-window.chat.plan-dependency', defaultMessage: 'Dependency' },
    planDependenciesSkipped: {
        id: 'app.agent-window.chat.plan-dependencies-skipped',
        defaultMessage: 'Dependency analysis is disabled for this plan.',
    },
    planDependencyInstallSkipped: {
        id: 'app.agent-window.chat.plan-dependency-install-skipped',
        defaultMessage: 'Dependencies will not be installed automatically.',
    },
    planSubject: { id: 'app.agent-window.chat.plan-subject', defaultMessage: 'Affected resource' },
    planRequired: { id: 'app.agent-window.chat.plan-required', defaultMessage: 'Required' },
    planActual: { id: 'app.agent-window.chat.plan-actual', defaultMessage: 'Current' },
    planOriginalDetails: {
        id: 'app.agent-window.chat.plan-original-details',
        defaultMessage: 'Original details',
    },
    planCompatibilityAccepted: {
        id: 'app.agent-window.chat.plan-compatibility-accepted',
        defaultMessage: '{name}: accepts conflicts for {dimensions}.',
    },
    planDetailsUnavailable: {
        id: 'app.agent-window.chat.plan-details-unavailable',
        defaultMessage: 'This older record does not contain the installation details.',
    },
    technicalDetails: {
        id: 'app.agent-window.chat.technical-details',
        defaultMessage: 'Technical details',
    },
    planIssueSourceUnavailable: {
        id: 'app.agent-window.chat.plan-issue-source-unavailable',
        defaultMessage: 'Source unavailable',
    },
    planIssueIdentityUnconfirmed: {
        id: 'app.agent-window.chat.plan-issue-identity-unconfirmed',
        defaultMessage: 'Resource identity is unconfirmed',
    },
    planIssueVersionConflict: {
        id: 'app.agent-window.chat.plan-issue-version-conflict',
        defaultMessage: 'Conflicting resource versions',
    },
    planIssueCompatibilityUnknown: {
        id: 'app.agent-window.chat.plan-issue-compatibility-unknown',
        defaultMessage: 'Compatibility has not been confirmed',
    },
    planIssueCompatibilityMismatch: {
        id: 'app.agent-window.chat.plan-issue-compatibility-mismatch',
        defaultMessage: 'Game or loader compatibility conflict',
    },
    planIssueArtifactMismatch: {
        id: 'app.agent-window.chat.plan-issue-artifact-mismatch',
        defaultMessage: 'Selected file does not match',
    },
    planIssueInvalidPath: {
        id: 'app.agent-window.chat.plan-issue-invalid-path',
        defaultMessage: 'Invalid installation path',
    },
    planIssueMissingDependency: {
        id: 'app.agent-window.chat.plan-issue-missing-dependency',
        defaultMessage: 'Required dependency is missing',
    },
    planIssueDependencyAnalysisFailed: {
        id: 'app.agent-window.chat.plan-issue-dependency-analysis-failed',
        defaultMessage: 'Dependency analysis failed',
    },
    planIssueStaleRevision: {
        id: 'app.agent-window.chat.plan-issue-stale-revision',
        defaultMessage: 'Plan is out of date',
    },
    planIssueManualDownloadRequired: {
        id: 'app.agent-window.chat.plan-issue-manual-download-required',
        defaultMessage: 'A manual download is required',
    },
    planIssueFileNotWritable: {
        id: 'app.agent-window.chat.plan-issue-file-not-writable',
        defaultMessage: 'Target file is not writable',
    },
    planIssueFileWriteSkipped: {
        id: 'app.agent-window.chat.plan-issue-file-write-skipped',
        defaultMessage: 'A file will be skipped',
    },
    reviewLauncherChanges: {
        id: 'app.agent-window.chat.review-launcher-changes',
        defaultMessage: 'Review launcher changes',
    },
    editingMessage: {
        id: 'app.agent-window.chat.editing-message',
        defaultMessage:
            'Editing message · later replies will be regenerated. Applied changes remain.',
    },
    saveAndResend: {
        id: 'app.agent-window.chat.save-and-resend',
        defaultMessage: 'Save and resend',
    },
    argumentUnset: { id: 'app.agent-window.chat.argument-unset', defaultMessage: 'Not set' },
    argumentEmpty: { id: 'app.agent-window.chat.argument-empty', defaultMessage: 'Empty' },
    argumentYes: { id: 'app.agent-window.chat.argument-yes', defaultMessage: 'Yes' },
    argumentNo: { id: 'app.agent-window.chat.argument-no', defaultMessage: 'No' },
    argumentName: { id: 'app.agent-window.chat.argument-name', defaultMessage: 'Name' },
    argumentInstanceId: {
        id: 'app.agent-window.chat.argument-instance-id',
        defaultMessage: 'Instance',
    },
    argumentGameVersion: {
        id: 'app.agent-window.chat.argument-game-version',
        defaultMessage: 'Minecraft version',
    },
    argumentLoader: { id: 'app.agent-window.chat.argument-loader', defaultMessage: 'Mod loader' },
    argumentLoaderVersion: {
        id: 'app.agent-window.chat.argument-loader-version',
        defaultMessage: 'Loader version',
    },
    argumentVersion: { id: 'app.agent-window.chat.argument-version', defaultMessage: 'Version' },
    argumentSource: { id: 'app.agent-window.chat.argument-source', defaultMessage: 'Source' },
    argumentProjectId: {
        id: 'app.agent-window.chat.argument-project-id',
        defaultMessage: 'Project',
    },
    argumentReleaseId: {
        id: 'app.agent-window.chat.argument-release-id',
        defaultMessage: 'Release',
    },
    argumentResourceType: {
        id: 'app.agent-window.chat.argument-resource-type',
        defaultMessage: 'Resource type',
    },
    argumentPlanId: { id: 'app.agent-window.chat.argument-plan-id', defaultMessage: 'Plan' },
    argumentRevision: { id: 'app.agent-window.chat.argument-revision', defaultMessage: 'Revision' },
    argumentEntryId: {
        id: 'app.agent-window.chat.argument-entry-id',
        defaultMessage: 'Content entry',
    },
    argumentEnabled: { id: 'app.agent-window.chat.argument-enabled', defaultMessage: 'Enabled' },
    argumentPath: { id: 'app.agent-window.chat.argument-path', defaultMessage: 'Path' },
    argumentDirectory: {
        id: 'app.agent-window.chat.argument-directory',
        defaultMessage: 'Directory',
    },
    argumentWorldId: { id: 'app.agent-window.chat.argument-world-id', defaultMessage: 'World' },
    argumentServerId: { id: 'app.agent-window.chat.argument-server-id', defaultMessage: 'Server' },
    argumentJobId: { id: 'app.agent-window.chat.argument-job-id', defaultMessage: 'Task' },
    argumentOperationId: {
        id: 'app.agent-window.chat.argument-operation-id',
        defaultMessage: 'Operation',
    },
    argumentMemory: { id: 'app.agent-window.chat.argument-memory', defaultMessage: 'Memory' },
    argumentMinMb: {
        id: 'app.agent-window.chat.argument-min-mb',
        defaultMessage: 'Minimum memory (MiB)',
    },
    argumentMaxMb: {
        id: 'app.agent-window.chat.argument-max-mb',
        defaultMessage: 'Maximum memory (MiB)',
    },
    argumentJavaId: {
        id: 'app.agent-window.chat.argument-java-id',
        defaultMessage: 'Java runtime',
    },
    argumentAccountId: {
        id: 'app.agent-window.chat.argument-account-id',
        defaultMessage: 'Account',
    },
    argumentContent: { id: 'app.agent-window.chat.argument-content', defaultMessage: 'Content' },
    argumentText: { id: 'app.agent-window.chat.argument-text', defaultMessage: 'Text' },
    argumentCommand: { id: 'app.agent-window.chat.argument-command', defaultMessage: 'Command' },
    argumentValue: { id: 'app.agent-window.chat.argument-value', defaultMessage: 'Value' },
    argumentKey: { id: 'app.agent-window.chat.argument-key', defaultMessage: 'Setting' },
    argumentOverwrite: {
        id: 'app.agent-window.chat.argument-overwrite',
        defaultMessage: 'Overwrite',
    },
    argumentRecursive: {
        id: 'app.agent-window.chat.argument-recursive',
        defaultMessage: 'Include subdirectories',
    },
    argumentDestination: {
        id: 'app.agent-window.chat.argument-destination',
        defaultMessage: 'Destination',
    },
    argumentTarget: { id: 'app.agent-window.chat.argument-target', defaultMessage: 'Target' },
    argumentFormat: { id: 'app.agent-window.chat.argument-format', defaultMessage: 'Format' },
    argumentGroup: { id: 'app.agent-window.chat.argument-group', defaultMessage: 'Group' },
    argumentGroupId: { id: 'app.agent-window.chat.argument-group-id', defaultMessage: 'Group' },
    argumentModLoader: {
        id: 'app.agent-window.chat.argument-mod-loader',
        defaultMessage: 'Mod loader',
    },
    argumentModLoaderVersion: {
        id: 'app.agent-window.chat.argument-mod-loader-version',
        defaultMessage: 'Loader version',
    },
    argumentChannel: { id: 'app.agent-window.chat.argument-channel', defaultMessage: 'Channel' },
    argumentSnapshotId: {
        id: 'app.agent-window.chat.argument-snapshot-id',
        defaultMessage: 'Backup snapshot',
    },
    argumentConfirm: { id: 'app.agent-window.chat.argument-confirm', defaultMessage: 'Confirm' },
    argumentIgnoreDependencies: {
        id: 'app.agent-window.chat.argument-ignore-dependencies',
        defaultMessage: 'Skip dependencies',
    },
    argumentAcceptCompatibilityNodes: {
        id: 'app.agent-window.chat.argument-accept-compatibility-nodes',
        defaultMessage: 'Accept compatibility conflicts',
    },
    approvalExpired: {
        id: 'app.agent-window.chat.approval-expired',
        defaultMessage: 'This plan expired. Retry to prepare a fresh plan for review.',
    },
    receiptApplied: { id: 'app.agent-window.chat.receipt-applied', defaultMessage: 'Applied' },
    receiptFailed: { id: 'app.agent-window.chat.receipt-failed', defaultMessage: 'Failed' },
    receiptSkipped: { id: 'app.agent-window.chat.receipt-skipped', defaultMessage: 'Not executed' },
    receiptUnknown: {
        id: 'app.agent-window.chat.receipt-unknown',
        defaultMessage: 'Not confirmed',
    },

    nativeUnavailable: {
        id: 'app.agent-window.chat.native-unavailable',
        defaultMessage: 'Agent is unavailable. Reopen the window and retry.',
    },
    nativeConnecting: {
        id: 'app.agent-window.chat.native-connecting',
        defaultMessage: 'Connecting…',
    },
    taskComputerUse: {
        id: 'app.agent-window.chat.task-computer-use',
        defaultMessage: 'Allow launcher UI',
    },
    attachmentUnreadable: {
        id: 'app.agent-window.chat.attachment-unreadable',
        defaultMessage: 'Could not read this file. Attach a text file or paste its contents.',
    },
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
    statusUnread: { id: 'app.agent-window.chat.status-unread', defaultMessage: 'New result' },
    more: { id: 'app.agent-window.chat.more', defaultMessage: 'More actions' },
    rename: { id: 'app.agent-window.chat.rename', defaultMessage: 'Rename' },
    deleteChat: { id: 'app.agent-window.chat.delete-chat', defaultMessage: 'Delete' },
    pinChat: { id: 'app.agent-window.chat.pin-chat', defaultMessage: 'Pin to top' },
    unpinChat: { id: 'app.agent-window.chat.unpin-chat', defaultMessage: 'Unpin' },
    copyChat: { id: 'app.agent-window.chat.copy-chat', defaultMessage: 'Copy conversation' },
    pinned: { id: 'app.agent-window.chat.pinned', defaultMessage: 'Pinned' },
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
    suggestCrashHint: {
        id: 'app.agent-window.chat.suggest-crash-hint',
        defaultMessage: 'Read the crash report and find the cause',
    },
    suggestSmoothHint: {
        id: 'app.agent-window.chat.suggest-smooth-hint',
        defaultMessage: 'Check mods and settings that slow it down',
    },
    suggestBackupHint: {
        id: 'app.agent-window.chat.suggest-backup-hint',
        defaultMessage: 'Save a ZIP before you change anything',
    },
    suggestPackHint: {
        id: 'app.agent-window.chat.suggest-pack-hint',
        defaultMessage: 'Pick compatible mods for your memory',
    },
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
        defaultMessage: 'Describe what you want. Type / for shortcuts, @ for an instance',
    },
    slashTitle: { id: 'app.agent-window.chat.slash-title', defaultMessage: 'Shortcuts' },
    mentionTitle: {
        id: 'app.agent-window.chat.mention-title',
        defaultMessage: 'Work on an instance',
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
    dropFiles: {
        id: 'app.agent-window.chat.drop-files',
        defaultMessage: 'Drop files to attach them',
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
    copyText: { id: 'app.agent-window.chat.copy-text', defaultMessage: 'Copy text' },
    toolFamilyLauncher: {
        id: 'app.agent-window.chat.tool-family-launcher',
        defaultMessage: 'Looks at or manages the launcher itself, like its version and updates',
    },
    toolFamilyAccount: {
        id: 'app.agent-window.chat.tool-family-account',
        defaultMessage: 'Looks at or manages your Minecraft accounts, like which one you play as',
    },
    toolFamilySkin: {
        id: 'app.agent-window.chat.tool-family-skin',
        defaultMessage: 'Looks at or changes your skins and capes',
    },
    toolFamilyJava: {
        id: 'app.agent-window.chat.tool-family-java',
        defaultMessage: 'Looks at, installs, or picks the Java runtimes that start Minecraft',
    },
    toolFamilyCore: {
        id: 'app.agent-window.chat.tool-family-core',
        defaultMessage:
            'Looks at or changes an instance’s core components, like JAR mods added to Minecraft.jar',
    },
    toolFamilyModpack: {
        id: 'app.agent-window.chat.tool-family-modpack',
        defaultMessage:
            'Checks and installs modpacks, as a new instance or into one you already have',
    },
    toolFamilyDownload: {
        id: 'app.agent-window.chat.tool-family-download',
        defaultMessage: 'Checks on or manages the launcher’s downloads and download history',
    },
    toolFamilyFile: {
        id: 'app.agent-window.chat.tool-family-file',
        defaultMessage: 'Reads or changes files in an instance’s or server’s folder',
    },
    toolFamilyArchive: {
        id: 'app.agent-window.chat.tool-family-archive',
        defaultMessage: 'Makes or unpacks ZIP files in an instance’s or server’s folder',
    },
    toolFamilyNbt: {
        id: 'app.agent-window.chat.tool-family-nbt',
        defaultMessage: 'Reads or edits Minecraft’s own data files, like a world’s level.dat',
    },
    toolFamilyScreenshot: {
        id: 'app.agent-window.chat.tool-family-screenshot',
        defaultMessage: 'Looks at or organizes your screenshots and their groups',
    },
    toolFamilySchematic: {
        id: 'app.agent-window.chat.tool-family-schematic',
        defaultMessage:
            'Opens schematics and edits or saves copies, leaving the original files as they are',
    },
    toolFamilySeed: {
        id: 'app.agent-window.chat.tool-family-seed',
        defaultMessage:
            'Works out what a world’s seed generates, like biomes, structures, and ores',
    },
    toolFamilyServerList: {
        id: 'app.agent-window.chat.tool-family-server-list',
        defaultMessage:
            'Reads or edits an instance’s server list, and checks whether a server is online',
    },
    toolFamilyServer: {
        id: 'app.agent-window.chat.tool-family-server',
        defaultMessage:
            'Sets up or runs local servers on your computer, like starting or stopping one',
    },
    toolFamilyMultiplayer: {
        id: 'app.agent-window.chat.tool-family-multiplayer',
        defaultMessage:
            'Looks at or manages online play with friends, like hosting or joining a room',
    },
    toolFamilySettings: {
        id: 'app.agent-window.chat.tool-family-settings',
        defaultMessage: 'Reads or changes the launcher’s settings',
    },
    toolFamilyPrivacy: {
        id: 'app.agent-window.chat.tool-family-privacy',
        defaultMessage:
            'Reads or changes your privacy choices, like anonymous telemetry and Discord Rich Presence',
    },
    toolFamilyNetwork: {
        id: 'app.agent-window.chat.tool-family-network',
        defaultMessage:
            'Checks or changes how the launcher connects to the internet, like its proxy',
    },
    toolFamilyStorage: {
        id: 'app.agent-window.chat.tool-family-storage',
        defaultMessage:
            'Checks how much disk space the launcher uses, and clears caches it can rebuild',
    },
    toolFamilyDatabase: {
        id: 'app.agent-window.chat.tool-family-database',
        defaultMessage: 'Looks after the launcher’s own database, like checking or backing it up',
    },
    toolFamilyShortcut: {
        id: 'app.agent-window.chat.tool-family-shortcut',
        defaultMessage: 'Puts a shortcut on your desktop that starts an instance',
    },
    toolFamilyNews: {
        id: 'app.agent-window.chat.tool-family-news',
        defaultMessage: 'Reads the latest Minecraft news headlines',
    },
    toolFamilyInterface: {
        id: 'app.agent-window.chat.tool-family-interface',
        defaultMessage:
            'Looks at or uses the launcher’s main window, like clicking a button for you',
    },
    toolHelpInstanceList: {
        id: 'app.agent-window.chat.tool-help-instance-list',
        defaultMessage: 'Looks at the list of your instances',
    },
    toolHelpInstanceGet: {
        id: 'app.agent-window.chat.tool-help-instance-get',
        defaultMessage: 'Reads an instance’s details, like its Minecraft version and mod loader',
    },
    toolHelpInstanceCreate: {
        id: 'app.agent-window.chat.tool-help-instance-create',
        defaultMessage: 'Sets up a new instance and starts installing it',
    },
    toolHelpWorldList: {
        id: 'app.agent-window.chat.tool-help-world-list',
        defaultMessage: 'Lists the singleplayer worlds in an instance',
    },
    toolHelpWorldGet: {
        id: 'app.agent-window.chat.tool-help-world-get',
        defaultMessage: 'Reads a world’s details, like its game mode and difficulty',
    },
    toolHelpWorldBackup: {
        id: 'app.agent-window.chat.tool-help-world-backup',
        defaultMessage: 'Saves a copy of a world as a ZIP file in the instance’s backups folder',
    },
    toolHelpResourceSearch: {
        id: 'app.agent-window.chat.tool-help-resource-search',
        defaultMessage: 'Searches a site such as Modrinth for mods, resource packs, or shaders',
    },
    toolHelpResourceListReleases: {
        id: 'app.agent-window.chat.tool-help-resource-list-releases',
        defaultMessage: 'Lists the versions of a mod or pack that fit an instance',
    },
    toolHelpResourcePrepareInstall: {
        id: 'app.agent-window.chat.tool-help-resource-prepare-install',
        defaultMessage:
            'Plans an install and checks what else it needs, without installing anything yet',
    },
    toolHelpResourceExecutePlan: {
        id: 'app.agent-window.chat.tool-help-resource-execute-plan',
        defaultMessage: 'Starts the install that a checked plan describes',
    },
    toolHelpResourceCheckUpdates: {
        id: 'app.agent-window.chat.tool-help-resource-check-updates',
        defaultMessage:
            'Checks installed mods and packs for updates that fit, without updating them',
    },
    toolHelpResourcePrepareUpdate: {
        id: 'app.agent-window.chat.tool-help-resource-prepare-update',
        defaultMessage:
            'Plans updates and checks that they work together, without applying them yet',
    },
    toolHelpResourceExecuteChanges: {
        id: 'app.agent-window.chat.tool-help-resource-execute-changes',
        defaultMessage: 'Carries out a checked update plan, replacing the old files',
    },
    toolHelpResourceHistoryList: {
        id: 'app.agent-window.chat.tool-help-resource-history-list',
        defaultMessage:
            'Shows recent changes to an instance’s mods, and which old versions can be restored',
    },
    toolHelpResourceHistoryRollback: {
        id: 'app.agent-window.chat.tool-help-resource-history-rollback',
        defaultMessage: 'Puts a mod back to a version you had before',
    },
    toolHelpContentList: {
        id: 'app.agent-window.chat.tool-help-content-list',
        defaultMessage: 'Lists what’s installed in an instance and whether each item is turned on',
    },
    toolHelpLogsList: {
        id: 'app.agent-window.chat.tool-help-logs-list',
        defaultMessage: 'Lists an instance’s logs and crash reports without opening them',
    },
    toolHelpLogsRead: {
        id: 'app.agent-window.chat.tool-help-logs-read',
        defaultMessage: 'Reads a game log to find what went wrong',
    },
    toolHelpBackupGetConfig: {
        id: 'app.agent-window.chat.tool-help-backup-get-config',
        defaultMessage: 'Checks whether an instance has snapshots turned on, and how many it has',
    },
    toolHelpBackupCreate: {
        id: 'app.agent-window.chat.tool-help-backup-create',
        defaultMessage: 'Takes a snapshot of an instance that you can restore later',
    },
    toolHelpLaunchSetMemory: {
        id: 'app.agent-window.chat.tool-help-launch-set-memory',
        defaultMessage: 'Sets how much memory an instance’s game can use',
    },
    toolHelpInstallJobGet: {
        id: 'app.agent-window.chat.tool-help-install-job-get',
        defaultMessage: 'Checks how an install is going and whether it worked',
    },
    toolHelpOperationsGet: {
        id: 'app.agent-window.chat.tool-help-operations-get',
        defaultMessage:
            'Checks whether something the launcher started in the background has finished',
    },
    toolHelpProcessList: {
        id: 'app.agent-window.chat.tool-help-process-list',
        defaultMessage: 'Checks whether the launcher has a game running right now',
    },
    toolHelpSystemGetMemory: {
        id: 'app.agent-window.chat.tool-help-system-get-memory',
        defaultMessage: 'Checks how much memory your computer has, and how much is free',
    },
    toolHelpMetadataListLoaderVersions: {
        id: 'app.agent-window.chat.tool-help-metadata-list-loader-versions',
        defaultMessage:
            'Looks up the versions of a mod loader, such as Fabric, for a Minecraft version',
    },
    toolFamilyInstance: {
        id: 'app.agent-window.chat.tool-family-instance',
        defaultMessage: 'Looks at or manages your game instances',
    },
    toolFamilyWorld: {
        id: 'app.agent-window.chat.tool-family-world',
        defaultMessage: 'Looks at or manages an instance’s singleplayer worlds',
    },
    toolFamilyResource: {
        id: 'app.agent-window.chat.tool-family-resource',
        defaultMessage: 'Finds and manages mods, resource packs, and shaders',
    },
    toolFamilyContent: {
        id: 'app.agent-window.chat.tool-family-content',
        defaultMessage: 'Looks at or changes what’s installed in an instance',
    },
    toolFamilyBackup: {
        id: 'app.agent-window.chat.tool-family-backup',
        defaultMessage:
            'Looks at or manages an instance’s snapshots, the backups it can be restored from',
    },
    toolFamilyLogs: {
        id: 'app.agent-window.chat.tool-family-logs',
        defaultMessage: 'Looks through an instance’s logs and crash reports',
    },
    toolFamilyLaunch: {
        id: 'app.agent-window.chat.tool-family-launch',
        defaultMessage: 'Reads or changes how an instance starts, like its memory and Java',
    },
    toolFamilyJob: {
        id: 'app.agent-window.chat.tool-family-job',
        defaultMessage:
            'Checks on or manages work the launcher does in the background, like installs',
    },
    toolFamilyProcess: {
        id: 'app.agent-window.chat.tool-family-process',
        defaultMessage: 'Checks on or stops a game the launcher started',
    },
    toolFamilySystem: {
        id: 'app.agent-window.chat.tool-family-system',
        defaultMessage: 'Uses your computer, like checking its memory or opening a link',
    },
    toolFamilyMetadata: {
        id: 'app.agent-window.chat.tool-family-metadata',
        defaultMessage: 'Looks up which Minecraft and mod loader versions exist',
    },
    toolGeneric: { id: 'app.agent-window.chat.tool-generic', defaultMessage: 'A launcher tool' },
    why: { id: 'app.agent-window.chat.why', defaultMessage: 'Why?' },
    askWhy: { id: 'app.agent-window.chat.ask-why', defaultMessage: 'Why change {name}?' },
    tourName: { id: 'app.agent-window.chat.tour-name', defaultMessage: 'Quick tour' },
    tourStep: {
        id: 'app.agent-window.chat.tour-step',
        defaultMessage: 'Step {current} of {total}',
    },
    tourComposeTitle: {
        id: 'app.agent-window.chat.tour-compose-title',
        defaultMessage: 'Say what you want',
    },
    tourComposeBody: {
        id: 'app.agent-window.chat.tour-compose-body',
        defaultMessage:
            'Write it in your own words, the way you’d ask a friend. There are no commands to learn.',
    },
    tourSlash: {
        id: 'app.agent-window.chat.tour-slash',
        defaultMessage: 'Shortcuts for common jobs',
    },
    tourMention: {
        id: 'app.agent-window.chat.tour-mention',
        defaultMessage: 'Pick an instance to work on',
    },
    tourPickersTitle: {
        id: 'app.agent-window.chat.tour-pickers-title',
        defaultMessage: 'Where it works, and with what',
    },
    tourInstance: {
        id: 'app.agent-window.chat.tour-instance',
        defaultMessage: 'The instance it works on, or the whole launcher',
    },
    tourModel: {
        id: 'app.agent-window.chat.tour-model',
        defaultMessage: 'The AI model it uses, from your launcher settings',
    },
    tourSafetyTitle: {
        id: 'app.agent-window.chat.tour-safety-title',
        defaultMessage: 'Nothing changes without you',
    },
    tourSafetyBody: {
        id: 'app.agent-window.chat.tour-safety-body',
        defaultMessage:
            'After you send, the agent looks first and shows you a plan. It only makes changes once you approve.',
    },
    tourStop: { id: 'app.agent-window.chat.tour-stop', defaultMessage: 'Stop it at any time' },
    tourSkip: { id: 'app.agent-window.chat.tour-skip', defaultMessage: 'Skip' },
    tourBack: { id: 'app.agent-window.chat.tour-back', defaultMessage: 'Back' },
    tourNext: { id: 'app.agent-window.chat.tour-next', defaultMessage: 'Next' },
    tourFinish: { id: 'app.agent-window.chat.tour-finish', defaultMessage: 'Got it' },
    replayTour: { id: 'app.agent-window.chat.replay-tour', defaultMessage: 'Take the tour again' },
    copied: { id: 'app.agent-window.chat.copied', defaultMessage: 'Copied' },
    copyCode: { id: 'app.agent-window.chat.copy-code', defaultMessage: 'Copy code' },
    linkCopied: { id: 'app.agent-window.chat.link-copied', defaultMessage: 'Link copied' },
    copyError: { id: 'app.agent-window.chat.copy-error', defaultMessage: 'Couldn’t copy' },
    thinking: { id: 'app.agent-window.chat.thinking', defaultMessage: 'Thinking…' },
    working: { id: 'app.agent-window.chat.working', defaultMessage: 'Working…' },
    applying: { id: 'app.agent-window.chat.applying', defaultMessage: 'Applying changes…' },
    escToStop: { id: 'app.agent-window.chat.esc-to-stop', defaultMessage: 'to stop' },
    stepsDone: { id: 'app.agent-window.chat.steps-done', defaultMessage: '{count} steps' },
    stepsRunning: {
        id: 'app.agent-window.chat.steps-running',
        defaultMessage: 'Working · {count} steps',
    },
    earlierSteps: {
        id: 'app.agent-window.chat.earlier-steps',
        defaultMessage: 'Show {count} earlier steps',
    },
    stepsDoneNamed: {
        id: 'app.agent-window.chat.steps-done-named',
        defaultMessage: '{title} · {count} steps',
    },
    answerAgain: { id: 'app.agent-window.chat.answer-again', defaultMessage: 'Answer again' },
    markTurn: { id: 'app.agent-window.chat.mark-turn', defaultMessage: 'Mark in the navigator' },
    unmarkTurn: { id: 'app.agent-window.chat.unmark-turn', defaultMessage: 'Remove the mark' },
    marked: { id: 'app.agent-window.chat.marked', defaultMessage: 'Marked' },
    tookTime: { id: 'app.agent-window.chat.took-time', defaultMessage: 'Took {duration}' },
    pasteName: { id: 'app.agent-window.chat.paste-name', defaultMessage: 'Pasted text' },
    pasteCrashName: {
        id: 'app.agent-window.chat.paste-crash-name',
        defaultMessage: 'Crash report',
    },
    pasteLogName: { id: 'app.agent-window.chat.paste-log-name', defaultMessage: 'Game log' },
    lineCount: { id: 'app.agent-window.chat.line-count', defaultMessage: '{count} lines' },
    changesPill: {
        id: 'app.agent-window.chat.changes-pill',
        defaultMessage: 'Changes in this task',
    },
    changesTitle: {
        id: 'app.agent-window.chat.changes-title',
        defaultMessage: 'What this task changed',
    },
    changesUndo: { id: 'app.agent-window.chat.changes-undo', defaultMessage: 'Ask to undo them' },
    undoAllPrompt: {
        id: 'app.agent-window.chat.undo-all-prompt',
        defaultMessage: 'Undo all the changes in this task',
    },
    previewFile: { id: 'app.agent-window.chat.preview-file', defaultMessage: 'Preview {name}' },
    moreLines: { id: 'app.agent-window.chat.more-lines', defaultMessage: '{count} more lines' },
    pastedAsFile: {
        id: 'app.agent-window.chat.pasted-as-file',
        defaultMessage: 'Long text was attached as a file',
    },
    pasteAsText: { id: 'app.agent-window.chat.paste-as-text', defaultMessage: 'Paste as text' },
    planKicker: { id: 'app.agent-window.chat.plan-kicker', defaultMessage: 'Proposed changes' },
    manualChange: { id: 'app.agent-window.chat.manual-change', defaultMessage: 'Do it yourself' },
    changeKindUpdate: { id: 'app.agent-window.chat.change-kind-update', defaultMessage: 'Update' },
    changeKindRollback: {
        id: 'app.agent-window.chat.change-kind-rollback',
        defaultMessage: 'Roll back',
    },
    changeKindNew: { id: 'app.agent-window.chat.change-kind-new', defaultMessage: 'New' },
    changeKindInstall: {
        id: 'app.agent-window.chat.change-kind-install',
        defaultMessage: 'Install',
    },
    changeKindSetting: {
        id: 'app.agent-window.chat.change-kind-setting',
        defaultMessage: 'Setting',
    },
    changeKindBackup: { id: 'app.agent-window.chat.change-kind-backup', defaultMessage: 'Backup' },
    changeKindRemove: { id: 'app.agent-window.chat.change-kind-remove', defaultMessage: 'Remove' },
    includeChange: { id: 'app.agent-window.chat.include-change', defaultMessage: 'Include {name}' },
    excludedCount: {
        id: 'app.agent-window.chat.excluded-count',
        defaultMessage: 'Skipping {count}: {names}',
    },
    nothingSelected: {
        id: 'app.agent-window.chat.nothing-selected',
        defaultMessage: 'Keep at least one change',
    },
    planAppliedPartial: {
        id: 'app.agent-window.chat.plan-applied-partial',
        defaultMessage: 'Applied {done} · skipped {skipped}',
    },
    railLabel: { id: 'app.agent-window.chat.rail-label', defaultMessage: 'Your messages' },
    jumpTo: { id: 'app.agent-window.chat.jump-to', defaultMessage: 'Jump to “{text}”' },
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
    planStopped: { id: 'app.agent-window.chat.plan-stopped', defaultMessage: 'Not carried out' },
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
    conversationTitle: {
        id: 'app.agent-window.chat.conversation-title',
        defaultMessage: 'Conversation',
    },
    detailLabel: { id: 'app.agent-window.chat.detail-label', defaultMessage: 'Steps shown' },
    detailHint: {
        id: 'app.agent-window.chat.detail-hint',
        defaultMessage: 'How much of the agent’s work each answer shows.',
    },
    detailConcise: { id: 'app.agent-window.chat.detail-concise', defaultMessage: 'Concise' },
    detailBalanced: { id: 'app.agent-window.chat.detail-balanced', defaultMessage: 'Balanced' },
    detailDetailed: { id: 'app.agent-window.chat.detail-detailed', defaultMessage: 'Detailed' },
    instructionsLabel: {
        id: 'app.agent-window.chat.instructions-label',
        defaultMessage: 'Standing instructions',
    },
    instructionsHint: {
        id: 'app.agent-window.chat.instructions-hint',
        defaultMessage:
            'Sent with every request, such as the language to answer in or the loader you prefer.',
    },
    instructionsPlaceholder: {
        id: 'app.agent-window.chat.instructions-placeholder',
        defaultMessage: 'For example: answer in English, prefer Fabric mods, never delete a world.',
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
    shortcutPalette: {
        id: 'app.agent-window.chat.shortcut-palette',
        defaultMessage: 'Open the command menu',
    },
    shortcutSlash: {
        id: 'app.agent-window.chat.shortcut-slash',
        defaultMessage: 'Shortcuts at the start of a request',
    },
    shortcutMention: {
        id: 'app.agent-window.chat.shortcut-mention',
        defaultMessage: 'Pick the instance to work on',
    },
    shortcutSwitch: {
        id: 'app.agent-window.chat.shortcut-switch',
        defaultMessage: 'Previous or next task',
    },
    palette: { id: 'app.agent-window.chat.palette', defaultMessage: 'Command menu' },
    paletteSearch: {
        id: 'app.agent-window.chat.palette-search',
        defaultMessage: 'Type a command or search tasks…',
    },
    paletteActions: { id: 'app.agent-window.chat.palette-actions', defaultMessage: 'Actions' },
    paletteTasks: { id: 'app.agent-window.chat.palette-tasks', defaultMessage: 'Tasks' },
    togglePanel: {
        id: 'app.agent-window.chat.toggle-panel',
        defaultMessage: 'Show or hide progress',
    },
    shortcutSidebar: {
        id: 'app.agent-window.chat.shortcut-sidebar',
        defaultMessage: 'Show or hide the task list',
    },
    shortcutSettings: {
        id: 'app.agent-window.chat.shortcut-settings',
        defaultMessage: 'Open settings',
    },
    editMessage: {
        id: 'app.agent-window.chat.edit-message',
        defaultMessage: 'Edit and send again',
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
