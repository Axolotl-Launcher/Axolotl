import type { InstanceLoader } from '../lib/types'

export interface ProjectTagData {
    gameVersions: { version: string; version_type: string; date: string }[]
    loaders: { name: string; supported_project_types: string[]; icon?: string }[]
    categories: { name: string; project_type: string; header: string; icon?: string }[]
    licenses: { id: string; name: string }[]
}

export interface ProjectCard {
    id: string
    slug: string | null
    title: string
    description: string
    categories: string[]
    client_side: string
    server_side: string
    project_type: string
    downloads: number
    followers: number
    license: { id: string; name: string } | string | null
    loaders: string[]
    game_versions: string[]
    updated: string | null
    date_modified: string | null
    author?: string | null
    display_categories?: string[]
    latest_version?: string | null
    date_created?: string | null
    color?: number | null
}

export interface ProjectVersion {
    id: string
    name: string
    version_number: string
    version_type: 'release' | 'beta' | 'alpha'
    game_versions: string[]
    loaders: string[]
    date_published: string
    downloads: number
    featured?: boolean
    changelog?: string
    dependencies?: {
        project_id: string | null
        version_id: string | null
        dependency_type: 'required' | 'optional' | 'incompatible' | 'embedded'
        file_name: string | null
    }[]
    files: { filename: string; size: number; primary: boolean }[]
}

export interface ProjectDetail extends ProjectCard {
    /** Description markdown; images are local copies or removed (alt text kept). */
    body?: string
    published?: string | null
    additional_categories?: string[]
    issues_url?: string | null
    source_url?: string | null
    wiki_url?: string | null
    discord_url?: string | null
    donation_urls?: { platform: string | null; url: string | null }[]
    team?: { username: string; role: string | null; avatar: string | null }[]
    /** Number of versions the project has published (the snapshot keeps the latest ones). */
    versions_total?: number
    gallery: {
        url: string
        featured: boolean
        title: string | null
        description: string | null
        created?: string | null
    }[]
    versions: ProjectVersion[]
}

export type DiscoverType = 'mod' | 'modpack' | 'resourcepack' | 'shader' | 'datapack'

export interface DiscoverData {
    total_hits: number
    hits: ProjectCard[]
}

export interface MinecraftVersions {
    latest: { release: string; snapshot: string }
    releases: { id: string; releaseTime: string }[]
    snapshots: { id: string; releaseTime: string }[]
    /** Old betas and alphas. */
    old?: { id: string; releaseTime: string; type: 'old_beta' | 'old_alpha' }[]
}

/**
 * Daedalus loader manifests per game version: the newest 60 builds and the
 * stable ones among them. Quilt's manifest carries no stable flags, so its
 * stable builds follow the app's `is_stable_version` rule (no alpha, beta, rc,
 * snapshot or pre marker).
 */
export type LoaderManifests = Record<
    'fabric' | 'quilt' | 'forge' | 'neo',
    Record<string, { stable: boolean; versions: string[]; stableVersions?: string[] }>
>

const discoverLoaders: Record<DiscoverType, () => Promise<DiscoverData>> = {
    mod: () => import('./discover-mod.json').then((module) => module.default as DiscoverData),
    modpack: () =>
        import('./discover-modpack.json').then((module) => module.default as DiscoverData),
    resourcepack: () =>
        import('./discover-resourcepack.json').then((module) => module.default as DiscoverData),
    shader: () => import('./discover-shader.json').then((module) => module.default as DiscoverData),
    datapack: () =>
        import('./discover-datapack.json').then((module) => module.default as DiscoverData),
}

const projectLoaders = import.meta.glob<{ default: ProjectDetail }>('./project/*.json')

const cache = new Map<string, unknown>()

export function loadDiscover(type: DiscoverType): Promise<DiscoverData> {
    const key = `discover:${type}`
    const hit = cache.get(key) as Promise<DiscoverData> | undefined
    if (hit) return hit
    const promise = discoverLoaders[type]()
    cache.set(key, promise)
    return promise
}

export function loadProject(id: string): Promise<ProjectDetail> {
    const key = `project:${id}`
    const hit = cache.get(key) as Promise<ProjectDetail> | undefined
    if (hit) return hit
    const loader = projectLoaders[`./project/${id}.json`]
    if (!loader) return Promise.reject(new Error(`Unknown project ${id}`))
    const promise = loader().then((module) => module.default)
    cache.set(key, promise)
    return promise
}

export function loadTags(): Promise<ProjectTagData> {
    return import('./tags.json').then((module) => module.default as unknown as ProjectTagData)
}

export function loadMinecraft(): Promise<MinecraftVersions> {
    return import('./minecraft.json').then(
        (module) => module.default as unknown as MinecraftVersions,
    )
}

export function loadLoaders(): Promise<LoaderManifests> {
    return import('./loaders.json').then((module) => module.default as unknown as LoaderManifests)
}

/**
 * Files of the instance fixtures (`data/instance-content.json`), one row per
 * file: `[file_name, folder, size, project_id, version_id, version_number,
 * title, project_type]`. A modpack's rows are its pack version's
 * `modrinth.index.json` files resolved through Labrinth's `version_files`, plus
 * the jars and packs in its overrides (not resolved: `null` project fields).
 */
export type InstanceContentRow = [
    string,
    string,
    number,
    string | null,
    string | null,
    string | null,
    string | null,
    string,
]

export interface InstanceContentData {
    fetchedAt: string
    instances: Record<string, InstanceContentRow[]>
    /** Projects without an icon on Modrinth. */
    withoutIcon: string[]
}

export function loadInstanceContent(): Promise<InstanceContentData> {
    return import('./instance-content.json').then(
        (module) => module.default as unknown as InstanceContentData,
    )
}

export function projectIcon(id: string): string {
    return `/launcher-preview/projects/${id}.webp`
}

export const loaderSlugs: Partial<Record<InstanceLoader, keyof LoaderManifests>> = {
    fabric: 'fabric',
    quilt: 'quilt',
    forge: 'forge',
    neoforge: 'neo',
}
