import { computed, ref } from 'vue'

import type { DiscoverData, ProjectCard, ProjectDetail } from '../../data'
import { loadDiscover, loadInstanceContent, loadProject, projectIcon } from '../../data'
import type { PreviewClock } from '../clock'
import { contentFileName } from '../content-search'
import type { InstanceLoader } from '../types'
import type { InstancesState } from './instances'

export type InstallPhase =
    | 'queued'
    | 'resolving_project'
    | 'downloading'
    | 'installing'
    | 'completed'
    | 'failed'
    | 'canceled'

export interface InstallJob {
    id: string
    project_id: string
    project_title: string
    project_icon: string | null
    project_type: string
    version_id: string
    version_name: string
    file_name: string
    size: number
    instance_id: string
    instance_name: string
    phase: InstallPhase
    progress: number
    speed: number
    bytes_done: number
    created_at: string
    finished_at: string | null
    error: string | null
}

export interface ContentItem {
    id: string
    /** `null` for a file Modrinth does not know (the app lists those by file name). */
    project_id: string | null
    version_id: string | null
    name: string
    file_name: string
    enabled: boolean
    icon: string | null
    version_number: string | null
    project_type: string
    size: number
    installed_at: string
    /** Folder inside the instance (`mods`, `resourcepacks`, ...). */
    folder?: string
    /** A file of the linked modpack's version (its `modrinth.index.json` or overrides). */
    pack_member?: boolean
    /** The release the modpack's version ships for a pack member (`restore_pack_member_default`). */
    pack_default?: Pick<ContentItem, 'version_id' | 'version_number' | 'file_name' | 'size'>
    /** A pack member whose file the visitor removed (`materialization_state = 'removed'`). */
    removed?: boolean
}

/** Folder of the instance a project type is installed into. */
export function contentFolderFor(projectType: string): string {
    switch (projectType) {
        case 'resourcepack':
            return 'resourcepacks'
        case 'shader':
            return 'shaderpacks'
        case 'datapack':
            return 'datapacks'
        default:
            return 'mods'
    }
}

/** Instances whose files are listed in `data/instance-content.json` under another key. */
const CONTENT_KEYS: Record<string, string> = { 'fabric-26-3': 'custom-fabric' }
/** Mods the preview user turned off in their own instance. */
const DISABLED_FILES = new Set(['ferritecore-9.0.0-fabric.jar'])

function pickVersion(project: ProjectDetail, loader: InstanceLoader, gameVersion: string) {
    const matching = project.versions.filter(
        (version) =>
            version.game_versions.includes(gameVersion) &&
            version.loaders.some(
                (entry) => entry === loader || (loader === 'vanilla' && entry === 'datapack'),
            ),
    )
    return (
        matching.find((version) => version.version_type === 'release') ??
        matching[0] ??
        project.versions[0] ??
        null
    )
}

/**
 * The download queue: one job runs at a time (the launcher's install pipeline
 * is serial), progress advances with the shared clock, and a finished job
 * installs its content into the target instance.
 */
export function createDownloadsState(clock: PreviewClock, instances: InstancesState) {
    const jobs = ref<InstallJob[]>([])
    const contents = ref<Record<string, ContentItem[]>>({})
    const history = ref<InstallJob[]>([])
    let active: { cancel: () => void } | null = null

    const contentsLoaded = ref(false)
    let contentsPromise: Promise<void> | null = null

    /**
     * Loads the instances' files: a modpack instance holds its pack version's
     * files (`modrinth.index.json` and overrides), the Fabric instance the mods
     * its user installed. The list is a separate chunk, read on first use.
     */
    function ensureContents() {
        contentsPromise ??= loadInstanceContent().then((data) => {
            const withoutIcon = new Set(data.withoutIcon)
            const next = { ...contents.value }
            for (const instance of instances.instances.value) {
                const key =
                    instance.link?.type === 'modrinth_modpack'
                        ? instance.link.project_id
                        : CONTENT_KEYS[instance.id]
                const rows = key ? data.instances[key] : undefined
                if (!rows) continue
                const installed = Date.parse(instance.created)
                // A pack's files arrive with the pack; hand-installed mods one after another.
                const handInstalled = instance.link?.type !== 'modrinth_modpack'
                const packMember = !handInstalled
                const items = rows.map(
                    (
                        [
                            fileName,
                            folder,
                            size,
                            projectId,
                            versionId,
                            versionNumber,
                            title,
                            projectType,
                        ],
                        index,
                    ): ContentItem => ({
                        id: `${folder}/${fileName}`,
                        project_id: projectId,
                        version_id: versionId,
                        name: title ?? fileName,
                        file_name: fileName,
                        enabled: !DISABLED_FILES.has(fileName),
                        icon:
                            projectId && !withoutIcon.has(projectId)
                                ? projectIcon(projectId)
                                : null,
                        version_number: versionNumber,
                        project_type: projectType,
                        size,
                        installed_at: new Date(
                            installed + (handInstalled ? index * 3_600_000 : 0),
                        ).toISOString(),
                        folder,
                        pack_member: packMember,
                        pack_default: packMember
                            ? {
                                  version_id: versionId,
                                  version_number: versionNumber,
                                  file_name: fileName,
                                  size,
                              }
                            : undefined,
                    }),
                )
                next[instance.id] = [...(next[instance.id] ?? []), ...items]
            }
            contents.value = next
            contentsLoaded.value = true
        })
        return contentsPromise
    }

    function contentFor(instanceId: string) {
        void ensureContents()
        return computed(() => contents.value[instanceId] ?? [])
    }

    function update(job: InstallJob) {
        const index = jobs.value.findIndex((entry) => entry.id === job.id)
        if (index > -1)
            jobs.value = jobs.value.map((entry, i) => (i === index ? { ...job } : entry))
    }

    function finish(job: InstallJob, phase: InstallPhase, error: string | null = null) {
        job.phase = phase
        job.finished_at = new Date().toISOString()
        job.error = error
        job.speed = 0
        if (phase === 'completed') job.progress = 1
        jobs.value = jobs.value.filter((entry) => entry.id !== job.id)
        history.value = [{ ...job }, ...history.value]
        active = null
        startNext()
    }

    function run(job: InstallJob) {
        const steps: { phase: InstallPhase; duration: number }[] = [
            { phase: 'resolving_project', duration: 900 },
            { phase: 'downloading', duration: 4200 },
            { phase: 'installing', duration: 1400 },
        ]
        let step = 0
        let elapsed = 0
        job.phase = steps[0].phase
        update(job)
        const stop = clock.every((delta) => {
            elapsed += delta
            const current = steps[step]
            if (elapsed >= current.duration) {
                elapsed -= current.duration
                step += 1
                if (step >= steps.length) {
                    stop()
                    const items = contents.value[job.instance_id] ?? []
                    // A new release replaces the project's file and keeps its on/off state.
                    const replaced = items.find((item) => item.project_id === job.project_id)
                    contents.value = {
                        ...contents.value,
                        [job.instance_id]: [
                            {
                                id: `${job.project_id}-${job.version_id}`,
                                project_id: job.project_id,
                                version_id: job.version_id,
                                name: job.project_title,
                                file_name: job.file_name,
                                enabled: replaced?.enabled ?? true,
                                icon: job.project_icon,
                                version_number: job.version_name,
                                project_type: job.project_type,
                                size: job.size,
                                installed_at: replaced?.installed_at ?? new Date().toISOString(),
                                folder: replaced?.folder ?? contentFolderFor(job.project_type),
                                // A pack member keeps its membership, now with a version override.
                                pack_member: replaced?.pack_member,
                                pack_default: replaced?.pack_default,
                            },
                            ...items.filter((item) => item.project_id !== job.project_id),
                        ],
                    }
                    finish(job, 'completed')
                    return
                }
                job.phase = steps[step].phase
                job.progress = step === 1 ? 0 : job.progress
            }
            if (job.phase === 'downloading') {
                const share = Math.min(1, elapsed / steps[step].duration)
                job.progress = share
                job.bytes_done = Math.round(job.size * share)
                job.speed = Math.round(job.size / (steps[step].duration / 1000))
            }
            update(job)
        })
        active = { cancel: stop }
    }

    function startNext() {
        if (active) return
        const next = jobs.value.find((job) => job.phase === 'queued')
        if (next) run(next)
    }

    async function enqueue(input: {
        project: ProjectCard
        instanceId: string
        versionId?: string
        /** `resource_chinese_filenames`: save the file as `[中文名]file.jar`. */
        chineseFileNames?: boolean
    }) {
        const instance = instances.get(input.instanceId)
        if (!instance) return null
        const detail = await loadProject(input.project.id).catch(() => null)
        if (!detail) return null
        const version =
            detail.versions.find((entry) => entry.id === input.versionId) ??
            pickVersion(detail, instance.loader, instance.game_version)
        if (!version) return null
        const file = version.files.find((entry) => entry.primary) ?? version.files[0]
        const fileName = await contentFileName(
            file?.filename ?? `${detail.slug}.jar`,
            detail.slug,
            input.chineseFileNames ?? false,
        )
        const job: InstallJob = {
            id: `job-${Math.random().toString(36).slice(2, 10)}`,
            project_id: detail.id,
            project_title: detail.title,
            project_icon: projectIcon(detail.id),
            project_type: detail.project_type,
            version_id: version.id,
            version_name: version.version_number,
            file_name: fileName,
            size: file?.size ?? 1_048_576,
            instance_id: instance.id,
            instance_name: instance.name,
            phase: 'queued',
            progress: 0,
            speed: 0,
            bytes_done: 0,
            created_at: new Date().toISOString(),
            finished_at: null,
            error: null,
        }
        jobs.value = [...jobs.value, job]
        startNext()
        return job
    }

    function cancel(jobId: string) {
        const job = jobs.value.find((entry) => entry.id === jobId)
        if (!job) return
        if (job.phase === 'queued') {
            jobs.value = jobs.value.filter((entry) => entry.id !== jobId)
            history.value = [
                { ...job, phase: 'canceled', finished_at: new Date().toISOString() },
                ...history.value,
            ]
            return
        }
        active?.cancel()
        finish(job, 'canceled')
    }

    function retry(jobId: string) {
        const job = history.value.find((entry) => entry.id === jobId)
        if (!job) return
        history.value = history.value.filter((entry) => entry.id !== jobId)
        const next: InstallJob = {
            ...job,
            id: `job-${Math.random().toString(36).slice(2, 10)}`,
            phase: 'queued',
            progress: 0,
            bytes_done: 0,
            speed: 0,
            error: null,
            finished_at: null,
        }
        jobs.value = [...jobs.value, next]
        startNext()
    }

    function clearHistory() {
        history.value = []
    }

    function toggleContent(instanceId: string, itemId: string) {
        contents.value = {
            ...contents.value,
            [instanceId]: (contents.value[instanceId] ?? []).map((item) =>
                item.id === itemId ? { ...item, enabled: !item.enabled } : item,
            ),
        }
    }

    function removeContent(instanceId: string, itemId: string) {
        contents.value = {
            ...contents.value,
            [instanceId]: (contents.value[instanceId] ?? []).filter((item) => item.id !== itemId),
        }
    }

    const activeJobs = computed(() => jobs.value)
    const historyJobs = computed(() => history.value)
    const hasActivity = computed(() => jobs.value.length > 0)

    return {
        jobs,
        history,
        activeJobs,
        historyJobs,
        hasActivity,
        contents,
        contentsLoaded,
        ensureContents,
        contentFor,
        enqueue,
        cancel,
        retry,
        clearHistory,
        toggleContent,
        removeContent,
    }
}

export type DownloadsState = ReturnType<typeof createDownloadsState>

export async function loadDiscoverData(
    type: 'mod' | 'modpack' | 'resourcepack' | 'shader' | 'datapack',
): Promise<DiscoverData> {
    return loadDiscover(type)
}
