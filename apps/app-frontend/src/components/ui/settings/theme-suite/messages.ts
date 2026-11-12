import { defineMessages } from '@modrinth/ui'

export const themeSuiteMessages = defineMessages({
    title: { id: 'app.theme-suite.title', defaultMessage: 'Theme suite' },
    description: {
        id: 'app.theme-suite.description',
        defaultMessage:
            'Choose how the launcher looks. Theme packs can bring their own colors, styles and pages.',
    },
    overviewTitle: { id: 'app.theme-suite.overview-title', defaultMessage: 'Theme source' },
    overviewDescription: {
        id: 'app.theme-suite.overview-description',
        defaultMessage: 'Pick the colors and styling the launcher uses.',
    },
    builtInName: { id: 'app.theme-suite.built-in-name', defaultMessage: 'Built-in theme' },
    builtInDescription: {
        id: 'app.theme-suite.built-in-description',
        defaultMessage:
            'The default Axolotl colors, following the selected light, dark or OLED mode.',
    },
    installedTitle: { id: 'app.theme-suite.installed-title', defaultMessage: 'Installed packs' },
    noPacks: {
        id: 'app.theme-suite.no-packs',
        defaultMessage: 'No theme packs installed yet. Import a pack to get started.',
    },
    importPack: { id: 'app.theme-suite.import-pack', defaultMessage: 'Import pack' },
    exportSnapshot: { id: 'app.theme-suite.export-snapshot', defaultMessage: 'Export snapshot' },
    exportSnapshotUnavailable: {
        id: 'app.theme-suite.export-snapshot-unavailable',
        defaultMessage: 'Exporting a theme snapshot is not available yet.',
    },
    customColors: { id: 'app.theme-suite.custom-colors', defaultMessage: 'Custom colors' },
    customColorsDescription: {
        id: 'app.theme-suite.custom-colors-description',
        defaultMessage: 'Pick your own background, surface and text colors.',
    },
    removePack: { id: 'app.theme-suite.remove-pack', defaultMessage: 'Remove' },
    removePackConfirm: {
        id: 'app.theme-suite.remove-pack-confirm',
        defaultMessage: 'Remove {name}? This cannot be undone.',
    },
    removePackData: {
        id: 'app.theme-suite.remove-pack-data',
        defaultMessage: 'Also delete saved pack data',
    },
    removePackCancel: { id: 'app.theme-suite.remove-pack-cancel', defaultMessage: 'Keep pack' },
    packPermissions: { id: 'app.theme-suite.pack-permissions', defaultMessage: 'Permissions' },
    capabilityTokens: { id: 'app.theme-suite.capability-tokens', defaultMessage: 'Color tokens' },
    capabilityStyles: { id: 'app.theme-suite.capability-styles', defaultMessage: 'Styles' },
    capabilityPages: { id: 'app.theme-suite.capability-pages', defaultMessage: 'Pages' },
    importTitle: { id: 'app.theme-suite.import-title', defaultMessage: 'Import theme pack' },
    importDescription: {
        id: 'app.theme-suite.import-description',
        defaultMessage:
            'Theme packs are .zip archives that contain a manifest.json file describing their colors, styles and pages.',
    },
    chooseArchive: { id: 'app.theme-suite.choose-archive', defaultMessage: 'Choose file' },
    changeArchive: { id: 'app.theme-suite.change-archive', defaultMessage: 'Change file' },
    archiveHint: {
        id: 'app.theme-suite.archive-hint',
        defaultMessage: 'Select a theme pack archive (.zip) to install.',
    },
    archiveFilter: {
        id: 'app.theme-suite.archive-filter',
        defaultMessage: 'Theme pack archive',
    },
    archiveLimit: {
        id: 'app.theme-suite.archive-limit',
        defaultMessage: 'Archives can be up to {size}.',
    },
    noArchiveChosen: {
        id: 'app.theme-suite.no-archive-chosen',
        defaultMessage: 'No archive selected yet.',
    },
    invalidArchive: {
        id: 'app.theme-suite.invalid-archive',
        defaultMessage: 'Only .zip archives with a valid manifest.json file can be installed.',
    },
    previewTitle: { id: 'app.theme-suite.preview-title', defaultMessage: 'Ready to install' },
    install: { id: 'app.theme-suite.install', defaultMessage: 'Install' },
    installing: { id: 'app.theme-suite.installing', defaultMessage: 'Installing…' },
    installFailed: {
        id: 'app.theme-suite.install-failed',
        defaultMessage: 'The theme pack could not be installed.',
    },
    colorsTitle: { id: 'app.theme-suite.colors-title', defaultMessage: 'Custom colors' },
    colorsDescription: {
        id: 'app.theme-suite.colors-description',
        defaultMessage:
            'Adjust the colors used in {theme} mode. Saving switches the launcher to custom colors.',
    },
    themeLight: { id: 'app.theme-suite.theme-light', defaultMessage: 'light' },
    themeDark: { id: 'app.theme-suite.theme-dark', defaultMessage: 'dark' },
    background: { id: 'app.theme-suite.background', defaultMessage: 'Background' },
    surface: { id: 'app.theme-suite.surface', defaultMessage: 'Surface' },
    text: { id: 'app.theme-suite.text', defaultMessage: 'Text' },
    colorsPreview: { id: 'app.theme-suite.colors-preview', defaultMessage: 'Preview' },
    colorsPreviewText: {
        id: 'app.theme-suite.colors-preview-text',
        defaultMessage: 'The quick brown fox jumps over the lazy dog.',
    },
    resetColors: { id: 'app.theme-suite.reset-colors', defaultMessage: 'Reset' },
    saveColors: { id: 'app.theme-suite.save-colors', defaultMessage: 'Save' },
    close: { id: 'app.theme-suite.close', defaultMessage: 'Close' },
    done: { id: 'app.theme-suite.done', defaultMessage: 'Done' },
    activeBadge: { id: 'app.theme-suite.active-badge', defaultMessage: 'Active' },
    activate: { id: 'app.theme-suite.activate', defaultMessage: 'Use' },
    sizeLabel: { id: 'app.theme-suite.size-label', defaultMessage: 'Size: {size}' },
    versionLabel: { id: 'app.theme-suite.version-label', defaultMessage: 'Version {version}' },
    engineLabel: {
        id: 'app.theme-suite.engine-label',
        defaultMessage: 'Requires Axolotl {version}',
    },
    back: { id: 'app.theme-suite.back', defaultMessage: 'Back' },
    loading: { id: 'app.theme-suite.loading', defaultMessage: 'Loading theme packs…' },
    loadFailed: {
        id: 'app.theme-suite.load-failed',
        defaultMessage: 'Theme packs could not be loaded.',
    },
    noDescription: {
        id: 'app.theme-suite.no-description',
        defaultMessage: 'No description provided.',
    },
    missingPackTitle: {
        id: 'app.theme-suite.missing-pack-title',
        defaultMessage: 'Theme pack unavailable',
    },
    missingPackText: {
        id: 'app.theme-suite.missing-pack-text',
        defaultMessage:
            'The theme pack "{packId}" is no longer installed. The built-in theme is being used instead.',
    },
})
