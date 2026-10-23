import { categoryIconMap, loaderIconMap } from '@modrinth/assets/generated-icons.ts'
import ClientIcon from '@modrinth/assets/icons/client.svg?component'
import ServerIcon from '@modrinth/assets/icons/server.svg?component'
import { defineMessage, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import type {
    FilterOption,
    FilterType,
    FilterValue,
    ProjectType,
    SortType,
    Tags,
} from '@modrinth/ui/src/utils/search.ts'
import {
    DEFAULT_MOD_LOADERS,
    DEFAULT_PLUGIN_LOADERS,
    DEFAULT_SHADER_LOADERS,
    formatCategory,
    formatCategoryHeader,
    formatLoader,
} from '@modrinth/ui/src/utils/tag-messages.ts'
import { computed, readonly, type Ref, ref } from 'vue'
import { type LocationQueryRaw, type LocationQueryValue, useRoute } from 'vue-router'

/**
 * Port of `useSearch` (packages/ui/src/utils/search.ts): the filter types of a project type,
 * the selected filters, sort and paging, and their URL query form. The browse page keeps its
 * own parameters (instance, source, return path) next to the filter ones; those are skipped
 * when the query is read instead of being reported as unknown filter types.
 */

export function getCategoryIcon(name: string) {
    return name ? categoryIconMap[name.toLowerCase()] : undefined
}

export function getLoaderIcon(name: string) {
    return name ? loaderIconMap[name.toLowerCase()] : undefined
}

export function getTagIcon(name: string) {
    return getLoaderIcon(name) ?? getCategoryIcon(name)
}

const SERVER_HEADER_ORDER = [
    'minecraft_server_features',
    'minecraft_server_gameplay',
    'minecraft_server_meta',
    'minecraft_server_community',
]

type TagCategory = Tags['categories'][number]

/** `sortedCategories` (packages/utils/utils.ts). */
export function sortedCategories(
    tags: Tags,
    formatCategoryName: (name: string) => string,
    locale: string,
): TagCategory[] {
    return tags.categories.slice().sort((a, b) => {
        const headerCompare = a.header.localeCompare(b.header)
        if (headerCompare !== 0) {
            const aServerIdx = SERVER_HEADER_ORDER.indexOf(a.header)
            const bServerIdx = SERVER_HEADER_ORDER.indexOf(b.header)
            if (aServerIdx !== -1 && bServerIdx !== -1) return aServerIdx - bServerIdx
            return headerCompare
        }
        if (a.header === 'performance impact' && b.header === 'performance impact') {
            const order = ['potato', 'low', 'medium', 'high', 'screenshot']
            return order.indexOf(a.name) - order.indexOf(b.name)
        }
        if (a.name === 'pokemon') return -1
        if (b.name === 'pokemon') return 1
        return formatCategoryName(a.name).localeCompare(formatCategoryName(b.name), locale, {
            numeric: true,
        })
    })
}

const ALL_PROJECT_TYPES: ProjectType[] = [
    'mod',
    'modpack',
    'resourcepack',
    'shader',
    'datapack',
    'plugin',
    'server',
]

const PLUGIN_PLATFORMS = ['bungeecord', 'waterfall', 'velocity', 'geyser']

const PROJECT_TYPE_EXCLUSION_FILTERS: Partial<Record<ProjectType, ProjectType[]>> = {
    mod: ['plugin', 'datapack'],
    plugin: ['mod', 'datapack'],
    datapack: ['mod', 'plugin'],
}

export const DEFAULT_SORT_TYPES: SortType[] = [
    { name: 'relevance', display: 'Relevance' },
    { name: 'downloads', display: 'Downloads' },
    { name: 'follows', display: 'Followers' },
    { name: 'newest', display: 'Project published' },
    { name: 'created', display: 'Project created' },
    { name: 'updated', display: 'Project modified' },
    { name: 'firstPublished', display: 'First release published' },
    { name: 'latestPublished', display: 'Latest release published' },
]

export function useSearch(
    projectTypes: Ref<ProjectType[]>,
    tags: Ref<Tags>,
    providedFilters: Ref<FilterValue[]>,
    options: { sortOptions?: readonly SortType[]; persistentQueryParams?: string[] } = {},
) {
    const query = ref('')
    const maxResults = ref(20)
    const sortTypes: readonly SortType[] = readonly(options.sortOptions ?? DEFAULT_SORT_TYPES)
    const currentSortType: Ref<SortType> = ref(sortTypes[0])
    const route = useRoute()
    const currentPage = ref(1)
    const currentFilters: Ref<FilterValue[]> = ref<FilterValue[]>([])
    const toggledGroups = ref<string[]>([])
    const overriddenProvidedFilterTypes = ref<string[]>([])

    const { formatMessage, locale } = useVIntl()
    const dependsOnFilterName = defineMessage({
        id: 'search.filter_type.compatible_dependency_project_ids',
        defaultMessage: 'Depends on',
    })
    const includedContentFilterName = defineMessage({
        id: 'search.filter_type.included_content',
        defaultMessage: 'Included content',
    })
    const formatCategoryName = (categoryName: string) => formatCategory(formatMessage, categoryName)

    const formatExcludeProjectTypeLabel = (projectType: ProjectType): string => {
        switch (projectType) {
            case 'mod':
                return formatMessage(
                    defineMessage({
                        id: 'search.filter_type.advanced.exclude_mod',
                        defaultMessage: 'Exclude mods',
                    }),
                )
            case 'plugin':
                return formatMessage(
                    defineMessage({
                        id: 'search.filter_type.advanced.exclude_plugin',
                        defaultMessage: 'Exclude plugins',
                    }),
                )
            case 'datapack':
                return formatMessage(
                    defineMessage({
                        id: 'search.filter_type.advanced.exclude_datapack',
                        defaultMessage: 'Exclude data packs',
                    }),
                )
            default:
                return projectType
        }
    }

    const filters = computed(() => {
        const categoryFilters: Record<string, FilterType> = {}
        const categories = sortedCategories(tags.value, formatCategoryName, locale.value)
        for (const category of categories) {
            const filterTypeId = `category_${category.project_type}_${category.header}`
            if (!categoryFilters[filterTypeId]) {
                categoryFilters[filterTypeId] = {
                    id: filterTypeId,
                    formatted_name: formatCategoryHeader(formatMessage, category.header),
                    supported_project_types:
                        category.project_type === 'mod'
                            ? ['mod', 'plugin', 'datapack']
                            : ([category.project_type] as ProjectType[]),
                    display: 'all',
                    query_param: category.header === 'resolutions' ? 'g' : 'f',
                    supports_negative_filter: true,
                    searchable: false,
                    options: [],
                }
            }
            categoryFilters[filterTypeId].options.push({
                id: category.name,
                formatted_name: formatCategory(formatMessage, category.name),
                icon: getCategoryIcon(category.name),
                value: `categories:${category.name}`,
                method: category.header === 'resolutions' ? 'or' : 'and',
            })
        }

        const excludeableProjectTypes: ProjectType[] = []
        for (const projectType of projectTypes.value) {
            for (const target of PROJECT_TYPE_EXCLUSION_FILTERS[projectType] ?? []) {
                if (!excludeableProjectTypes.includes(target)) excludeableProjectTypes.push(target)
            }
        }

        const loaderOption = (name: string): FilterOption => ({
            id: name,
            formatted_name: formatLoader(formatMessage, name),
            icon: getLoaderIcon(name),
            method: 'or',
            value: `categories:${name}`,
        })

        const filterTypes: FilterType[] = [
            ...Object.values(categoryFilters),
            {
                id: 'compatible_dependency_project_ids',
                formatted_name: formatMessage(
                    projectTypes.value.includes('modpack')
                        ? includedContentFilterName
                        : dependsOnFilterName,
                ),
                supported_project_types: ALL_PROJECT_TYPES,
                query_param: 'dep',
                supports_negative_filter: true,
                display: 'depends-on-project',
                searchable: false,
                options: [],
                allows_custom_options: 'and',
                custom_option_field: 'compatible_dependency_project_ids',
                ordering: projectTypes.value.includes('modpack') ? undefined : -999,
            },
            {
                id: 'environment',
                formatted_name: formatMessage(
                    defineMessage({
                        id: 'search.filter_type.environment',
                        defaultMessage: 'Environment',
                    }),
                ),
                supported_project_types: ['mod', 'modpack'],
                display: 'all',
                query_param: 'e',
                supports_negative_filter: false,
                searchable: false,
                options: [
                    {
                        id: 'client',
                        formatted_name: formatMessage(
                            defineMessage({
                                id: 'search.filter_type.environment.client',
                                defaultMessage: 'Client',
                            }),
                        ),
                        icon: ClientIcon,
                        method: 'environment',
                        environment: 'client',
                    },
                    {
                        id: 'server',
                        formatted_name: formatMessage(
                            defineMessage({
                                id: 'search.filter_type.environment.server',
                                defaultMessage: 'Server',
                            }),
                        ),
                        icon: ServerIcon,
                        method: 'environment',
                        environment: 'server',
                    },
                ],
            },
            {
                id: 'game_version',
                formatted_name: formatMessage(
                    defineMessage({
                        id: 'search.filter_type.game_version',
                        defaultMessage: 'Game version',
                    }),
                ),
                supported_project_types: ALL_PROJECT_TYPES,
                display: 'scrollable',
                query_param: 'v',
                supports_negative_filter: false,
                toggle_groups: [
                    {
                        id: 'all_versions',
                        formatted_name: formatMessage(
                            defineMessage({
                                id: 'search.filter_type.game_version.all_versions',
                                defaultMessage: 'Show all versions',
                            }),
                        ),
                        query_param: 'h',
                    },
                ],
                searchable: true,
                options: tags.value.gameVersions.map((gameVersion) => ({
                    id: gameVersion.version,
                    toggle_group:
                        gameVersion.version_type !== 'release' ? 'all_versions' : undefined,
                    value: `game_versions:${gameVersion.version}`,
                    query_value: gameVersion.version,
                    method: 'or',
                })),
                ordering: projectTypes.value.includes('mod')
                    ? 2
                    : projectTypes.value.includes('shader')
                      ? -1
                      : undefined,
            },
            {
                id: 'mod_loader',
                formatted_name: formatMessage(
                    defineMessage({
                        id: 'search.filter_type.mod_loader',
                        defaultMessage: 'Loader',
                    }),
                ),
                supported_project_types: ['mod'],
                display: 'expandable',
                query_param: 'g',
                supports_negative_filter: true,
                default_values: DEFAULT_MOD_LOADERS,
                searchable: false,
                options: tags.value.loaders
                    .filter(
                        (loader) =>
                            loader.supported_project_types.includes('mod') &&
                            !loader.supported_project_types.includes('plugin') &&
                            !loader.supported_project_types.includes('datapack'),
                    )
                    .map((loader) => loaderOption(loader.name)),
                ordering: projectTypes.value.includes('mod') ? 1 : undefined,
            },
            {
                id: 'modpack_loader',
                formatted_name: formatMessage(
                    defineMessage({
                        id: 'search.filter_type.modpack_loader',
                        defaultMessage: 'Loader',
                    }),
                ),
                supported_project_types: ['modpack'],
                display: 'all',
                query_param: 'g',
                supports_negative_filter: true,
                searchable: false,
                options: tags.value.loaders
                    .filter((loader) => loader.supported_project_types.includes('modpack'))
                    .map((loader) => loaderOption(loader.name)),
            },
            {
                id: 'plugin_loader',
                formatted_name: formatMessage(
                    defineMessage({
                        id: 'search.filter_type.plugin_loader',
                        defaultMessage: 'Loader',
                    }),
                ),
                supported_project_types: ['plugin'],
                display: 'expandable',
                default_values: DEFAULT_PLUGIN_LOADERS,
                query_param: 'g',
                supports_negative_filter: true,
                searchable: false,
                options: tags.value.loaders
                    .filter(
                        (loader) =>
                            loader.supported_project_types.includes('plugin') &&
                            !PLUGIN_PLATFORMS.includes(loader.name),
                    )
                    .map((loader) => loaderOption(loader.name)),
            },
            {
                id: 'plugin_platform',
                formatted_name: formatMessage(
                    defineMessage({
                        id: 'search.filter_type.plugin_platform',
                        defaultMessage: 'Platform',
                    }),
                ),
                supported_project_types: ['plugin'],
                display: 'all',
                query_param: 'g',
                supports_negative_filter: true,
                searchable: false,
                options: tags.value.loaders
                    .filter((loader) => PLUGIN_PLATFORMS.includes(loader.name))
                    .map((loader) => loaderOption(loader.name)),
            },
            {
                id: 'shader_loader',
                formatted_name: formatMessage(
                    defineMessage({
                        id: 'search.filter_type.shader_loader',
                        defaultMessage: 'Loader',
                    }),
                ),
                supported_project_types: ['shader'],
                query_param: 'g',
                supports_negative_filter: true,
                searchable: false,
                display: 'expandable',
                default_values: DEFAULT_SHADER_LOADERS,
                options: tags.value.loaders
                    .filter((loader) => loader.supported_project_types.includes('shader'))
                    .map((loader) => loaderOption(loader.name)),
            },
            {
                id: 'license',
                formatted_name: formatMessage(
                    defineMessage({ id: 'search.filter_type.license', defaultMessage: 'License' }),
                ),
                supported_project_types: [
                    'mod',
                    'modpack',
                    'resourcepack',
                    'shader',
                    'plugin',
                    'datapack',
                ],
                query_param: 'l',
                supports_negative_filter: true,
                display: 'all',
                searchable: false,
                options: [
                    {
                        id: 'open_source',
                        formatted_name: formatMessage(
                            defineMessage({
                                id: 'search.filter_type.license.open_source',
                                defaultMessage: 'Open source',
                            }),
                        ),
                        method: 'and',
                        value: 'open_source:true',
                    },
                ],
            },
            {
                id: 'project_id',
                formatted_name: formatMessage(
                    defineMessage({
                        id: 'search.filter_type.project_id',
                        defaultMessage: 'Project ID',
                    }),
                ),
                supported_project_types: ALL_PROJECT_TYPES,
                query_param: 'pid',
                supports_negative_filter: true,
                display: 'none',
                searchable: false,
                options: [],
                allows_custom_options: 'and',
            },
            {
                id: 'advanced',
                formatted_name: formatMessage(
                    defineMessage({
                        id: 'search.filter_type.advanced',
                        defaultMessage: 'Advanced',
                    }),
                ),
                supported_project_types: ['mod', 'plugin', 'datapack'],
                display: 'toggle',
                query_param: 'a',
                searchable: false,
                ordering: -1000,
                supports_negative_filter: false,
                options: excludeableProjectTypes.map((target) => ({
                    id: target,
                    formatted_name: formatExcludeProjectTypeLabel(target),
                    method: 'and',
                    value: `all_project_types:${target}`,
                })),
            },
        ]

        return filterTypes
            .filter((filterType) =>
                filterType.supported_project_types.some((projectType) =>
                    projectTypes.value.includes(projectType),
                ),
            )
            .sort((a, b) => (b.ordering ?? 0) - (a.ordering ?? 0))
    })

    /** The filters a search uses: the user's, minus types the target provides, plus the provided ones. */
    const effectiveFilters = computed(() => {
        const validProvidedFilters = providedFilters.value.filter(
            (providedFilter) => !overriddenProvidedFilterTypes.value.includes(providedFilter.type),
        )
        const filteredFilters = currentFilters.value.filter(
            (userFilter) =>
                !validProvidedFilters.some(
                    (providedFilter) => providedFilter.type === userFilter.type,
                ),
        )
        return [...filteredFilters, ...validProvidedFilters]
    })

    /** Serialized request (`limit`, `index`, `query`, filters, `offset`): a change triggers a search. */
    const requestParams = computed(() => {
        const params = [`limit=${maxResults.value}`, `index=${currentSortType.value.name}`]
        if (query.value.length > 0) params.push(`query=${encodeURIComponent(query.value)}`)
        params.push(
            `filters=${encodeURIComponent(
                JSON.stringify(
                    effectiveFilters.value.map((filter) => [
                        filter.type,
                        filter.option,
                        !!filter.negative,
                    ]),
                ),
            )}`,
        )
        params.push(`types=${projectTypes.value.join(',')}`)
        if (currentPage.value !== 1)
            params.push(`offset=${(currentPage.value - 1) * maxResults.value}`)
        return `?${params.join('&')}`
    })

    readQueryParams()

    function readQueryParams() {
        const readParams = new Set<string>(options.persistentQueryParams ?? [])

        loadQueryParam(['l'], (openSource) => {
            if (
                openSource === 'true' &&
                !currentFilters.value.some(
                    (filter) => filter.type === 'license' && filter.option === 'open_source',
                )
            ) {
                currentFilters.value.push({
                    type: 'license',
                    option: 'open_source',
                    negative: false,
                })
                readParams.add('l')
            }
        })
        loadQueryParam(['nf'], (filter) => {
            const set = typeof filter === 'string' ? new Set([filter]) : new Set(filter)
            typesLoop: for (const type of filters.value) {
                for (const option of type.options) {
                    const value = getOptionValue(option, false)
                    if (
                        set.has(value) &&
                        !currentFilters.value.some(
                            (selected) =>
                                selected.type === type.id && selected.option === option.id,
                        )
                    ) {
                        currentFilters.value.push({
                            type: type.id,
                            option: option.id,
                            negative: true,
                        })
                        readParams.add(type.query_param)
                        set.delete(value)
                        if (set.size === 0) break typesLoop
                    }
                }
            }
        })
        loadQueryParam(['s'], (sort) => {
            currentSortType.value =
                sortTypes.find((sortType) => sortType.name === sort) ?? sortTypes[0]
            readParams.add('s')
        })
        loadQueryParam(['m'], (count) => {
            maxResults.value = Number(count)
            readParams.add('m')
        })
        loadQueryParam(['o'], (offset) => {
            currentPage.value = Math.ceil(Number(offset) / maxResults.value) + 1
            readParams.add('o')
        })
        loadQueryParam(['page'], (page) => {
            currentPage.value = Number(page)
            readParams.add('page')
        })
        loadQueryParam(['q'], (queryValue) => {
            query.value = String(queryValue)
            readParams.add('q')
        })

        for (const key of Object.keys(route.query).filter((key) => !readParams.has(key))) {
            const types = filters.value.filter((type) => type.query_param === key)
            if (types.length === 0) continue
            for (const value of getParamValuesAsArray(route.query[key])) {
                const negative =
                    value.startsWith('!=') || (!value.includes(':') && value.includes('!='))
                let matched = false
                for (const type of types) {
                    const option = type.options.find(
                        (candidate) => getOptionValue(candidate, negative) === value,
                    )
                    if (!option) continue
                    currentFilters.value.push({ type: type.id, option: option.id, negative })
                    matched = true
                    break
                }
                if (!matched) {
                    const customType = types.find((type) => type.allows_custom_options)
                    if (customType) {
                        currentFilters.value.push({
                            type: customType.id,
                            option: value.startsWith('!=')
                                ? value.slice(2)
                                : value.replace('!=', ':'),
                            negative,
                        })
                    }
                }
            }
        }
    }

    function createPageParams(): LocationQueryRaw {
        const items: Record<string, string[]> = {}
        if (query.value) items.q = [query.value]
        currentFilters.value.forEach((filterValue) => {
            const type = filters.value.find((candidate) => candidate.id === filterValue.type)
            const option = type?.options.find((candidate) => candidate.id === filterValue.option)
            if (type && (option || type.allows_custom_options)) {
                const value = option
                    ? getOptionValue(option, filterValue.negative)
                    : filterValue.negative
                      ? `!=${filterValue.option}`
                      : filterValue.option
                if (items[type.query_param]) items[type.query_param].push(value)
                else items[type.query_param] = [value]
            }
        })
        toggledGroups.value.forEach((groupId) => {
            const group = filters.value
                .flatMap((filter) => filter.toggle_groups ?? [])
                .find((candidate) => candidate.id === groupId)
            if (group?.query_param) items[group.query_param] = [String(true)]
        })
        if (currentSortType.value.name !== 'relevance') items.s = [currentSortType.value.name]
        if (maxResults.value !== 20) items.m = [String(maxResults.value)]
        if (currentPage.value > 1) items.page = [String(currentPage.value)]
        return items
    }

    function loadQueryParam(
        params: string[],
        provider: (param: LocationQueryValue | LocationQueryValue[]) => void,
    ) {
        for (const param of params) {
            if (param in route.query) {
                provider(route.query[param])
                return
            }
        }
    }

    return {
        query,
        currentSortType,
        currentFilters,
        toggledGroups,
        currentPage,
        maxResults,
        overriddenProvidedFilterTypes,
        sortTypes,
        filters,
        effectiveFilters,
        requestParams,
        createPageParams,
    }
}

export function getOptionValue(option: FilterOption, negative?: boolean): string {
    let value = option.method === 'or' || option.method === 'and' ? option.value : option.id
    if (negative === true) value = value.replace(':', '!=')
    if (option.query_value) value = option.query_value
    return value
}

function getParamValuesAsArray(value: LocationQueryValue | LocationQueryValue[]): string[] {
    if (value === null || value === undefined) return []
    if (typeof value === 'string') return [value]
    return value.filter((entry): entry is string => entry !== null)
}
