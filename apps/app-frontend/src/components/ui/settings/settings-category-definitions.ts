import { defineMessage, type MessageDescriptor } from '@modrinth/ui'

export type SettingsCategoryId =
    | 'interface'
    | 'home-navigation'
    | 'language-translation'
    | 'shortcut-settings'
    | 'ai'
    | 'mcp'
    | 'launch-defaults'
    | 'instance-sync'
    | 'java-performance'
    | 'content-downloads'
    | 'network-multiplayer'
    | 'network-connection'
    | 'storage-backups'
    | 'privacy-data'
    | 'third-party-accounts'
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
    },
    {
        id: 'home-navigation',
        name: defineMessage({
            id: 'app.settings.tabs.home-navigation',
            defaultMessage: 'Home & navigation',
        }),
        group: 'launcher',
    },
    {
        id: 'shortcut-settings',
        name: defineMessage({
            id: 'app.settings.tabs.shortcut-settings',
            defaultMessage: 'Keyboard shortcuts',
        }),
        group: 'launcher',
    },
    {
        id: 'language-translation',
        name: defineMessage({
            id: 'app.settings.tabs.language-translation',
            defaultMessage: 'Language & translation',
        }),
        group: 'launcher',
    },
    {
        id: 'ai',
        name: defineMessage({
            id: 'app.settings.tabs.ai',
            defaultMessage: 'AI features',
        }),
        group: 'launcher',
        flushContent: true,
    },
    {
        id: 'mcp',
        name: defineMessage({
            id: 'app.settings.tabs.mcp',
            defaultMessage: 'MCP',
        }),
        group: 'launcher',
    },
    {
        id: 'launch-defaults',
        name: defineMessage({
            id: 'app.settings.tabs.launch-defaults',
            defaultMessage: 'Launch defaults',
        }),
        group: 'game',
    },
    {
        id: 'instance-sync',
        name: defineMessage({
            id: 'app.settings.tabs.instance-sync',
            defaultMessage: 'Instance synchronization',
        }),
        group: 'game',
    },
    {
        id: 'java-performance',
        name: defineMessage({
            id: 'app.settings.tabs.java-performance',
            defaultMessage: 'Java & performance',
        }),
        group: 'game',
    },
    {
        id: 'content-downloads',
        name: defineMessage({
            id: 'app.settings.tabs.content-downloads',
            defaultMessage: 'Content & downloads',
        }),
        group: 'game',
    },
    {
        id: 'network-multiplayer',
        name: defineMessage({
            id: 'app.settings.tabs.network-multiplayer',
            defaultMessage: 'Network & multiplayer',
        }),
        group: 'game',
    },
    {
        id: 'storage-backups',
        name: defineMessage({
            id: 'app.settings.tabs.storage-backups',
            defaultMessage: 'Storage & backups',
        }),
        group: 'data-privacy',
    },
    {
        id: 'third-party-accounts',
        name: defineMessage({
            id: 'app.settings.tabs.third-party-accounts',
            defaultMessage: 'Third-party accounts',
        }),
        group: 'data-privacy',
    },
    {
        id: 'privacy-data',
        name: defineMessage({
            id: 'app.settings.tabs.privacy-data',
            defaultMessage: 'Privacy & data sharing',
        }),
        group: 'data-privacy',
    },
    {
        id: 'updates',
        name: defineMessage({
            id: 'app.settings.tabs.updates',
            defaultMessage: 'Updates',
        }),
        group: 'support',
    },
    {
        id: 'about',
        name: defineMessage({
            id: 'app.settings.tabs.about',
            defaultMessage: 'About',
        }),
        group: 'support',
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
