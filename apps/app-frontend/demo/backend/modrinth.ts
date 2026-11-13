import type { Labrinth } from '@modrinth/api-client'

import type {
    ProjectDocument,
    ReleaseDocument,
    ResourceRelease,
    ResourceType,
    SourceProjectMetadata,
    SourceReleaseRef,
} from '@/helpers/resources'

/** A Modrinth search hit as recorded (data/discover/*.json). */
export interface RecordedHit {
    id: string
    slug: string
    title: string
    description: string
    author: string | null
    categories: string[]
    display_categories?: string[]
    client_side: Labrinth.Projects.v2.Environment
    server_side: Labrinth.Projects.v2.Environment
    project_type: Labrinth.Projects.v2.ProjectType
    downloads: number
    followers: number
    license: { id: string; name: string } | null
    loaders: string[]
    game_versions: string[]
    date_created: string | null
    date_modified: string | null
    color?: number | null
}

interface RecordedVersion {
    id: string
    name: string
    version_number: string
    version_type: Labrinth.Versions.v2.VersionType
    game_versions: string[]
    loaders: string[]
    featured: boolean
    date_published: string
    downloads: number
    changelog?: string | null
    dependencies: {
        project_id: string | null
        version_id: string | null
        dependency_type: Labrinth.Versions.v2.DependencyType
        file_name: string | null
    }[]
    files: { filename: string; size: number; primary: boolean }[]
}

/** A Modrinth project page as recorded (data/projects/*.json): the hit plus its page. */
export interface RecordedProject extends RecordedHit {
    body: string
    published?: string | null
    additional_categories?: string[]
    issues_url?: string | null
    source_url?: string | null
    wiki_url?: string | null
    discord_url?: string | null
    donation_urls?: { platform: string | null; url: string | null }[]
    gallery: {
        url: string
        featured: boolean
        title: string | null
        description: string | null
        created?: string | null
    }[]
    team: { username: string; role: string | null; avatar: string | null }[]
    versions: RecordedVersion[]
}

/** Discover types the demo recorded, by the backend's resource type. */
export const recordedTypes: Partial<Record<ResourceType, string>> = {
    modPack: 'modpack',
    mod: 'mod',
    resourcePack: 'resourcepack',
    shader: 'shader',
    dataPack: 'datapack',
}

const resourceTypes: Record<string, ResourceType> = {
    modpack: 'modPack',
    mod: 'mod',
    resourcepack: 'resourcePack',
    shader: 'shader',
    datapack: 'dataPack',
}

const searchFiles = import.meta.glob<{ total_hits: number; hits: RecordedHit[] }>(
    '../data/discover/*.json',
    { import: 'default' },
)
const projectFiles = import.meta.glob<RecordedProject>('../data/projects/*.json', {
    import: 'default',
})
const icons = import.meta.glob<string>('../assets/projects/*.webp', {
    eager: true,
    query: '?url',
    import: 'default',
})

export function projectIcon(projectId: string): string | undefined {
    return icons[`../assets/projects/${projectId}.webp`]
}

/** Recorded hits of one Discover type, in Modrinth's relevance order. */
export async function recordedHits(type: string): Promise<RecordedHit[]> {
    const file = searchFiles[`../data/discover/${type}.json`]
    return file ? (await file()).hits : []
}

let hitsByLocator: Promise<Map<string, RecordedHit>> | null = null

/** Every recorded hit by ID and by slug, since a project route may carry either. */
function allHits() {
    hitsByLocator ??= Promise.all(Object.values(searchFiles).map((file) => file())).then(
        (files) => {
            const map = new Map<string, RecordedHit>()
            for (const hit of files.flatMap((file) => file.hits)) {
                map.set(hit.id, hit)
                map.set(hit.slug, hit)
            }
            return map
        },
    )
    return hitsByLocator
}

const versionOwners = new Map<string, string>()

/** The recorded project an ID or slug names, or null when the demo has none. */
export async function recordedProject(locator: string): Promise<RecordedProject | null> {
    const id = (await allHits()).get(locator)?.id ?? locator
    const file = projectFiles[`../data/projects/${id}.json`]
    if (!file) return null
    const project = await file()
    for (const version of project.versions) versionOwners.set(version.id, project.id)
    return project
}

/** The recorded project that published a version, once that project has been read. */
export function versionOwner(versionId: string) {
    return versionOwners.get(versionId) ?? null
}

const environments: Record<string, string> = {
    'required/unsupported': 'client_only',
    'unsupported/required': 'server_only',
    'required/optional': 'client_only_server_optional',
    'optional/required': 'server_only_client_optional',
    'required/required': 'client_and_server',
    'optional/optional': 'client_or_server',
}

/** The environment tag Modrinth derives from a project's two side fields, when they name one. */
export function environmentTag(hit: RecordedHit): string | null {
    return environments[`${hit.client_side}/${hit.server_side}`] ?? null
}

/** What the backend's Modrinth provider reports for a project (providers/modrinth.rs). */
export function projectMetadata(hit: RecordedHit | RecordedProject): SourceProjectMetadata {
    const environment = environmentTag(hit)
    return {
        environments: environment ? [environment] : [],
        project: { source: 'modrinth', projectId: hit.id },
        slug: hit.slug,
        title: hit.title,
        summary: hit.description,
        iconUrl: projectIcon(hit.id) ?? null,
        types: [resourceTypes[hit.project_type] ?? 'mod'],
        authors: hit.author ? [hit.author] : [],
        license: hit.license?.id ?? null,
        downloads: hit.downloads,
        followers: hit.followers,
        createdAt: hit.date_created,
        projectPublishedAt: hit.date_created,
        updatedAt: hit.date_modified,
        firstPublishedAt: null,
        latestPublishedAt: hit.date_modified,
        categories: hit.display_categories ?? hit.categories,
        gameVersions: hit.game_versions,
        loaders: hit.loaders,
        gallery: 'gallery' in hit ? hit.gallery.map((image) => image.url) : [],
    }
}

/** The legacy (v2) project the app renders its project page from. */
function projectProtocol(project: RecordedProject): Labrinth.Projects.v2.Project {
    const created = project.published ?? project.date_created ?? new Date(0).toISOString()
    return {
        id: project.id,
        slug: project.slug,
        project_type: project.project_type,
        team: project.id,
        organization: null,
        title: project.title,
        description: project.description,
        body: project.body,
        published: created,
        updated: project.date_modified ?? created,
        approved: created,
        status: 'approved',
        license: project.license ?? { id: 'LicenseRef-Unknown', name: '' },
        client_side: project.client_side,
        server_side: project.server_side,
        downloads: project.downloads,
        followers: project.followers,
        categories: project.display_categories ?? project.categories,
        additional_categories: project.additional_categories ?? [],
        game_versions: project.game_versions,
        loaders: project.loaders,
        versions: project.versions.map((version) => version.id),
        icon_url: projectIcon(project.id),
        issues_url: project.issues_url ?? undefined,
        source_url: project.source_url ?? undefined,
        wiki_url: project.wiki_url ?? undefined,
        discord_url: project.discord_url ?? undefined,
        donation_urls: (project.donation_urls ?? []).flatMap((link, index) =>
            link.url
                ? [{ id: String(index), platform: link.platform ?? 'Other', url: link.url }]
                : [],
        ),
        gallery: project.gallery.map((image, ordering) => ({
            url: image.url,
            featured: image.featured,
            title: image.title ?? undefined,
            description: image.description ?? undefined,
            created: image.created ?? created,
            ordering,
        })),
        color: project.color ?? undefined,
        thread_id: '',
        monetization_status: 'monetized',
    }
}

/** A project page: what `resource_locate` answers for a recorded project. */
export function projectDocument(project: RecordedProject): ProjectDocument {
    return {
        metadata: projectMetadata(project),
        description: { source: 'modrinth', format: 'markdown', body: project.body },
        protocol: projectProtocol(project),
    }
}

/** The same page in the v3 protocol; the demo records no Modrinth server projects. */
export function projectDocumentV3(project: RecordedProject): ProjectDocument {
    const document = projectDocument(project)
    const link = (url: string | null | undefined) =>
        url ? { platform: null, donation: false, url } : undefined
    return {
        ...document,
        protocol: {
            ...(document.protocol as Labrinth.Projects.v2.Project),
            name: project.title,
            summary: project.description,
            project_types: [project.project_type],
            games: ['minecraft-java'],
            environment: document.metadata.environments,
            mrpack_loaders: project.project_type === 'modpack' ? project.loaders : undefined,
            link_urls: {
                issues: link(project.issues_url),
                source: link(project.source_url),
                wiki: link(project.wiki_url),
                discord: link(project.discord_url),
            },
            minecraft_server: null,
            minecraft_java_server: null,
        },
    }
}

function versionProtocol(
    projectId: string,
    version: RecordedVersion,
): Labrinth.Versions.v2.Version {
    return {
        id: version.id,
        project_id: projectId,
        author_id: '',
        featured: version.featured,
        name: version.name,
        version_number: version.version_number,
        changelog: version.changelog ?? '',
        changelog_url: null,
        date_published: version.date_published,
        downloads: version.downloads,
        version_type: version.version_type,
        status: 'listed',
        files: version.files.map((file) => ({
            hashes: { sha512: '', sha1: '' },
            url: '',
            filename: file.filename,
            primary: file.primary,
            size: file.size,
        })),
        dependencies: version.dependencies.map(
            ({ project_id, version_id, dependency_type, file_name }) =>
                project_id
                    ? {
                          project_id,
                          version_id: version_id ?? undefined,
                          dependency_type,
                          file_name: file_name ?? undefined,
                      }
                    : {
                          version_id: version_id ?? '',
                          dependency_type,
                          file_name: file_name ?? undefined,
                      },
        ),
        game_versions: version.game_versions,
        loaders: version.loaders,
    }
}

/** A version of a recorded project, in the backend's neutral release shape. */
export function release(projectId: string, version: RecordedVersion): ResourceRelease {
    const reference: SourceReleaseRef = {
        project: { source: 'modrinth', projectId },
        releaseId: version.id,
    }
    return {
        reference,
        version: version.version_number,
        channel: version.version_type,
        publishedAt: version.date_published,
        compatibility: { gameVersions: version.game_versions, loaders: version.loaders },
        artifacts: version.files.map((file) => ({
            reference: { release: reference, artifactId: file.filename },
            fileName: file.filename,
            role: file.primary ? 'primary' : 'supporting',
            available: true,
            integrity: { size: file.size, hashes: [] },
            acquisition: { kind: 'unavailable' },
            dependencies: [],
        })),
        equivalentReleases: [],
    }
}

export function releaseDocument(
    project: RecordedProject,
    version: RecordedVersion,
): ReleaseDocument {
    return {
        release: release(project.id, version),
        changelog: version.changelog
            ? { source: 'modrinth', format: 'markdown', body: version.changelog }
            : null,
        protocol: versionProtocol(project.id, version),
    }
}

/** The project's team, with the members the recording kept. */
export function teamMembers(project: RecordedProject) {
    return project.team.map((member, ordering) => ({
        team_id: project.id,
        user: {
            id: member.username,
            username: member.username,
            avatar_url: member.avatar ?? undefined,
            created: project.published ?? project.date_created ?? '',
            role: 'developer',
            badges: 0,
        },
        role: member.role ?? 'Member',
        is_owner: ordering === 0,
        permissions: null,
        organization_permissions: null,
        accepted: true,
        payouts_split: null,
        ordering,
    }))
}
