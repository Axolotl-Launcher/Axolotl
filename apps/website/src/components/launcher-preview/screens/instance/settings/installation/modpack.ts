import type { ProjectDetail, ProjectVersion } from '../../../../data'
import type { PreviewState } from '../../../../lib/state'
import type { ContentItem } from '../../../../lib/state/downloads'
import type { GameInstance, InstanceLoader, ReleaseChannel } from '../../../../lib/types'
import type { UpdaterVersion } from '../../../../shared/content-tab/components/modals/ContentUpdaterModal.vue'

/*
 * The linked-modpack services the Installation tab calls
 * (`get_linked_modpack_info`, `get_project_versions`, `update_repair_modrinth`,
 * `update_managed_modrinth_version`), answered from the project snapshots in
 * `LP/data/project` and run as install stages on the window's clock.
 */

export interface LinkedModpackInfo {
    project: { id: string; slug: string | null; title: string; icon_url: string }
    version: { id: string; version_number: string } | null
    update: { provider: 'modrinth'; current_version_id: string; target_version_id: string } | null
}

export function projectIconUrl(projectId: string) {
    return `/launcher-preview/projects/${projectId}.webp`
}

export function loadModpackProject(projectId: string): Promise<ProjectDetail> {
    return import('../../../../data').then((data) => data.loadProject(projectId))
}

const CHANNEL_RANK: Record<ReleaseChannel, number> = { release: 0, beta: 1, alpha: 2 }

function versionTypeFallbacks(channel: ReleaseChannel): string[][] {
    switch (channel) {
        case 'release':
            return [['release'], ['beta'], ['alpha']]
        case 'beta':
            return [['release', 'beta'], ['alpha']]
        case 'alpha':
            return [['release', 'beta', 'alpha']]
    }
}

/** `check_modpack_update` of `state/instances/commands/list_content.rs`. */
export function checkModpackUpdate(
    installed: ProjectVersion,
    versions: ProjectVersion[],
    preferredChannel: ReleaseChannel,
): ProjectVersion | null {
    const installedChannel = installed.version_type as ReleaseChannel
    const effectiveChannel =
        CHANNEL_RANK[preferredChannel] >= CHANNEL_RANK[installedChannel]
            ? preferredChannel
            : installedChannel
    for (const versionTypes of versionTypeFallbacks(effectiveChannel)) {
        if (!versions.some((version) => versionTypes.includes(version.version_type))) continue
        const newer = versions
            .filter(
                (version) =>
                    version.id !== installed.id &&
                    Date.parse(version.date_published) > Date.parse(installed.date_published) &&
                    versionTypes.includes(version.version_type),
            )
            .sort((a, b) => Date.parse(b.date_published) - Date.parse(a.date_published))
        return newer[0] ?? null
    }
    return null
}

export async function getLinkedModpackInfo(
    instance: GameInstance,
): Promise<LinkedModpackInfo | null> {
    if (instance.link?.type !== 'modrinth_modpack') return null
    const projectId = instance.link.project_id
    const versionId = instance.link.version_id
    const project = await loadModpackProject(projectId)
    const version = project.versions.find((entry) => entry.id === versionId) ?? null
    const update = version
        ? checkModpackUpdate(version, project.versions, instance.update_channel)
        : null
    return {
        project: {
            id: project.id,
            slug: project.slug,
            title: project.title,
            icon_url: projectIconUrl(project.id),
        },
        version: version ? { id: version.id, version_number: version.version_number } : null,
        update: update
            ? {
                  provider: 'modrinth',
                  current_version_id: versionId,
                  target_version_id: update.id,
              }
            : null,
    }
}

export function toUpdaterVersion(version: ProjectVersion, projectId: string): UpdaterVersion {
    return {
        id: version.id,
        project_id: projectId,
        name: version.name,
        version_number: version.version_number,
        version_type: version.version_type,
        game_versions: version.game_versions,
        loaders: version.loaders,
        date_published: version.date_published,
        changelog: version.changelog ?? null,
    }
}

const KNOWN_LOADERS: InstanceLoader[] = ['fabric', 'forge', 'quilt', 'neoforge']

/**
 * A modpack install job on an existing instance: the pack files download
 * (`pack_installing`), then the game and loader install (`minecraft_installing`).
 */
export function runPackInstall(
    preview: PreviewState,
    instanceId: string,
    apply: (instance: GameInstance) => void,
) {
    const target = preview.instances.get(instanceId)
    if (!target) return
    target.install_stage = 'pack_installing'
    let elapsed = 0
    let applied = false
    const stop = preview.clock.every((delta) => {
        elapsed += delta
        const current = preview.instances.get(instanceId)
        if (!current) {
            stop()
            return
        }
        if (!applied && elapsed >= 2600) {
            applied = true
            apply(current)
            current.install_stage = 'minecraft_installing'
        }
        if (elapsed >= 3900) {
            stop()
            current.install_stage = 'installed'
            current.modified = new Date().toISOString()
        }
    })
}

/** The instance after `update_managed_modrinth_version` installed `version`. */
export function applyPackVersion(instance: GameInstance, version: UpdaterVersion) {
    if (instance.link?.type !== 'modrinth_modpack') return
    instance.link = { ...instance.link, version_id: version.id }
    if (version.game_versions[0]) instance.game_version = version.game_versions[0]
    const loader = version.loaders.find((entry): entry is InstanceLoader =>
        KNOWN_LOADERS.includes(entry as InstanceLoader),
    )
    if (loader && loader !== instance.loader) {
        instance.loader = loader
        instance.loader_components = instance.loader_version
            ? [
                  {
                      instanceId: instance.id,
                      kind: loader,
                      version: instance.loader_version,
                      role: 'primary',
                  },
              ]
            : []
    }
}

/**
 * The pack's own files (`modrinth.index.json` and its overrides) as the content
 * list holds them, for a reinstall that resets the instance to the pack.
 */
export async function packContent(projectId: string): Promise<ContentItem[] | null> {
    const data = await import('../../../../data')
    const content = await data.loadInstanceContent()
    const rows = content.instances[projectId]
    if (!rows) return null
    const withoutIcon = new Set(content.withoutIcon)
    const now = new Date().toISOString()
    return rows.map(
        ([fileName, folder, size, rowProjectId, versionId, versionNumber, title, projectType]) => ({
            id: `${folder}/${fileName}`,
            project_id: rowProjectId,
            version_id: versionId,
            name: title ?? fileName,
            file_name: fileName,
            enabled: true,
            icon:
                rowProjectId && !withoutIcon.has(rowProjectId)
                    ? projectIconUrl(rowProjectId)
                    : null,
            version_number: versionNumber,
            project_type: projectType,
            size,
            installed_at: now,
        }),
    )
}
