import { reactive, watch } from 'vue'

import type { PreviewState } from '../../../lib/state'
import { previewDomain, usePreviewDomain } from '../../../lib/state'
import type { GameInstance } from '../../../lib/types'

/*
 * Log files of the instances (`logs/` and `crash-reports/` in the instance
 * folder), shared by the Logs tab (historical sources) and the Files tab
 * (folder listing).
 */

export type LogType = 'InfoLog' | 'CrashReport'

export interface InstanceLogFile {
    filename: string
    log_type: LogType
    /** Folder relative to the instance root. */
    folder: 'logs' | 'crash-reports'
    size: number
    modified: string
    /** Text content, one entry per line. */
    lines: string[]
}

const HOUR = 60 * 60 * 1000
const DAY = 24 * HOUR

function pad(value: number) {
    return String(value).padStart(2, '0')
}

function datePart(date: Date) {
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
}

function timePart(date: Date) {
    return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
}

/** A short session log in the format Minecraft writes to `logs/*.log`. */
function sessionLog(instance: GameInstance, start: Date): string[] {
    const at = (offsetSeconds: number) => timePart(new Date(start.getTime() + offsetSeconds * 1000))
    const loader =
        instance.loader === 'fabric'
            ? `Fabric Loader ${instance.loader_version ?? ''}`.trim()
            : instance.loader === 'forge'
              ? `Forge ${instance.loader_version ?? ''}`.trim()
              : null
    const lines = [
        loader
            ? `[${at(0)}] [main/INFO]: Loading Minecraft ${instance.game_version} with ${loader}`
            : `[${at(0)}] [main/INFO]: Building optimized datafixer`,
        `[${at(2)}] [Render thread/INFO]: Backend library: LWJGL version 3.3.3-snapshot`,
        `[${at(4)}] [Render thread/INFO]: Reloading ResourceManager: vanilla${loader ? `, ${instance.loader}` : ''}`,
        `[${at(5)}] [Render thread/INFO]: Sound engine started`,
        `[${at(6)}] [Render thread/INFO]: Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas`,
        `[${at(31)}] [Server thread/INFO]: Starting integrated minecraft server version ${instance.game_version}`,
        `[${at(33)}] [Server thread/INFO]: Preparing start region for dimension minecraft:overworld`,
        `[${at(36)}] [Render thread/WARN]: Ignoring unknown attribute 'minecraft:generic.step_height'`,
        `[${at(1840)}] [Server thread/INFO]: Saving chunks for level 'ServerLevel[World]'/minecraft:overworld`,
        `[${at(1842)}] [Render thread/INFO]: Stopping!`,
    ]
    return lines
}

function seedFiles(instance: GameInstance): InstanceLogFile[] {
    const lastPlayed = new Date(instance.last_played ?? instance.created)
    const files: InstanceLogFile[] = []
    const latest = sessionLog(instance, lastPlayed)
    files.push({
        filename: 'latest.log',
        log_type: 'InfoLog',
        folder: 'logs',
        size: latest.join('\n').length,
        modified: new Date(lastPlayed.getTime() + 1842 * 1000).toISOString(),
        lines: latest,
    })
    for (let index = 1; index <= 2; index += 1) {
        const start = new Date(lastPlayed.getTime() - index * 2 * DAY - 3 * HOUR)
        const lines = sessionLog(instance, start)
        files.push({
            filename: `${datePart(start)}-1.log.gz`,
            log_type: 'InfoLog',
            folder: 'logs',
            size: Math.round(lines.join('\n').length * 0.31),
            modified: new Date(start.getTime() + 1842 * 1000).toISOString(),
            lines,
        })
    }
    return files
}

function createLogsDomain(state: PreviewState) {
    const files = reactive<Record<string, InstanceLogFile[]>>({})

    /** Game sessions already reflected in the files, by process id. */
    const started = new Set<string>()
    const finished = new Set<string>()

    function filesFor(instanceId: string): InstanceLogFile[] {
        if (!files[instanceId]) {
            const instance = state.instances.get(instanceId)
            files[instanceId] = instance ? seedFiles(instance) : []
            syncSessions()
        }
        return files[instanceId]
    }

    /** Applies the sessions the preview ran (the app's `process_listener` events). */
    function syncSessions() {
        for (const entry of Object.values(state.processes.lastByInstance.value)) {
            const instanceId = entry.instance.id
            if (!files[instanceId]) continue
            if (!started.has(entry.uuid)) {
                started.add(entry.uuid)
                startSession(instanceId)
            }
            const running = state.processes.processes.value.some((item) => item.uuid === entry.uuid)
            if (!running && !finished.has(entry.uuid)) {
                finished.add(entry.uuid)
                finishSession(instanceId, entry.logs, new Date().toISOString())
            }
        }
    }

    function deleteFile(instanceId: string, filename: string) {
        files[instanceId] = filesFor(instanceId).filter((file) => file.filename !== filename)
    }

    /**
     * A game session began: Minecraft archives the previous latest.log as
     * `<date>-<n>.log.gz` and starts a new latest.log.
     */
    function startSession(instanceId: string) {
        const list = filesFor(instanceId)
        const latest = list.find((file) => file.filename === 'latest.log')
        if (!latest) return
        const day = datePart(new Date(latest.modified))
        let index = 1
        while (list.some((file) => file.filename === `${day}-${index}.log.gz`)) index += 1
        files[instanceId] = [
            { ...latest, filename: `${day}-${index}.log.gz`, size: Math.round(latest.size * 0.31) },
            ...list.filter((file) => file !== latest),
        ]
    }

    /** The session ended: its output is the new latest.log. */
    function finishSession(instanceId: string, lines: string[], finishedAt: string) {
        const text = lines.join('\n')
        files[instanceId] = [
            {
                filename: 'latest.log',
                log_type: 'InfoLog',
                folder: 'logs',
                size: text.length,
                modified: finishedAt,
                lines: [...lines],
            },
            ...filesFor(instanceId).filter((file) => file.filename !== 'latest.log'),
        ]
    }

    const stop = watch(
        () => [
            state.processes.processes.value.map((item) => item.uuid).join(),
            Object.values(state.processes.lastByInstance.value)
                .map((item) => item.uuid)
                .join(),
        ],
        syncSessions,
    )

    return { value: { files, filesFor, deleteFile }, dispose: stop }
}

export type InstanceLogsDomain = ReturnType<typeof createLogsDomain>['value']

export function useInstanceLogs(): InstanceLogsDomain {
    return usePreviewDomain('instance-logs', createLogsDomain)
}

export function instanceLogs(preview: PreviewState): InstanceLogsDomain {
    return previewDomain(preview, 'instance-logs', createLogsDomain)
}
