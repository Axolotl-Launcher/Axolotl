import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

import zhCN from '~/locales/launcher-preview/discover.zh-CN'

import { registerPreviewMessages } from '../../lib/i18n'

registerPreviewMessages('zh-CN', zhCN)

/**
 * Strings of Discover, Favorites, the project pages and the content install flow that the
 * desktop app owns (apps/app-frontend pages, components and providers). Ids and English text
 * are the app's own; strings of the shared browse layout live with the ported components
 * under their @modrinth/ui ids, which the website already translates.
 */
export const messages = defineMessages({
    tabModpacks: { id: 'app.browse.project-type.modpacks', defaultMessage: 'Modpacks' },
    tabMods: { id: 'app.browse.project-type.mods', defaultMessage: 'Mods' },
    tabResourcepacks: {
        id: 'app.browse.project-type.resourcepacks',
        defaultMessage: 'Resource Packs',
    },
    tabDatapacks: { id: 'app.browse.project-type.datapacks', defaultMessage: 'Data Packs' },
    tabMaps: { id: 'app.browse.project-type.maps', defaultMessage: 'Maps' },
    tabShaders: { id: 'app.browse.project-type.shaders', defaultMessage: 'Shaders' },
    tabServers: { id: 'app.browse.project-type.servers', defaultMessage: 'Servers' },
    tabFavorites: { id: 'app.browse.project-type.favorites', defaultMessage: 'Favorites' },
    discoverContent: { id: 'app.browse.discover-content', defaultMessage: 'Discover content' },
    discoverServers: { id: 'app.browse.discover-servers', defaultMessage: 'Discover servers' },
    discoverMaps: { id: 'app.browse.discover-maps', defaultMessage: 'Discover maps' },
    mapsUnavailable: { id: 'app.browse.maps-unavailable', defaultMessage: 'Maps unavailable' },
    mapsUnavailableDescription: {
        id: 'app.browse.maps-unavailable-description',
        defaultMessage: 'Configure a CurseForge API key to browse and install maps.',
    },
    serverSourceUnavailable: {
        id: 'app.browse.server-source-unavailable',
        defaultMessage: 'The server content source is unavailable. Try again later.',
    },
    sourceFailed: {
        id: 'app.browse.source-failed',
        defaultMessage:
            '{source} could not return results for this search. Check the filters or try again later.',
    },
    add: { id: 'app.browse.add', defaultMessage: 'Add' },
    chooseInstance: { id: 'app.browse.choose-instance', defaultMessage: 'Choose instance' },
    installSelected: {
        id: 'app.browse.install-selected',
        defaultMessage: 'Install {count} content',
    },
    preparingSelected: {
        id: 'app.browse.preparing-selected',
        defaultMessage: 'Preparing {completed}/{total}',
    },
    selected: { id: 'app.browse.selected', defaultMessage: 'Selected' },
    addToFavorites: { id: 'app.content-favorites.add', defaultMessage: 'Add to favorites' },
    removeFromFavorites: {
        id: 'app.content-favorites.remove',
        defaultMessage: 'Remove from favorites',
    },
    favoritesLoading: {
        id: 'app.content-favorites.loading',
        defaultMessage: 'Updating favorites…',
    },
    noCompatibleVersion: {
        id: 'app.browse.no-compatible-version',
        defaultMessage: 'No compatible version was found for the selected instance.',
    },
    addServersToInstance: {
        id: 'app.browse.add-servers-to-instance',
        defaultMessage: 'Adding server to instance',
    },
    addToAnInstance: { id: 'app.browse.add-to-an-instance', defaultMessage: 'Add to an instance' },
    environmentProvidedByServer: {
        id: 'search.filter.locked.server-environment.title',
        defaultMessage: 'Only client-side mods can be added to the server instance',
    },
    gameVersionProvidedByInstance: {
        id: 'search.filter.locked.instance-game-version.title',
        defaultMessage: 'Game version is provided by the instance',
    },
    gameVersionProvidedByServer: {
        id: 'search.filter.locked.server-game-version.title',
        defaultMessage: 'Game version is provided by the server',
    },
    hideAddedServers: {
        id: 'app.browse.hide-added-servers',
        defaultMessage: 'Hide servers already added',
    },
    installingToServer: { id: 'app.browse.server.installing', defaultMessage: 'Installing' },
    backToInstance: { id: 'app.browse.back-to-instance', defaultMessage: 'Back to instance' },
    serverInstanceContentWarning: {
        id: 'app.browse.server-instance-content-warning',
        defaultMessage:
            'Adding content can break compatibility when joining the server. Any added content will also be lost when you update the server instance content.',
    },
    modLoaderProvidedByInstance: {
        id: 'search.filter.locked.instance-loader.title',
        defaultMessage: 'Loader is provided by the instance',
    },
    modLoaderProvidedByServer: {
        id: 'search.filter.locked.server-loader.title',
        defaultMessage: 'Loader is provided by the server',
    },
    providedByInstance: {
        id: 'search.filter.locked.instance',
        defaultMessage: 'Provided by the instance',
    },
    providedByServer: {
        id: 'search.filter.locked.server',
        defaultMessage: 'Provided by the server',
    },
    syncFilterButton: {
        id: 'search.filter.locked.instance.sync',
        defaultMessage: 'Sync with instance',
    },
    allSources: { id: 'app.browse.source.all', defaultMessage: 'All sources' },
    modrinthSource: { id: 'app.browse.source.modrinth', defaultMessage: 'Modrinth' },
    mcArchiveSource: { id: 'app.browse.source.mcarchive', defaultMessage: 'MCArchive' },
    listView: { id: 'app.browse.display-mode.list', defaultMessage: 'List' },
    compactListView: { id: 'app.browse.display-mode.compact-list', defaultMessage: 'Compact list' },
    gridView: { id: 'app.browse.display-mode.grid', defaultMessage: 'Grid' },
    switchView: { id: 'app.browse.display-mode.switch', defaultMessage: 'Switch view' },
    openInMcmod: { id: 'app.project.open-in-mcmod', defaultMessage: 'Open in MC Mod' },
    translateProject: { id: 'app.project.translation.translate', defaultMessage: 'Translate' },
    showOriginal: { id: 'app.project.translation.show-original', defaultMessage: 'Show original' },
    translating: { id: 'app.project.translation.translating', defaultMessage: 'Translating…' },
    autoTranslateHintTitle: {
        id: 'app.translation.auto-translate-hint.title',
        defaultMessage: 'Use automatic translation',
    },
    autoTranslateHintText: {
        id: 'app.translation.auto-translate-hint.text',
        defaultMessage:
            'You can turn this on in Settings → Language & translation so browse results and project pages translate for you.',
    },
    favoritesTitle: { id: 'app.content-favorites.title', defaultMessage: 'Favorites' },
    favoritesSearch: { id: 'app.content-favorites.search', defaultMessage: 'Search favorites' },
    allContentTypes: { id: 'app.content-favorites.type.all', defaultMessage: 'All content types' },
    recentlySaved: {
        id: 'app.content-favorites.sort.recently-saved',
        defaultMessage: 'Recently saved',
    },
    favoritesBrowseResources: {
        id: 'app.content-favorites.browse-resources',
        defaultMessage: 'Browse resources',
    },
    favoritesClearFilters: {
        id: 'app.content-favorites.clear-filters',
        defaultMessage: 'Clear search and filters',
    },
    favoritesEmptyTitle: {
        id: 'app.content-favorites.empty-title',
        defaultMessage: 'No favorites yet',
    },
    favoritesEmptyDescription: {
        id: 'app.content-favorites.empty-description',
        defaultMessage:
            'Bookmark mods, resource packs, data packs, and shaders to install them later.',
    },
    favoritesNoMatchesTitle: {
        id: 'app.content-favorites.no-matches-title',
        defaultMessage: 'No matching favorites',
    },
    favoritesNoMatchesDescription: {
        id: 'app.content-favorites.no-matches-description',
        defaultMessage: 'Try a different search or content type.',
    },
    favoritesUnavailableTitle: {
        id: 'app.content-favorites.unavailable-title',
        defaultMessage: '{provider} project {projectId}',
    },
    favoritesUnavailableDescription: {
        id: 'app.content-favorites.unavailable-description',
        defaultMessage: 'This project is currently unavailable. You can remove it from favorites.',
    },
    instanceSelectorTitle: {
        id: 'app.browse.instance-selector.title',
        defaultMessage: 'Choose an instance',
    },
    instanceSelectorSearch: {
        id: 'app.browse.instance-selector.search',
        defaultMessage: 'Search instances',
    },
    instanceSelectorNoInstances: {
        id: 'app.browse.instance-selector.no-instances',
        defaultMessage: 'Create an instance before installing content',
    },
    instanceSelectorNoResults: {
        id: 'app.browse.instance-selector.no-results',
        defaultMessage: 'No matching instances',
    },
    instanceSelectorSelect: {
        id: 'app.browse.instance-selector.select',
        defaultMessage: 'Choose {name}',
    },
    instanceSelectorCreate: {
        id: 'app.browse.instance-selector.create',
        defaultMessage: 'Create instance',
    },
    switchTargetTitle: {
        id: 'app.browse.instance-selector.switch-title',
        defaultMessage: 'Switch installation target?',
    },
    switchTargetDescription: {
        id: 'app.browse.instance-selector.switch-description',
        defaultMessage:
            '{count, plural, one {# selected project is} other {# selected projects are}} resolved for the current target. Switching will discard the resolved versions.',
    },
    currentTarget: {
        id: 'app.browse.instance-selector.current-target',
        defaultMessage: 'Current target',
    },
    newTarget: { id: 'app.browse.instance-selector.new-target', defaultMessage: 'New target' },
    installCurrent: {
        id: 'app.browse.instance-selector.install-current',
        defaultMessage: 'Install to current instance',
    },
    installingCurrent: {
        id: 'app.browse.instance-selector.installing-current',
        defaultMessage: 'Preparing installation…',
    },
    clearAndSwitch: {
        id: 'app.browse.instance-selector.clear-and-switch',
        defaultMessage: 'Clear and switch',
    },
    instanceSelectorCancel: { id: 'app.browse.instance-selector.cancel', defaultMessage: 'Cancel' },
    selectionNoTarget: {
        id: 'app.content-selection.no-target',
        defaultMessage: 'Select a target instance before adding content.',
    },
    selectionPreviewFailed: {
        id: 'app.content-selection.preview-failed',
        defaultMessage: 'Some selected content could not be prepared. Remove it or try again.',
    },
    selectionTargetChanged: {
        id: 'app.content-selection.target-changed',
        defaultMessage:
            'The selected content belongs to another instance. Switch back or clear it first.',
    },
    selectionDuplicate: {
        id: 'app.content-selection.duplicate-content',
        defaultMessage: '{project} is already installed or selected from another source.',
    },
    comingSoon: { id: 'app.project.coming-soon', defaultMessage: 'Coming soon' },
    loadFailed: {
        id: 'app.project.load-failed',
        defaultMessage: 'The project could not be loaded. Try again.',
    },
    backToBrowse: {
        id: 'app.project.install-context.back-to-browse',
        defaultMessage: 'Back to discover',
    },
    backToInstanceContent: {
        id: 'app.project.install-context.back-to-instance-content',
        defaultMessage: 'Back to instance content',
    },
    installContentToInstance: {
        id: 'app.project.install-context.install-content-to-instance',
        defaultMessage: 'Install content to instance',
    },
    alreadyInstalled: {
        id: 'app.project.install-button.already-installed',
        defaultMessage: 'This project is already installed',
    },
    switchVersion: {
        id: 'app.project.install-button.switch-version',
        defaultMessage: 'Switch version',
    },
    projectNoCompatibleVersion: {
        id: 'app.project.install-button.no-compatible-version',
        defaultMessage: 'No compatible version was found for this instance.',
    },
    translationFailed: {
        id: 'app.project.translation.failed',
        defaultMessage: 'Translation failed. The original content was kept. Try again.',
    },
    translationFailedTitle: {
        id: 'app.project.translation.failed-title',
        defaultMessage: 'Translation failed',
    },
    translationNetworkFailed: {
        id: 'app.translation.error.network',
        defaultMessage:
            'The translation service could not be reached. Check your network or proxy.',
    },
    viewDependents: { id: 'project.actions.view-dependents', defaultMessage: 'View dependents' },
    viewProjectTypeDependents: {
        id: 'project.actions.view-project-type-dependents',
        defaultMessage: 'View {projectType} dependents',
    },
    viewModpacks: { id: 'project.actions.view-modpacks', defaultMessage: 'View modpacks' },
    versionsAlreadyInstalled: {
        id: 'app.project.versions.already-installed',
        defaultMessage: 'Already installed',
    },
    allVersions: { id: 'app.project.version.all-versions', defaultMessage: 'All versions' },
    versionInstalling: { id: 'app.project.version.installing', defaultMessage: 'Installing' },
    downloadInBrowser: {
        id: 'app.project.version.download-in-browser',
        defaultMessage: 'Download in browser',
    },
    changelogTitle: { id: 'app.resources.changelog.title', defaultMessage: 'Changelog' },
    changelogNoReleases: {
        id: 'app.resources.changelog.no-releases',
        defaultMessage: 'No releases are available.',
    },
    changelogListFailed: {
        id: 'app.resources.changelog.list-failed',
        defaultMessage: 'Releases could not be loaded.',
    },
    changelogMore: { id: 'app.resources.changelog.more', defaultMessage: 'Load more releases' },
    changelogEmpty: {
        id: 'app.resources.changelog.empty',
        defaultMessage: 'No changelog was provided for this release.',
    },
    changelogFailed: {
        id: 'app.resources.changelog.failed',
        defaultMessage: 'The changelog could not be loaded.',
    },
    changelogTranslate: {
        id: 'app.resources.changelog.translate',
        defaultMessage: 'Translate changelog',
    },
    changelogOriginal: { id: 'app.resources.changelog.original', defaultMessage: 'Show original' },
    changelogTranslationFailed: {
        id: 'app.resources.changelog.translation-failed',
        defaultMessage: 'Translation failed. The original changelog is still available.',
    },
    changelogExhausted: {
        id: 'app.resources.changelog.exhausted',
        defaultMessage: 'This source’s release pagination limit has been reached.',
    },
    descriptionRetry: {
        id: 'app.resources.description.retry',
        defaultMessage: 'Retry description',
    },
    descriptionFailed: {
        id: 'app.resources.description.failed',
        defaultMessage: 'The description could not be loaded.',
    },
    zoomIn: { id: 'app.instance.screenshots.zoom-in', defaultMessage: 'View at full size' },
    zoomOut: { id: 'app.instance.screenshots.zoom-out', defaultMessage: 'Fit to window' },
    operationHeader: {
        id: 'app.resources.operation.header',
        defaultMessage: 'Review content changes',
    },
    operationLoading: {
        id: 'app.resources.operation.loading',
        defaultMessage: 'Resolving releases and dependencies…',
    },
    operationArtifact: {
        id: 'app.resources.operation.artifact',
        defaultMessage: '{source} · release {release} · {file}',
    },
    operationOptional: {
        id: 'app.resources.operation.optional',
        defaultMessage: 'Available dependencies',
    },
    operationSatisfied: {
        id: 'app.resources.operation.satisfied',
        defaultMessage: 'Already satisfied',
    },
    operationChanges: {
        id: 'app.resources.operation.changes',
        defaultMessage:
            '{count, plural, one {# file will change} other {# files will change}} in {instance}.',
    },
    operationAdd: { id: 'app.resources.operation.add', defaultMessage: 'Add {name}' },
    operationRemove: { id: 'app.resources.operation.remove', defaultMessage: 'Remove {name}' },
    operationRequiredBy: {
        id: 'app.resources.operation.required-by',
        defaultMessage: 'Required by {names}',
    },
    operationOptionalFor: {
        id: 'app.resources.operation.optional-for',
        defaultMessage: 'Optional dependency of {names}',
    },
    operationCancel: { id: 'app.resources.operation.cancel', defaultMessage: 'Cancel' },
    operationApply: { id: 'app.resources.operation.apply', defaultMessage: 'Queue changes' },
    operationEmpty: {
        id: 'app.resources.operation.empty',
        defaultMessage: 'No file changes are needed.',
    },
    operationUnknownDependencies: {
        id: 'launcher-preview.discover.operation.unknown-dependencies',
        defaultMessage:
            "{count, plural, one {# required dependency is} other {# required dependencies are}} not in the preview's data and will be left out. The desktop app installs them.",
    },
    operationWorld: {
        id: 'app.resources.operation.world',
        defaultMessage: 'Target world for data packs',
    },
    operationNoWorlds: {
        id: 'app.resources.operation.no-worlds',
        defaultMessage:
            'Create a singleplayer world in this instance before installing data packs.',
    },
    operationRetry: { id: 'app.resources.operation.retry', defaultMessage: 'Retry' },
    operationAssessment: {
        id: 'app.resources.operation.assessment',
        defaultMessage: 'Game version: {game}; loader: {loader}',
    },
    operationCompatibility: {
        id: 'app.resources.operation.compatibility',
        defaultMessage:
            '{state, select, compatible {Compatible} incompatible {Incompatible} other {Unknown}}',
    },
    operationAcceptDimension: {
        id: 'app.resources.operation.accept-dimension',
        defaultMessage:
            '{dimension, select, gameVersion {Accept Minecraft compatibility exception} other {Accept loader compatibility exception}}',
    },
    noCompatibleVersions: {
        id: 'app.content-install.no-compatible-versions',
        defaultMessage:
            'No available versions match {compatibilityLabel}. Select a version to install anyway. Matching dependencies will still be installed.',
    },
    installFailed: {
        id: 'app.content-install.error.failed',
        defaultMessage: 'Could not install the selected content.',
    },
    missingVersion: {
        id: 'app.content-install.error.missing-version',
        defaultMessage: 'The selected release is no longer available.',
    },
    projectNotFound: {
        id: 'app.content-install.error.project-not-found',
        defaultMessage: 'Could not find project {project}.',
    },
    modpackInstallTitle: { id: 'app.modpack-install.title', defaultMessage: 'Install modpack' },
    modpackVersion: { id: 'app.modpack-install.version', defaultMessage: 'Version' },
    modpackInstanceName: {
        id: 'app.modpack-install.instance-name',
        defaultMessage: 'Instance name',
    },
    modpackInstanceNamePlaceholder: {
        id: 'app.modpack-install.instance-name-placeholder',
        defaultMessage: 'My modpack instance',
    },
    modpackAlreadyInstalled: {
        id: 'app.modpack-install.already-installed',
        defaultMessage: 'This version is already installed',
    },
    modpackAlreadyInstalledDescription: {
        id: 'app.modpack-install.already-installed-description',
        defaultMessage:
            'Creating another instance will not change these existing instances: {names}',
    },
    modpackInstall: { id: 'app.modpack-install.install', defaultMessage: 'Install' },
    modpackSelectVersion: {
        id: 'app.modpack-install.select-version',
        defaultMessage: 'Select a version',
    },
    modpackFolderName: {
        id: 'app.modpack-install.folder-name',
        defaultMessage: 'Instance folder: {name}',
    },
    modpackFolderNameConflict: {
        id: 'app.modpack-install.folder-name-conflict',
        defaultMessage:
            'A folder with this name already exists. The instance folder will be: {name}',
    },
    retainedInstanceTitle: {
        id: 'app.content-install.retained-instance.title',
        defaultMessage: 'Created instance retained',
    },
    retainedInstanceDescription: {
        id: 'app.content-install.retained-instance.description',
        defaultMessage:
            '{name} was created. The content installation was cancelled; you can manage or remove the instance from your library.',
    },
})
