import type { MessageDescriptor } from '@modrinth/ui/src/composables/i18n.ts'

import { messages } from './messages'

export type SettingsCategoryId =
    | 'interface'
    | 'home-navigation'
    | 'language-translation'
    | 'shortcut-settings'
    | 'ai'
    | 'launch-defaults'
    | 'instance-sync'
    | 'java-performance'
    | 'content-downloads'
    | 'network-multiplayer'
    | 'network-connection'
    | 'storage-backups'
    | 'privacy-data'
    | 'updates'
    | 'about'
    | 'logs'
    | 'feature-flags'

export type SettingsGroupId = 'launcher' | 'game' | 'data-privacy' | 'support' | 'developer'

export interface SettingsCategoryDefinition {
    id: SettingsCategoryId
    name: MessageDescriptor
    group: SettingsGroupId
    flushContent?: boolean
    developerOnly?: boolean
    onboardingId?: string
}

/**
 * The desktop app's categories (settings-category-definitions.ts), in its order:
 * by daily-use priority within each group, the developer group hidden unless
 * developer mode is on.
 */
export const settingsCategoryDefinitions: SettingsCategoryDefinition[] = [
    {
        id: 'interface',
        name: messages.appSettingsTabsInterface,
        group: 'launcher',
        onboardingId: 'settings-tab-interface',
    },
    {
        id: 'home-navigation',
        name: messages.appSettingsTabsHomeNavigation,
        group: 'launcher',
        onboardingId: 'settings-tab-home-navigation',
    },
    {
        id: 'shortcut-settings',
        name: messages.appSettingsTabsShortcutSettings,
        group: 'launcher',
        onboardingId: 'settings-tab-shortcut-settings',
    },
    {
        id: 'language-translation',
        name: messages.appSettingsTabsLanguageTranslation,
        group: 'launcher',
        onboardingId: 'settings-tab-language-translation',
    },
    {
        id: 'ai',
        name: messages.appSettingsTabsAi,
        group: 'launcher',
        flushContent: true,
        onboardingId: 'settings-tab-ai',
    },
    {
        id: 'launch-defaults',
        name: messages.appSettingsTabsLaunchDefaults,
        group: 'game',
        onboardingId: 'settings-tab-launch-defaults',
    },
    {
        id: 'instance-sync',
        name: messages.appSettingsTabsInstanceSync,
        group: 'game',
        onboardingId: 'settings-tab-instance-sync',
    },
    {
        id: 'java-performance',
        name: messages.appSettingsTabsJavaPerformance,
        group: 'game',
        onboardingId: 'settings-tab-java-performance',
    },
    {
        id: 'content-downloads',
        name: messages.appSettingsTabsContentDownloads,
        group: 'game',
        onboardingId: 'settings-tab-content-downloads',
    },
    {
        id: 'network-multiplayer',
        name: messages.appSettingsTabsNetworkMultiplayer,
        group: 'game',
        onboardingId: 'settings-tab-network-multiplayer',
    },
    {
        id: 'storage-backups',
        name: messages.appSettingsTabsStorageBackups,
        group: 'data-privacy',
        onboardingId: 'settings-tab-storage-backups',
    },
    {
        id: 'privacy-data',
        name: messages.appSettingsTabsPrivacyData,
        group: 'data-privacy',
        onboardingId: 'settings-tab-privacy-data',
    },
    {
        id: 'updates',
        name: messages.appSettingsTabsUpdates,
        group: 'support',
        onboardingId: 'settings-tab-updates',
    },
    {
        id: 'about',
        name: messages.appSettingsTabsAbout,
        group: 'support',
        onboardingId: 'settings-tab-about',
    },
    {
        id: 'network-connection',
        name: messages.appSettingsTabsNetworkConnection,
        group: 'support',
    },
    {
        id: 'logs',
        name: messages.appSettingsTabsLogs,
        group: 'support',
        onboardingId: 'settings-tab-logs',
    },
    {
        id: 'feature-flags',
        name: messages.settingsFeatureFlagsTitle,
        group: 'developer',
        developerOnly: true,
    },
]

export function getVisibleSettingsCategoryDefinitions(developerMode: boolean) {
    return settingsCategoryDefinitions.filter(
        (category) => !category.developerOnly || developerMode,
    )
}
