import { computed, reactive, ref } from 'vue'

import type { PreviewClock } from '../clock'
import type { GameInstance } from '../types'
import type { InstancesState } from './instances'

export type LaunchStage =
    | 'preparing_instance'
    | 'resolving_minecraft'
    | 'resolving_loader'
    | 'preparing_java'
    | 'downloading_minecraft'
    | 'running'
    | 'stopping'

export interface ProcessState {
    uuid: string
    instance: GameInstance
    stage: LaunchStage
    progress: number
    started_at: string
    /** The game's own log output (latest.log), one line per entry. */
    logs: string[]
    exit_code: number | null
}

export interface ProcessContext {
    /** Name of the account the game is launched with. */
    playerName: () => string | null
    /** Enabled mods of an instance, as `id version` pairs the loader prints at startup. */
    mods: (instanceId: string) => { id: string; version: string }[]
    /** Loads what `mods` reads; the launch steps run while it loads. */
    prepare: () => Promise<void>
}

/** The launcher's preparation stages before the game process starts. */
const LAUNCH_STEPS: { stage: LaunchStage; duration: number }[] = [
    { stage: 'preparing_instance', duration: 600 },
    { stage: 'resolving_minecraft', duration: 700 },
    { stage: 'resolving_loader', duration: 800 },
    { stage: 'preparing_java', duration: 500 },
    { stage: 'downloading_minecraft', duration: 900 },
]

const LOADER_NAMES: Partial<Record<GameInstance['loader'], string>> = {
    fabric: 'Fabric Loader',
    quilt: 'Quilt Loader',
    forge: 'Forge',
    neoforge: 'NeoForge',
}

function timestamp(date = new Date()) {
    return date.toTimeString().slice(0, 8)
}

function line(thread: string, message: string, level = 'INFO') {
    return `[${timestamp()}] [${thread}/${level}]: ${message}`
}

/** Startup output of the game in the format Minecraft writes to latest.log. */
function startupLog(instance: GameInstance, context: ProcessContext): string[][] {
    const loader = LOADER_NAMES[instance.loader]
    const mods = context.mods(instance.id)
    const player = context.playerName() ?? 'Player'
    const boot: string[] = []
    if (loader && (instance.loader === 'fabric' || instance.loader === 'quilt')) {
        boot.push(
            line(
                'main',
                `Loading Minecraft ${instance.game_version} with ${loader} ${instance.loader_version ?? ''}`.trim(),
            ),
        )
        boot.push(line('main', `Loading ${mods.length + 2} mods:`))
        boot.push(`\t- minecraft ${instance.game_version}`)
        boot.push(`\t- ${instance.loader}loader ${instance.loader_version ?? ''}`.trimEnd())
        for (const mod of mods) boot.push(`\t- ${mod.id} ${mod.version}`)
    } else if (loader) {
        boot.push(
            line('main', `ModLauncher running: ${loader} ${instance.loader_version ?? ''}`.trim()),
        )
        boot.push(
            line('main', `Loading ${mods.length} mods for Minecraft ${instance.game_version}`),
        )
    } else {
        boot.push(line('main', 'Building optimized datafixer'))
    }
    return [
        boot,
        [
            line('Render thread', `Setting user: ${player}`),
            line('Render thread', 'Backend library: LWJGL version 3.3.3-snapshot'),
        ],
        [
            line(
                'Render thread',
                `Reloading ResourceManager: vanilla${loader ? `, ${instance.loader}` : ''}`,
            ),
            line('Render thread', 'Sound engine started'),
        ],
        [line('Render thread', 'Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas')],
    ]
}

export function createProcessesState(
    clock: PreviewClock,
    instances: InstancesState,
    context: ProcessContext,
) {
    const processes = ref<ProcessState[]>([])
    const selectedUuid = ref<string | null>(null)
    const pendingInstanceId = ref<string | null>(null)
    /** The most recent process of each instance, kept after exit for the Logs tab. */
    const lastByInstance = ref<Record<string, ProcessState>>({})

    const selected = computed(
        () =>
            processes.value.find((entry) => entry.uuid === selectedUuid.value) ??
            processes.value[0] ??
            null,
    )

    function isRunning(instanceId: string) {
        return processes.value.some((entry) => entry.instance.id === instanceId)
    }

    function isBusy(instanceId: string) {
        return (
            pendingInstanceId.value === instanceId ||
            processes.value.some(
                (entry) => entry.instance.id === instanceId && entry.stage !== 'running',
            )
        )
    }

    function appendLog(entry: ProcessState, text: string) {
        entry.logs = [...entry.logs, text].slice(-2000)
    }

    function startGame(entry: ProcessState) {
        entry.stage = 'running'
        entry.progress = 1
        entry.started_at = new Date().toISOString()
        pendingInstanceId.value = null
        const batches = startupLog(entry.instance, context)
        let index = 0
        const stop = clock.every(() => {
            if (entry.stage !== 'running' || index >= batches.length) {
                stop()
                return
            }
            for (const text of batches[index]) appendLog(entry, text)
            index += 1
        })
    }

    function launch(instance: GameInstance): ProcessState | null {
        if (isRunning(instance.id) || pendingInstanceId.value) return null
        // Reactive, so the stage and progress changes below reach every screen.
        const entry = reactive<ProcessState>({
            uuid: `proc-${Math.random().toString(36).slice(2, 10)}`,
            instance,
            stage: 'preparing_instance',
            progress: 0,
            started_at: new Date().toISOString(),
            logs: [],
            exit_code: null,
        }) as ProcessState
        pendingInstanceId.value = instance.id
        void context.prepare()
        processes.value = [...processes.value, entry]
        lastByInstance.value = { ...lastByInstance.value, [instance.id]: entry }
        selectedUuid.value = entry.uuid

        let step = 0
        let elapsed = 0
        const stop = clock.every((delta) => {
            const current = LAUNCH_STEPS[step]
            elapsed += delta
            entry.stage = current.stage
            entry.progress = Math.min(1, elapsed / current.duration)
            if (elapsed < current.duration) return
            elapsed = 0
            step += 1
            if (step >= LAUNCH_STEPS.length) {
                stop()
                startGame(entry)
            }
        })
        return entry
    }

    function stop(uuid?: string) {
        const entry = uuid
            ? processes.value.find((item) => item.uuid === uuid)
            : (selected.value ?? undefined)
        if (!entry || entry.stage === 'stopping') return
        const wasRunning = entry.stage === 'running'
        entry.stage = 'stopping'
        if (wasRunning) appendLog(entry, line('Render thread', 'Stopping!'))
        clock.after(700, () => {
            entry.exit_code = 0
            processes.value = processes.value.filter((item) => item.uuid !== entry.uuid)
            if (pendingInstanceId.value === entry.instance.id) pendingInstanceId.value = null
            if (selectedUuid.value === entry.uuid) {
                selectedUuid.value = processes.value[0]?.uuid ?? null
            }
            if (wasRunning) {
                instances.touch(entry.instance.id, Date.now() - Date.parse(entry.started_at))
            }
        })
    }

    function select(uuid: string) {
        selectedUuid.value = uuid
    }

    function clearLogs(uuid: string) {
        const entry =
            processes.value.find((item) => item.uuid === uuid) ??
            Object.values(lastByInstance.value).find((item) => item.uuid === uuid)
        if (entry) entry.logs = []
    }

    return {
        processes,
        selected,
        pendingInstanceId,
        lastByInstance,
        isRunning,
        isBusy,
        launch,
        stop,
        select,
        clearLogs,
        appendLog,
    }
}

export type ProcessesState = ReturnType<typeof createProcessesState>
