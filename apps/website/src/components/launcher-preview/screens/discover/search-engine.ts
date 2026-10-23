import type { FilterType, FilterValue } from '@modrinth/ui/src/utils/search.ts'

import type { DiscoverType, ProjectCard, ProjectDetail } from '../../data'
import { projectIcon } from '../../data'

/**
 * The resource search of the launcher (packages/app-lib/src/resources/search.rs and the Modrinth
 * provider) run over the Modrinth snapshot pools: the same supported orderings, the same
 * filter expression semantics (`new_filters`), cursor style paging with `has_more`, and the
 * same per-source failures. The snapshot holds the first page of each Modrinth search, so
 * results are that pool filtered and ordered locally.
 */

export type ResourceSourceId = 'modrinth' | 'mcarchive'

export interface SearchHit {
    project_id: string
    provider_project_id: string
    provider: 'modrinth'
    providers: 'modrinth'[]
    project_type: string
    slug: string
    title: string
    description: string
    author: string
    icon_url: string
    categories: string[]
    display_categories: string[]
    versions: string[]
    downloads: number
    follows: number
    date_created: string
    date_modified: string
    resource_dates: {
        createdAt: string | null
        projectPublishedAt: string | null
        updatedAt: string | null
        firstPublishedAt: string | null
        latestPublishedAt: string | null
    }
    latest_version: string
    license: string
    client_side: string
    server_side: string
    gallery: string[]
    featured_gallery: string | null
    color: null
    installed: boolean
}

export class SourceFailure extends Error {
    constructor(public source: ResourceSourceId) {
        super(`${source} search failed`)
    }
}

/** Orderings the Modrinth provider supports (`supported_search_sorts`). */
const MODRINTH_SORTS = new Set(['relevance', 'downloads', 'follows', 'newest', 'updated'])

const LOADER_TAGS = new Set([
    'fabric',
    'forge',
    'quilt',
    'neoforge',
    'paper',
    'bukkit',
    'spigot',
    'datapack',
    'optifine',
    'iris',
])

const PLUGIN_LOADERS = new Set([
    'paper',
    'spigot',
    'bukkit',
    'purpur',
    'sponge',
    'folia',
    'velocity',
    'waterfall',
    'bungeecord',
])

/** OSI-approved licenses Modrinth's `open_source` facet accepts. */
const OPEN_SOURCE_LICENSES = new Set(
    [
        '0BSD',
        'AFL-3.0',
        'AGPL-3.0',
        'AGPL-3.0-only',
        'AGPL-3.0-or-later',
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
        'GPL-2.0-only',
        'GPL-2.0-or-later',
        'GPL-3.0',
        'GPL-3.0-only',
        'GPL-3.0-or-later',
        'ISC',
        'LGPL-2.1',
        'LGPL-2.1-only',
        'LGPL-2.1-or-later',
        'LGPL-3.0',
        'LGPL-3.0-only',
        'LGPL-3.0-or-later',
        'MIT',
        'MIT-0',
        'MPL-1.1',
        'MPL-2.0',
        'MS-PL',
        'NCSA',
        'OSL-3.0',
        'PostgreSQL',
        'Unlicense',
        'UPL-1.0',
        'Zlib',
    ].map((id) => id.toLowerCase()),
)

export function licenseId(license: ProjectCard['license']): string {
    if (!license) return ''
    return typeof license === 'string' ? license : license.id
}

function isOpenSource(license: string) {
    return OPEN_SOURCE_LICENSES.has(license.toLowerCase())
}

/** The v2 sides of a project mapped back to Modrinth's v3 environment values. */
export function environmentOf(clientSide: string, serverSide: string): string {
    const client = clientSide as 'required' | 'optional' | 'unsupported' | 'unknown'
    const server = serverSide as 'required' | 'optional' | 'unsupported' | 'unknown'
    if (client === 'unknown' || server === 'unknown') return 'unknown'
    if (client === 'required' && server === 'required') return 'client_and_server'
    if (client === 'optional' && server === 'optional') return 'client_or_server'
    if (client === 'required' && server === 'optional') return 'client_only_server_optional'
    if (client === 'optional' && server === 'required') return 'server_only_client_optional'
    if (client !== 'unsupported' && server === 'unsupported') return 'client_only'
    if (client === 'unsupported' && server !== 'unsupported') return 'server_only'
    return 'unknown'
}

function environmentGroups(client: boolean, server: boolean): string[][] {
    if (client && server)
        return [
            [
                'client_only_server_optional',
                'server_only_client_optional',
                'client_and_server',
                'client_or_server',
                'client_or_server_prefers_both',
            ],
        ]
    if (client)
        return [
            [
                'client_only',
                'client_only_server_optional',
                'client_or_server_prefers_both',
                'client_or_server',
            ],
        ]
    if (server)
        return [
            [
                'server_only',
                'dedicated_server_only',
                'server_only_client_optional',
                'client_or_server_prefers_both',
                'client_or_server',
            ],
        ]
    return []
}

/** Loader tags of a search hit (the provider falls back to known loader categories). */
function hitLoaders(card: ProjectCard) {
    return card.loaders?.length
        ? card.loaders
        : card.categories.filter((tag) => LOADER_TAGS.has(tag))
}

/** `all_project_types` of a hit: data packs and plugins are mods with matching loaders. */
function allProjectTypes(card: ProjectCard) {
    const types = new Set<string>([card.project_type])
    const loaders = hitLoaders(card)
    if (card.project_type === 'mod') {
        if (loaders.includes('datapack')) types.add('datapack')
        if (loaders.some((loader) => PLUGIN_LOADERS.has(loader))) types.add('plugin')
        if (!loaders.some((loader) => loader !== 'datapack' && !PLUGIN_LOADERS.has(loader)))
            types.delete('mod')
    }
    return types
}

export function toSearchHit(
    card: ProjectCard,
    projectType: string,
    installed: boolean,
    detail?: ProjectDetail | null,
): SearchHit {
    const loaders = hitLoaders(card)
    const gallery = (detail?.gallery ?? []).map((image) => image.url)
    return {
        project_id: card.id,
        provider_project_id: card.id,
        provider: 'modrinth',
        providers: ['modrinth'],
        project_type: projectType,
        slug: card.slug ?? card.id,
        title: card.title,
        description: card.description,
        author: card.author ?? '',
        icon_url: projectIcon(card.id),
        categories: [...card.categories, ...loaders],
        display_categories: card.categories,
        versions: card.game_versions,
        downloads: card.downloads,
        follows: card.followers,
        date_created: '',
        date_modified: card.date_modified ?? card.updated ?? '',
        resource_dates: {
            createdAt: null,
            projectPublishedAt: card.date_created ?? null,
            updatedAt: card.date_modified ?? card.updated ?? null,
            firstPublishedAt: null,
            latestPublishedAt: null,
        },
        latest_version: '',
        license: licenseId(card.license),
        client_side: card.client_side,
        server_side: card.server_side,
        gallery,
        featured_gallery: gallery[0] ?? null,
        color: null,
        installed,
    }
}

interface Condition {
    field: string
    values: string[]
    mode: 'all' | 'any' | 'none'
}

/**
 * Builds the filter conditions the way `useSearch` builds `new_filters`: `and` options become
 * one condition each, `or` options are grouped per field, negative options become `NOT IN`.
 */
function buildConditions(
    filters: FilterValue[],
    filterTypes: FilterType[],
    projectType: string,
): { conditions: Condition[]; dependencyFilters: FilterValue[] } {
    const conditions: Condition[] = []
    const orGroups: Record<string, string[]> = {}
    const negativeByField: Record<string, string[]> = {}
    const dependencyFilters: FilterValue[] = []

    for (const filterValue of filters) {
        if (filterValue.type === 'compatible_dependency_project_ids') {
            dependencyFilters.push(filterValue)
            continue
        }
        const type = filterTypes.find((candidate) => candidate.id === filterValue.type)
        if (!type || type.id === 'advanced') continue
        let option = type.options.find((candidate) => candidate.id === filterValue.option)
        if (!option && type.allows_custom_options) {
            option = {
                id: filterValue.option,
                method: type.allows_custom_options,
                value: type.custom_option_field
                    ? `${type.custom_option_field}:${filterValue.option}`
                    : filterValue.option,
            }
        }
        if (!option || (option.method !== 'or' && option.method !== 'and')) continue
        const separator = option.value.indexOf(':')
        const field = option.value.slice(0, separator)
        const value = option.value.slice(separator + 1)
        if (!field || !value) continue
        if (filterValue.negative) (negativeByField[field] ??= []).push(value)
        else if (option.method === 'or') (orGroups[field] ??= []).push(value)
        else conditions.push({ field, values: [value], mode: 'all' })
    }
    for (const [field, values] of Object.entries(orGroups))
        conditions.push({ field, values, mode: 'any' })
    for (const [field, values] of Object.entries(negativeByField))
        conditions.push({ field, values, mode: 'none' })

    const client = filters.some(
        (filter) => filter.type === 'environment' && filter.option === 'client',
    )
    const server = filters.some(
        (filter) => filter.type === 'environment' && filter.option === 'server',
    )
    for (const group of environmentGroups(client, server))
        conditions.push({ field: 'environment', values: group, mode: 'any' })

    const excludedTypes = filters
        .filter((filter) => filter.type === 'advanced')
        .map((filter) => filter.option)
    if (excludedTypes.length)
        conditions.push({ field: 'all_project_types', values: excludedTypes, mode: 'none' })

    void projectType
    return { conditions, dependencyFilters }
}

function fieldValues(card: ProjectCard, field: string): string[] {
    switch (field) {
        case 'categories':
            return [...card.categories, ...hitLoaders(card)]
        case 'game_versions':
            return card.game_versions
        case 'open_source':
            return [isOpenSource(licenseId(card.license)) ? 'true' : 'false']
        case 'project_id':
            return [card.id]
        case 'environment':
            return [environmentOf(card.client_side, card.server_side)]
        case 'all_project_types':
            return [...allProjectTypes(card)]
        default:
            return []
    }
}

function matchesConditions(card: ProjectCard, conditions: Condition[]) {
    return conditions.every((condition) => {
        const values = fieldValues(card, condition.field)
        if (condition.mode === 'all')
            return condition.values.every((value) => values.includes(value))
        if (condition.mode === 'any')
            return condition.values.some((value) => values.includes(value))
        return !condition.values.some((value) => values.includes(value))
    })
}

function dependencyIds(detail: ProjectDetail | null | undefined, types: string[]) {
    const ids = new Set<string>()
    for (const version of detail?.versions ?? []) {
        for (const dependency of version.dependencies ?? []) {
            if (!dependency.project_id) continue
            if (types.includes(dependency.dependency_type)) ids.add(dependency.project_id)
        }
    }
    return ids
}

function parseDependencyOption(option: string) {
    const separator = option.indexOf(':')
    if (separator === -1) return { projectId: option, types: ['required'] }
    const types = option
        .slice(0, separator)
        .split(',')
        .filter((type) => ['required', 'optional', 'embedded'].includes(type))
    const projectId = option.slice(separator + 1)
    return types.length && projectId
        ? { projectId, types }
        : { projectId: option, types: ['required'] }
}

function textScore(card: ProjectCard, tokens: string[]): number {
    const title = card.title.toLowerCase()
    const slug = (card.slug ?? '').toLowerCase()
    const author = (card.author ?? '').toLowerCase()
    const description = card.description.toLowerCase()
    let score = 0
    for (const token of tokens) {
        if (title === token) score += 100
        else if (title.startsWith(token)) score += 60
        else if (title.includes(token)) score += 40
        else if (slug.includes(token) || author.includes(token)) score += 20
        else if (description.includes(token)) score += 10
        else return -1
    }
    return score
}

function timeOf(value: string | null | undefined) {
    const time = value ? Date.parse(value) : Number.NaN
    return Number.isNaN(time) ? 0 : time
}

export interface ResourceSearchInput {
    projectType: DiscoverType
    pool: ProjectCard[]
    text: string
    filters: FilterValue[]
    filterTypes: FilterType[]
    sort: string
    limit: number
    page: number
    source: 'all' | 'modrinth' | 'mcarchive'
    installedIds: Set<string>
    details: (id: string) => ProjectDetail | null | undefined
}

export interface ResourceSearchPage {
    hits: SearchHit[]
    total: number
    hasMore: boolean
    page: number
    failures: ResourceSourceId[]
}

/** Whether MCArchive takes part in an "All sources" search (its `validate_query`). */
function mcArchiveApplies(input: ResourceSearchInput) {
    if (input.projectType !== 'mod' || input.sort !== 'relevance') return false
    const gameVersions = input.filters.filter(
        (filter) => filter.type === 'game_version' && !filter.negative,
    )
    const others = input.filters.filter((filter) => filter.type !== 'game_version')
    return gameVersions.length <= 1 && others.length === 0
}

export function runResourceSearch(input: ResourceSearchInput): ResourceSearchPage {
    const sources: ResourceSourceId[] =
        input.source === 'all'
            ? ['modrinth', ...(mcArchiveApplies(input) ? (['mcarchive'] as const) : [])]
            : [input.source]
    const failures: ResourceSourceId[] = []
    let entries: ProjectCard[] = []

    for (const source of sources) {
        if (source === 'mcarchive') {
            // No MCArchive snapshot: the source cannot answer, like an unreachable provider.
            failures.push('mcarchive')
            continue
        }
        if (!MODRINTH_SORTS.has(input.sort)) {
            failures.push('modrinth')
            continue
        }
        const tokens = input.text.trim().toLowerCase().split(/\s+/).filter(Boolean)
        const { conditions, dependencyFilters } = buildConditions(
            input.filters,
            input.filterTypes,
            input.projectType,
        )
        const scored = input.pool
            .map((card, index) => ({
                card,
                index,
                score: tokens.length ? textScore(card, tokens) : 0,
            }))
            .filter((entry) => entry.score >= 0)
            .filter((entry) => matchesConditions(entry.card, conditions))
            .filter((entry) =>
                dependencyFilters.every((filter) => {
                    const { projectId, types } = parseDependencyOption(filter.option)
                    const fields =
                        input.projectType === 'modpack'
                            ? ['required', 'optional', 'embedded']
                            : types
                    const has = dependencyIds(input.details(entry.card.id), fields).has(projectId)
                    return filter.negative ? !has : has
                }),
            )
        const sorted = scored.slice().sort((a, b) => {
            switch (input.sort) {
                case 'downloads':
                    return b.card.downloads - a.card.downloads
                case 'follows':
                    return b.card.followers - a.card.followers
                case 'newest':
                    return timeOf(b.card.date_created) - timeOf(a.card.date_created)
                case 'updated':
                    return (
                        timeOf(b.card.date_modified ?? b.card.updated) -
                        timeOf(a.card.date_modified ?? a.card.updated)
                    )
                default:
                    return tokens.length
                        ? b.score - a.score || a.index - b.index
                        : a.index - b.index
            }
        })
        entries = sorted.map((entry) => entry.card)
    }

    if (failures.length === sources.length) {
        throw new SourceFailure(failures[0])
    }

    // Cursor paging cannot skip past the last page: a stale page number lands on the last one.
    const lastPage = Math.max(1, Math.ceil(entries.length / input.limit))
    const page = Math.min(Math.max(1, input.page), lastPage)
    const offset = (page - 1) * input.limit
    const pageEntries = entries.slice(offset, offset + input.limit)
    return {
        hits: pageEntries.map((card) =>
            toSearchHit(
                card,
                input.projectType,
                input.installedIds.has(card.id),
                input.details(card.id),
            ),
        ),
        total: entries.length,
        hasMore: offset + input.limit < entries.length,
        page,
        failures,
    }
}

export function sourceName(source: ResourceSourceId) {
    return source === 'mcarchive' ? 'MCArchive' : 'Modrinth'
}

/**
 * A project's own categories (v2 `categories`). The snapshot keeps the search index's list,
 * which also names the project's loaders.
 */
export function projectCategories(project: { categories: string[]; loaders: string[] }) {
    return project.categories.filter((category) => !project.loaders.includes(category))
}
