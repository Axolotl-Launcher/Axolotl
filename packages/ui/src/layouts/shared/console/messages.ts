import { defineMessages } from '#ui/composables/i18n'

export const consoleMessages = defineMessages({
    exportLog: { id: 'console.log.export', defaultMessage: 'Save complete log' },
    exportCrashContext: {
        id: 'console.crash.export-context',
        defaultMessage: 'Export crash context',
    },
    searchLogs: { id: 'console.search.placeholder', defaultMessage: 'Search logs' },
    toggleWrap: { id: 'console.log.toggle-wrap', defaultMessage: 'Toggle line wrapping' },
    scrollToBottom: {
        id: 'console.log-viewport.scroll-to-bottom',
        defaultMessage: 'Scroll to bottom',
    },
    wrapLabel: { id: 'console.log.wrap-label', defaultMessage: 'Wrap' },
    emptyInstanceTitle: {
        id: 'console.empty.instance-title',
        defaultMessage: 'No logs yet',
    },
    emptyInstanceDescription: {
        id: 'console.empty.instance-description',
        defaultMessage: 'Click the Play button to start receiving live logs.',
    },
    emptyServerTitle: {
        id: 'console.empty.server-title',
        defaultMessage: 'Welcome to your Modrinth server!',
    },
    emptyServerDescription: {
        id: 'console.empty.server-description',
        defaultMessage: 'Click the start button to start the server!',
    },
    shareLogs: { id: 'console.share-modal.title', defaultMessage: 'Share Logs' },
    deleteLogFile: { id: 'console.delete-modal.title', defaultMessage: 'Delete log file' },
    deleteIrreversible: {
        id: 'console.delete-modal.irreversible-title',
        defaultMessage: 'This is irreversible',
    },
    deleteConfirmation: {
        id: 'console.delete-modal.confirmation',
        defaultMessage:
            'Deleting this log file cannot be undone. Are you sure you want to continue?',
    },
    localCrashHeader: {
        id: 'console.crash.local-header',
        defaultMessage:
            '{findings, plural, one {# local diagnosis result} other {# local diagnosis results}} from {sources, plural, one {# related file} other {# related files}}',
    },
    fallbackFindingAction: {
        id: 'console.crash.finding.fallback-action',
        defaultMessage: 'Review the evidence below and the Mods matched from the local instance.',
    },
    fallbackFindingTitle: {
        id: 'console.crash.finding.fallback-title',
        defaultMessage: 'Unknown diagnosis: {finding}',
    },
    knownSignatureTitle: {
        id: 'console.crash.finding.known-signature.title',
        defaultMessage: 'A known crash signature was detected',
    },
    knownSignatureAction: {
        id: 'console.crash.finding.known-signature.action',
        defaultMessage:
            'Review the evidence and the related Minecraft, loader, Java, graphics, or Mod versions before changing the instance.',
    },
    matchedMod: {
        id: 'console.crash.finding.matched-mod',
        defaultMessage: 'Matched Mod: {identity}{modId} - {fileName}',
    },
    modChange: {
        id: 'console.crash.mod-change',
        defaultMessage: '{kind}: {filename}',
    },
    modChangesTitle: {
        id: 'console.crash.mod-changes.title',
        defaultMessage: 'Mod files changed since the last successful launch',
    },
    modChangesSummary: {
        id: 'console.crash.mod-changes.summary',
        defaultMessage: '{added} added, {removed} removed, {modified} modified',
    },
    modChangeAdded: {
        id: 'console.crash.mod-change.added',
        defaultMessage: 'Added',
    },
    modChangeRemoved: {
        id: 'console.crash.mod-change.removed',
        defaultMessage: 'Removed',
    },
    modChangeModified: {
        id: 'console.crash.mod-change.modified',
        defaultMessage: 'Modified',
    },
    windowsEventsTitle: {
        id: 'console.crash.windows-events.title',
        defaultMessage: 'Related Windows application events',
    },
    problemsDetected: {
        id: 'console.crash.problems-detected',
        defaultMessage: '{count, plural, one {# problem detected} other {# problems detected}}',
    },
    commandInputDisabled: {
        id: 'console.command.disabled-placeholder',
        defaultMessage: 'Command input disabled',
    },
    commandPlaceholder: {
        id: 'console.command.placeholder',
        defaultMessage: 'Send a command',
    },
    serverNotRunning: {
        id: 'console.command.server-not-running-placeholder',
        defaultMessage: 'Server is not running',
    },
    deleteFailedTitle: {
        id: 'console.notification.delete-failed',
        defaultMessage: 'Failed to delete log file',
    },
    shareFailedTitle: {
        id: 'console.notification.share-failed',
        defaultMessage: 'Failed to share logs',
    },
    shareTruncatedWarning: {
        id: 'console.notification.share-truncated',
        defaultMessage: 'The log is too large, so only the last 9 MB was uploaded.',
    },
    unknownError: { id: 'console.notification.unknown-error', defaultMessage: 'Unknown error.' },
})

export const localFindingMessages = defineMessages({
    jvmArgumentsTitle: {
        id: 'console.crash.finding.jvm-arguments.title',
        defaultMessage: 'Invalid JVM arguments',
    },
    jvmArgumentsAction: {
        id: 'console.crash.finding.jvm-arguments.action',
        defaultMessage: 'Remove the reported custom JVM argument, then launch the instance again.',
    },
    outOfMemoryTitle: {
        id: 'console.crash.finding.out-of-memory.title',
        defaultMessage: 'Minecraft ran out of memory',
    },
    outOfMemoryAction: {
        id: 'console.crash.finding.out-of-memory.action',
        defaultMessage:
            'Increase the instance memory allocation or remove memory-heavy mods and resource packs.',
    },
    openglUnsupportedTitle: {
        id: 'console.crash.finding.opengl-unsupported.title',
        defaultMessage: 'OpenGL is not supported by the active graphics driver',
    },
    openglUnsupportedAction: {
        id: 'console.crash.finding.opengl-unsupported.action',
        defaultMessage:
            'Install the graphics driver from the GPU manufacturer and ensure Minecraft uses the intended GPU.',
    },
    pixelFormatTitle: {
        id: 'console.crash.finding.pixel-format.title',
        defaultMessage: 'The graphics driver could not set a pixel format',
    },
    pixelFormatAction: {
        id: 'console.crash.finding.pixel-format.action',
        defaultMessage:
            'Update or reinstall the graphics driver and disable conflicting overlays before retrying.',
    },
    openj9Title: {
        id: 'console.crash.finding.openj9.title',
        defaultMessage: 'The selected OpenJ9 runtime is incompatible',
    },
    openj9Action: {
        id: 'console.crash.finding.openj9.action',
        defaultMessage:
            'Select a HotSpot-based Java runtime such as Eclipse Temurin or the bundled Minecraft runtime.',
    },
    javaTooNewTitle: {
        id: 'console.crash.finding.java-too-new.title',
        defaultMessage: 'The Java runtime is too new for this instance',
    },
    javaTooNewAction: {
        id: 'console.crash.finding.java-too-new.action',
        defaultMessage:
            'Select the Java major version expected by this Minecraft and mod-loader version.',
    },
    javaIncompatibleTitle: {
        id: 'console.crash.finding.java-incompatible.title',
        defaultMessage: 'A mod requires a different Java version',
    },
    javaIncompatibleAction: {
        id: 'console.crash.finding.java-incompatible.action',
        defaultMessage:
            'Use a compatible Java runtime or install a build of the reported mod for this Java version.',
    },
    jdkRuntimeTitle: {
        id: 'console.crash.finding.jdk-runtime.title',
        defaultMessage: 'A JDK runtime was selected instead of a JRE',
    },
    jdkRuntimeAction: {
        id: 'console.crash.finding.jdk-runtime.action',
        defaultMessage: 'Select a standard HotSpot Java runtime for this Minecraft version.',
    },
    java32BitTitle: {
        id: 'console.crash.finding.java-32bit.title',
        defaultMessage: 'A 32-bit Java runtime cannot allocate the requested memory',
    },
    java32BitAction: {
        id: 'console.crash.finding.java-32bit.action',
        defaultMessage: 'Install and select a 64-bit Java runtime, then retry the launch.',
    },
    java11RequiredTitle: {
        id: 'console.crash.finding.java-11-required.title',
        defaultMessage: 'A Mod requires Java 11',
    },
    java11RequiredAction: {
        id: 'console.crash.finding.java-11-required.action',
        defaultMessage:
            'Select Java 11 or install a Mod build compatible with the selected Java version.',
    },
    forgeIncompleteTitle: {
        id: 'console.crash.finding.forge-incomplete.title',
        defaultMessage: 'The Forge installation is incomplete',
    },
    forgeIncompleteAction: {
        id: 'console.crash.finding.forge-incomplete.action',
        defaultMessage: 'Repair or reinstall the Forge loader for this instance.',
    },
    duplicateModTitle: {
        id: 'console.crash.finding.duplicate-mod.title',
        defaultMessage: 'Duplicate Mods are installed',
    },
    duplicateModAction: {
        id: 'console.crash.finding.duplicate-mod.action',
        defaultMessage: 'Keep only one compatible version of each Mod in the mods folder.',
    },
    incompatibleModsTitle: {
        id: 'console.crash.finding.incompatible-mods.title',
        defaultMessage: 'The installed Mods are incompatible',
    },
    incompatibleModsAction: {
        id: 'console.crash.finding.incompatible-mods.action',
        defaultMessage:
            'Follow the compatibility details in the evidence and update, remove, or replace the conflicting Mods.',
    },
    missingDependencyTitle: {
        id: 'console.crash.finding.missing-dependency.title',
        defaultMessage: 'A Mod dependency is missing or unsupported',
    },
    missingDependencyAction: {
        id: 'console.crash.finding.missing-dependency.action',
        defaultMessage:
            'Install the required dependency version or use a Mod build matching this Minecraft version.',
    },
    diskSpaceTitle: {
        id: 'console.crash.finding.disk-space.title',
        defaultMessage: 'The disk ran out of free space',
    },
    diskSpaceAction: {
        id: 'console.crash.finding.disk-space.action',
        defaultMessage:
            'Free space on the drive containing the instance, then retry the launch or installation.',
    },
    fileInUseTitle: {
        id: 'console.crash.finding.file-in-use.title',
        defaultMessage: 'Another process is using a required file',
    },
    fileInUseAction: {
        id: 'console.crash.finding.file-in-use.action',
        defaultMessage:
            'Close the program named in the evidence, including other launchers, backup tools, or antivirus scans, then retry.',
    },
    connectorIncompatibleFabricModsTitle: {
        id: 'console.crash.finding.connector-incompatible-fabric-mods.title',
        defaultMessage: 'Sinytra Connector found incompatible Fabric Mods',
    },
    connectorIncompatibleFabricModsAction: {
        id: 'console.crash.finding.connector-incompatible-fabric-mods.action',
        defaultMessage:
            'Remove or replace the Fabric Mods named in the Connector error with Forge-compatible alternatives.',
    },
    missingEmbeddiumTitle: {
        id: 'console.crash.finding.missing-embeddium.title',
        defaultMessage: 'Oculus requires Embeddium',
    },
    missingEmbeddiumAction: {
        id: 'console.crash.finding.missing-embeddium.action',
        defaultMessage:
            'Install the Embeddium version required by Oculus for this Minecraft and Forge version.',
    },
    missingIndiumTitle: {
        id: 'console.crash.finding.missing-indium.title',
        defaultMessage: 'A Mod requires Indium',
    },
    missingIndiumAction: {
        id: 'console.crash.finding.missing-indium.action',
        defaultMessage:
            'Install the Indium version compatible with the installed Fabric Loader and Sodium version.',
    },
    modIdLimitTitle: {
        id: 'console.crash.finding.mod-id-limit.title',
        defaultMessage: 'Too many Mods exceeded the ID limit',
    },
    modIdLimitAction: {
        id: 'console.crash.finding.mod-id-limit.action',
        defaultMessage:
            'Remove unused Mods or split the installation into smaller compatible profiles.',
    },
    forgeErrorTitle: {
        id: 'console.crash.finding.forge-error.title',
        defaultMessage: 'Forge reported a game error',
    },
    forgeErrorAction: {
        id: 'console.crash.finding.forge-error.action',
        defaultMessage:
            'Review the Forge failure evidence and test the named Mod without recent changes.',
    },
    modLoaderErrorTitle: {
        id: 'console.crash.finding.mod-loader-error.title',
        defaultMessage: 'The Mod loader reported a failure',
    },
    modLoaderErrorAction: {
        id: 'console.crash.finding.mod-loader-error.action',
        defaultMessage:
            'Repair the loader installation and verify that the listed Mod files match this game version.',
    },
    modLoaderFailureTitle: {
        id: 'console.crash.finding.mod-loader-failure.title',
        defaultMessage: 'The Mod loader failed before identifying a Mod file',
    },
    modLoaderFailureAction: {
        id: 'console.crash.finding.mod-loader-failure.action',
        defaultMessage:
            'Repair the loader installation and follow the failure message shown in the evidence.',
    },
    stackAnalysisTitle: {
        id: 'console.crash.finding.stack-analysis.title',
        defaultMessage: 'The stack trace points to an installed Mod',
    },
    stackAnalysisAction: {
        id: 'console.crash.finding.stack-analysis.action',
        defaultMessage:
            'Update or temporarily remove the matched Mod, then test the instance again.',
    },
    shortOutputTitle: {
        id: 'console.crash.finding.short-output.title',
        defaultMessage: 'The game stopped before producing a useful log',
    },
    shortOutputAction: {
        id: 'console.crash.finding.short-output.action',
        defaultMessage:
            'Retry once, then verify Java, the loader installation, and the launcher output for an earlier error.',
    },
    extractedModTitle: {
        id: 'console.crash.finding.extracted-mod.title',
        defaultMessage: 'An extracted Mod was found',
    },
    extractedModAction: {
        id: 'console.crash.finding.extracted-mod.action',
        defaultMessage:
            'Remove the extracted directory from the mods folder and install the original jar file.',
    },
    mixinBootstrapTitle: {
        id: 'console.crash.finding.mixin-bootstrap.title',
        defaultMessage: 'Mixin bootstrap is missing',
    },
    mixinBootstrapAction: {
        id: 'console.crash.finding.mixin-bootstrap.action',
        defaultMessage:
            'Repair the mod loader installation and verify that every mod targets the installed loader.',
    },
    mixinFailureTitle: {
        id: 'console.crash.finding.mixin-failure.title',
        defaultMessage: 'A Mod Mixin failed to apply',
    },
    mixinFailureAction: {
        id: 'console.crash.finding.mixin-failure.action',
        defaultMessage:
            'Update or remove the matched Mod and check that its Minecraft and loader versions are compatible.',
    },
    fabricSolutionTitle: {
        id: 'console.crash.finding.fabric-solution.title',
        defaultMessage: 'Fabric found an incompatible Mod or missing dependency',
    },
    fabricSolutionAction: {
        id: 'console.crash.finding.fabric-solution.action',
        defaultMessage:
            'Apply the dependency changes listed in the evidence before launching again.',
    },
    modConfigTitle: {
        id: 'console.crash.finding.mod-config.title',
        defaultMessage: 'A Mod configuration file could not be read',
    },
    modConfigAction: {
        id: 'console.crash.finding.mod-config.action',
        defaultMessage:
            'Back up and remove the named configuration file so the Mod can regenerate it.',
    },
    optifineIncompatibleTitle: {
        id: 'console.crash.finding.optifine-incompatible.title',
        defaultMessage: 'OptiFine conflicts with the installed loader or Mod',
    },
    optifineIncompatibleAction: {
        id: 'console.crash.finding.optifine-incompatible.action',
        defaultMessage:
            'Install a compatible OptiFine build or remove OptiFine and the conflicting shader Mod.',
    },
    resourcePackTitle: {
        id: 'console.crash.finding.resource-pack.title',
        defaultMessage: 'A shader or resource pack triggered a graphics error',
    },
    resourcePackAction: {
        id: 'console.crash.finding.resource-pack.action',
        defaultMessage:
            'Disable the active shader and resource packs, then re-enable them one at a time.',
    },
    largeResourcePackTitle: {
        id: 'console.crash.finding.large-resource-pack.title',
        defaultMessage: 'The active resource pack is too large for the graphics configuration',
    },
    largeResourcePackAction: {
        id: 'console.crash.finding.large-resource-pack.action',
        defaultMessage: 'Disable the resource pack or use a lower-resolution version.',
    },
    shadersOptifineTitle: {
        id: 'console.crash.finding.shaders-optifine.title',
        defaultMessage: 'Shaders Mod and OptiFine are installed together',
    },
    shadersOptifineAction: {
        id: 'console.crash.finding.shaders-optifine.action',
        defaultMessage:
            'Remove the separate Shaders Mod because OptiFine already provides shader support.',
    },
    multipleForgeVersionsTitle: {
        id: 'console.crash.finding.multiple-forge-versions.title',
        defaultMessage: 'The version profile contains multiple Forge versions',
    },
    multipleForgeVersionsAction: {
        id: 'console.crash.finding.multiple-forge-versions.action',
        defaultMessage:
            'Repair the instance so its version profile contains only one Forge installation.',
    },
    forgeJavaIncompatibleTitle: {
        id: 'console.crash.finding.forge-java-incompatible.title',
        defaultMessage: 'This Forge version is incompatible with the selected Java runtime',
    },
    forgeJavaIncompatibleAction: {
        id: 'console.crash.finding.forge-java-incompatible.action',
        defaultMessage: 'Use the Java version expected by this Forge release or update Forge.',
    },
    contentVerificationTitle: {
        id: 'console.crash.finding.content-verification.title',
        defaultMessage: 'A jar failed signature verification',
    },
    contentVerificationAction: {
        id: 'console.crash.finding.content-verification.action',
        defaultMessage:
            'Remove and reinstall the file named in the evidence from a trusted source.',
    },
    optifineWorldTitle: {
        id: 'console.crash.finding.optifine-world.title',
        defaultMessage: 'OptiFine prevented the world from loading',
    },
    optifineWorldAction: {
        id: 'console.crash.finding.optifine-world.action',
        defaultMessage:
            'Remove OptiFine or install a build compatible with this Minecraft and Forge version.',
    },
    nightconfigBugTitle: {
        id: 'console.crash.finding.nightconfig-bug.title',
        defaultMessage: 'NightConfig could not read a configuration file',
    },
    nightconfigBugAction: {
        id: 'console.crash.finding.nightconfig-bug.action',
        defaultMessage:
            'Back up the config folder, remove the damaged configuration, and let the Mod regenerate it.',
    },
    modFilenameTitle: {
        id: 'console.crash.finding.mod-filename.title',
        defaultMessage: 'A Mod filename contains unsupported characters',
    },
    modFilenameAction: {
        id: 'console.crash.finding.mod-filename.action',
        defaultMessage: 'Rename or reinstall the Mod jar using a simple Latin-letter filename.',
    },
    definiteModTitle: {
        id: 'console.crash.finding.definite-mod.title',
        defaultMessage: 'A specific Mod caused the crash',
    },
    definiteModAction: {
        id: 'console.crash.finding.definite-mod.action',
        defaultMessage:
            'Update, repair, or temporarily remove the Mod identified by the evidence and matched jar.',
    },
    definiteModFabricTitle: {
        id: 'console.crash.finding.definite-mod-fabric.title',
        defaultMessage: 'Fabric identified a specific Mod failure',
    },
    definiteModFabricAction: {
        id: 'console.crash.finding.definite-mod-fabric.action',
        defaultMessage:
            'Update or temporarily remove the Mod identified by the Fabric loader evidence.',
    },
    intelDriverTitle: {
        id: 'console.crash.finding.intel-driver.title',
        defaultMessage: 'The Intel graphics driver crashed',
    },
    intelDriverAction: {
        id: 'console.crash.finding.intel-driver.action',
        defaultMessage:
            'Install a current Intel graphics driver or run Minecraft on another available GPU.',
    },
    amdDriverTitle: {
        id: 'console.crash.finding.amd-driver.title',
        defaultMessage: 'The AMD graphics driver crashed',
    },
    amdDriverAction: {
        id: 'console.crash.finding.amd-driver.action',
        defaultMessage:
            'Clean-install a current AMD graphics driver and retry without graphics overlays.',
    },
    nvidiaDriverTitle: {
        id: 'console.crash.finding.nvidia-driver.title',
        defaultMessage: 'The NVIDIA graphics driver crashed',
    },
    nvidiaDriverAction: {
        id: 'console.crash.finding.nvidia-driver.action',
        defaultMessage:
            'Clean-install a current NVIDIA graphics driver and retry without graphics overlays.',
    },
    manualDebugCrashTitle: {
        id: 'console.crash.finding.manual-debug-crash.title',
        defaultMessage: 'The debug crash shortcut was triggered',
    },
    manualDebugCrashAction: {
        id: 'console.crash.finding.manual-debug-crash.action',
        defaultMessage: 'Launch again and avoid holding the manual debug-crash key combination.',
    },
    suspectedModTitle: {
        id: 'console.crash.finding.suspected-mod.title',
        defaultMessage: 'The crash report suspects one or more Mods',
    },
    suspectedModAction: {
        id: 'console.crash.finding.suspected-mod.action',
        defaultMessage:
            'Update or temporarily remove the suspected and locally matched Mods, then retry.',
    },
    modInitializationTitle: {
        id: 'console.crash.finding.mod-initialization.title',
        defaultMessage: 'A Mod failed to initialize',
    },
    modInitializationAction: {
        id: 'console.crash.finding.mod-initialization.action',
        defaultMessage:
            'Update the named Mod and verify that all of its required dependencies are installed.',
    },
    specificBlockTitle: {
        id: 'console.crash.finding.specific-block.title',
        defaultMessage: 'A specific block caused the crash',
    },
    specificBlockAction: {
        id: 'console.crash.finding.specific-block.action',
        defaultMessage:
            'Use a world backup or a world editor to remove the block at the coordinates in the evidence.',
    },
    specificEntityTitle: {
        id: 'console.crash.finding.specific-entity.title',
        defaultMessage: 'A specific entity caused the crash',
    },
    specificEntityAction: {
        id: 'console.crash.finding.specific-entity.action',
        defaultMessage:
            'Use a world backup or a world editor to remove the entity at the coordinates in the evidence.',
    },
})
