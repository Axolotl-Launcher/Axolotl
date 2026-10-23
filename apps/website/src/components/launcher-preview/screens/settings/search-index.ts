import type { MessageDescriptor } from '@modrinth/ui/src/composables/i18n.ts'

import type { SettingsCategoryId } from './category-definitions'
import { messages } from './messages'

/**
 * Item-level settings search index, ported entry for entry from the desktop app
 * (apps/app-frontend/src/components/ui/settings/settings-search-index.ts). Each
 * entry names the category it lives in and the anchor its row renders, so a
 * result scrolls to and flashes that row.
 */
export interface SettingsSearchMessage extends MessageDescriptor {
    /**
     * The index's own fallback text where it differs from the catalogue. The app
     * adds a descriptor's `defaultMessage` to the searchable text, so it stays
     * findable by it.
     */
    indexDefault?: string
}

export interface SettingsSearchEntry {
    id: string
    categoryId: SettingsCategoryId
    targetId?: string
    label: SettingsSearchMessage
    description?: SettingsSearchMessage
    keywords?: SettingsSearchMessage[]
}

function message(descriptor: MessageDescriptor, indexDefault?: string): SettingsSearchMessage {
    return indexDefault === undefined ? descriptor : { ...descriptor, indexDefault }
}

export const settingsSearchEntries: SettingsSearchEntry[] = [
    {
        id: 'network-tests',
        categoryId: 'network-connection',
        targetId: 'settings-target-network-tests',
        label: message(messages.appSettingsNetworkTitle),
        keywords: [message(messages.appSettingsNetworkSearch)],
    },
    {
        id: 'appearance-color-theme',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-color-theme',
        label: message(messages.appAppearanceSettingsColorThemeTitle),
        description: message(messages.appAppearanceSettingsColorThemeDescription),
    },
    {
        id: 'appearance-accent-color',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-accent-color',
        label: message(messages.appAppearanceSettingsAccentColorTitle),
        description: message(messages.appAppearanceSettingsAccentColorDescription),
    },
    {
        id: 'appearance-launcher-background',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-launcher-background',
        label: message(messages.appAppearanceSettingsCustomBackgroundTitle),
        description: message(
            messages.appAppearanceSettingsCustomBackgroundDescription,
            'Choose a custom image and fine-tune how it blends with the launcher interface.',
        ),
    },
    {
        id: 'appearance-ui-font',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-ui-font',
        label: message(messages.appAppearanceSettingsFontsUiFont),
        description: message(messages.appAppearanceSettingsFontsUiFontDescription),
    },
    {
        id: 'appearance-mono-font',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-mono-font',
        label: message(messages.appAppearanceSettingsFontsMonoFont),
        description: message(messages.appAppearanceSettingsFontsMonoFontDescription),
    },
    {
        id: 'appearance-transparent-background',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-transparent-background',
        label: message(messages.appAppearanceSettingsTransparentBackgroundTitle),
        description: message(messages.appAppearanceSettingsTransparentBackgroundDescription),
    },
    {
        id: 'appearance-advanced-rendering',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-advanced-rendering',
        label: message(messages.appAppearanceSettingsAdvancedRenderingTitle),
        description: message(
            messages.appAppearanceSettingsAdvancedRenderingDescription,
            'Enable advanced visual effects that may affect performance.',
        ),
    },
    {
        id: 'appearance-page-transitions',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-page-transitions',
        label: message(messages.appAppearanceSettingsPageTransitionsTitle),
    },
    {
        id: 'appearance-hidden-nav-items',
        categoryId: 'home-navigation',
        targetId: 'settings-target-appearance-hidden-nav-items',
        label: message(messages.appAppearanceSettingsHiddenNavItemsTitle),
    },
    {
        id: 'appearance-home-layout',
        categoryId: 'home-navigation',
        targetId: 'settings-target-appearance-home-layout',
        label: message(messages.appAppearanceSettingsHomeLayoutTitle),
    },
    {
        id: 'appearance-default-landing-page',
        categoryId: 'home-navigation',
        targetId: 'settings-target-appearance-default-landing-page',
        label: message(messages.appAppearanceSettingsDefaultLandingPageTitle),
    },
    {
        id: 'appearance-sidebar-instance-limit',
        categoryId: 'home-navigation',
        targetId: 'settings-target-appearance-sidebar-instance-limit',
        label: message(messages.appAppearanceSettingsSidebarInstanceCountTitle),
    },
    {
        id: 'defaults-window-title',
        categoryId: 'launch-defaults',
        targetId: 'settings-target-defaults-window-title',
        label: message(messages.appSettingsDefaultsWindowTitleEnabled),
        description: message(messages.appSettingsDefaultsWindowTitleEnabledDescription),
    },
    {
        id: 'logs-level',
        categoryId: 'logs',
        targetId: 'settings-target-logs-level',
        label: message(messages.appSettingsLogsLevelTitle),
        description: message(messages.appSettingsLogsLevelDescription),
    },
    {
        id: 'logs-export',
        categoryId: 'logs',
        targetId: 'settings-target-logs-export',
        label: message(messages.appSettingsLogsExportTitle),
        description: message(messages.appSettingsLogsExportDescription),
    },
    {
        id: 'appearance-show-scroll-top',
        categoryId: 'home-navigation',
        targetId: 'settings-target-appearance-show-scroll-top',
        label: message(messages.appAppearanceSettingsShowScrollTopTitle),
        description: message(messages.appAppearanceSettingsShowScrollTopDescription),
    },
    {
        id: 'shortcuts-quick-scroll',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-enable',
        label: message(messages.appShortcutSettingsEnable),
        description: message(messages.appShortcutSettingsScrollDescription),
    },
    {
        id: 'shortcuts-scroll-home',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcutScrollTop',
        label: message(messages.appShortcutSettingsHomeAction),
        description: message(messages.appShortcutSettingsHomeActionDescription),
    },
    {
        id: 'shortcuts-scroll-end',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcutScrollBottom',
        label: message(messages.appShortcutSettingsEndAction),
        description: message(messages.appShortcutSettingsEndActionDescription),
    },
    {
        id: 'shortcuts-scroll-page-up',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcutScrollUp',
        label: message(messages.appShortcutSettingsPageUpAction),
        description: message(messages.appShortcutSettingsPageUpActionDescription),
    },
    {
        id: 'shortcuts-scroll-page-down',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcutScrollDown',
        label: message(messages.appShortcutSettingsPageDownAction),
        description: message(messages.appShortcutSettingsPageDownActionDescription),
    },
    {
        id: 'shortcuts-nav-home',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-nav',
        label: message(messages.appShortcutSettingsNavHome),
        description: message(messages.appShortcutSettingsNavHomeDescription),
    },
    {
        id: 'shortcuts-nav-worlds',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-nav',
        label: message(messages.appShortcutSettingsNavWorlds),
        description: message(messages.appShortcutSettingsNavWorldsDescription),
    },
    {
        id: 'shortcuts-nav-discover',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-nav',
        label: message(messages.appShortcutSettingsNavDiscover),
        description: message(messages.appShortcutSettingsNavDiscoverDescription),
    },
    {
        id: 'shortcuts-nav-skins',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-nav',
        label: message(messages.appShortcutSettingsNavSkins),
        description: message(messages.appShortcutSettingsNavSkinsDescription),
    },
    {
        id: 'shortcuts-nav-multiplayer',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-nav',
        label: message(messages.appShortcutSettingsNavMultiplayer),
        description: message(messages.appShortcutSettingsNavMultiplayerDescription),
    },
    {
        id: 'shortcuts-nav-library',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-nav',
        label: message(messages.appShortcutSettingsNavLibrary),
        description: message(messages.appShortcutSettingsNavLibraryDescription),
    },
    {
        id: 'shortcuts-nav-lab',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-nav',
        label: message(messages.appShortcutSettingsNavLab),
        description: message(messages.appShortcutSettingsNavLabDescription),
    },
    {
        id: 'shortcuts-nav-downloads',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-nav',
        label: message(messages.appShortcutSettingsNavDownloads),
        description: message(messages.appShortcutSettingsNavDownloadsDescription),
    },
    {
        id: 'shortcuts-nav-create',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-nav',
        label: message(messages.appShortcutSettingsNavCreate),
        description: message(messages.appShortcutSettingsNavCreateDescription),
    },
    {
        id: 'shortcuts-nav-settings',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-nav',
        label: message(messages.appShortcutSettingsNavSettings),
        description: message(messages.appShortcutSettingsNavSettingsDescription),
    },
    {
        id: 'appearance-native-decorations',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-native-decorations',
        label: message(messages.appAppearanceSettingsNativeDecorationsTitle),
    },
    {
        id: 'appearance-advanced-interface',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-advanced-interface',
        label: message(messages.appAppearanceSettingsSectionsAdvancedInterface),
    },
    {
        id: 'appearance-home-navigation',
        categoryId: 'home-navigation',
        targetId: 'settings-target-appearance-home-navigation',
        label: message(messages.appAppearanceSettingsSectionsHomeNavigation),
    },
    {
        id: 'appearance-content-behavior',
        categoryId: 'content-downloads',
        targetId: 'settings-target-appearance-content-behavior',
        label: message(messages.appAppearanceSettingsSectionsContentBehavior),
    },
    {
        id: 'appearance-close-behavior',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-close-behavior',
        label: message(messages.appAppearanceSettingsCloseBehaviorTitle),
        keywords: [
            message(messages.appAppearanceSettingsCloseBehaviorClose),
            message(messages.appAppearanceSettingsCloseBehaviorLightweight),
            message(messages.appAppearanceSettingsLightweightModeTitle, 'Lightweight mode'),
        ],
    },
    {
        id: 'launch-minimize-launcher',
        categoryId: 'launch-defaults',
        targetId: 'settings-target-launch-minimize',
        label: message(messages.appAppearanceSettingsMinimizeLauncherTitle),
    },
    {
        id: 'launch-lightweight-mode',
        categoryId: 'launch-defaults',
        targetId: 'settings-target-launch-lightweight-mode',
        label: message(messages.appAppearanceSettingsLightweightModeTitle),
    },
    {
        id: 'appearance-show-play-time',
        categoryId: 'home-navigation',
        targetId: 'settings-target-appearance-show-play-time',
        label: message(messages.appAppearanceSettingsShowPlayTimeTitle),
    },
    {
        id: 'appearance-hide-nametag',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-hide-nametag',
        label: message(messages.appAppearanceSettingsHideNametagTitle),
    },
    {
        id: 'appearance-unknown-pack-warning',
        categoryId: 'content-downloads',
        targetId: 'settings-target-appearance-unknown-pack-warning',
        label: message(messages.appAppearanceSettingsUnknownPackWarningTitle),
    },
    {
        id: 'content-auto-install-dependencies',
        categoryId: 'content-downloads',
        targetId: 'settings-target-content-auto-install-dependencies',
        label: message(messages.appAppearanceSettingsAutoInstallDependenciesTitle),
    },
    {
        id: 'content-skip-nonessential-warnings',
        categoryId: 'content-downloads',
        targetId: 'settings-target-content-skip-nonessential-warnings',
        label: message(messages.appAppearanceSettingsSkipNonEssentialWarningsTitle),
    },
    {
        id: 'language-launcher-language',
        categoryId: 'language-translation',
        targetId: 'settings-target-language',
        label: message(messages.appSettingsTabsLanguage),
    },
    {
        id: 'translation-service',
        categoryId: 'language-translation',
        targetId: 'settings-target-translation-service',
        label: message(messages.appTranslationSettingsProvider),
    },
    {
        id: 'translation-auto-translate',
        categoryId: 'language-translation',
        targetId: 'settings-target-translation-auto-translate',
        label: message(messages.appTranslationSettingsAutoTranslate),
    },
    {
        id: 'translation-cache',
        categoryId: 'language-translation',
        targetId: 'settings-target-translation-cache',
        label: message(messages.appTranslationSettingsCache),
    },
    {
        id: 'translation-display',
        categoryId: 'language-translation',
        targetId: 'settings-target-translation-display',
        label: message(messages.appTranslationSettingsDisplaySectionTitle),
    },
    {
        id: 'ai-providers',
        categoryId: 'ai',
        targetId: 'settings-target-ai-providers',
        label: message(messages.appAiSettingsTitle),
        keywords: [message(messages.appSettingsTabsAi, 'AI')],
    },
    {
        id: 'log-share',
        categoryId: 'launch-defaults',
        targetId: 'settings-target-log-share',
        label: message(messages.appLogShareSettingsTitle),
        keywords: [
            message(messages.appLogShareSettingsShareProvider),
            message(messages.appLogShareSettingsAiSource),
            message(messages.appLogagentSourceLocal),
            message(messages.appLogagentSourceCloud),
            message(messages.appLogagentSettingsAnalysisTitle),
            message(messages.appLogagentSettingsSharingTitle),
            message(messages.appLogagentSettingsDisplayTitle),
        ],
    },
    {
        id: 'shared-logs',
        categoryId: 'launch-defaults',
        targetId: 'settings-target-shared-logs',
        label: message(messages.appLogShareSharedLogsTitle),
        keywords: [message(messages.appLogShareSettingsTitle)],
    },
    {
        id: 'privacy-telemetry',
        categoryId: 'privacy-data',
        targetId: 'settings-target-privacy-telemetry',
        label: message(messages.appSettingsPrivacyTelemetry, 'Telemetry'),
    },
    {
        id: 'privacy-section',
        categoryId: 'privacy-data',
        targetId: 'settings-target-privacy',
        label: message(messages.appSettingsPrivacySectionTitle),
    },
    {
        id: 'privacy-discord-rpc',
        categoryId: 'privacy-data',
        targetId: 'settings-target-privacy-discord-rpc',
        label: message(messages.appSettingsPrivacyDiscordRpc, 'Discord rich presence'),
    },
    {
        id: 'privacy-external-scheme',
        categoryId: 'privacy-data',
        targetId: 'settings-target-privacy-external-scheme',
        label: message(messages.appSettingsPrivacyExternalScheme),
        description: message(
            messages.appSettingsPrivacyExternalSchemeDescription,
            'Respond to axolotl:// links from browsers and other apps.',
        ),
    },
    {
        id: 'privacy-privileged-scheme',
        categoryId: 'privacy-data',
        targetId: 'settings-target-privacy-privileged-scheme',
        label: message(messages.appSettingsPrivacyPrivilegedScheme),
        description: message(
            messages.appSettingsPrivacyPrivilegedSchemeDescription,
            'Let axolotl:// links change settings and stop game processes.',
        ),
    },
    {
        id: 'instance-sync-game-options',
        categoryId: 'instance-sync',
        targetId: 'settings-target-instance-sync-game_options',
        label: message(messages.appSettingsSyncedOptionsGameSettings),
        keywords: [message(messages.appSettingsTabsInstanceSync, 'Instance sync')],
    },
    {
        id: 'instance-sync-multiplayer-servers',
        categoryId: 'instance-sync',
        targetId: 'settings-target-instance-sync-multiplayer_servers',
        label: message(messages.appSettingsSyncedOptionsMultiplayerServers),
    },
    {
        id: 'instance-sync-resource-packs',
        categoryId: 'instance-sync',
        targetId: 'settings-target-instance-sync-resource_packs',
        label: message(messages.appSettingsSyncedOptionsResourcePacks),
    },
    {
        id: 'instance-sync-data-packs',
        categoryId: 'instance-sync',
        targetId: 'settings-target-instance-sync-data_packs',
        label: message(messages.appSettingsSyncedOptionsDataPacks),
    },
    {
        id: 'instance-sync-command-history',
        categoryId: 'instance-sync',
        targetId: 'settings-target-instance-sync-command_history',
        label: message(messages.appSettingsSyncedOptionsCommandHistory),
    },
    {
        id: 'instance-sync-creative-hotbars',
        categoryId: 'instance-sync',
        targetId: 'settings-target-instance-sync-creative_hotbars',
        label: message(messages.appSettingsSyncedOptionsCreativeHotbars),
    },
    {
        id: 'java-installations',
        categoryId: 'java-performance',
        label: message(messages.appSettingsTabsJavaInstallations),
    },
    {
        id: 'java-memory',
        categoryId: 'java-performance',
        targetId: 'settings-target-java-memory',
        label: message(messages.appSettingsDefaultsMemory),
    },
    {
        id: 'java-arguments',
        categoryId: 'java-performance',
        targetId: 'settings-target-java-arguments',
        label: message(messages.appSettingsDefaultsJavaArguments),
    },
    {
        id: 'defaults-window',
        categoryId: 'launch-defaults',
        targetId: 'settings-target-defaults-window',
        label: message(messages.appSettingsDefaultsFullscreen),
        keywords: [
            message(messages.appSettingsDefaultsWidth, 'Window width'),
            message(messages.appSettingsTabsDefaultInstanceOptions),
        ],
    },
    {
        id: 'defaults-environment',
        categoryId: 'launch-defaults',
        targetId: 'settings-target-defaults-environment',
        label: message(messages.appSettingsDefaultsEnvironmentVariables),
    },
    {
        id: 'defaults-launch-hooks',
        categoryId: 'launch-defaults',
        targetId: 'settings-target-defaults-launch-hooks',
        label: message(messages.appSettingsDefaultsPreLaunchHook),
        keywords: [
            message(messages.appSettingsDefaultsWrapperHook),
            message(messages.appSettingsDefaultsPostExitHook),
        ],
    },
    {
        id: 'defaults-after-launch',
        categoryId: 'launch-defaults',
        targetId: 'settings-target-defaults-after-launch',
        label: message(messages.appSettingsDefaultsSectionsAfterLaunch),
        keywords: [
            message(messages.appAppearanceSettingsLightweightModeTitle, 'Lightweight mode'),
            message(messages.appAppearanceSettingsMinimizeLauncherTitle),
        ],
    },
    {
        id: 'resources-download-mirrors',
        categoryId: 'content-downloads',
        targetId: 'settings-target-resources-download-mirrors',
        label: message(messages.appSettingsResourcesDownloadMirrors, 'Download mirrors'),
        keywords: [message(messages.appSettingsTabsResourceManagement)],
    },
    {
        id: 'resources-curseforge-api-key',
        categoryId: 'content-downloads',
        targetId: 'settings-target-resources-curseforge-api-key',
        label: message(messages.appSettingsResourcesCurseforgeApiKey),
        description: message(messages.appSettingsResourcesCurseforgeApiKeyDescription),
        keywords: [message(messages.appSettingsResourcesCurseforgeApiKeyField)],
    },
    {
        id: 'resources-chinese-names',
        categoryId: 'content-downloads',
        targetId: 'settings-target-resources-chinese-names',
        label: message(messages.appSettingsResourcesChineseNames),
    },
    {
        id: 'resources-preserve-names',
        categoryId: 'content-downloads',
        targetId: 'settings-target-resources-preserve-names',
        label: message(messages.appSettingsResourcesPreserveNames),
    },
    {
        id: 'resources-data-storage',
        categoryId: 'storage-backups',
        targetId: 'settings-target-resources-data-storage',
        label: message(messages.appSettingsResourcesSectionsDataStorage),
    },
    {
        id: 'resources-download-behavior',
        categoryId: 'content-downloads',
        targetId: 'settings-target-resources-download-behavior',
        label: message(messages.appSettingsResourcesSectionsDownloadBehavior),
    },
    {
        id: 'resources-network-auth',
        categoryId: 'network-multiplayer',
        targetId: 'settings-target-resources-network-auth',
        label: message(messages.appSettingsResourcesSectionsNetworkAuthentication),
    },
    {
        id: 'resources-missing-content',
        categoryId: 'content-downloads',
        targetId: 'settings-target-resources-missing-content',
        label: message(messages.appSettingsResourcesSectionsMissingContent),
    },
    {
        id: 'resources-ignore-ssl-errors',
        categoryId: 'content-downloads',
        targetId: 'settings-target-resources-ignore-ssl-errors',
        label: message(messages.appSettingsResourcesIgnoreSslErrors),
        description: message(messages.appSettingsResourcesIgnoreSslErrorsDescription),
        keywords: [message(messages.appSettingsResourcesProxySettings)],
    },
    {
        id: 'resources-download-concurrency',
        categoryId: 'content-downloads',
        targetId: 'settings-target-resources-maximum-downloads',
        label: message(messages.appSettingsResourcesMaximumDownloads),
        keywords: [message(messages.appSettingsResourcesMaximumWrites)],
    },
    {
        id: 'resources-proxy',
        categoryId: 'network-multiplayer',
        targetId: 'settings-target-resources-proxy',
        label: message(messages.appSettingsResourcesProxySettings),
        keywords: [
            message(messages.appSettingsResourcesProxyMode),
            message(messages.appSettingsTabsResourceManagement),
        ],
    },
    {
        id: 'network-mojang-auth-source',
        categoryId: 'network-multiplayer',
        targetId: 'settings-target-network-mojang-auth-source',
        label: message(messages.appSettingsResourcesMojangAuthService),
    },
    {
        id: 'resources-missing-content-import',
        categoryId: 'content-downloads',
        targetId: 'settings-target-resources-missing-content-import',
        label: message(messages.appSettingsResourcesMissingContentAutoImport),
    },
    {
        id: 'resources-database-backups',
        categoryId: 'storage-backups',
        targetId: 'settings-target-resources-database-backups',
        label: message(messages.appSettingsResourcesDatabaseBackups),
        keywords: [message(messages.appSettingsTabsResourceManagement)],
    },
    {
        id: 'storage-app-directory',
        categoryId: 'storage-backups',
        targetId: 'settings-target-storage-app-directory',
        label: message(messages.appSettingsResourcesAxolotlDataDirectory),
    },
    {
        id: 'storage-minecraft-directories',
        categoryId: 'storage-backups',
        targetId: 'settings-target-storage-minecraft-directories',
        label: message(messages.appSettingsResourcesMinecraftDirectories),
    },
    {
        id: 'storage-cache',
        categoryId: 'storage-backups',
        targetId: 'settings-target-storage-overview',
        label: message(messages.appSettingsResourcesAppCache),
    },
    {
        id: 'multiplayer-public-nodes',
        categoryId: 'network-multiplayer',
        targetId: 'terracotta-public-nodes-title',
        label: message(messages.appMultiplayerTerracottaPublicNodes),
    },
    {
        id: 'storage-overview',
        categoryId: 'storage-backups',
        targetId: 'settings-target-storage-overview',
        label: message(messages.appSettingsStorageAnalysisTitle),
        description: message(messages.appSettingsStorageAnalysisDescription),
    },
    {
        id: 'updates-history',
        categoryId: 'updates',
        label: message(messages.appSettingsUpdatesAnnouncementsHistory, 'Release history'),
    },
    {
        id: 'about-product',
        categoryId: 'about',
        targetId: 'settings-target-about-product',
        label: message(messages.appSettingsTabsAbout),
    },
    {
        id: 'about-replay-tour',
        categoryId: 'about',
        targetId: 'settings-target-about-replay-tour',
        label: message(messages.appSettingsAboutReplayOnboarding),
    },
    {
        id: 'feature-flags',
        categoryId: 'feature-flags',
        label: message(messages.settingsFeatureFlagsTitle),
    },
]

export function getSettingsSearchTargetId(entry: SettingsSearchEntry): string {
    return entry.targetId ?? `settings-category-${entry.categoryId}`
}
