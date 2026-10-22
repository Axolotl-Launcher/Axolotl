import { defineMessage, defineMessages } from '@modrinth/ui'

export const retainedInstanceMessages = defineMessages({
    title: {
        id: 'app.content-install.retained-instance.title',
        defaultMessage: 'Created instance retained',
    },
    description: {
        id: 'app.content-install.retained-instance.description',
        defaultMessage:
            '{name} was created. The content installation was cancelled; you can manage or remove the instance from your library.',
    },
})

export const errorMessages = defineMessages({
    installFailed: {
        id: 'app.content-install.error.failed',
        defaultMessage: 'Could not install the selected content.',
    },
    missingCurseForgeFile: {
        id: 'app.content-install.error.missing-curseforge-file',
        defaultMessage:
            'The selected CurseForge project or file is unavailable. Reopen the project and try again.',
    },
    reviewRequired: {
        id: 'app.content-install.error.review-required',
        defaultMessage: 'Review the content changes before installing.',
    },
    previewUnavailable: {
        id: 'app.content-install.error.preview-unavailable',
        defaultMessage:
            'The installation preview is unavailable. Reopen the project and try again.',
    },
    unsupportedType: {
        id: 'app.content-install.error.unsupported-type',
        defaultMessage: 'This content type cannot be installed here.',
    },
    missingTarget: {
        id: 'app.content-install.error.missing-target',
        defaultMessage: 'The selected project or instance is no longer available.',
    },
    missingVersion: {
        id: 'app.content-install.error.missing-version',
        defaultMessage: 'The selected release is no longer available.',
    },
    unsupportedProvider: {
        id: 'app.content-install.error.unsupported-provider',
        defaultMessage: 'This content source cannot be used for this installation.',
    },
    projectNotFound: {
        id: 'app.content-install.error.project-not-found',
        defaultMessage: 'Could not find project {project}.',
    },
    invalidCurseForgeProject: {
        id: 'app.content-install.error.invalid-curseforge-project',
        defaultMessage:
            'The CurseForge project reference is invalid. Reopen the project and try again.',
    },
    noCurseForgeFiles: {
        id: 'app.content-install.error.no-curseforge-files',
        defaultMessage: 'This CurseForge project has no available files.',
    },
    reinstallUnavailable: {
        id: 'app.content-install.error.reinstall-unavailable',
        defaultMessage:
            'The selected CurseForge modpack release is unavailable for reinstallation.',
    },
    missingGameVersion: {
        id: 'app.content-install.error.missing-game-version',
        defaultMessage: 'This CurseForge modpack does not specify a Minecraft version.',
    },
})

export const noCompatibleVersionsMessage = defineMessage({
    id: 'app.content-install.no-compatible-versions',
    defaultMessage:
        'No available versions match {compatibilityLabel}. Select a version to install anyway. Matching dependencies will still be installed.',
})
export const curseForgeWorldInvalidProjectMessage = defineMessage({
    id: 'app.worlds.install-map.invalid-project',
    defaultMessage: 'The selected project is not a CurseForge map.',
})
export const curseForgeWorldUnavailableMessage = defineMessage({
    id: 'app.browse.maps-no-installable-file',
    defaultMessage: 'The selected CurseForge map does not have an installable file.',
})
export const curseForgeWorldInstanceNotReadyMessage = defineMessage({
    id: 'app.worlds.install-map.instance-not-ready',
    defaultMessage: 'Wait for this instance to finish installing before adding a map.',
})
export const curseForgeWorldUnknownInstanceMessage = defineMessage({
    id: 'app.worlds.install-map.unknown-instance',
    defaultMessage: 'The selected instance is no longer available.',
})
export const manualDownloadsTitleMessage = defineMessage({
    id: 'app.curseforge.manual-downloads.notification-title',
    defaultMessage: 'Some CurseForge files need manual download',
})
export const manualDownloadsPartialMessage = defineMessage({
    id: 'app.curseforge.manual-downloads.notification-partial',
    defaultMessage:
        'Installed {installed, number} files automatically, but {manual, number} could not be downloaded ({list}). Open the download list to finish those files.',
})
export const manualDownloadsFailedMessage = defineMessage({
    id: 'app.curseforge.manual-downloads.notification-failed',
    defaultMessage:
        '{manual, number} CurseForge files could not be downloaded automatically ({list}). Open the download list to install them manually.',
})
export const manualDownloadsListAndMoreMessage = defineMessage({
    id: 'app.curseforge.manual-downloads.list-and-more',
    defaultMessage: '{list}, and {count, number} more',
})
export const manualDownloadsFilesCountMessage = defineMessage({
    id: 'app.curseforge.manual-downloads.files-count',
    defaultMessage: '{count, number} files',
})
export const manualDownloadsImportedTitleMessage = defineMessage({
    id: 'app.curseforge.manual-downloads.imported-title',
    defaultMessage: 'CurseForge files imported',
})
export const manualDownloadsImportedMessage = defineMessage({
    id: 'app.curseforge.manual-downloads.imported',
    defaultMessage: 'Imported {count, number} downloaded files into the instance.',
})
export const automaticDownloadsFailedTitleMessage = defineMessage({
    id: 'app.curseforge.automatic-downloads-failed.notification-title',
    defaultMessage: 'Some CurseForge downloads failed',
})
export const automaticDownloadsFailedMessage = defineMessage({
    id: 'app.curseforge.automatic-downloads-failed.notification-body',
    defaultMessage:
        '{failed, number} files failed after retrying ({list}). See Downloads for the recorded errors.',
})
export const dependencyNotesTitleMessage = defineMessage({
    id: 'app.curseforge.dependency-notes.notification-title',
    defaultMessage: 'CurseForge dependency notes',
})
export const dependencyNotesMessage = defineMessage({
    id: 'app.curseforge.dependency-notes.notification-body',
    defaultMessage:
        '{optional, plural, =0 {No optional dependencies were skipped.} one {# optional dependency was skipped.} other {# optional dependencies were skipped.}} {incompatible, plural, =0 {No incompatible dependencies were detected.} one {# incompatible dependency was detected.} other {# incompatible dependencies were detected.}} {skipped, plural, =0 {No other dependencies were skipped.} one {# dependency was skipped.} other {# dependencies were skipped.}}',
})
export const dependenciesInstalledTitleMessage = defineMessage({
    id: 'app.content-install.dependencies-installed.notification-title',
    defaultMessage: 'Dependencies installed',
})
export const dependenciesInstalledMessage = defineMessage({
    id: 'app.content-install.dependencies-installed.notification-body',
    defaultMessage:
        'Installed {count, plural, one {# dependency} other {# dependencies}} automatically: {list}',
})
export const dependenciesSkippedTitleMessage = defineMessage({
    id: 'app.content-install.dependencies-skipped.notification-title',
    defaultMessage: 'Some dependencies were skipped',
})
export const dependenciesSkippedMessage = defineMessage({
    id: 'app.content-install.dependencies-skipped.notification-body',
    defaultMessage: 'The following dependencies were not installed: {list}',
})
export const sha1VerifiedModrinthFallbackMessage = defineMessage({
    id: 'app.content-install.preview.sha1-verified-modrinth-fallback',
    defaultMessage: 'Modrinth fallback verified by SHA-1',
})
export const unavailableCurseForgeProjectMessage = defineMessage({
    id: 'app.content-install.preview.unavailable-curseforge-project',
    defaultMessage: 'Unavailable CurseForge project',
})
export const skippedReasonMessages = {
    already_installed: defineMessage({
        id: 'app.content-install.preview.skip.already-installed',
        defaultMessage: 'Already installed',
    }),
    no_compatible_version: defineMessage({
        id: 'app.content-install.preview.skip.no-compatible-version',
        defaultMessage: 'No compatible version found',
    }),
    modrinth_lookup_failed: defineMessage({
        id: 'app.content-install.preview.skip.modrinth-lookup-failed',
        defaultMessage: 'Could not verify the Modrinth fallback',
    }),
    embedded: defineMessage({
        id: 'app.content-install.preview.skip.embedded',
        defaultMessage: 'Embedded in the project',
    }),
    tool: defineMessage({
        id: 'app.content-install.preview.skip.tool',
        defaultMessage: 'Development tool, not installed',
    }),
    unsupported_project_type: defineMessage({
        id: 'app.content-install.preview.skip.unsupported-project-type',
        defaultMessage: 'Unsupported project type',
    }),
    optional: defineMessage({
        id: 'app.content-install.preview.skip.optional',
        defaultMessage: 'Optional dependency',
    }),
    incompatible: defineMessage({
        id: 'app.content-install.preview.skip.incompatible',
        defaultMessage: 'Incompatible dependency',
    }),
    missing_version: defineMessage({
        id: 'app.content-install.preview.skip.missing-version',
        defaultMessage: 'Referenced version was not found',
    }),
    conflicting_dependency: defineMessage({
        id: 'app.content-install.preview.skip.conflicting-dependency',
        defaultMessage: 'Conflicting dependency version',
    }),
    duplicate_project: defineMessage({
        id: 'app.content-install.preview.skip.duplicate-project',
        defaultMessage: 'Already included',
    }),
    quilt_fabric_api: defineMessage({
        id: 'app.content-install.preview.skip.quilt-fabric-api',
        defaultMessage: 'Replaced for quilt',
    }),
    excluded_by_user: defineMessage({
        id: 'app.content-install.preview.skip.excluded-by-user',
        defaultMessage: 'Excluded by user',
    }),
    dependency_cycle: defineMessage({
        id: 'app.content-install.preview.skip.dependency-cycle',
        defaultMessage: 'Dependency cycle detected',
    }),
    dependency_depth_exceeded: defineMessage({
        id: 'app.content-install.preview.skip.dependency-depth-exceeded',
        defaultMessage: 'Dependency depth limit reached',
    }),
} as const
export const curseForgeNetworkFailureTitleMessage = defineMessage({
    id: 'app.curseforge.network-download-failed.notification-title',
    defaultMessage: 'Could not download from CurseForge',
})
export const curseForgeNetworkFailureMessage = defineMessage({
    id: 'app.curseforge.network-download-failed.notification-body',
    defaultMessage:
        'Could not connect to CurseForge to download this file. Your network or proxy may be blocking CurseForge. Turn off or change your VPN/proxy, try another network, then retry the download.',
})
export const modpackInstalledTitleMessage = defineMessage({
    id: 'app.curseforge.modpack-installed.title',
    defaultMessage: 'CurseForge modpack installed',
})
export const modpackInstalledBodyMessage = defineMessage({
    id: 'app.curseforge.modpack-installed.body',
    defaultMessage: 'Installed {count, number} content files from CurseForge.',
})
