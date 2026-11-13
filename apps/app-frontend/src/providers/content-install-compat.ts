import type { Labrinth } from '@modrinth/api-client'

import {
    type CurseForgeFile,
    type CurseForgeProject,
    getCurseForgeImageUrl,
} from '@/helpers/curseforge'
import type { GameInstance } from '@/helpers/types'

export const LOADER_ORDER = ['vanilla', 'fabric', 'quilt', 'neoforge', 'forge']
export const SUPPORTED_LOADERS: Set<string> = new Set([
    'vanilla',
    'forge',
    'fabric',
    'quilt',
    'neoforge',
])
export const VANILLA_COMPATIBLE_LOADERS: Set<string> = new Set(['minecraft', 'datapack'])

const RESOLVABLE_PROJECT_TYPES = new Set<string>([
    'mod',
    'plugin',
    'datapack',
    'resourcepack',
    'shader',
    'modpack',
])

function isResolvableContentType(value: string): value is Labrinth.Content.v3.ContentType {
    return RESOLVABLE_PROJECT_TYPES.has(value)
}

export type InstallTargetInstance = Pick<
    GameInstance,
    'id' | 'name' | 'game_version' | 'loader'
> & {
    icon_path?: string | null
}

export function resolveContentType(
    projectType?: Labrinth.Projects.v2.ProjectType,
): Labrinth.Content.v3.ContentType {
    return projectType && isResolvableContentType(projectType) ? projectType : 'mod'
}

/** Only the version/loader pair is needed to judge compatibility. */
export type VersionTarget = Pick<GameInstance, 'game_version' | 'loader'>

export function isVersionCompatible(
    version: Labrinth.Versions.v2.Version,
    project: Labrinth.Projects.v2.Project,
    instance: VersionTarget,
) {
    if (project.project_type === 'resourcepack') return true

    return (
        version.game_versions.includes(instance.game_version) &&
        (project.project_type === 'mod'
            ? version.loaders.includes(instance.loader) || version.loaders.includes('datapack')
            : true)
    )
}

export function findPreferredVersion(
    versions: Labrinth.Versions.v2.Version[],
    project: Labrinth.Projects.v2.Project,
    instance: VersionTarget,
) {
    const projectType = project.project_type ?? 'mod'
    if (projectType === 'resourcepack') return versions[0]

    return (
        versions.find(
            (v) =>
                v.game_versions.includes(instance.game_version) &&
                (projectType === 'mod' ? v.loaders.includes(instance.loader) : true),
        ) ?? versions.find((v) => isVersionCompatible(v, project, instance))
    )
}

export function sortLoaders(loaders: string[]): string[] {
    return loaders.slice().sort((a, b) => {
        const aIdx = LOADER_ORDER.indexOf(a)
        const bIdx = LOADER_ORDER.indexOf(b)
        if (aIdx === -1 && bIdx === -1) return a.localeCompare(b)
        if (aIdx === -1) return 1
        if (bIdx === -1) return -1
        return aIdx - bIdx
    })
}

export function curseForgeProjectType(classId?: number): Labrinth.Projects.v2.ProjectType {
    switch (classId) {
        case 5:
            return 'plugin'
        case 12:
            return 'resourcepack'
        case 6945:
            return 'datapack'
        case 4471:
            return 'modpack'
        case 6552:
            return 'shader'
        default:
            return 'mod'
    }
}

export function curseForgeLoader(value: string): string | null {
    switch (value.toLowerCase().replaceAll(' ', '')) {
        case 'forge':
            return 'forge'
        case 'fabric':
        case 'fabricloader':
            return 'fabric'
        case 'quilt':
            return 'quilt'
        case 'neoforge':
            return 'neoforge'
        default:
            return null
    }
}

export function curseForgeGameVersions(file: CurseForgeFile): string[] {
    return file.gameVersions.filter(
        (value) =>
            !curseForgeLoader(value) &&
            (/^(?:\d+\.\d+(?:\.\d+)?(?:-(?:pre|rc)\d+)?|\d{2}w\d{2}[a-z])$/i.test(value) ||
                value.toLowerCase().includes('snapshot')),
    )
}

export function mapCurseForgeVersion(
    file: CurseForgeFile,
    projectId: number,
    projectType: Labrinth.Projects.v2.ProjectType,
): Labrinth.Versions.v2.Version {
    const loaders = [
        ...new Set(file.gameVersions.map(curseForgeLoader).filter(Boolean)),
    ] as string[]
    return {
        id: file.id.toString(),
        project_id: `curseforge:${projectId}`,
        name: file.displayName,
        version_number: file.displayName,
        game_versions: curseForgeGameVersions(file),
        loaders:
            loaders.length > 0 && (projectType === 'mod' || projectType === 'modpack')
                ? loaders
                : ['minecraft'],
        date_published: file.fileDate,
        version_type:
            file.releaseType === 1 ? 'release' : file.releaseType === 2 ? 'beta' : 'alpha',
        files: [
            {
                filename: file.fileName,
                url: file.downloadUrl ?? '',
                primary: true,
                size: file.fileLength,
                hashes: {},
            },
        ],
    } as unknown as Labrinth.Versions.v2.Version
}

export function mapCurseForgeProject(
    project: CurseForgeProject,
    files: CurseForgeFile[],
): Labrinth.Projects.v2.Project {
    const projectType = curseForgeProjectType(project.classId)
    const versions = files.map((file) => mapCurseForgeVersion(file, project.id, projectType))
    return {
        id: `curseforge:${project.id}`,
        slug: project.slug,
        title: project.name,
        description: project.summary,
        project_type: projectType,
        icon_url: getCurseForgeImageUrl(project.logo?.thumbnailUrl ?? project.logo?.url) ?? null,
        versions: versions.map((version) => version.id),
        game_versions: [...new Set(versions.flatMap((version) => version.game_versions))],
        loaders: [...new Set(versions.flatMap((version) => version.loaders))],
        organization: null,
        team: '',
    } as unknown as Labrinth.Projects.v2.Project
}

export function curseForgeLoaderType(loader: string): number | undefined {
    switch (loader) {
        case 'forge':
            return 1
        case 'fabric':
            return 4
        case 'quilt':
            return 5
        case 'neoforge':
            return 6
        default:
            return undefined
    }
}

export function projectPageUrl(project: { project_type: string; slug: string }): string {
    return `https://modrinth.com/${project.project_type}/${project.slug}`
}

export function curseForgePageUrl(project: {
    slug: string
    links?: { websiteUrl?: string }
}): string {
    if (project.links?.websiteUrl) return project.links.websiteUrl
    return `https://www.curseforge.com/minecraft/mc-mods/${project.slug}`
}

export function getInstallTargets(versions: Labrinth.Versions.v2.Version[]) {
    const targets: { game_version: string; loader: string }[] = []
    const seen = new Set<string>()

    for (const version of versions) {
        for (const gameVersion of version.game_versions) {
            for (const loader of version.loaders) {
                const key = `${gameVersion}\0${loader}`
                if (seen.has(key)) continue
                seen.add(key)
                targets.push({ game_version: gameVersion, loader })
            }
        }
    }

    return targets
}
