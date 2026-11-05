import type { Labrinth } from '@modrinth/api-client'

import type {
    ContentRefreshMode,
    ContentRefreshState,
    ContentRefreshTask,
} from '@/helpers/content-refresh'
import type {
    InstanceContentSnapshot,
    InstanceContentSnapshotItem,
    LinkedModpackInfo,
} from '@/helpers/instance'

import modpacks from '../data/modpacks.json'
import { projectDocument, projectIcon, recordedProject, releaseDocument } from './modrinth'
import type { Handlers } from './types'

/**
 * One installed file as recorded (data/instance-content.json): `[file_name, folder,
 * size, project_id, version_id, version_number, title, project_type]`; files in a
 * pack's overrides carry no project.
 */
type ContentRow = [
    string,
    string,
    number,
    string | null,
    string | null,
    string | null,
    string | null,
    string,
]

/** Which recording each demo instance shows: its modpack's files, or a hand-picked set. */
const recordings: Record<string, string> = {
    'fabulously-optimized': '1KVo5zza',
    'create-plus': 't1tOiUHZ',
    cobblemon: '5FFgwNNP',
    'better-mc': 'shFhR8Vx',
    'zombie-invade': 'l9m9tuPN',
    'fabric-26-3': 'custom-fabric',
}

let recorded: Promise<Record<string, ContentRow[]>> | null = null

function recordedContent(instanceId: string) {
    recorded ??= import('../data/instance-content.json').then(
        (module) => module.default.instances as unknown as Record<string, ContentRow[]>,
    )
    const key = recordings[instanceId]
    return key ? recorded.then((instances) => instances[key] ?? []) : Promise.resolve([])
}

function snapshotItem(instanceId: string, row: ContentRow, index: number, packManaged: boolean) {
    const [fileName, folder, size, projectId, versionId, versionNumber, title, projectType] = row
    const path = `${folder}/${fileName}`
    const id = `${instanceId}:${index}`
    const item: InstanceContentSnapshotItem = {
        fileId: `file:${id}`,
        entryId: `entry:${id}`,
        memberId: packManaged ? `member:${id}` : null,
        ownershipKind: packManaged ? 'pack_managed' : 'user_added',
        materializationState: 'present',
        overrideKind: 'none',
        expectedRelativePath: path,
        required: packManaged,
        projectType,
        provider: projectId ? 'modrinth' : null,
        providerProjectId: projectId,
        providerReleaseId: versionId,
        content: {
            id: `entry:${id}`,
            file_name: fileName,
            file_path: path,
            size,
            enabled: true,
            project_type: projectType,
            project: {
                id: projectId ?? `local:${path}`,
                slug: projectId ?? path,
                title: title ?? fileName,
                icon_url: projectId ? projectIcon(projectId) : undefined,
            },
            version:
                versionId && versionNumber
                    ? { id: versionId, version_number: versionNumber, file_name: fileName }
                    : undefined,
            update: null,
            origin_provider: projectId ? 'modrinth' : null,
            installedSource: projectId ? 'modrinth' : null,
            external: !projectId,
            provider_refs: projectId
                ? [{ provider: 'modrinth', project_id: projectId, version_id: versionId }]
                : [],
        },
        capabilities: {
            canToggle: true,
            canDelete: !packManaged,
            canUpdate: false,
            canChangeVersion: !packManaged && !!projectId,
            canRestorePackDefault: false,
        },
        dependency: null,
    }
    return item
}

/** The modpack an instance was installed from, at its pinned version (data/modpacks.json). */
async function linkedModpack(instanceId: string): Promise<LinkedModpackInfo | null> {
    const projectId = recordings[instanceId]
    const pack = modpacks.packs[projectId as keyof typeof modpacks.packs]
    const project = pack ? await recordedProject(projectId) : null
    const version = project?.versions.find((entry) => entry.id === pack.version_id)
    if (!project || !version) return null
    return {
        project: projectDocument(project).protocol as Labrinth.Projects.v2.Project,
        version: releaseDocument(project, version).protocol as Labrinth.Versions.v2.Version,
        owner: null,
        update: null,
        update_version: null,
    }
}

async function snapshot(instanceId: string): Promise<InstanceContentSnapshot> {
    const metadata = await linkedModpack(instanceId)
    const rows = await recordedContent(instanceId)
    return {
        instanceId,
        revision: 1,
        pack: metadata && {
            name: metadata.project.title,
            iconPath: metadata.project.icon_url ?? null,
            provider: 'modrinth',
            projectId: metadata.project.id,
            versionId: metadata.version.id,
            reconciled: true,
            canUpdate: false,
            metadata,
        },
        items: rows.map((row, index) => snapshotItem(instanceId, row, index, !!metadata)),
        pendingManualDownloads: [],
        warnings: [],
    }
}

export const contentHandlers: Handlers = {
    'plugin:instance|instance_get_content_snapshot': ({ instanceId }) =>
        snapshot(String(instanceId)),
    'plugin:axolotl-resources|resource_get_content_refresh_state': async ({
        instanceId,
    }): Promise<ContentRefreshState> => ({
        snapshot: await snapshot(String(instanceId)),
        task: null,
        sequence: 0,
    }),
    // The recorded content is already current, so a refresh finishes as soon as it starts.
    'plugin:axolotl-resources|resource_start_content_refresh': async ({
        instanceId,
        mode,
    }): Promise<ContentRefreshTask> => {
        const files = (await recordedContent(String(instanceId))).length
        return {
            id: `refresh:${String(instanceId)}`,
            mode: mode as ContentRefreshMode,
            running: false,
            scanned: files,
            checked: files,
            failed: 0,
            unresolved: 0,
            errors: [],
        }
    },
    'plugin:instance|instance_get_linked_modpack_info': ({ instanceId }) =>
        linkedModpack(String(instanceId)),
    'plugin:instance|instance_get_post_upgrade_notice': () => null,
    'plugin:process|process_get_by_instance_id': () => [],
    'plugin:datapacks|list_datapacks': () => [],
}
