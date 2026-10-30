import type { ComputedRef, Ref } from 'vue'
import { computed, ref, watch } from 'vue'

import { defineMessages, useVIntl } from '#ui/composables/i18n'

import type { ContentItem } from '../types'
import { pruneMetadataFilterSelections } from './content-filter-state'
import { contentOwnership, installedSource } from './content-provenance'
import { getContentTabMap } from './content-tab-memory'

export interface MetadataFilterOption {
    value: string
    label: string
}

export interface MetadataFilterCategory {
    key: string
    label: string
    searchable?: boolean
    options: MetadataFilterOption[]
}

interface MetadataFilterDefinition {
    key: string
    label: string
    searchable?: boolean
    values: (item: ContentItem) => string[]
    labelFor: (value: string) => string
    /** Preferred option order; unlisted values (including 未知) sort after ordered ones. */
    order?: string[]
}

const UNKNOWN = 'unknown'

const openSourceLicenseIds = new Set([
    '0BSD',
    'AFL-3.0',
    'AGPL-3.0',
    'Apache-2.0',
    'Artistic-2.0',
    'BSD-2-Clause',
    'BSD-3-Clause',
    'BSL-1.0',
    'CDDL-1.0',
    'ECL-2.0',
    'EPL-1.0',
    'EPL-2.0',
    'EUPL-1.1',
    'EUPL-1.2',
    'GPL-2.0',
    'GPL-3.0',
    'ISC',
    'LGPL-2.1',
    'LGPL-3.0',
    'MIT',
    'MPL-2.0',
    'NCSA',
    'OSL-3.0',
    'PostgreSQL',
    'Python-2.0',
    'Unlicense',
    'UPL-1.0',
    'Zlib',
])

type EnvironmentFilterValue = 'client' | 'server' | 'client_and_server' | 'singleplayer'

function getEnvironmentFilterValue(environment?: string): EnvironmentFilterValue | undefined {
    switch (environment) {
        case 'client_only':
            return 'client'
        case 'server_only':
        case 'dedicated_server_only':
            return 'server'
        case 'client_and_server':
        case 'client_only_server_optional':
        case 'server_only_client_optional':
        case 'client_or_server':
        case 'client_or_server_prefers_both':
            return 'client_and_server'
        case 'singleplayer_only':
            return 'singleplayer'
        default:
            return undefined
    }
}

function isOpenSource(item: ContentItem): boolean {
    const licenseId = item.project?.license?.id.replace(/-(?:only|or-later)$/, '')
    return !!licenseId && openSourceLicenseIds.has(licenseId)
}

const loaderKeys = new Set(['fabric', 'forge', 'neoforge', 'quilt'])

const messages = defineMessages({
    categoryAuthor: {
        id: 'content.metadata-filter.author',
        defaultMessage: 'Author',
    },
    categoryEnvironment: {
        id: 'content.metadata-filter.environment',
        defaultMessage: 'Environment',
    },
    categoryLoader: {
        id: 'content.metadata-filter.loader',
        defaultMessage: 'Loader',
    },
    categorySource: {
        id: 'content.metadata-filter.installed-source',
        defaultMessage: 'Installation source',
    },
    categoryOwnership: {
        id: 'content.metadata-filter.ownership',
        defaultMessage: 'Content ownership',
    },
    sourceModrinth: { id: 'content.metadata-filter.source.modrinth', defaultMessage: 'Modrinth' },
    sourceMcarchive: {
        id: 'content.metadata-filter.source.mcarchive',
        defaultMessage: 'MCArchive',
    },
    sourcePlanetMinecraft: {
        id: 'content.metadata-filter.source.planet-minecraft',
        defaultMessage: 'Planet Minecraft',
    },
    ownershipUser: {
        id: 'content.metadata-filter.ownership.user-added',
        defaultMessage: 'User added',
    },
    ownershipPack: {
        id: 'content.metadata-filter.ownership.pack-managed',
        defaultMessage: 'Pack managed',
    },
    ownershipLocal: {
        id: 'content.metadata-filter.ownership.local-discovered',
        defaultMessage: 'Locally discovered',
    },
    categoryExternal: {
        id: 'content.metadata-filter.external',
        defaultMessage: 'External files',
    },
    categoryOpenSource: {
        id: 'content.metadata-filter.open-source',
        defaultMessage: 'Open source',
    },
    optionUnknown: {
        id: 'content.metadata-filter.unknown',
        defaultMessage: 'Unknown',
    },
    optionClient: {
        id: 'content.metadata-filter.environment.client',
        defaultMessage: 'Client',
    },
    optionServer: {
        id: 'content.metadata-filter.environment.server',
        defaultMessage: 'Server',
    },
    optionClientAndServer: {
        id: 'content.metadata-filter.environment.client-and-server',
        defaultMessage: 'Client & server',
    },
    optionSingleplayer: {
        id: 'content.metadata-filter.environment.singleplayer',
        defaultMessage: 'Singleplayer',
    },
    optionOtherLoader: {
        id: 'content.metadata-filter.loader.other',
        defaultMessage: 'Other',
    },
    loaderFabric: {
        id: 'content.metadata-filter.loader.fabric',
        defaultMessage: 'Fabric',
    },
    loaderForge: {
        id: 'content.metadata-filter.loader.forge',
        defaultMessage: 'Forge',
    },
    loaderNeoForge: {
        id: 'content.metadata-filter.loader.neoforge',
        defaultMessage: 'NeoForge',
    },
    loaderQuilt: {
        id: 'content.metadata-filter.loader.quilt',
        defaultMessage: 'Quilt',
    },
    optionSourceCurseforge: {
        id: 'content.metadata-filter.source.curseforge',
        defaultMessage: 'CurseForge',
    },
    optionExternal: {
        id: 'content.metadata-filter.external.external',
        defaultMessage: 'External file',
    },
    optionLinked: {
        id: 'content.metadata-filter.external.linked',
        defaultMessage: 'Online project',
    },
    optionOpenSource: {
        id: 'content.metadata-filter.open-source.open',
        defaultMessage: 'Open source',
    },
    optionClosedSource: {
        id: 'content.metadata-filter.open-source.closed',
        defaultMessage: 'Closed source',
    },
})

export function useContentMetadataFilters(
    items: Ref<ContentItem[]> | ComputedRef<ContentItem[]>,
    persistKey?: string,
    initialExcluded?: Record<string, string[]>,
    filterOptionsReady?: Ref<boolean> | ComputedRef<boolean>,
) {
    const { formatMessage } = useVIntl()

    const definitions = computed<MetadataFilterDefinition[]>(() => {
        return [
            {
                key: 'author',
                label: formatMessage(messages.categoryAuthor),
                searchable: true,
                values: (item) => (item.owner?.name ? [item.owner.name] : [UNKNOWN]),
                labelFor: (value) =>
                    value === UNKNOWN ? formatMessage(messages.optionUnknown) : value,
            },
            {
                key: 'environment',
                label: formatMessage(messages.categoryEnvironment),
                order: ['client', 'server', 'client_and_server', 'singleplayer'],
                values: (item) => {
                    const value = getEnvironmentFilterValue(item.environment)
                    return value ? [value] : [UNKNOWN]
                },
                labelFor: (value) => {
                    switch (value) {
                        case 'client':
                            return formatMessage(messages.optionClient)
                        case 'server':
                            return formatMessage(messages.optionServer)
                        case 'client_and_server':
                            return formatMessage(messages.optionClientAndServer)
                        case 'singleplayer':
                            return formatMessage(messages.optionSingleplayer)
                        default:
                            return formatMessage(messages.optionUnknown)
                    }
                },
            },
            {
                key: 'loader',
                label: formatMessage(messages.categoryLoader),
                order: ['fabric', 'forge', 'neoforge', 'quilt'],
                values: (item) => {
                    if (!item.loader) return [UNKNOWN]
                    return loaderKeys.has(item.loader) ? [item.loader] : ['other']
                },
                labelFor: (value) => {
                    switch (value) {
                        case 'fabric':
                            return formatMessage(messages.loaderFabric)
                        case 'forge':
                            return formatMessage(messages.loaderForge)
                        case 'neoforge':
                            return formatMessage(messages.loaderNeoForge)
                        case 'quilt':
                            return formatMessage(messages.loaderQuilt)
                        case 'other':
                            return formatMessage(messages.optionOtherLoader)
                        default:
                            return formatMessage(messages.optionUnknown)
                    }
                },
            },
            {
                key: 'source',
                label: formatMessage(messages.categorySource),
                order: ['modrinth', 'curseforge', 'mcarchive', 'planet_minecraft'],
                values: (item) => [installedSource(item)],
                labelFor: (value) => {
                    switch (value) {
                        case 'modrinth':
                            return formatMessage(messages.sourceModrinth)
                        case 'curseforge':
                            return formatMessage(messages.optionSourceCurseforge)
                        case 'mcarchive':
                            return formatMessage(messages.sourceMcarchive)
                        case 'planet_minecraft':
                            return formatMessage(messages.sourcePlanetMinecraft)
                        case UNKNOWN:
                            return formatMessage(messages.optionUnknown)
                        default:
                            return formatMessage(messages.optionUnknown)
                    }
                },
            },
            {
                key: 'ownership',
                label: formatMessage(messages.categoryOwnership),
                order: ['user_added', 'pack_managed', 'local_discovered'],
                values: (item) => [contentOwnership(item)],
                labelFor: (value) => {
                    switch (value) {
                        case 'user_added':
                            return formatMessage(messages.ownershipUser)
                        case 'pack_managed':
                            return formatMessage(messages.ownershipPack)
                        case 'local_discovered':
                            return formatMessage(messages.ownershipLocal)
                        default:
                            return formatMessage(messages.optionUnknown)
                    }
                },
            },
            {
                key: 'external',
                label: formatMessage(messages.categoryExternal),
                order: ['linked', 'external'],
                values: (item) => [item.external ? 'external' : 'linked'],
                labelFor: (value) =>
                    value === 'external'
                        ? formatMessage(messages.optionExternal)
                        : formatMessage(messages.optionLinked),
            },
            {
                key: 'open_source',
                label: formatMessage(messages.categoryOpenSource),
                order: ['open', 'closed'],
                values: (item) => {
                    if (isOpenSource(item)) return ['open']
                    return item.project?.license ? ['closed'] : [UNKNOWN]
                },
                labelFor: (value) => {
                    switch (value) {
                        case 'open':
                            return formatMessage(messages.optionOpenSource)
                        case 'closed':
                            return formatMessage(messages.optionClosedSource)
                        default:
                            return formatMessage(messages.optionUnknown)
                    }
                },
            },
        ]
    })
    const metadataFilterCategories = computed<MetadataFilterCategory[]>(() =>
        definitions.value
            .map((definition) => {
                const options = new Map<string, MetadataFilterOption>()
                const counts = new Map<string, number>()
                for (const item of items.value) {
                    const seen = new Set<string>()
                    for (const value of definition.values(item)) {
                        if (seen.has(value)) continue
                        seen.add(value)
                        if (!options.has(value)) {
                            options.set(value, {
                                value,
                                label: definition.labelFor(value),
                            })
                        }
                        counts.set(value, (counts.get(value) ?? 0) + 1)
                    }
                }

                const total = items.value.length
                const visible = [...options.values()]
                    .filter((option) => (counts.get(option.value) ?? 0) !== total)
                    .sort((a, b) => {
                        if (a.value === UNKNOWN) return 1
                        if (b.value === UNKNOWN) return -1
                        const order = definition.order
                        if (order) {
                            const indexA = order.indexOf(a.value)
                            const indexB = order.indexOf(b.value)
                            if (indexA !== -1 && indexB !== -1) {
                                return indexA - indexB
                            }
                            if (indexA !== -1) return -1
                            if (indexB !== -1) return 1
                        }
                        return a.label.localeCompare(b.label, undefined, {
                            numeric: true,
                        })
                    })

                return {
                    key: definition.key,
                    label: definition.label,
                    searchable: definition.searchable,
                    options: visible,
                }
            })
            .filter((category) => category.options.length > 0),
    )
    const metadataFilterValidationOptions = computed<MetadataFilterCategory[]>(() =>
        definitions.value
            .map((definition) => {
                const values = new Set<string>()
                for (const item of items.value) {
                    for (const value of definition.values(item)) values.add(value)
                }
                return {
                    key: definition.key,
                    label: definition.label,
                    options: [...values].map((value) => ({
                        value,
                        label: definition.labelFor(value),
                    })),
                }
            })
            .filter((category) => category.options.length > 0),
    )

    // ---- 选择状态（排除式：勾选 = 显示，取消勾选 = 隐藏；默认全部勾选） ----

    const metadataFilterMemory = getContentTabMap<string, Record<string, string[]>>(
        'metadataFilters',
    )
    const excluded = ref<Record<string, string[]>>(
        initialExcluded ?? (persistKey ? (metadataFilterMemory.get(persistKey) ?? {}) : {}),
    )

    function optionsByKey(key: string): MetadataFilterOption[] {
        return (
            metadataFilterCategories.value.find((category) => category.key === key)?.options ?? []
        )
    }

    function getExcludedSet(key: string): Set<string> {
        return new Set(excluded.value[key] ?? [])
    }

    function getSelectedValues(key: string): string[] {
        const excludedSet = getExcludedSet(key)
        return optionsByKey(key)
            .filter((option) => !excludedSet.has(option.value))
            .map((option) => option.value)
    }

    function setCategorySelection(key: string, selectedValues: string[]) {
        const selectedSet = new Set(selectedValues)
        const nextExcluded = optionsByKey(key)
            .filter((option) => !selectedSet.has(option.value))
            .map((option) => option.value)
        if (nextExcluded.length === 0) {
            const { [key]: _removed, ...rest } = excluded.value
            excluded.value = rest
        } else {
            excluded.value = { ...excluded.value, [key]: nextExcluded }
        }
    }

    function setExcludedValues(nextExcluded: Record<string, string[]>) {
        excluded.value = Object.fromEntries(
            Object.entries(nextExcluded).map(([key, values]) => [key, [...values]]),
        )
    }

    function isCategoryFiltering(key: string): boolean {
        const options = optionsByKey(key)
        if (options.length === 0) return false
        const excludedSet = getExcludedSet(key)
        return excludedSet.size > 0
    }

    // 选项变化时修剪失效的排除值（选项消失 → 自动从排除集移除）。
    watch(
        [metadataFilterValidationOptions, () => filterOptionsReady?.value ?? true],
        ([categories]) => {
            const next = pruneMetadataFilterSelections(
                excluded.value,
                categories,
                filterOptionsReady?.value ?? true,
            )
            if (JSON.stringify(next) !== JSON.stringify(excluded.value)) excluded.value = next
        },
        { immediate: true },
    )

    watch(
        excluded,
        (value) => {
            if (persistKey) metadataFilterMemory.set(persistKey, value)
        },
        { deep: true },
    )

    function applyMetadataFilters(source: ContentItem[]): ContentItem[] {
        const active = definitions.value.filter((definition) => isCategoryFiltering(definition.key))
        if (active.length === 0) return source

        return source.filter((item) =>
            active.every((definition) => {
                const options = optionsByKey(definition.key)
                const excludedSet = getExcludedSet(definition.key)
                // 该分类所有选项都被取消勾选 → 没有任何允许值 → 任何条目都不满足该分类
                if (excludedSet.size === options.length) return false
                return definition.values(item).some((value) => !excludedSet.has(value))
            }),
        )
    }

    return {
        metadataFilterCategories,
        excluded,
        getSelectedValues,
        setCategorySelection,
        setExcludedValues,
        isCategoryFiltering,
        applyMetadataFilters,
    }
}
