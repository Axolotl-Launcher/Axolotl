import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

import zhCN from '~/locales/launcher-preview/create.zh-CN'

import { registerPreviewMessages } from '../../lib/i18n'

registerPreviewMessages('zh-CN', zhCN)

/**
 * Strings of the create-instance area, with the desktop app's own ids and English
 * text: `create.*` from `apps/app-frontend/src/pages/Create.vue`, `creation-flow.*`
 * from `packages/ui/src/components/flows/creation-flow-modal`, `drop.*` from the
 * drop-flow modals (`packages/ui/src/components/flows/drop`) and the drop help page,
 * `app.drop.*` / `app.symlink-capability.*` from the import notifications
 * (`useDropImport.ts`, `creation-modal.ts`), `app.instance.icon-picker.*` from
 * `InstanceIconPickerModal.vue` and `unknown-pack-warning-modal.*` from
 * `UnknownPackWarningModal.vue`. `launcher-preview.create.*` strings belong to the
 * simulated macOS folder panel, which the desktop app leaves to the OS.
 */
export const pageMessages = defineMessages({
    title: {
        id: 'create.title',
        defaultMessage: 'Create Instance',
    },
    subtitle: {
        id: 'create.subtitle',
        defaultMessage: 'Start a new adventure or bring your existing worlds',
    },
    newTitle: {
        id: 'create.new.title',
        defaultMessage: 'Start Fresh',
    },
    newDescription: {
        id: 'create.new.description',
        defaultMessage: 'Create a new Minecraft instance from scratch.',
    },
    importTitle: {
        id: 'create.import.title',
        defaultMessage: 'Import Existing',
    },
    importDescription: {
        id: 'create.import.description',
        defaultMessage: 'Import instances from other launchers or install a modpack.',
    },
    back: {
        id: 'create.back',
        defaultMessage: 'Back to Library',
    },
})

/** `creationFlowMessages` of `creation-flow-context.ts`. */
export const flowMessages = defineMessages({
    createInstanceTitle: {
        id: 'creation-flow.title.create-instance',
        defaultMessage: 'Create instance',
    },
    createInstanceButton: {
        id: 'creation-flow.button.create-instance',
        defaultMessage: 'Create instance',
    },
    importInstanceTitle: {
        id: 'creation-flow.title.import-instance',
        defaultMessage: 'Import instance',
    },
    chooseModpackTitle: {
        id: 'creation-flow.title.choose-modpack',
        defaultMessage: 'Choose modpack',
    },
})

export const setupTypeMessages = defineMessages({
    instanceTypeTitle: {
        id: 'creation-flow.modal.setup-type.title.instance',
        defaultMessage: 'Choose instance type',
    },
    customSetupTitle: {
        id: 'creation-flow.modal.setup-type.option.custom-setup.title',
        defaultMessage: 'Custom setup',
    },
    customSetupDescription: {
        id: 'creation-flow.modal.setup-type.option.custom-setup.description',
        defaultMessage: 'Start from scratch by picking a loader and game version.',
    },
    modpackBaseTitle: {
        id: 'creation-flow.modal.setup-type.option.modpack-base.title',
        defaultMessage: 'Install modpack',
    },
    modpackBaseDescription: {
        id: 'creation-flow.modal.setup-type.option.modpack-base.description',
        defaultMessage: 'Browse modpacks on Modrinth or import one from a file.',
    },
    importInstanceTitle: {
        id: 'creation-flow.modal.setup-type.option.import-instance.title',
        defaultMessage: 'Import instance',
    },
    importInstanceDescription: {
        id: 'creation-flow.modal.setup-type.option.import-instance.description',
        defaultMessage: 'Import an instance from Prism, CurseForge, or similar.',
    },
    instanceDescription: {
        id: 'creation-flow.modal.setup-type.instance.description',
        defaultMessage:
            'An instance is a Minecraft setup with a specific loader, version, and mods.',
    },
})

export const customSetupMessages = defineMessages({
    selectIcon: {
        id: 'creation-flow.modal.custom-setup.icon.select',
        defaultMessage: 'Select icon',
    },
    removeIcon: {
        id: 'creation-flow.modal.custom-setup.icon.remove',
        defaultMessage: 'Remove icon',
    },
    gameDirLabel: {
        id: 'creation-flow.modal.custom-setup.game-dir.label',
        defaultMessage: 'Game directory',
    },
    gameDirManaged: {
        id: 'creation-flow.modal.custom-setup.game-dir.managed',
        defaultMessage: 'Axolotl directory',
    },
    nameLabel: {
        id: 'creation-flow.modal.custom-setup.name.label',
        defaultMessage: 'Name',
    },
    instanceNamePlaceholder: {
        id: 'creation-flow.modal.custom-setup.name.placeholder',
        defaultMessage: 'Enter instance name',
    },
    loaderLabel: {
        id: 'creation-flow.modal.custom-setup.loader.label',
        defaultMessage: 'Mod Loader',
    },
    adjunctLabel: {
        id: 'creation-flow.modal.custom-setup.adjunct.label',
        defaultMessage: 'Additional loaders',
    },
    adjunctHint: {
        id: 'creation-flow.modal.custom-setup.adjunct.hint',
        defaultMessage: 'A compatible version will be selected before downloading.',
    },
    adjunctSelectGameVersion: {
        id: 'creation-flow.modal.custom-setup.adjunct.select-game-version',
        defaultMessage: 'Select a game version to check compatibility.',
    },
    adjunctCheckingCompatibility: {
        id: 'creation-flow.modal.custom-setup.adjunct.checking-compatibility',
        defaultMessage: 'Checking compatibility...',
    },
    adjunctPrimaryUnsupported: {
        id: 'creation-flow.modal.custom-setup.adjunct.primary-unsupported',
        defaultMessage: 'The selected mod loader does not support this game version.',
    },
    adjunctUnsupported: {
        id: 'creation-flow.modal.custom-setup.adjunct.unsupported',
        defaultMessage: 'No compatible {loader} version is available for this game version.',
    },
    adjunctCompatibilityLoadFailed: {
        id: 'creation-flow.modal.custom-setup.adjunct.compatibility-load-failed',
        defaultMessage: 'Failed to verify {loader} compatibility.',
    },
    adjunctOptiFabricUnsupported: {
        id: 'creation-flow.modal.custom-setup.adjunct.optifabric-unsupported',
        defaultMessage: 'OptiFine requires OptiFabric, but no compatible version is available.',
    },
    adjunctOptiFabricLoadFailed: {
        id: 'creation-flow.modal.custom-setup.adjunct.optifabric-load-failed',
        defaultMessage: 'Failed to verify OptiFabric compatibility.',
    },
    noVersionsAvailable: {
        id: 'creation-flow.modal.custom-setup.options.no-versions-available',
        defaultMessage: 'No versions available',
    },
    noGameVersionsForLoader: {
        id: 'creation-flow.modal.custom-setup.game-version.no-supported-versions',
        defaultMessage: 'No game versions support this loader',
    },
    gameVersionsLoadFailed: {
        id: 'creation-flow.modal.custom-setup.game-version.load-failed',
        defaultMessage: 'Failed to load game versions',
    },
    selectGameVersion: {
        id: 'creation-flow.modal.custom-setup.game-version.placeholder',
        defaultMessage: 'Select game version',
    },
    searchGameVersion: {
        id: 'creation-flow.modal.custom-setup.game-version.search-placeholder',
        defaultMessage: 'Search game version...',
    },
    loaderVersionLabel: {
        id: 'creation-flow.modal.custom-setup.loader-version.label',
        defaultMessage: 'Mod Loader version',
    },
    willInstallLoaderVersion: {
        id: 'creation-flow.modal.custom-setup.will-install-loader-version',
        defaultMessage: 'Will install: {loader} {version}',
    },
    selectLoaderVersion: {
        id: 'creation-flow.modal.custom-setup.loader-version.placeholder',
        defaultMessage: 'Select loader version',
    },
    searchLoaderVersion: {
        id: 'creation-flow.modal.custom-setup.loader-version.search-placeholder',
        defaultMessage: 'Search loader version...',
    },
    stableLoaderVersionType: {
        id: 'creation-flow.modal.custom-setup.loader-version-type.stable',
        defaultMessage: 'Stable',
    },
    latestLoaderVersionType: {
        id: 'creation-flow.modal.custom-setup.loader-version-type.latest',
        defaultMessage: 'Latest',
    },
    otherLoaderVersionType: {
        id: 'creation-flow.modal.custom-setup.loader-version-type.other',
        defaultMessage: 'Custom',
    },
    loaderUnsupportedTooltip: {
        id: 'creation-flow.modal.custom-setup.loader.unsupported-tooltip',
        defaultMessage: 'This loader does not support the selected game version',
    },
    loaderCompatibilityLoadFailed: {
        id: 'creation-flow.modal.custom-setup.loader.compatibility-load-failed',
        defaultMessage: 'Failed to verify loader compatibility',
    },
    noSuchVersionsAvailable: {
        id: 'creation-flow.modal.custom-setup.loader.no-such-versions-available',
        defaultMessage: 'No such versions available',
    },
    releaseVersionType: {
        id: 'creation-flow.modal.custom-setup.game-version-type.release',
        defaultMessage: 'Release',
    },
    snapshotVersionType: {
        id: 'creation-flow.modal.custom-setup.game-version-type.snapshot',
        defaultMessage: 'Snapshot',
    },
    alphaVersionType: {
        id: 'creation-flow.modal.custom-setup.game-version-type.alpha',
        defaultMessage: 'April Fools',
    },
    ancientVersionType: {
        id: 'creation-flow.modal.custom-setup.game-version-type.ancient',
        defaultMessage: 'Ancient',
    },
})

export const importStageMessages = defineMessages({
    selectFile: {
        id: 'creation-flow.modal.import-instance.select-file',
        defaultMessage: 'Select file to import',
    },
    selectFileDescription: {
        id: 'creation-flow.modal.import-instance.select-file.description',
        defaultMessage: 'Import a modpack file or launcher archive',
    },
    selectFolder: {
        id: 'creation-flow.modal.import-instance.select-folder',
        defaultMessage: 'Select folder to import',
    },
    selectFolderDescription: {
        id: 'creation-flow.modal.import-instance.select-folder.description',
        defaultMessage: 'Import a launcher folder or .minecraft folder',
    },
    importPrompt: {
        id: 'creation-flow.modal.import-instance.import-prompt',
        defaultMessage:
            'Drag & drop launcher folders, modpack files, or .minecraft folders to import an instance in one click',
    },
})

export const iconPickerMessages = defineMessages({
    title: {
        id: 'app.instance.icon-picker.title',
        defaultMessage: 'Create an instance icon',
    },
    description: {
        id: 'app.instance.icon-picker.description',
        defaultMessage: 'Combine a background and a Minecraft element, or upload your own image.',
    },
    background: {
        id: 'app.instance.icon-picker.background',
        defaultMessage: 'Background',
    },
    surpriseMe: {
        id: 'app.instance.icon-picker.surprise-me',
        defaultMessage: 'Surprise me',
    },
    upload: {
        id: 'app.instance.icon-picker.upload',
        defaultMessage: 'Upload image',
    },
    useIcon: {
        id: 'app.instance.icon-picker.use-icon',
        defaultMessage: 'Use this icon',
    },
    saving: {
        id: 'app.instance.icon-picker.saving',
        defaultMessage: 'Saving...',
    },
    loadError: {
        id: 'app.instance.icon-picker.load-error',
        defaultMessage: 'Failed to load the bundled icon.',
    },
    groupOriginal: {
        id: 'app.instance.icon-picker.group.original',
        defaultMessage: 'Original icons',
    },
    groupModrinth3d: {
        id: 'app.instance.icon-picker.group.modrinth-3d',
        defaultMessage: 'Modrinth 3D icons',
    },
})

export const iconBackgroundMessages = defineMessages({
    transparent: {
        id: 'app.instance.icon-picker.background.transparent',
        defaultMessage: 'Transparent',
    },
    grass: {
        id: 'app.instance.icon-picker.background.grass',
        defaultMessage: 'Grass',
    },
    ocean: {
        id: 'app.instance.icon-picker.background.ocean',
        defaultMessage: 'Ocean',
    },
    amethyst: {
        id: 'app.instance.icon-picker.background.amethyst',
        defaultMessage: 'Amethyst',
    },
    sunset: {
        id: 'app.instance.icon-picker.background.sunset',
        defaultMessage: 'Sunset',
    },
    cherry: {
        id: 'app.instance.icon-picker.background.cherry',
        defaultMessage: 'Cherry',
    },
    nether: {
        id: 'app.instance.icon-picker.background.nether',
        defaultMessage: 'Nether',
    },
    slime: {
        id: 'app.instance.icon-picker.background.slime',
        defaultMessage: 'Slime',
    },
    deepDark: {
        id: 'app.instance.icon-picker.background.deep-dark',
        defaultMessage: 'Deep Dark',
    },
    stone: {
        id: 'app.instance.icon-picker.background.stone',
        defaultMessage: 'Stone',
    },
    midnight: {
        id: 'app.instance.icon-picker.background.midnight',
        defaultMessage: 'Midnight',
    },
})

export const confirmDropMessages = defineMessages({
    title: {
        id: 'drop.confirm.title',
        defaultMessage: 'What would you like to import this as?',
    },
    cancel: {
        id: 'drop.confirm.cancel',
        defaultMessage: 'Cancel',
    },
    help: {
        id: 'drop.confirm.help',
        defaultMessage: 'What can I drop?',
    },
    notThisType: {
        id: 'drop.confirm.not-this',
        defaultMessage: 'Import this as something else',
    },
    dotMinecraftTitle: {
        id: 'drop.confirm.as-dot-minecraft',
        defaultMessage: '.minecraft',
    },
    dotMinecraftDesc: {
        id: 'drop.confirm.as-dot-minecraft-desc',
        defaultMessage: 'Scan a .minecraft folder to create instances',
    },
    launcherTitle: {
        id: 'drop.confirm.as-launcher',
        defaultMessage: 'Launcher',
    },
    hmclLauncherTitle: {
        id: 'drop.confirm.as-hmcl-launcher',
        defaultMessage: 'HMCL Launcher',
    },
    modTitle: {
        id: 'drop.confirm.as-mod',
        defaultMessage: 'Mod',
    },
    modDesc: {
        id: 'drop.confirm.as-mod-desc',
        defaultMessage: 'Install a mod JAR to a Minecraft instance',
    },
    instanceTitle: {
        id: 'drop.confirm.as-instance',
        defaultMessage: 'Instance',
    },
    instanceDesc: {
        id: 'drop.confirm.as-instance-desc',
        defaultMessage: 'Import Minecraft instances from another launcher',
    },
    modpackTitle: {
        id: 'drop.confirm.as-modpack',
        defaultMessage: 'Modpack',
    },
    modpackDesc: {
        id: 'drop.confirm.as-modpack-desc',
        defaultMessage: 'Create a new instance from a modpack',
    },
    resourcePackTitle: {
        id: 'drop.confirm.as-resource-pack',
        defaultMessage: 'Resource Pack',
    },
    resourcePackDesc: {
        id: 'drop.confirm.as-resource-pack-desc',
        defaultMessage: 'Install a resource pack to an instance',
    },
    dataPackTitle: {
        id: 'drop.confirm.as-data-pack',
        defaultMessage: 'Data Pack',
    },
    dataPackDesc: {
        id: 'drop.confirm.as-data-pack-desc',
        defaultMessage: 'Install a data pack to an save',
    },
    shaderPackTitle: {
        id: 'drop.confirm.as-shader-pack',
        defaultMessage: 'Shader Pack',
    },
    shaderPackDesc: {
        id: 'drop.confirm.as-shader-pack-desc',
        defaultMessage: 'Install a shader pack to an instance',
    },
    worldTitle: {
        id: 'drop.confirm.as-world',
        defaultMessage: 'World Save',
    },
    worldDesc: {
        id: 'drop.confirm.as-world-desc',
        defaultMessage: 'Import a world save into an instance',
    },
    schematicTitle: {
        id: 'drop.confirm.as-schematic',
        defaultMessage: 'Schematic',
    },
    schematicDesc: {
        id: 'drop.confirm.as-schematic-desc',
        defaultMessage: 'Import a Litematic schematic',
    },
})

export const launcherImportMessages = defineMessages({
    title: {
        id: 'drop.launcher_import.title',
        defaultMessage: 'Import instances from {launcherName}',
    },
    subtitle: {
        id: 'drop.launcher_import.subtitle',
        defaultMessage: 'Scanned {count} instances in {basePath}',
    },
    selectAll: {
        id: 'drop.launcher_import.select_all',
        defaultMessage: 'Select All',
    },
    deselectAll: {
        id: 'drop.launcher_import.deselect_all',
        defaultMessage: 'Deselect All',
    },
    selected: {
        id: 'drop.launcher_import.selected',
        defaultMessage: '{n} selected',
    },
    cancel: {
        id: 'drop.launcher_import.cancel',
        defaultMessage: 'Cancel',
    },
    importAction: {
        id: 'drop.launcher_import.import',
        defaultMessage: 'Import ({n})',
    },
    noInstances: {
        id: 'drop.launcher_import.no_instances',
        defaultMessage: 'No instances found',
    },
})

export const symlinkMessages = defineMessages({
    title: {
        id: 'drop.symlink_method.title',
        defaultMessage: 'Choose import method',
    },
    subtitle: {
        id: 'drop.symlink_method.subtitle',
        defaultMessage: 'Importing {n} instance(s)',
    },
    method: {
        id: 'drop.symlink_method.method',
        defaultMessage: 'Import method',
    },
    gameDirLabel: {
        id: 'drop.symlink_method.game-dir.label',
        defaultMessage: 'Game directory',
    },
    gameDirIsolated: {
        id: 'drop.symlink_method.game-dir.isolated',
        defaultMessage: 'Version isolated',
    },
    gameDirNotIsolated: {
        id: 'drop.symlink_method.game-dir.not-isolated',
        defaultMessage: 'Version shared (.minecraft/)',
    },
    instance: {
        id: 'drop.symlink_method.instance',
        defaultMessage: 'Instance',
    },
    copyTitle: {
        id: 'drop.symlink_method.copy_title',
        defaultMessage: 'Copy files',
    },
    copyDesc: {
        id: 'drop.symlink_method.copy_desc',
        defaultMessage: 'Copy to Axolotl directory',
    },
    symlinkTitle: {
        id: 'drop.symlink_method.symlink_title',
        defaultMessage: 'Symbolic link',
    },
    symlinkDesc: {
        id: 'drop.symlink_method.symlink_desc',
        defaultMessage: 'Reference original location',
    },
    requiresAdmin: {
        id: 'drop.symlink_method.requires_admin',
        defaultMessage:
            'Administrator authorization (UAC) will be requested once when creating the link',
    },
    unsupportedWarning: {
        id: 'drop.symlink_method.unsupported_warning',
        defaultMessage: 'Symbolic links are not supported on this system',
    },
    gameVersionLabel: {
        id: 'drop.symlink_method.game_version',
        defaultMessage: 'Game version',
    },
    selectGameVersion: {
        id: 'drop.symlink_method.select_game_version',
        defaultMessage: 'Select game version',
    },
    searchGameVersion: {
        id: 'drop.symlink_method.search_game_version',
        defaultMessage: 'Search game versions',
    },
    loaderLabel: {
        id: 'drop.symlink_method.loader',
        defaultMessage: 'Loader',
    },
    loaderVersionLabel: {
        id: 'drop.symlink_method.loader_version',
        defaultMessage: 'Loader version',
    },
    latest: {
        id: 'drop.symlink_method.latest',
        defaultMessage: 'Latest',
    },
    selectLoaderVersion: {
        id: 'drop.symlink_method.select_loader_version',
        defaultMessage: 'Select loader version',
    },
    searchLoaderVersion: {
        id: 'drop.symlink_method.search_loader_version',
        defaultMessage: 'Search loader versions',
    },
    detecting: {
        id: 'drop.symlink_method.detecting',
        defaultMessage: 'Detecting...',
    },
    importPathLabel: {
        id: 'drop.symlink_method.import_path',
        defaultMessage: 'Version path',
    },
    importPathTooltip: {
        id: 'drop.symlink_method.import_path_tooltip',
        defaultMessage: 'Import path',
    },
    runtimeRootLabel: {
        id: 'drop.symlink_method.runtime_root',
        defaultMessage: 'Root',
    },
    runtimeRootTooltip: {
        id: 'drop.symlink_method.runtime_root_tooltip',
        defaultMessage: 'Runtime root',
    },
    statsLabel: {
        id: 'drop.symlink_method.stats',
        defaultMessage: 'Estimate',
    },
    statsCache: {
        id: 'drop.symlink_method.stats_cache',
        defaultMessage: 'Cache',
    },
    statsCacheTooltip: {
        id: 'drop.symlink_method.stats_cache_tooltip',
        defaultMessage: 'Axolotl cache',
    },
    statsLocal: {
        id: 'drop.symlink_method.stats_local',
        defaultMessage: 'Reuse',
    },
    statsLocalTooltip: {
        id: 'drop.symlink_method.stats_local_tooltip',
        defaultMessage: 'Local reuse',
    },
    statsNetwork: {
        id: 'drop.symlink_method.stats_network',
        defaultMessage: 'Download',
    },
    statsNetworkTooltip: {
        id: 'drop.symlink_method.stats_network_tooltip',
        defaultMessage: 'Needs download',
    },
    statsMigrate: {
        id: 'drop.symlink_method.stats_migrate',
        defaultMessage: 'Migrate',
    },
    statsMigrateTooltip: {
        id: 'drop.symlink_method.stats_migrate_tooltip',
        defaultMessage: 'Needs migration',
    },
    statsFiles: {
        id: 'drop.symlink_method.stats_files',
        defaultMessage: '{files} files',
    },
    notAvailable: {
        id: 'drop.symlink_method.not_available',
        defaultMessage: 'Unavailable',
    },
    loaderVersionUnknownTooltip: {
        id: 'drop.symlink_method.loader_version_unknown_tooltip',
        defaultMessage: 'Loader version not detected. A custom version will be used.',
    },
    loaderTypeUnknownTooltip: {
        id: 'drop.symlink_method.loader_type_unknown_tooltip',
        defaultMessage: 'Loader not recognized. Mods may not load correctly.',
    },
    missing: {
        id: 'drop.symlink_method.missing',
        defaultMessage: 'Missing',
    },
    unrecognized: {
        id: 'drop.symlink_method.unrecognized',
        defaultMessage: 'Unrecognized',
    },
    custom: {
        id: 'drop.symlink_method.custom',
        defaultMessage: 'Custom',
    },
    customTooltip: {
        id: 'drop.symlink_method.custom_tooltip',
        defaultMessage:
            'Usually you should not customize it, unless you are sure it was detected incorrectly.',
    },
    missingPath: {
        id: 'drop.symlink_method.missing_path',
        defaultMessage: 'No import path available',
    },
    planFailed: {
        id: 'drop.symlink_method.plan_failed',
        defaultMessage: 'Import estimate failed',
    },
    cancel: {
        id: 'drop.symlink_method.cancel',
        defaultMessage: 'Cancel',
    },
    resetChanges: {
        id: 'drop.symlink_method.reset_changes',
        defaultMessage: 'Reset changes',
    },
    confirm: {
        id: 'drop.symlink_method.confirm',
        defaultMessage: 'Confirm',
    },
    next: {
        id: 'drop.symlink_method.next',
        defaultMessage: 'Next',
    },
    instanceNavLabel: {
        id: 'drop.symlink_method.instance_nav_label',
        defaultMessage: 'Switch to {name}',
    },
})

export const genericInstallMessages = defineMessages({
    title: {
        id: 'drop.generic_install.title',
        defaultMessage: 'Install {type}',
    },
    searchPlaceholder: {
        id: 'drop.generic_install.search',
        defaultMessage: 'Search instances...',
    },
    install: {
        id: 'drop.generic_install.install',
        defaultMessage: 'Install',
    },
    cancel: {
        id: 'drop.generic_install.cancel',
        defaultMessage: 'Cancel',
    },
    createNew: {
        id: 'drop.generic_install.create_new',
        defaultMessage: 'Create new instance...',
    },
    noInstances: {
        id: 'drop.generic_install.no_instances',
        defaultMessage: 'No instances found',
    },
})

export const unknownPackMessages = defineMessages({
    header: {
        id: 'unknown-pack-warning-modal.header',
        defaultMessage: 'Confirm installation',
    },
    warningTitle: {
        id: 'unknown-pack-warning-modal.warning.title',
        defaultMessage: 'Unknown file warning',
    },
    warningBody: {
        id: 'unknown-pack-warning-modal.warning.body',
        defaultMessage: `We couldn't find this file on Modrinth. We strongly recommend only installing files from sources you trust.`,
    },
    body: {
        id: 'unknown-pack-warning-modal.body',
        defaultMessage: `A file is only reviewed if it’s uploaded to Modrinth, regardless of its file format (including .mrpack).`,
    },
    malwareStatement: {
        id: 'unknown-pack-warning-modal.malware-statement',
        defaultMessage: `Malware is often distributed through modpack files by sharing them on platforms like Discord.`,
    },
    dontShowAgain: {
        id: 'unknown-pack-warning-modal.dont-show-again',
        defaultMessage: `Don't show this warning again`,
    },
    installAnyway: {
        id: 'unknown-pack-warning-modal.install-anyway',
        defaultMessage: `Install anyway`,
    },
})

/** `pages/help/DropHelp.vue`. */
export const dropHelpMessages = defineMessages({
    title: {
        id: 'drop.help.title',
        defaultMessage: 'What can I drop into Axolotl?',
    },
    instancesTitle: {
        id: 'drop.help.instances-title',
        defaultMessage: 'Launcher Instances',
    },
    instancesDesc: {
        id: 'drop.help.instances-desc',
        defaultMessage:
            'Drag folders from other launchers (MultiMC, Prism Launcher, HMCL, etc.) to import Minecraft instances.',
    },
    instancesFormats: {
        id: 'drop.help.instances-formats',
        defaultMessage: 'MultiMC / Prism Launcher / PolyMC / HMCL instance folders',
    },
    modsTitle: {
        id: 'drop.help.mods-title',
        defaultMessage: 'Mods',
    },
    modsDesc: {
        id: 'drop.help.mods-desc',
        defaultMessage: 'Drag mod JAR files to install them into a Minecraft instance.',
    },
    modsFormats: {
        id: 'drop.help.mods-formats',
        defaultMessage: '.jar',
    },
    modpacksTitle: {
        id: 'drop.help.modpacks-title',
        defaultMessage: 'Modpacks',
    },
    modpacksDesc: {
        id: 'drop.help.modpacks-desc',
        defaultMessage: 'Drag modpack ZIP/MRPACK files to create a new instance.',
    },
    modpacksFormats: {
        id: 'drop.help.modpacks-formats',
        defaultMessage: '.mrpack / .zip',
    },
    resourcePacksTitle: {
        id: 'drop.help.resource-packs-title',
        defaultMessage: 'Resource Packs',
    },
    resourcePacksDesc: {
        id: 'drop.help.resource-packs-desc',
        defaultMessage: 'Drag resource pack ZIP files to install them into an instance.',
    },
    resourcePacksFormats: {
        id: 'drop.help.resource-packs-formats',
        defaultMessage: '.zip',
    },
    dataPacksTitle: {
        id: 'drop.help.data-packs-title',
        defaultMessage: 'Data Packs',
    },
    dataPacksDesc: {
        id: 'drop.help.data-packs-desc',
        defaultMessage: "Drag data pack ZIP files to install them into a world's datapacks folder.",
    },
    dataPacksFormats: {
        id: 'drop.help.data-packs-formats',
        defaultMessage: '.zip',
    },
    shaderPacksTitle: {
        id: 'drop.help.shader-packs-title',
        defaultMessage: 'Shader Packs',
    },
    shaderPacksDesc: {
        id: 'drop.help.shader-packs-desc',
        defaultMessage: 'Drag shader pack ZIP files to install them into an instance.',
    },
    shaderPacksFormats: {
        id: 'drop.help.shader-packs-formats',
        defaultMessage: '.zip',
    },
    worldsTitle: {
        id: 'drop.help.worlds-title',
        defaultMessage: 'World Saves',
    },
    worldsDesc: {
        id: 'drop.help.worlds-desc',
        defaultMessage: 'Drag world save folders to import them into an instance.',
    },
    worldsFormats: {
        id: 'drop.help.worlds-formats',
        defaultMessage: 'Minecraft world save folder (level.dat)',
    },
    schematicsTitle: {
        id: 'drop.help.schematics-title',
        defaultMessage: 'Schematics',
    },
    schematicsDesc: {
        id: 'drop.help.schematics-desc',
        defaultMessage: 'Drag .litematic files to import them as schematics.',
    },
    schematicsFormats: {
        id: 'drop.help.schematics-formats',
        defaultMessage: '.litematic',
    },
})

/** Notifications of the import flow (`App.vue`, `useDropImport.ts`, `creation-modal.ts`). */
export const dropNotificationMessages = defineMessages({
    processing: {
        id: 'app.drop.processing',
        defaultMessage: 'Processing {name}...',
    },
    unknownTitle: {
        id: 'app.drop.error.unknown-title',
        defaultMessage: 'Unknown file type',
    },
    unknownText: {
        id: 'app.drop.error.unknown-text',
        defaultMessage: 'Could not determine what kind of file this is.',
    },
    unknownDepthText: {
        id: 'app.drop.error.unknown-depth-text',
        defaultMessage:
            'The archive is nested too deeply to analyze. Unpack it to a folder and try again.',
    },
    unknownEncryptedText: {
        id: 'app.drop.error.unknown-encrypted-text',
        defaultMessage: 'The archive contains encrypted files and cannot be analyzed.',
    },
    processFailedTitle: {
        id: 'app.drop.process-failed-title',
        defaultMessage: 'Failed to process file',
    },
    nestedUnpackTitle: {
        id: 'app.drop.nested-unpack-title',
        defaultMessage: 'Nested archives detected',
    },
    nestedUnpackText: {
        id: 'app.drop.nested-unpack-text',
        defaultMessage:
            'This archive contains nested archives ({size}) that must be unpacked to analyze. This may take some time. Continue?',
    },
    nestedUnpackButton: {
        id: 'app.drop.nested-unpack-button',
        defaultMessage: 'Continue analysis',
    },
    forceAnalysisTitle: {
        id: 'app.drop.unknown-force-analysis-title',
        defaultMessage: 'Unable to identify file type',
    },
    forceAnalysisText: {
        id: 'app.drop.unknown-force-analysis-text',
        defaultMessage:
            'This archive needs to be extracted and deeply analyzed to determine its content type. This may take a while. Force analysis?',
    },
    forceAnalysisButton: {
        id: 'app.drop.unknown-force-analysis-button',
        defaultMessage: 'Force analysis',
    },
    forceAnalyzing: {
        id: 'app.drop.unknown-force-analyzing',
        defaultMessage: 'Force analyzing archive...',
    },
    forceAnalysisFailedTitle: {
        id: 'app.drop.unknown-force-analysis-failed-title',
        defaultMessage: 'Analysis failed',
    },
    forceAnalysisFailedText: {
        id: 'app.drop.unknown-force-analysis-failed-text',
        defaultMessage: 'Could not identify the file type even after deep analysis.',
    },
    scanning: {
        id: 'app.drop.scanning',
        defaultMessage: 'Scanning for instances',
    },
    scanFailed: {
        id: 'app.drop.scan-failed',
        defaultMessage: 'Failed to scan for instances',
    },
    noInstances: {
        id: 'app.drop.no-instances',
        defaultMessage: 'No instances found',
    },
    extractFailed: {
        id: 'app.drop.extract-failed',
        defaultMessage: 'Failed to extract archive',
    },
    instanceImportedTitle: {
        id: 'app.drop.instance-imported-title',
        defaultMessage: 'Instance imported',
    },
    instanceImportedText: {
        id: 'app.drop.instance-imported-text',
        defaultMessage: '{name} imported successfully.',
    },
    importFailedTitle: {
        id: 'app.drop.import-failed-title',
        defaultMessage: 'Import failed',
    },
    importFailedText: {
        id: 'app.drop.import-failed-text',
        defaultMessage: 'Failed to import {name}: {error}',
    },
    importProgressTitle: {
        id: 'app.drop.import-progress-title',
        defaultMessage: 'Importing instances…',
    },
    importProgressText: {
        id: 'app.drop.import-progress-text',
        defaultMessage: '{current} / {total} instances imported',
    },
    importCompletedTitle: {
        id: 'app.drop.import-completed-title',
        defaultMessage: 'Import completed',
    },
    importCompletedText: {
        id: 'app.drop.import-completed-text',
        defaultMessage: 'Successfully imported {count} instances',
    },
    importCompletedPartialText: {
        id: 'app.drop.import-completed-partial-text',
        defaultMessage: 'Imported {completed} of {total} instances ({failed} failed)',
    },
    modpackInstallFailed: {
        id: 'app.drop.modpack-install-failed',
        defaultMessage: 'Failed to install modpack',
    },
    modpackInstalling: {
        id: 'app.drop.modpack-installing',
        defaultMessage: 'Installing modpack...',
    },
    modpackInstalled: {
        id: 'app.drop.modpack-installed-success',
        defaultMessage: 'Modpack installed successfully',
    },
    installingFile: {
        id: 'app.drop.installing-file',
        defaultMessage: 'Installing {name}...',
    },
    contentInstalledTitle: {
        id: 'app.drop.content-installed-title',
        defaultMessage: 'Content installed',
    },
    contentInstalledText: {
        id: 'app.drop.content-installed-text',
        defaultMessage: 'File has been installed to the instance.',
    },
    worldImportedTitle: {
        id: 'app.drop.world-imported-title',
        defaultMessage: 'World imported',
    },
    worldImportedText: {
        id: 'app.drop.world-imported-text',
        defaultMessage: 'World save has been imported successfully.',
    },
    installFailedTitle: {
        id: 'app.drop.install-failed-title',
        defaultMessage: 'Installation failed',
    },
    symlinkUnsupportedTitle: {
        id: 'app.symlink-capability.unsupported.title',
        defaultMessage: 'Shared instances are unavailable',
    },
    symlinkUnsupportedBody: {
        id: 'app.symlink-capability.unsupported',
        defaultMessage: 'This system does not support creating symbolic links.',
    },
})

/** The simulated macOS open panel that stands in for the native folder dialog. */
export const folderPanelMessages = defineMessages({
    cancel: {
        id: 'launcher-preview.create.folder-panel.cancel',
        defaultMessage: 'Cancel',
    },
    open: {
        id: 'launcher-preview.create.folder-panel.open',
        defaultMessage: 'Open',
    },
    back: {
        id: 'launcher-preview.create.folder-panel.back',
        defaultMessage: 'Back',
    },
    forward: {
        id: 'launcher-preview.create.folder-panel.forward',
        defaultMessage: 'Forward',
    },
    favorites: {
        id: 'launcher-preview.create.folder-panel.favorites',
        defaultMessage: 'Favorites',
    },
    name: {
        id: 'launcher-preview.create.folder-panel.name',
        defaultMessage: 'Name',
    },
    dateModified: {
        id: 'launcher-preview.create.folder-panel.date-modified',
        defaultMessage: 'Date Modified',
    },
    emptyFolder: {
        id: 'launcher-preview.create.folder-panel.empty',
        defaultMessage: 'This folder is empty',
    },
})
