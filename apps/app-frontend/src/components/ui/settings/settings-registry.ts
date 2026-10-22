import {
    ArchiveIcon,
    BotIcon,
    CoffeeIcon,
    CpuIcon,
    FileTextIcon,
    GameIcon,
    GaugeIcon,
    GlobeIcon,
    InfoIcon,
    KeyboardIcon,
    LanguagesIcon,
    LayoutTemplateIcon,
    PaintbrushIcon,
    RefreshCwIcon,
    ShieldIcon,
    ToggleRightIcon,
    UsersIcon,
} from '@modrinth/assets'
import { commonMessages, defineMessages, type MessageDescriptor } from '@modrinth/ui'
import { type Component, defineAsyncComponent } from 'vue'

import {
    getVisibleSettingsCategoryDefinitions,
    type SettingsCategoryDefinition,
    settingsCategoryDefinitions,
    type SettingsCategoryId,
    type SettingsGroupId,
} from './settings-category-definitions'
import { settingsSearchEntries, type SettingsSearchEntry } from './settings-search-index'

export interface SettingsCategory extends SettingsCategoryDefinition {
    icon: Component
    content: Component
    entries: SettingsSearchEntry[]
}

export interface SettingsGroup {
    id: SettingsGroupId
    name: MessageDescriptor
    icon: Component
    categories: SettingsCategory[]
}

const categoryContent: Record<SettingsCategoryId, Pick<SettingsCategory, 'icon' | 'content'>> = {
    interface: {
        icon: PaintbrushIcon,
        content: defineAsyncComponent(() => import('./AppearanceSettings.vue')),
    },
    'home-navigation': {
        icon: LayoutTemplateIcon,
        content: defineAsyncComponent(() => import('./HomeNavigationSettings.vue')),
    },
    'language-translation': {
        icon: LanguagesIcon,
        content: defineAsyncComponent(() => import('./LanguageTranslationSettings.vue')),
    },
    ai: { icon: BotIcon, content: defineAsyncComponent(() => import('./AISettings.vue')) },
    'shortcut-settings': {
        icon: KeyboardIcon,
        content: defineAsyncComponent(() => import('./KeybindSettings.vue')),
    },
    'java-performance': {
        icon: CoffeeIcon,
        content: defineAsyncComponent(() => import('./JavaSettings.vue')),
    },
    'launch-defaults': {
        icon: GameIcon,
        content: defineAsyncComponent(() => import('./DefaultInstanceSettings.vue')),
    },
    'instance-sync': {
        icon: RefreshCwIcon,
        content: defineAsyncComponent(() => import('./instances/index.vue')),
    },
    'content-downloads': {
        icon: GaugeIcon,
        content: defineAsyncComponent(() => import('./ContentDownloadSettings.vue')),
    },
    'network-connection': {
        icon: GlobeIcon,
        content: defineAsyncComponent(() => import('./NetworkConnectionSettings.vue')),
    },
    'network-multiplayer': {
        icon: UsersIcon,
        content: defineAsyncComponent(() => import('./NetworkMultiplayerSettings.vue')),
    },
    'storage-backups': {
        icon: ArchiveIcon,
        content: defineAsyncComponent(() => import('./StorageBackupSettings.vue')),
    },
    'privacy-data': {
        icon: ShieldIcon,
        content: defineAsyncComponent(() => import('./PrivacySettings.vue')),
    },
    updates: {
        icon: RefreshCwIcon,
        content: defineAsyncComponent(() => import('./UpdateSettings.vue')),
    },
    logs: {
        icon: FileTextIcon,
        content: defineAsyncComponent(() => import('./LogsSettings.vue')),
    },
    about: { icon: InfoIcon, content: defineAsyncComponent(() => import('./AboutSettings.vue')) },
    'feature-flags': {
        icon: ToggleRightIcon,
        content: defineAsyncComponent(() => import('./FeatureFlagSettings.vue')),
    },
}

const messages = defineMessages({
    launcher: { id: 'app.settings.groups.launcher', defaultMessage: 'Interface' },
    game: { id: 'app.settings.groups.game', defaultMessage: 'Game' },
    dataPrivacy: { id: 'app.settings.groups.data-privacy', defaultMessage: 'Data & privacy' },
    support: { id: 'app.settings.groups.support', defaultMessage: 'App & support' },
    developer: { id: 'app.settings.groups.developer', defaultMessage: 'Developer' },
})

export const settingsCategories: SettingsCategory[] = settingsCategoryDefinitions.map(
    (definition) => ({
        ...definition,
        ...categoryContent[definition.id],
        entries: settingsSearchEntries.filter((entry) => entry.categoryId === definition.id),
    }),
)

const settingsGroupDefinitions: Array<{
    id: SettingsGroupId
    name: MessageDescriptor
    icon: Component
}> = [
    {
        id: 'launcher',
        name: messages.launcher,
        icon: GaugeIcon,
    },
    {
        id: 'game',
        name: messages.game,
        icon: GameIcon,
    },
    {
        id: 'data-privacy',
        name: messages.dataPrivacy,
        icon: ShieldIcon,
    },
    {
        id: 'support',
        name: messages.support,
        icon: InfoIcon,
    },
    {
        id: 'developer',
        name: messages.developer,
        icon: CpuIcon,
    },
]

export function getVisibleSettingsCategories(developerMode: boolean): SettingsCategory[] {
    const visibleIds = new Set(
        getVisibleSettingsCategoryDefinitions(developerMode).map((category) => category.id),
    )
    return settingsCategories.filter((category) => visibleIds.has(category.id))
}

export function getVisibleSettingsGroups(developerMode: boolean): SettingsGroup[] {
    const categories = getVisibleSettingsCategories(developerMode)
    return settingsGroupDefinitions
        .map((group) => ({
            ...group,
            categories: categories.filter((category) => category.group === group.id),
        }))
        .filter((group) => group.categories.length > 0)
}

export const settingsPageTitle: MessageDescriptor = commonMessages.settingsLabel
