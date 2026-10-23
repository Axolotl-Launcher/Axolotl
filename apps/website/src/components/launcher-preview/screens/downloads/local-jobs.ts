import { ref } from 'vue'

import { type PreviewState, usePreviewDomain } from '../../lib/state'
import type { GameInstance } from '../../lib/types'

/**
 * Install jobs that do not download a project: duplicating an instance
 * (`install_duplicate_instance`, provider Local) and repairing one
 * (`install_existing_instance`, provider Minecraft). The shared download queue
 * only runs content installs, so these run here, with the backend's phases
 * (`packages/app-lib/src/install/runner`), and the Downloads page lists them
 * next to the shared jobs.
 */
export type LocalJobKind = 'duplicate_instance' | 'install_existing_instance'

export type LocalJobStatus =
    'queued' | 'running' | 'canceling' | 'succeeded' | 'failed' | 'canceled'

export interface LocalJob {
    job_id: string
    kind: LocalJobKind
    provider: 'local' | 'minecraft'
    status: LocalJobStatus
    phase: string
    progress: { current: number; total: number } | null
    title: string
    icon: string | null
    instance_id: string | null
    source_instance_id: string | null
    created: string
    modified: string
    finished: string | null
    error: string | null
}

interface Step {
    phase: string
    duration: number
    /** Units the progress counts (files or bytes); 0 = indeterminate. */
    total: number
}

/** Files copied from the source instance folder (example value of the preview's instances). */
const COPIED_FILES = 1284
/** Bytes of the Minecraft files the job checks (example value). */
const CHECKED_BYTES = 28_734_112

const STEPS: Record<LocalJobKind, Step[]> = {
    duplicate_instance: [
        { phase: 'preparing_instance', duration: 1600, total: COPIED_FILES },
        { phase: 'resolving_minecraft', duration: 500, total: 0 },
        { phase: 'downloading_minecraft', duration: 900, total: CHECKED_BYTES },
    ],
    install_existing_instance: [
        { phase: 'downloading_minecraft', duration: 2200, total: CHECKED_BYTES },
        { phase: 'resolving_loader', duration: 600, total: 0 },
    ],
}

const CANCELED_MESSAGE = 'Install was canceled'

function now() {
    return new Date().toISOString()
}

export function createLocalJobs(preview: PreviewState) {
    const jobs = ref<LocalJob[]>([])
    const runners = new Map<string, () => void>()
    let counter = 0

    function patch(jobId: string, changes: Partial<LocalJob>) {
        jobs.value = jobs.value.map((job) =>
            job.job_id === jobId ? { ...job, ...changes, modified: now() } : job,
        )
    }

    function get(jobId: string) {
        return jobs.value.find((job) => job.job_id === jobId)
    }

    function setInstallStage(instanceId: string | null, stage: GameInstance['install_stage']) {
        if (!instanceId) return
        const instance = preview.instances.get(instanceId)
        if (instance) instance.install_stage = stage
    }

    function run(jobId: string) {
        const job = get(jobId)
        if (!job) return
        const steps = STEPS[job.kind]
        let index = 0
        let elapsed = 0
        setInstallStage(job.instance_id, 'minecraft_installing')
        patch(jobId, {
            status: 'running',
            phase: steps[0].phase,
            progress: steps[0].total ? { current: 0, total: steps[0].total } : null,
        })
        const stop = preview.clock.every((delta) => {
            const current = get(jobId)
            if (!current || current.status !== 'running') {
                stop()
                runners.delete(jobId)
                return
            }
            elapsed += delta
            const step = steps[index]
            if (elapsed >= step.duration) {
                elapsed -= step.duration
                index += 1
                if (index >= steps.length) {
                    stop()
                    runners.delete(jobId)
                    setInstallStage(current.instance_id, 'installed')
                    patch(jobId, {
                        status: 'succeeded',
                        phase: 'finalizing',
                        progress: null,
                        finished: now(),
                    })
                    return
                }
                const next = steps[index]
                patch(jobId, {
                    phase: next.phase,
                    progress: next.total ? { current: 0, total: next.total } : null,
                })
                return
            }
            if (step.total) {
                patch(jobId, {
                    progress: {
                        current: Math.round(step.total * Math.min(1, elapsed / step.duration)),
                        total: step.total,
                    },
                })
            }
        })
        runners.set(jobId, stop)
    }

    function add(input: {
        kind: LocalJobKind
        title: string
        icon: string | null
        instanceId: string | null
        sourceInstanceId: string | null
    }) {
        const job: LocalJob = {
            job_id: `local-${Date.now().toString(36)}-${(++counter).toString(36)}`,
            kind: input.kind,
            provider: input.kind === 'duplicate_instance' ? 'local' : 'minecraft',
            status: 'queued',
            phase:
                input.kind === 'duplicate_instance'
                    ? 'preparing_instance'
                    : 'downloading_minecraft',
            progress: null,
            title: input.title,
            icon: input.icon,
            instance_id: input.instanceId,
            source_instance_id: input.sourceInstanceId,
            created: now(),
            modified: now(),
            finished: null,
            error: null,
        }
        jobs.value = [job, ...jobs.value]
        preview.clock.after(150, () => run(job.job_id))
        return job
    }

    /**
     * `install_duplicate_instance`: the backend creates the copy right away
     * (same name, version, loader, icon and link) and installs it in a job.
     */
    function duplicate(source: GameInstance) {
        const created = preview.instances.create({
            name: source.name,
            gameVersion: source.game_version,
            loader: source.loader,
            loaderVersion: source.loader_version ?? null,
            iconPath: source.icon_path ?? null,
            link: source.link ?? null,
        })
        const copy = preview.instances.get(created.id)
        if (copy) {
            const stamp = now()
            copy.created = stamp
            copy.modified = stamp
            copy.last_played = undefined
            copy.submitted_time_played = 0
            copy.recent_time_played = 0
            copy.install_stage = 'not_installed'
        }
        return add({
            kind: 'duplicate_instance',
            title: source.name,
            icon: source.icon_path ?? null,
            instanceId: created.id,
            sourceInstanceId: source.id,
        })
    }

    /** `install_existing_instance`: reinstalls the game files of an instance. */
    function repair(instance: GameInstance) {
        if (jobs.value.some((job) => job.instance_id === instance.id && isActive(job))) return null
        return add({
            kind: 'install_existing_instance',
            title: instance.name,
            icon: instance.icon_path ?? null,
            instanceId: instance.id,
            sourceInstanceId: null,
        })
    }

    function cancel(jobId: string) {
        const job = get(jobId)
        if (!job || !isActive(job)) return
        runners.get(jobId)?.()
        runners.delete(jobId)
        patch(jobId, { status: 'canceling' })
        preview.clock.after(350, () => {
            const current = get(jobId)
            if (!current) return
            let instanceId = current.instance_id
            if (current.kind === 'duplicate_instance' && instanceId) {
                // The job's cleanup deletes the instance it created.
                preview.instances.remove(instanceId)
                instanceId = null
            } else {
                setInstallStage(instanceId, 'not_installed')
            }
            patch(jobId, {
                status: 'canceled',
                phase: 'finalizing',
                progress: null,
                finished: now(),
                error: CANCELED_MESSAGE,
                instance_id: instanceId,
            })
        })
    }

    /** `retry_job_as_new`: a fresh job for the same request; the record stays. */
    function retry(jobId: string) {
        const job = get(jobId)
        if (!job || isActive(job)) return
        if (job.kind === 'duplicate_instance') {
            const source = job.source_instance_id
                ? preview.instances.get(job.source_instance_id)
                : null
            if (source) duplicate(source)
            return
        }
        const instance = job.instance_id ? preview.instances.get(job.instance_id) : null
        if (instance) repair(instance)
    }

    function remove(jobId: string) {
        jobs.value = jobs.value.filter((job) => job.job_id !== jobId)
    }

    function clearHistory() {
        jobs.value = jobs.value.filter((job) => isActive(job))
    }

    function dispose() {
        for (const stop of runners.values()) stop()
        runners.clear()
    }

    return {
        value: { jobs, duplicate, repair, cancel, retry, remove, clearHistory },
        dispose,
    }
}

export function isActive(job: { status: string }) {
    return (
        job.status === 'queued' ||
        job.status === 'running' ||
        job.status === 'canceling' ||
        job.status === 'waiting_for_user'
    )
}

export type LocalJobs = ReturnType<typeof createLocalJobs>['value']

export function useLocalJobs(): LocalJobs {
    return usePreviewDomain('downloads:local-jobs', createLocalJobs)
}
