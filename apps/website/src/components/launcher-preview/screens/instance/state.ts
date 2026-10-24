import type {
    ContentItem as LayoutContentItem,
    ContentOwner,
} from '@modrinth/ui/src/layouts/shared/content-tab/types.ts'
import { computed, reactive, ref, watch } from 'vue'

import type { ProjectDetail, ProjectVersion } from '../../data'
import { loadProject } from '../../data'
import type { PreviewState } from '../../lib/state'
import { previewDomain, usePreview, usePreviewDomain } from '../../lib/state'
import { contentFolderFor, type ContentItem as SharedContentItem } from '../../lib/state/downloads'
import type { GameInstance } from '../../lib/types'
import type { HomeWorld } from '../home/home-utils'
import { useHomeState } from '../home/state'
import { contentCompatibility } from './content/compatibility'
import { compatibleVersions, type ContentProjectType } from './content/content-types'

/*
 * State of the instance page shared by its tabs.
 *
 * Content: the instance's installed files live in the preview's shared download
 * state (`preview.downloads.contents`): a modpack instance holds its pack
 * version's real files (`data/instance-content.json`, from the version's
 * `modrinth.index.json`), the Fabric instance the mods its user installed, and
 * a project installed from Browse joins them. Toggling or removing a file here
 * is seen by Browse's "installed" badges and the game's startup log too. This
 * domain keeps the per-item metadata the content list needs next to it: the
 * project snapshot's owner, releases and dependencies, and the `.old` file an
 * update leaves for a rollback.
 *
 * Worlds: the worlds of an instance are the Home page's worlds (the app reads
 * both from the same backend list), so pinning, playing or deleting a world is
 * reflected on Home as well.
 */

/** Folder of the instance a content type is installed into. */
export const contentFolder = contentFolderFor

/** A release of an installed project, as the update and version pickers show it. */
export interface ContentRelease {
    id: string
    versionNumber: string
    name: string
    versionType: 'release' | 'beta' | 'alpha'
    fileName: string
    size: number
    datePublished: string
    gameVersions: string[]
    loaders: string[]
    changelog: string
}

export interface ContentMeta {
    projectId: string | null
    slug: string | null
    title: string
    description: string
    owner: ContentOwner | null
    downloads: number
    /** Releases the instance can run, newest first; empty without a project snapshot. */
    releases: ContentRelease[]
    /** Newest compatible release, when it is newer than the installed one. */
    update: ContentRelease | null
    /** Project ids this project requires (from the installed release's dependencies). */
    requires: string[]
    gameVersions: string[]
    loaders: string[]
    metadataUnavailable: boolean
}

/** The file an update kept as `<file>.old`, which a rollback restores. */
export interface ContentRollback {
    fileName: string
    versionId: string | null
    versionNumber: string | null
    size: number
}

/** Release types an update may move to on each update channel. */
const CHANNEL_TYPES: Record<string, ContentRelease['versionType'][]> = {
    release: ['release'],
    beta: ['release', 'beta'],
    alpha: ['release', 'beta', 'alpha'],
}

function toRelease(version: ProjectVersion): ContentRelease {
    const file = version.files.find((entry) => entry.primary) ?? version.files[0]
    return {
        id: version.id,
        versionNumber: version.version_number,
        name: version.name,
        versionType: version.version_type,
        fileName: file?.filename ?? `${version.version_number}.jar`,
        size: file?.size ?? 0,
        datePublished: version.date_published,
        gameVersions: version.game_versions,
        loaders: version.loaders,
        changelog: version.changelog ?? '',
    }
}

export function projectOwner(project: ProjectDetail): ContentOwner | null {
    const lead =
        project.team?.find((member) => member.role === 'Owner') ??
        project.team?.find((member) => member.username === project.author) ??
        project.team?.[0]
    const name = project.author || lead?.username
    if (!name) return null
    return {
        id: name,
        name,
        type: 'user',
        avatar_url: lead?.avatar ?? undefined,
    }
}

interface InstanceContentEntry {
    loading: boolean
    loaded: boolean
    error: string | null
}

function createContentDomain(state: PreviewState) {
    const entries = reactive<Record<string, InstanceContentEntry>>({})
    /** Metadata keyed by `${instanceId}:${itemId}`. */
    const meta = reactive<Record<string, ContentMeta>>({})
    /** `.old` files keyed by `${instanceId}:${itemId}` of the file that replaced them. */
    const rollbacks = reactive<Record<string, ContentRollback>>({})
    /** Update jobs queued from the content page: job id -> the file being replaced. */
    const pendingUpdates = new Map<string, { instanceId: string; previous: SharedContentItem }>()
    const projects = new Map<string, Promise<ProjectDetail | null>>()
    const pending = new Map<string, Promise<void>>()

    function entryFor(instanceId: string): InstanceContentEntry {
        if (!entries[instanceId]) {
            entries[instanceId] = { loading: false, loaded: false, error: null }
        }
        return entries[instanceId]
    }

    function sharedItems(instanceId: string): SharedContentItem[] {
        return state.downloads.contents.value[instanceId] ?? []
    }

    function writeShared(instanceId: string, items: SharedContentItem[]) {
        state.downloads.contents.value = { ...state.downloads.contents.value, [instanceId]: items }
    }

    /** The project snapshot, when the preview carries one (`data/project`). */
    function project(id: string): Promise<ProjectDetail | null> {
        let request = projects.get(id)
        if (!request) {
            request = loadProject(id).catch(() => {
                projects.delete(id)
                return null
            })
            projects.set(id, request)
        }
        return request
    }

    function buildMeta(
        instance: GameInstance,
        item: SharedContentItem,
        detail: ProjectDetail | null,
    ): ContentMeta {
        if (!detail) {
            return {
                projectId: item.project_id,
                slug: null,
                title: item.name,
                description: '',
                owner: null,
                downloads: 0,
                releases: [],
                update: null,
                requires: [],
                gameVersions: [],
                loaders: [],
                metadataUnavailable: !!item.project_id,
            }
        }
        const releases = compatibleVersions(detail, instance).map(toRelease)
        const installed = releases.find(
            (release) => release.id === item.version_id || release.fileName === item.file_name,
        )
        // The update check follows the instance's update channel (Settings > General).
        const allowed = CHANNEL_TYPES[instance.update_channel] ?? CHANNEL_TYPES.release
        const newest = releases.find((release) => allowed.includes(release.versionType)) ?? null
        const version = detail.versions.find(
            (entry) =>
                entry.id === item.version_id ||
                entry.files.some((file) => file.filename === item.file_name),
        )
        return {
            projectId: detail.id,
            slug: detail.slug,
            title: detail.title,
            description: detail.description,
            owner: projectOwner(detail),
            downloads: detail.downloads,
            releases,
            update:
                newest &&
                installed &&
                newest.id !== installed.id &&
                Date.parse(newest.datePublished) > Date.parse(installed.datePublished)
                    ? newest
                    : null,
            requires: (version?.dependencies ?? [])
                .filter((dependency) => dependency.dependency_type === 'required')
                .map((dependency) => dependency.project_id)
                .filter((id): id is string => !!id),
            gameVersions: version?.game_versions ?? [],
            loaders: version?.loaders ?? [],
            metadataUnavailable: false,
        }
    }

    async function resolveMeta(instanceId: string) {
        const instance = state.instances.get(instanceId)
        if (!instance) return
        for (const item of sharedItems(instanceId)) {
            const key = `${instanceId}:${item.id}`
            if (meta[key]) continue
            const detail = item.project_id ? await project(item.project_id) : null
            meta[key] = buildMeta(instance, item, detail)
        }
    }

    /** `get_content_items`: reads the instance's files, then their project metadata. */
    async function load(instanceId: string) {
        const entry = entryFor(instanceId)
        entry.loading = true
        try {
            await state.downloads.ensureContents()
            await resolveMeta(instanceId)
            entry.loaded = true
            entry.error = null
        } catch (error) {
            entry.error = error instanceof Error ? error.message : String(error)
        } finally {
            entry.loading = false
        }
    }

    /** A `bypass` refresh: the update check and metadata run again, then replace the old. */
    async function reload(instanceId: string) {
        await ensureLoaded(instanceId)
        const instance = state.instances.get(instanceId)
        if (!instance) return
        const entry = entryFor(instanceId)
        try {
            const fresh: Record<string, ContentMeta> = {}
            for (const item of sharedItems(instanceId)) {
                const detail = item.project_id ? await project(item.project_id) : null
                const key = `${instanceId}:${item.id}`
                fresh[key] =
                    !detail && meta[key] && !meta[key].metadataUnavailable
                        ? { ...meta[key], metadataUnavailable: true }
                        : buildMeta(instance, item, detail)
            }
            Object.assign(meta, fresh)
            entry.error = null
        } catch (error) {
            entry.error = error instanceof Error ? error.message : String(error)
        }
    }

    function ensureLoaded(instanceId: string): Promise<void> {
        const entry = entryFor(instanceId)
        if (entry.loaded) return resolveMeta(instanceId)
        let promise = pending.get(instanceId)
        if (!promise) {
            promise = load(instanceId).finally(() => pending.delete(instanceId))
            pending.set(instanceId, promise)
        }
        return promise
    }

    // Files installed or replaced elsewhere get their metadata too.
    watch(
        () => state.downloads.contents.value,
        (contents) => {
            for (const instanceId of Object.keys(contents)) {
                if (entries[instanceId]?.loaded) void resolveMeta(instanceId)
            }
        },
    )

    // An update job that finished left the replaced file as `<file>.old`.
    watch(
        () => state.downloads.history.value,
        (history) => {
            for (const job of history) {
                const expected = pendingUpdates.get(job.id)
                if (!expected) continue
                pendingUpdates.delete(job.id)
                if (job.phase !== 'completed') continue
                const previous = expected.previous
                if (previous.version_id === job.version_id) continue
                rollbacks[`${expected.instanceId}:${job.project_id}-${job.version_id}`] = {
                    fileName: previous.file_name,
                    versionId: previous.version_id,
                    versionNumber: previous.version_number,
                    size: previous.size,
                }
            }
        },
    )

    function metaFor(instanceId: string, itemId: string): ContentMeta | null {
        return meta[`${instanceId}:${itemId}`] ?? null
    }

    function rollbackFor(instanceId: string, itemId: string): ContentRollback | null {
        return rollbacks[`${instanceId}:${itemId}`] ?? null
    }

    /** Records the file an update job replaces, so a rollback can restore it. */
    function expectUpdate(jobId: string, instanceId: string, previous: SharedContentItem) {
        pendingUpdates.set(jobId, { instanceId, previous: { ...previous } })
    }

    function patchShared(
        instanceId: string,
        ids: string[],
        patch: (item: SharedContentItem) => SharedContentItem,
    ) {
        writeShared(
            instanceId,
            sharedItems(instanceId).map((item) => (ids.includes(item.id) ? patch(item) : item)),
        )
    }

    function setEnabled(instanceId: string, ids: string[], enabled: boolean) {
        patchShared(instanceId, ids, (item) => ({ ...item, enabled }))
    }

    /**
     * `remove_content_entry`: the file goes; a pack member stays listed as removed
     * (`override_kind = 'removed'`) so the pack does not bring it back.
     */
    function remove(instanceId: string, ids: string[]) {
        writeShared(
            instanceId,
            sharedItems(instanceId).flatMap((item) => {
                if (!ids.includes(item.id)) return [item]
                return item.pack_member ? [{ ...item, removed: true }] : []
            }),
        )
        for (const id of ids) {
            Reflect.deleteProperty(meta, `${instanceId}:${id}`)
            Reflect.deleteProperty(rollbacks, `${instanceId}:${id}`)
        }
    }

    /** `restore_pack_member_default`: the modpack's own release of the file, enabled. */
    function restorePackDefault(instanceId: string, itemId: string) {
        patchShared(instanceId, [itemId], (item) =>
            item.pack_default
                ? { ...item, ...item.pack_default, enabled: true, removed: false }
                : item,
        )
        Reflect.deleteProperty(rollbacks, `${instanceId}:${itemId}`)
    }

    /** `rollback_project`: swaps the `.old` file back in place of the update. */
    function rollback(instanceId: string, itemId: string) {
        const key = `${instanceId}:${itemId}`
        const previous = rollbacks[key]
        const instance = state.instances.get(instanceId)
        if (!previous || !instance) return
        patchShared(instanceId, [itemId], (item) => ({
            ...item,
            version_id: previous.versionId,
            version_number: previous.versionNumber,
            file_name: previous.fileName,
            size: previous.size,
        }))
        Reflect.deleteProperty(rollbacks, key)
        Reflect.deleteProperty(meta, key)
        void resolveMeta(instanceId)
    }

    /** `edit(id, { link: null })`: the pack's files stay, as content the user manages. */
    function unlinkPack(instanceId: string) {
        writeShared(
            instanceId,
            sharedItems(instanceId).map((item) =>
                item.pack_member ? { ...item, pack_member: false } : item,
            ),
        )
    }

    /** `resource_scan_backups`: the `.old` files the instance's updates left behind. */
    function backupFiles(instanceId: string) {
        return sharedItems(instanceId).flatMap((item) => {
            const previous = rollbacks[`${instanceId}:${item.id}`]
            if (!previous) return []
            const folder = item.folder ?? contentFolder(item.project_type)
            return [
                {
                    id: item.id,
                    historyId: item.id,
                    relativePath: `${folder}/${previous.fileName}.old`,
                    originalFileName: previous.fileName,
                    integrity: { size: previous.size },
                    rollbackAvailable: true,
                    recognized: true,
                    exists: true,
                },
            ]
        })
    }

    /** Deletes the `.old` file kept for `itemId`; false when it is already gone. */
    function discardBackup(instanceId: string, itemId: string) {
        const key = `${instanceId}:${itemId}`
        if (!rollbacks[key]) return false
        Reflect.deleteProperty(rollbacks, key)
        return true
    }

    /** Instances whose modpack content hint was dismissed (the app's instance cache). */
    const dismissedHints = new Set<string>()
    function hintMemory() {
        return dismissedHints
    }

    /** Adds a local file (dropped in or picked) the way the app lists unknown files. */
    function addLocalFile(
        instanceId: string,
        file: { name: string; size: number; type: ContentProjectType },
    ): SharedContentItem {
        const item: SharedContentItem = {
            id: `local-${Math.random().toString(36).slice(2, 10)}`,
            project_id: null,
            version_id: null,
            name: file.name.replace(/\.(jar|zip|disabled)$/i, ''),
            file_name: file.name,
            enabled: true,
            icon: null,
            version_number: null,
            project_type: file.type,
            size: file.size,
            installed_at: new Date().toISOString(),
            folder: contentFolder(file.type),
        }
        writeShared(instanceId, [item, ...sharedItems(instanceId)])
        return item
    }

    return {
        entries,
        entryFor,
        ensureLoaded,
        reload,
        sharedItems,
        metaFor,
        rollbackFor,
        expectUpdate,
        setEnabled,
        remove,
        restorePackDefault,
        rollback,
        unlinkPack,
        backupFiles,
        discardBackup,
        hintMemory,
        addLocalFile,
        project,
        rollbackSnapshot: (instanceId: string) =>
            Object.fromEntries(
                Object.entries(rollbacks)
                    .filter(([key]) => key.startsWith(`${instanceId}:`))
                    .map(([key, value]) => [key.slice(instanceId.length + 1), { ...value }]),
            ),
        restoreRollbackSnapshot: (
            instanceId: string,
            snapshot: Record<string, ContentRollback>,
        ) => {
            for (const [itemId, value] of Object.entries(snapshot))
                rollbacks[`${instanceId}:${itemId}`] = { ...value }
        },
    }
}

export type InstanceContentDomain = ReturnType<typeof createContentDomain>

/** The instance content domain of a preview window, for code that holds the state. */
export function instanceContent(state: PreviewState): InstanceContentDomain {
    return previewDomain(state, 'instance-content', (preview) => ({
        value: createContentDomain(preview),
    }))
}

/** The instance content domain (shared by the Content, Files and Logs tabs and the export modal). */
export function useInstanceContent(): InstanceContentDomain {
    return instanceContent(usePreview())
}

/** Installed content of an instance in the shape the content layout consumes. */
export function useInstanceContentItems(instanceId: () => string) {
    const domain = useInstanceContent()
    const preview = usePreview()
    const items = computed<LayoutContentItem[]>(() => {
        const id = instanceId()
        const shared = domain.sharedItems(id)
        // `requiredBy` is the reverse of the installed releases' required dependencies.
        const requiredBy = new Map<string, string[]>()
        for (const item of shared) {
            for (const projectId of domain.metaFor(id, item.id)?.requires ?? []) {
                if (item.project_id) {
                    requiredBy.set(projectId, [
                        ...(requiredBy.get(projectId) ?? []),
                        item.project_id,
                    ])
                }
            }
        }
        return shared.map((item) =>
            toLayoutItem(
                id,
                item,
                domain.metaFor(id, item.id),
                domain.rollbackFor(id, item.id),
                item.project_id ? (requiredBy.get(item.project_id) ?? []) : [],
                preview.instances.get(id),
            ),
        )
    })
    return { domain, items }
}

function dependencyRefs(projectIds: string[]) {
    return projectIds.map((projectId) => ({
        provider: 'modrinth' as const,
        projectId,
        releaseId: '',
    }))
}

export function toLayoutItem(
    instanceId: string,
    item: SharedContentItem,
    info: ContentMeta | null,
    rollback: ContentRollback | null,
    requiredBy: string[],
    instance?: GameInstance,
): LayoutContentItem {
    const fileName = item.enabled ? item.file_name : `${item.file_name}.disabled`
    const folder = item.folder ?? contentFolder(item.project_type)
    const packMember = item.pack_member === true
    const removed = item.removed === true
    const online = !!item.project_id
    const overrideKind = !packMember
        ? 'none'
        : removed
          ? 'removed'
          : !item.enabled
            ? 'disabled'
            : item.pack_default && item.version_id !== item.pack_default.version_id
              ? 'version'
              : 'none'
    return {
        id: item.id,
        file_name: fileName,
        file_path: `${folder}/${fileName}`,
        size: item.size,
        project_type: item.project_type,
        enabled: item.enabled && !removed,
        date_added: item.installed_at,
        update:
            info?.update && online && !packMember
                ? {
                      provider: 'modrinth',
                      project_id: item.project_id!,
                      current_version_id: item.version_id ?? '',
                      target_version_id: info.update.id,
                  }
                : null,
        origin_provider: online ? 'modrinth' : null,
        compatibility: instance ? contentCompatibility(item, info, instance) : null,
        installedSource: online ? 'modrinth' : null,
        external: !online,
        provider_refs: online
            ? [{ provider: 'modrinth', project_id: item.project_id!, version_id: item.version_id }]
            : [],
        rollback: rollback ? { file_name: `${rollback.fileName}.old` } : null,
        project: {
            id: item.project_id ?? `local:${item.file_name}`,
            slug: info?.slug ?? null,
            title: info?.title ?? item.name,
            icon_url: item.icon ?? undefined,
            description: info?.description,
        },
        version: {
            id: item.version_id ?? `local:${item.file_name}`,
            version_number: item.version_number ?? '',
            file_name: fileName,
        },
        owner: info?.owner ?? undefined,
        source_kind: packMember ? 'modrinth_modpack' : online ? 'modrinth' : 'local',
        instanceCapabilities: {
            canToggle: !removed,
            canDelete: !removed,
            canUpdate: online && !packMember && !removed,
            canChangeVersion: online && !packMember && !removed,
            canRestorePackDefault: !!item.pack_default && overrideKind !== 'none',
        },
        instanceMemberId: packMember ? `${instanceId}:${item.id}` : undefined,
        instanceMaterializationState: removed ? 'removed' : 'present',
        instanceOwnershipKind: packMember ? 'pack_managed' : 'user_added',
        instanceOverrideKind: overrideKind,
        dependency:
            info && online
                ? {
                      autoDependency: false,
                      requiredBy: dependencyRefs(requiredBy),
                      requires: dependencyRefs(info.requires),
                      orphaned: false,
                  }
                : null,
        installing: false,
        instanceResourceId: online ? `modrinth:${item.project_id}` : undefined,
        instanceEntryId: `${instanceId}:${item.id}`,
        instanceFileId: `${instanceId}:${item.id}`,
    } as LayoutContentItem
}

/* ------------------------------------------------------------------ worlds */

/** Folder name of a singleplayer world inside `saves/`. */
export function worldFolderName(world: HomeWorld): string {
    const segments = world.path.split(/[\\/]/).filter(Boolean)
    return segments[segments.length - 1] ?? world.name
}

/**
 * Worlds of the instances: the Home page's world list (the app's single
 * backend list), filtered per instance, plus the mutations the Worlds and
 * Files tabs need.
 */
export function useInstanceWorlds() {
    const home = useHomeState()

    function worldsFor(instanceId: string): HomeWorld[] {
        return home.worlds.value.filter((world) => world.instance_id === instanceId)
    }

    function removeWorld(world: HomeWorld) {
        home.worlds.value = home.worlds.value.filter((entry) => entry !== world)
    }

    function updateWorld(world: HomeWorld, patch: Partial<HomeWorld>) {
        home.worlds.value = home.worlds.value.map((entry) =>
            entry === world ? { ...entry, ...patch } : entry,
        )
    }

    function addWorld(world: HomeWorld) {
        home.worlds.value = [...home.worlds.value, world]
    }

    return { home, worldsFor, removeWorld, updateWorld, addWorld }
}

/* ------------------------------------------------------------- instance page */

/** The page-level UI state that survives navigating away and back. */
export function useInstancePageMemory() {
    return usePreviewDomain('instance-page', () => ({
        value: {
            /** Pinned content views (the app keeps them in localStorage). */
            pinnedViews: reactive<Record<string, unknown>>({}),
            /** Seconds the current launch has been running, per instance. */
            launchStartedAt: ref<Record<string, number>>({}),
        },
    }))
}

export function useInstanceFromRoute(id: () => string) {
    const preview = usePreview()
    return computed(() => preview.instances.get(id()))
}
