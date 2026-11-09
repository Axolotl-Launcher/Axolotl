import type { CurseForgeCapability } from '@/helpers/curseforge'
import type {
    ProjectDocument,
    Resource,
    ResourcePage,
    ResourceRef,
    ResourceRelease,
    ResourceSearchExpression,
    ResourceSearchQuery,
    ResourceSearchResult,
    SourceReleaseRef,
} from '@/helpers/resources'
import type { SchematicPage, SchematicSource } from '@/helpers/schematics'
import type { ThirdPartyAccount } from '@/helpers/third-party-accounts'

import wikiEntries from '../data/content-names.json'
import {
    environmentTag,
    projectDocument,
    projectDocumentV3,
    projectMetadata,
    type RecordedHit,
    recordedHits,
    type RecordedProject,
    recordedProject,
    recordedTypes,
    release,
    releaseDocument,
    teamMembers,
    versionOwner,
} from './modrinth'
import type { Handlers } from './types'

/** The demo browses Modrinth only: the other sources report themselves unavailable. */
const curseForge: CurseForgeCapability = {
    status: 'missing_key',
    configured: false,
    key_source: 'missing',
}

/** The MC 百科 entries the app bundles, for the recorded slugs: slug → [wiki ID, Chinese name]. */
const wiki = wikiEntries as unknown as Record<string, [number, string | null]>
const chineseName = (slug: string) => wiki[slug]?.[1] ?? null

/** Fields of the search filter the recordings can answer; others never narrow the results. */
function matchesPredicate(hit: RecordedHit, field: string, values: string[]) {
    switch (field) {
        case 'category':
            return values.some((value) => hit.categories.includes(value.replace(/^modrinth:/, '')))
        // Selected loaders arrive as their own field, unprefixed (resource-search-filter.ts).
        case 'loader':
            return values.some((value) => hit.loaders.includes(value))
        case 'gameVersion':
            return values.some((value) => hit.game_versions.includes(value))
        case 'license':
            return values.includes(hit.license?.id ?? '')
        case 'projectId':
            return values.includes(hit.id)
        // Browse sends the environment tags a side accepts, such as `client_only`.
        case 'environment': {
            const environment = environmentTag(hit)
            return environment !== null && values.includes(environment)
        }
        default:
            return true
    }
}

function matches(
    hit: RecordedHit,
    expression: ResourceSearchExpression | null | undefined,
): boolean {
    if (!expression) return true
    if (expression.kind !== 'predicate') {
        const terms = expression.terms.map((term) => matches(hit, term))
        return expression.kind === 'all' ? terms.every(Boolean) : terms.some(Boolean)
    }
    const result = matchesPredicate(hit, expression.field, expression.values)
    return expression.excluded ? !result : result
}

function matchesText(hit: RecordedHit, text: string) {
    if (!text) return true
    return [
        hit.title,
        hit.slug,
        hit.description,
        hit.author ?? '',
        chineseName(hit.slug) ?? '',
    ].some((value) => value.toLowerCase().includes(text))
}

const sortKeys: Partial<Record<ResourceSearchQuery['sort'], (hit: RecordedHit) => number>> = {
    downloads: (hit) => hit.downloads,
    followers: (hit) => hit.followers,
    created: (hit) => Date.parse(hit.date_created ?? '') || 0,
    projectPublished: (hit) => Date.parse(hit.date_created ?? '') || 0,
    firstPublished: (hit) => Date.parse(hit.date_created ?? '') || 0,
    projectUpdated: (hit) => Date.parse(hit.date_modified ?? '') || 0,
    latestPublished: (hit) => Date.parse(hit.date_modified ?? '') || 0,
}

function searchResult(hit: RecordedHit): ResourceSearchResult {
    const project = { source: 'modrinth' as const, projectId: hit.id }
    return {
        entry: {
            queryEntryId: hit.id,
            resourceId: null,
            defaultProject: project,
            candidates: [project],
            status: 'confirmed',
            basis: [],
        },
        projects: [projectMetadata(hit)],
    }
}

/** A Discover page over the recorded hits, filtered and sorted like Modrinth would. */
async function search(
    requestId: string,
    query: ResourceSearchQuery,
    cursor: string | null,
): Promise<ResourcePage<ResourceSearchResult>> {
    const types = query.types.flatMap((type) => recordedTypes[type] ?? [])
    const text = query.text.trim().toLowerCase()
    const hits = (await Promise.all(types.map(recordedHits)))
        .flat()
        .filter((hit) => matchesText(hit, text) && matches(hit, query.filter))
    const sortKey = sortKeys[query.sort]
    if (sortKey) hits.sort((a, b) => sortKey(b) - sortKey(a))
    const offset = Number(cursor ?? 0)
    const end = offset + query.pageSize
    return {
        requestId,
        entries: hits.slice(offset, end).map(searchResult),
        cursor: end < hits.length ? String(end) : null,
        hasMore: end < hits.length,
        total: { value: hits.length, precision: 'exact' },
        sourceFailures: [],
        exhaustedSources: [],
    }
}

async function requireProject(locator: string) {
    const project = await recordedProject(locator)
    if (!project) throw new Error(`The demo has no recording of project ${locator}`)
    return project
}

/** The project a resource reference names; a route's locator wins when it has one. */
function referencedProject(reference: unknown, locator: unknown) {
    const ref = reference as ResourceRef
    return requireProject(
        String(locator ?? (ref.kind === 'source' ? ref.project.projectId : ref.id)),
    )
}

async function documents(
    locators: string[],
    document: (project: RecordedProject) => ProjectDocument,
) {
    const projects = await Promise.all(locators.map(recordedProject))
    return projects.flatMap((project) => (project ? [document(project)] : []))
}

export const discoverHandlers: Handlers = {
    'plugin:curseforge|curseforge_capability': () => curseForge,
    'plugin:planet-minecraft|planet_minecraft_connector_available': () => false,
    'plugin:axolotl-resources|resource_favorites': () => [],
    // Favorites are not recorded in the demo, so groups stay empty and writes are no-ops.
    'plugin:axolotl-resources|resource_favorite_groups': () => ({
        groups: [],
        memberships: [],
    }),
    'plugin:axolotl-resources|resource_favorite_group_create': ({ name }) => ({
        id: `demo-favorite-group-${String(name)}`,
        name: String(name),
    }),
    'plugin:axolotl-resources|resource_favorite_group_rename': ({ id, newName }) => ({
        id: String(id),
        name: String(newName),
    }),
    'plugin:axolotl-resources|resource_favorite_group_delete': () => undefined,
    'plugin:axolotl-resources|resource_favorite_group_update_memberships': () => undefined,
    // Schematic sites are not recorded: they answer like sources the launcher cannot reach.
    'plugin:schematics|schematics_list': ({ sources }): SchematicPage => ({
        items: [],
        pageCount: 0,
        total: 0,
        failedSources: sources as SchematicSource[],
    }),
    'plugin:third-party-accounts|get_account': (): ThirdPartyAccount => ({
        providerId: 'shimu',
        available: false,
        status: 'disconnected',
        user: null,
        expiresAt: null,
        flowId: null,
        authorization: null,
    }),
    'plugin:axolotl-resources|resource_search': ({ requestId, query, cursor }) =>
        search(String(requestId), query as ResourceSearchQuery, (cursor as string | null) ?? null),
    'plugin:axolotl-resources|resource_locate': async ({ locator }) =>
        projectDocument(await requireProject(String(locator))),
    'plugin:axolotl-resources|resource_project_documents': ({ projectIds }) =>
        documents(projectIds as string[], projectDocument),
    'plugin:axolotl-resources|resource_project_protocol': ({ locators }) =>
        documents(locators as string[], projectDocumentV3),
    'plugin:axolotl-resources|resource_release_documents': async ({ releaseIds, projectId }) => {
        const ids = releaseIds as string[]
        const owners = new Set([
            ...(projectId ? [String(projectId)] : []),
            ...ids.flatMap((id) => versionOwner(id) ?? []),
        ])
        const projects = (await Promise.all([...owners].map(recordedProject))).flatMap(
            (project) => project ?? [],
        )
        return ids.flatMap((id) =>
            projects.flatMap((project) =>
                project.versions
                    .filter((version) => version.id === id)
                    .map((version) => releaseDocument(project, version)),
            ),
        )
    },
    'plugin:axolotl-resources|resource_list_releases': async ({
        requestId,
        reference,
        locator,
    }): Promise<ResourcePage<ResourceRelease>> => {
        const project = await referencedProject(reference, locator)
        return {
            requestId: String(requestId),
            entries: project.versions.map((version) => release(project.id, version)),
            cursor: null,
            hasMore: false,
            total: { value: project.versions.length, precision: 'exact' },
            sourceFailures: [],
            exhaustedSources: [],
        }
    },
    'plugin:axolotl-resources|resource_get_changelog': async ({ releaseRef }) => {
        const reference = releaseRef as SourceReleaseRef
        const project = await requireProject(reference.project.projectId)
        const version = project.versions.find((entry) => entry.id === reference.releaseId)
        if (!version) throw new Error(`The demo has no recording of release ${reference.releaseId}`)
        return {
            release: reference,
            publishedAt: version.date_published,
            text: { source: 'modrinth', format: 'markdown', body: version.changelog ?? '' },
        }
    },
    'plugin:cache|get_team': async ({ id }) => {
        const project = await recordedProject(String(id))
        return project ? teamMembers(project) : []
    },
    'plugin:axolotl-resources|resource_get': async ({ reference, locator }): Promise<Resource> => {
        const metadata = projectMetadata(await referencedProject(reference, locator))
        return {
            id: null,
            confirmedProjects: [metadata.project],
            sourceFailures: [],
            defaultProject: metadata.project,
            projects: [metadata],
        }
    },
    'plugin:content-search|lookup_chinese_content_names': ({ modrinthSlugs }) => ({
        modrinth: Object.fromEntries(
            (modrinthSlugs as string[]).flatMap((slug) => {
                const name = chineseName(slug)
                return name ? [[slug, name]] : []
            }),
        ),
        curseforge: {},
    }),
    'plugin:content-search|lookup_content_wiki_ids': ({ modrinthSlugs }) => ({
        modrinth: Object.fromEntries(
            (modrinthSlugs as string[]).flatMap((slug) =>
                wiki[slug] ? [[slug, wiki[slug][0]]] : [],
            ),
        ),
        curseforge: {},
    }),
}
