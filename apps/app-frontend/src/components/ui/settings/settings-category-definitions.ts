import { defineMessage, type MessageDescriptor } from '@modrinth/ui'

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
 * Ordered by daily-use priority within each group:
 * - launcher: how the shell looks and is operated, then optional AI
 * - game: how Minecraft launches and gets content
 * - data-privacy: disk footprint and what leaves the machine
 * - support: update, identity, then diagnostics
 * - developer: hidden unless developer mode
 */
export const settingsCategoryDefinitions: SettingsCategoryDefinition[] = [
    {
        id: 'interface',
        name: defineMessage({
            id: 'app.settings.tabs.interface',
            defaultMessage: 'Appearance',
        }),
        group: 'launcher',
        onboardingId: 'settings-tab-interface',
    },
    {
        id: 'home-navigation',
        name: defineMessage({
            id: 'app.settings.tabs.home-navigation',
            defaultMessage: 'Home & navigation',
        }),
        group: 'launcher',
        onboardingId: 'settings-tab-home-navigation',
    },
    {
        id: 'shortcut-settings',
        name: defineMessage({
            id: 'app.settings.tabs.shortcut-settings',
            defaultMessage: 'Keyboard shortcuts',
        }),
        group: 'launcher',
        onboardingId: 'settings-tab-shortcut-settings',
    },
    {
        id: 'language-translation',
        name: defineMessage({
            id: 'app.settings.tabs.language-translation',
            defaultMessage: 'Language & translation',
        }),
        group: 'launcher',
        onboardingId: 'settings-tab-language-translation',
    },
    {
        id: 'ai',
        name: defineMessage({
            id: 'app.settings.tabs.ai',
            defaultMessage: 'AI features',
        }),
        group: 'launcher',
        flushContent: true,
        onboardingId: 'settings-tab-ai',
    },
    {
        id: 'launch-defaults',
        name: defineMessage({
            id: 'app.settings.tabs.launch-defaults',
            defaultMessage: 'Launch defaults',
        }),
        group: 'game',
        onboardingId: 'settings-tab-launch-defaults',
    },
    {
        id: 'instance-sync',
        name: defineMessage({
            id: 'app.settings.tabs.instance-sync',
            defaultMessage: 'Instance synchronization',
        }),
        group: 'game',
        onboardingId: 'settings-tab-instance-sync',
    },
    {
        id: 'java-performance',
        name: defineMessage({
            id: 'app.settings.tabs.java-performance',
            defaultMessage: 'Java & performance',
        }),
        group: 'game',
        onboardingId: 'settings-tab-java-performance',
    },
    {
        id: 'content-downloads',
        name: defineMessage({
            id: 'app.settings.tabs.content-downloads',
            defaultMessage: 'Content & downloads',
        }),
        group: 'game',
        onboardingId: 'settings-tab-content-downloads',
    },
    {
        id: 'network-multiplayer',
        name: defineMessage({
            id: 'app.settings.tabs.network-multiplayer',
            defaultMessage: 'Network & multiplayer',
        }),
        group: 'game',
        onboardingId: 'settings-tab-network-multiplayer',
    },
    {
        id: 'storage-backups',
        name: defineMessage({
            id: 'app.settings.tabs.storage-backups',
            defaultMessage: 'Storage & backups',
        }),
        group: 'data-privacy',
        onboardingId: 'settings-tab-storage-backups',
    },
    {
        id: 'privacy-data',
        name: defineMessage({
            id: 'app.settings.tabs.privacy-data',
            defaultMessage: 'Privacy & data sharing',
        }),
        group: 'data-privacy',
        onboardingId: 'settings-tab-privacy-data',
    },
    {
        id: 'updates',
        name: defineMessage({
            id: 'app.settings.tabs.updates',
            defaultMessage: 'Updates',
        }),
        group: 'support',
        onboardingId: 'settings-tab-updates',
    },
    {
        id: 'about',
        name: defineMessage({
            id: 'app.settings.tabs.about',
            defaultMessage: 'About',
        }),
        group: 'support',
        onboardingId: 'settings-tab-about',
    },
    {
        id: 'network-connection',
        name: defineMessage({
            id: 'app.settings.tabs.network-connection',
            defaultMessage: 'Network tests',
        }),
        group: 'support',
    },
    {
        id: 'logs',
        name: defineMessage({
            id: 'app.settings.tabs.logs',
            defaultMessage: 'Logs & diagnostics',
        }),
        group: 'support',
        onboardingId: 'settings-tab-logs',
    },
    {
        id: 'feature-flags',
        name: defineMessage({
            id: 'settings.feature-flags.title',
            defaultMessage: 'Feature flags',
        }),
        group: 'developer',
        developerOnly: true,
    },
]

export function getVisibleSettingsCategoryDefinitions(developerMode: boolean) {
    return settingsCategoryDefinitions.filter(
        (category) => !category.developerOnly || developerMode,
    )
}
