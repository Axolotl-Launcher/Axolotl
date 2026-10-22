import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

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

test('normalizes settings search text before matching', () => {
    assert.equal(normalizeSettingsSearchText('  Proxy\nSettings  '), 'proxy settings')
    assert.deepEqual(
        filterSettingsSearchDocuments('proxy settings', [
            { item: 'proxy', text: 'Resource management Proxy settings Custom proxy' },
            { item: 'theme', text: 'Appearance Color theme' },
        ]).map(({ item }) => item),
        ['proxy'],
    )
})

test('returns no documents for an empty settings search', () => {
    assert.deepEqual(
        filterSettingsSearchDocuments('', [{ item: 'theme', text: 'Color theme' }]),
        [],
    )
})

test('matches setting categories, titles, descriptions, and static option keywords', () => {
    const documents = [
        { item: 'category', text: 'Resource management' },
        { item: 'title', text: 'Appearance Color theme' },
        {
            item: 'description',
            text: 'Resource management Proxy settings Connect through a network proxy',
        },
        { item: 'keyword', text: 'Resource management Proxy settings SOCKS5' },
    ]

    assert.deepEqual(
        filterSettingsSearchDocuments('resource management', documents).map(({ item }) => item),
        ['category', 'keyword', 'description'],
    )
    assert.deepEqual(
        filterSettingsSearchDocuments('color theme', documents).map(({ item }) => item),
        ['title'],
    )
    assert.deepEqual(
        filterSettingsSearchDocuments('network proxy', documents).map(({ item }) => item),
        ['description'],
    )
    assert.deepEqual(
        filterSettingsSearchDocuments('socks5', documents).map(({ item }) => item),
        ['keyword'],
    )
})

test('tolerates small spelling errors and limits the result set', () => {
    assert.deepEqual(
        filterSettingsSearchDocuments('colur them', [
            { item: 'theme', text: 'Appearance Color theme' },
            { item: 'proxy', text: 'Network proxy' },
        ]).map(({ item }) => item),
        ['theme'],
    )

    const documents = Array.from({ length: MAX_SETTINGS_SEARCH_RESULTS + 3 }, (_, index) => ({
        item: index,
        text: `Setting ${index}`,
    }))
    assert.equal(
        filterSettingsSearchDocuments('setting', documents).length,
        MAX_SETTINGS_SEARCH_RESULTS,
    )
})

test('settings search index has unique entries with categories', () => {
    assert.deepEqual(validateSettingsSearchEntries(), [])
})

test('settings search keywords are valid message descriptors', () => {
    for (const entry of settingsSearchEntries) {
        for (const keyword of entry.keywords ?? []) {
            assert.equal(typeof keyword.id, 'string', entry.id)
            assert.equal(typeof keyword.defaultMessage, 'string', entry.id)
        }
    }
})

test('legacy category names remain searchable after the taxonomy change', () => {
    const keywordText = settingsSearchEntries
        .flatMap((entry) => entry.keywords ?? [])
        .map((keyword) => keyword.defaultMessage)
        .join(' ')

    assert.equal(keywordText.includes('Resource management'), true)
    assert.equal(keywordText.includes('Default instance options'), true)
})

test('developer-only settings stay out of the normal search categories', () => {
    const visibleCategoryIds = new Set(
        getVisibleSettingsCategoryDefinitions(false).map((category) => category.id),
    )
    const visibleEntries = settingsSearchEntries.filter((entry) =>
        visibleCategoryIds.has(entry.categoryId),
    )

    assert.equal(visibleCategoryIds.has('feature-flags'), false)
    assert.equal(
        visibleEntries.some((entry) => entry.categoryId === 'feature-flags'),
        false,
    )
    assert.equal(
        settingsSearchEntries.some((entry) => entry.categoryId === 'feature-flags'),
        true,
    )
    assert.equal(
        getVisibleSettingsCategoryDefinitions(true).some(
            (category) => category.id === 'feature-flags',
        ),
        true,
    )
})

test('settings navigation groups preserve the intended Axolotl information architecture', () => {
    const categoriesForGroup = (
        group: 'launcher' | 'game' | 'data-privacy' | 'support' | 'developer',
        developerMode = false,
    ) =>
        getVisibleSettingsCategoryDefinitions(developerMode)
            .filter((category) => category.group === group)
            .map((category) => category.id)

    assert.deepEqual(categoriesForGroup('launcher'), [
        'interface',
        'home-navigation',
        'shortcut-settings',
        'language-translation',
        'ai',
    ])
    assert.deepEqual(categoriesForGroup('game'), [
        'launch-defaults',
        'instance-sync',
        'java-performance',
        'content-downloads',
        'network-multiplayer',
    ])
    assert.deepEqual(categoriesForGroup('data-privacy'), ['storage-backups', 'privacy-data'])
    assert.deepEqual(categoriesForGroup('support'), [
        'updates',
        'about',
        'network-connection',
        'logs',
    ])
    assert.deepEqual(categoriesForGroup('developer'), [])
    assert.deepEqual(categoriesForGroup('developer', true), ['feature-flags'])
})

test('Chinese contains every user-facing settings category label', () => {
    for (const category of settingsCategoryDefinitions) {
        if (category.id === 'feature-flags') continue

        assert.equal(typeof chineseLocale[category.name.id]?.message, 'string', category.name.id)
    }
})

test('every settings search result resolves to a category and a scroll target', () => {
    assert.deepEqual(validateSettingsSearchMappings(), [])

    const categoryIds = new Set(settingsCategoryDefinitions.map((category) => category.id))
    for (const entry of settingsSearchEntries) {
        assert.equal(categoryIds.has(entry.categoryId), true)

        const targetId = getSettingsSearchTargetId(entry)
        if (!entry.targetId) {
            assert.equal(targetId, `settings-category-${entry.categoryId}`)
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
        assert.equal(
            literal || interpolated,
            true,
            `${entry.id}: no anchor for ${targetId} in ${entry.categoryId}`,
        )
    }
})

test('storage analysis search targets its inline section', () => {
    const entry = settingsSearchEntries.find((entry) => entry.id === 'storage-overview')!
    assert.equal(entry.categoryId, 'storage-backups')
    assert.equal('page' in entry, false)
    assert.equal(getSettingsSearchTargetId(entry), 'settings-target-storage-overview')
})
