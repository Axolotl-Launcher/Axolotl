import ArchiveIcon from '@modrinth/assets/icons/archive.svg?component'
import BotIcon from '@modrinth/assets/icons/bot.svg?component'
import CoffeeIcon from '@modrinth/assets/icons/coffee.svg?component'
import CpuIcon from '@modrinth/assets/icons/cpu.svg?component'
import FileTextIcon from '@modrinth/assets/icons/file-text.svg?component'
import GameIcon from '@modrinth/assets/icons/game.svg?component'
import GaugeIcon from '@modrinth/assets/icons/gauge.svg?component'
import GlobeIcon from '@modrinth/assets/icons/globe.svg?component'
import InfoIcon from '@modrinth/assets/icons/info.svg?component'
import KeyboardIcon from '@modrinth/assets/icons/keyboard.svg?component'
import LanguagesIcon from '@modrinth/assets/icons/languages.svg?component'
import LayoutTemplateIcon from '@modrinth/assets/icons/layout-template.svg?component'
import PaintbrushIcon from '@modrinth/assets/icons/paintbrush.svg?component'
import RefreshCwIcon from '@modrinth/assets/icons/refresh-cw.svg?component'
import ShieldIcon from '@modrinth/assets/icons/shield.svg?component'
import ToggleRightIcon from '@modrinth/assets/icons/toggle-right.svg?component'
import UsersIcon from '@modrinth/assets/icons/users.svg?component'
import type { MessageDescriptor } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { type Component, defineAsyncComponent } from 'vue'

import {
    getVisibleSettingsCategoryDefinitions,
    type SettingsCategoryDefinition,
    settingsCategoryDefinitions,
    type SettingsCategoryId,
    type SettingsGroupId,
} from './category-definitions'
import { messages } from './messages'
import { settingsSearchEntries, type SettingsSearchEntry } from './search-index'

export type { SettingsCategoryId, SettingsGroupId } from './category-definitions'

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

/** Icons and lazily loaded bodies, as the app's settings-registry.ts pairs them. */
const categoryContent: Record<SettingsCategoryId, Pick<SettingsCategory, 'icon' | 'content'>> = {
    interface: {
        icon: PaintbrushIcon,
        content: defineAsyncComponent(() => import('./categories/AppearanceSettings.vue')),
    },
    'home-navigation': {
        icon: LayoutTemplateIcon,
        content: defineAsyncComponent(() => import('./categories/HomeNavigationSettings.vue')),
    },
    'language-translation': {
        icon: LanguagesIcon,
        content: defineAsyncComponent(() => import('./categories/LanguageTranslationSettings.vue')),
    },
    ai: {
        icon: BotIcon,
        content: defineAsyncComponent(() => import('./categories/AISettings.vue')),
    },
    'shortcut-settings': {
        icon: KeyboardIcon,
        content: defineAsyncComponent(() => import('./categories/KeybindSettings.vue')),
    },
    'java-performance': {
        icon: CoffeeIcon,
        content: defineAsyncComponent(() => import('./categories/JavaSettings.vue')),
    },
    'launch-defaults': {
        icon: GameIcon,
        content: defineAsyncComponent(() => import('./categories/DefaultInstanceSettings.vue')),
    },
    'instance-sync': {
        icon: RefreshCwIcon,
        content: defineAsyncComponent(() => import('./categories/instances/index.vue')),
    },
    'content-downloads': {
        icon: GaugeIcon,
        content: defineAsyncComponent(() => import('./categories/ContentDownloadSettings.vue')),
    },
    'network-connection': {
        icon: GlobeIcon,
        content: defineAsyncComponent(() => import('./categories/NetworkConnectionSettings.vue')),
    },
    'network-multiplayer': {
        icon: UsersIcon,
        content: defineAsyncComponent(() => import('./categories/NetworkMultiplayerSettings.vue')),
    },
    'storage-backups': {
        icon: ArchiveIcon,
        content: defineAsyncComponent(() => import('./categories/StorageBackupSettings.vue')),
    },
    'privacy-data': {
        icon: ShieldIcon,
        content: defineAsyncComponent(() => import('./categories/PrivacySettings.vue')),
    },
    updates: {
        icon: RefreshCwIcon,
        content: defineAsyncComponent(() => import('./categories/UpdateSettings.vue')),
    },
    logs: {
        icon: FileTextIcon,
        content: defineAsyncComponent(() => import('./categories/LogsSettings.vue')),
    },
    about: {
        icon: InfoIcon,
        content: defineAsyncComponent(() => import('./categories/AboutSettings.vue')),
    },
    'feature-flags': {
        icon: ToggleRightIcon,
        content: defineAsyncComponent(() => import('./categories/FeatureFlagSettings.vue')),
    },
}

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
    { id: 'launcher', name: messages.appSettingsGroupsLauncher, icon: GaugeIcon },
    { id: 'game', name: messages.appSettingsGroupsGame, icon: GameIcon },
    { id: 'data-privacy', name: messages.appSettingsGroupsDataPrivacy, icon: ShieldIcon },
    { id: 'support', name: messages.appSettingsGroupsSupport, icon: InfoIcon },
    { id: 'developer', name: messages.appSettingsGroupsDeveloper, icon: CpuIcon },
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
