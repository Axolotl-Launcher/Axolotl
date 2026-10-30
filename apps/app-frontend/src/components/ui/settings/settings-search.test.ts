import { readFileSync } from 'node:fs'

import { expect, it } from 'vitest'

import {
    getVisibleSettingsCategoryDefinitions,
    settingsCategoryDefinitions,
} from './settings-category-definitions.ts'
import {
    filterSettingsSearchDocuments,
    MAX_SETTINGS_SEARCH_RESULTS,
    normalizeSettingsSearchText,
} from './settings-search.ts'
import {
    getSettingsSearchTargetId,
    settingsSearchEntries,
    validateSettingsSearchEntries,
    validateSettingsSearchMappings,
} from './settings-search-index.ts'

const settingsComponentFiles = {
    interface: ['./AppearanceSettings.vue'],
    'home-navigation': ['./AppearanceSettings.vue'],
    'language-translation': ['./LanguageSettings.vue', './TranslationSettings.vue'],
    ai: ['./AISettings.vue'],
    mcp: ['./McpSettings.vue'],
    'shortcut-settings': ['./KeybindSettings.vue'],
    'java-performance': ['./JavaSettings.vue'],
    'launch-defaults': [
        './DefaultInstanceSettings.vue',
        './LogShareSettings.vue',
        './SharedLogsSettings.vue',
    ],
    'content-downloads': ['./AppearanceSettings.vue', './ResourceManagementSettings.vue'],
    'network-multiplayer': ['./ResourceManagementSettings.vue', './MultiplayerSettings.vue'],
    'network-connection': ['./NetworkConnectionSettings.vue'],
    'storage-backups': [
        './ResourceManagementSettings.vue',
        './StorageBackupSettings.vue',
        './StorageSettings.vue',
    ],
    'privacy-data': ['./PrivacySettings.vue'],
    'instance-sync': ['./instances/index.vue'],
    updates: ['./UpdateSettings.vue'],
    logs: ['./LogsSettings.vue'],
    about: ['./AboutSettings.vue'],
    'feature-flags': ['./FeatureFlagSettings.vue'],
} as const

const chineseLocale = JSON.parse(
    readFileSync(new URL('../../../locales/zh-CN/index.json', import.meta.url), 'utf8'),
) as Record<string, { message?: string }>

it('normalizes settings search text before matching', () => {
    expect(normalizeSettingsSearchText('  Proxy\nSettings  ')).toBe('proxy settings')
    expect(
        filterSettingsSearchDocuments('proxy settings', [
            { item: 'proxy', text: 'Resource management Proxy settings Custom proxy' },
            { item: 'theme', text: 'Appearance Color theme' },
        ]).map(({ item }) => item),
    ).toEqual(['proxy'])
})

it('returns no documents for an empty settings search', () => {
    expect(filterSettingsSearchDocuments('', [{ item: 'theme', text: 'Color theme' }])).toEqual([])
})

it('matches setting categories, titles, descriptions, and static option keywords', () => {
    const documents = [
        { item: 'category', text: 'Resource management' },
        { item: 'title', text: 'Appearance Color theme' },
        {
            item: 'description',
            text: 'Resource management Proxy settings Connect through a network proxy',
        },
        { item: 'keyword', text: 'Resource management Proxy settings SOCKS5' },
    ]

    expect(
        filterSettingsSearchDocuments('resource management', documents).map(({ item }) => item),
    ).toEqual(['category', 'keyword', 'description'])
    expect(filterSettingsSearchDocuments('color theme', documents).map(({ item }) => item)).toEqual(
        ['title'],
    )
    expect(
        filterSettingsSearchDocuments('network proxy', documents).map(({ item }) => item),
    ).toEqual(['description'])
    expect(filterSettingsSearchDocuments('socks5', documents).map(({ item }) => item)).toEqual([
        'keyword',
    ])
})

it('tolerates small spelling errors and limits the result set', () => {
    expect(
        filterSettingsSearchDocuments('colur them', [
            { item: 'theme', text: 'Appearance Color theme' },
            { item: 'proxy', text: 'Network proxy' },
        ]).map(({ item }) => item),
    ).toEqual(['theme'])

    const documents = Array.from({ length: MAX_SETTINGS_SEARCH_RESULTS + 3 }, (_, index) => ({
        item: index,
        text: `Setting ${index}`,
    }))
    expect(filterSettingsSearchDocuments('setting', documents).length).toBe(
        MAX_SETTINGS_SEARCH_RESULTS,
    )
})

it('settings search index has unique entries with categories', () => {
    expect(validateSettingsSearchEntries()).toEqual([])
})

it('settings search keywords are valid message descriptors', () => {
    for (const entry of settingsSearchEntries) {
        for (const keyword of entry.keywords ?? []) {
            expect(typeof keyword.id, entry.id).toBe('string')
            expect(typeof keyword.defaultMessage, entry.id).toBe('string')
        }
    }
})

it('legacy category names remain searchable after the taxonomy change', () => {
    const keywordText = settingsSearchEntries
        .flatMap((entry) => entry.keywords ?? [])
        .map((keyword) => keyword.defaultMessage)
        .join(' ')

    expect(keywordText.includes('Resource management')).toBe(true)
    expect(keywordText.includes('Default instance options')).toBe(true)
})

it('finds the default game language setting in English and Chinese', () => {
    const entry = settingsSearchEntries.find((item) => item.id === 'defaults-game-language')!
    const text = [
        entry.label.defaultMessage,
        entry.description?.defaultMessage,
        ...(entry.keywords?.map((keyword) => keyword.defaultMessage) ?? []),
    ]
        .filter(Boolean)
        .join(' ')
    const chineseText = [
        entry.label.id,
        entry.description?.id,
        ...(entry.keywords?.map((keyword) => keyword.id) ?? []),
    ]
        .map((id) => (id ? chineseLocale[id]?.message : undefined))
        .filter(Boolean)
        .join(' ')

    expect(
        filterSettingsSearchDocuments('game language', [{ item: entry.id, text }]).map(
            ({ item }) => item,
        ),
    ).toEqual([entry.id])
    expect(
        filterSettingsSearchDocuments('游戏语言', [{ item: entry.id, text: chineseText }]).map(
            ({ item }) => item,
        ),
    ).toEqual([entry.id])
})

it('developer-only settings stay out of the normal search categories', () => {
    const visibleCategoryIds = new Set(
        getVisibleSettingsCategoryDefinitions(false).map((category) => category.id),
    )
    const visibleEntries = settingsSearchEntries.filter((entry) =>
        visibleCategoryIds.has(entry.categoryId),
    )

    expect(visibleCategoryIds.has('feature-flags')).toBe(false)
    expect(visibleEntries.some((entry) => entry.categoryId === 'feature-flags')).toBe(false)
    expect(settingsSearchEntries.some((entry) => entry.categoryId === 'feature-flags')).toBe(true)
    expect(
        getVisibleSettingsCategoryDefinitions(true).some(
            (category) => category.id === 'feature-flags',
        ),
    ).toBe(true)
})

it('settings navigation groups preserve the intended Axolotl information architecture', () => {
    const categoriesForGroup = (
        group: 'launcher' | 'game' | 'data-privacy' | 'support' | 'developer',
        developerMode = false,
    ) =>
        getVisibleSettingsCategoryDefinitions(developerMode)
            .filter((category) => category.group === group)
            .map((category) => category.id)

    expect(categoriesForGroup('launcher')).toEqual([
        'interface',
        'home-navigation',
        'shortcut-settings',
        'language-translation',
        'ai',
        'mcp',
    ])
    expect(categoriesForGroup('game')).toEqual([
        'launch-defaults',
        'instance-sync',
        'java-performance',
        'content-downloads',
        'network-multiplayer',
    ])
    expect(categoriesForGroup('data-privacy')).toEqual(['storage-backups', 'privacy-data'])
    expect(categoriesForGroup('support')).toEqual([
        'updates',
        'about',
        'network-connection',
        'logs',
    ])
    expect(categoriesForGroup('developer')).toEqual([])
    expect(categoriesForGroup('developer', true)).toEqual(['feature-flags'])
})

it('Chinese contains every user-facing settings category label', () => {
    for (const category of settingsCategoryDefinitions) {
        if (category.id === 'feature-flags') continue

        expect(typeof chineseLocale[category.name.id]?.message, category.name.id).toBe('string')
    }
})

it('every settings search result resolves to a category and a scroll target', () => {
    expect(validateSettingsSearchMappings()).toEqual([])

    const categoryIds = new Set(settingsCategoryDefinitions.map((category) => category.id))
    for (const entry of settingsSearchEntries) {
        expect(categoryIds.has(entry.categoryId)).toBe(true)

        const targetId = getSettingsSearchTargetId(entry)
        if (!entry.targetId) {
            expect(targetId).toBe(`settings-category-${entry.categoryId}`)
            continue
        }

        const template = settingsComponentFiles[entry.categoryId]
            .map((file) => readFileSync(new URL(file, import.meta.url), 'utf8'))
            .join('\n')
        // Some rows build their anchors from a table rather than naming them
        // literally, which a text scan cannot resolve. Accept the known
        // interpolations, each tied to the prefix it produces, so the check
        // still fails for an anchor that is missing outright.
        const interpolations = [
            // Shortcut rows: `:id="`settings-target-${action.id}`"`
            { prefix: 'settings-target-', pattern: ':id="`settings-target-${action.id}`"' },
            // Instance-sync rows: `:id="`settings-target-instance-sync-${row.option}`"`
            {
                prefix: 'settings-target-instance-sync-',
                pattern: ':id="`settings-target-instance-sync-${row.option}`"',
            },
        ]
        const literal = template.includes(`id="${targetId}"`)
        const interpolated = interpolations.some(
            ({ prefix, pattern }) => targetId.startsWith(prefix) && template.includes(pattern),
        )
        expect(
            literal || interpolated,
            `${entry.id}: no anchor for ${targetId} in ${entry.categoryId}`,
        ).toBe(true)
    }
})

it('storage analysis search targets its inline section', () => {
    const entry = settingsSearchEntries.find((entry) => entry.id === 'storage-overview')!
    expect(entry.categoryId).toBe('storage-backups')
    expect('page' in entry).toBe(false)
    expect(getSettingsSearchTargetId(entry)).toBe('settings-target-storage-overview')
})
