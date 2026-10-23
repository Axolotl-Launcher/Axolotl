import { effectScope, reactive, ref, watch } from 'vue'

import { type PreviewState, usePreviewDomain } from '../../lib/state'
import type { GameInstance } from '../../lib/types'

/** `useGridGrouping.ts` */
export const UNGROUPED_GROUP_KEY = '__ungrouped__'
/** `useInstanceGroups.ts` */
export const FAVORITES_GROUP_ID = 'group:favorites'
/** `helpers/instance-groups.ts` */
export const MAX_INSTANCE_GROUP_NAME_LENGTH = 256

export type GridGroupingOption = 'Group' | 'Loader' | 'Game version' | 'None'
export type GridSortOption =
    'Name' | 'Last played' | 'Date created' | 'Date modified' | 'Game version'
export type GridSortDirection = 'asc' | 'desc'
/** `helpers/library-display-mode.ts` */
export type LibraryDisplayMode = 'standard' | 'cards'

export const defaultSortDirections: Record<GridSortOption, GridSortDirection> = {
    Name: 'asc',
    'Last played': 'desc',
    'Date created': 'desc',
    'Date modified': 'desc',
    'Game version': 'asc',
}

export interface GridDisplayState {
    group: GridGroupingOption
    sortBy: GridSortOption
    sortDirections: Partial<Record<GridSortOption, GridSortDirection>>
    collapsedGroups: string[]
}

export interface InstanceGroupDefinition {
    id: string
    name: string
}

export interface InstanceGroupMembershipChange {
    instance_id: string
    add_group_ids: string[]
    remove_group_ids: string[]
}

/**
 * Ids of the groups the preview user starts with. The shared instance state
 * names them in Chinese; the Library shows them under a name in the visitor's
 * language until they are renamed.
 */
export const SAMPLE_GROUP_IDS = { adventure: '冒险', performance: '性能优化' } as const

const LAUNCH_STAGES = new Set([
    'preparing_instance',
    'resolving_minecraft',
    'resolving_loader',
    'preparing_java',
    'downloading_minecraft',
])

/**
 * The Library's backend: what the app keeps in SQLite and localStorage
 * (`useGridGrouping` state, `library-display-mode`, instance groups and their
 * order) lives here for the lifetime of the preview window, and launches are
 * routed through the shared process simulation.
 */
export function createLibraryState(preview: PreviewState) {
    const scope = effectScope(true)

    /** `useStorage(`${label}-grid-display-state`)`, one record per grid label. */
    const gridStates = reactive(new Map<string, GridDisplayState>())
    const displayMode = ref<LibraryDisplayMode>('standard')
    /** Names of groups created or renamed in this window (group ids never change). */
    const groupNames = reactive(new Map<string, string>())
    /** Launches waiting for another instance's launch to finish preparing. */
    const startingInstanceIds = ref(new Set<string>())
    let groupCounter = 0

    // The preview user keeps their most played instance pinned in the Library.
    const pinnedInLibrary = preview.instances.get('fabulously-optimized')
    if (pinnedInLibrary && !pinnedInLibrary.groups.includes(FAVORITES_GROUP_ID)) {
        preview.instances.setGroups(pinnedInLibrary.id, [
            ...pinnedInLibrary.groups,
            FAVORITES_GROUP_ID,
        ])
    }

    function gridState(label: string): GridDisplayState {
        let state = gridStates.get(label)
        if (!state) {
            gridStates.set(label, {
                group: 'Group',
                sortBy: 'Name',
                sortDirections: {},
                collapsedGroups: [],
            })
            state = gridStates.get(label)!
        }
        return state
    }

    // Groups (helpers/instance-groups.ts commands)

    function listGroups(): InstanceGroupDefinition[] {
        return [
            { id: FAVORITES_GROUP_ID, name: 'Favorites' },
            ...preview.instances.groups.value.map((id) => ({
                id,
                name: groupNames.get(id) ?? id,
            })),
        ]
    }

    function createGroup(name: string): InstanceGroupDefinition {
        const id = `group:${Date.now().toString(36)}${(++groupCounter).toString(36)}`
        groupNames.set(id, name)
        preview.instances.createGroup(id)
        return { id, name }
    }

    function renameGroup(id: string, name: string): InstanceGroupDefinition {
        if (id !== FAVORITES_GROUP_ID) groupNames.set(id, name)
        return { id, name }
    }

    function deleteGroup(id: string) {
        if (id === FAVORITES_GROUP_ID) return
        preview.instances.deleteGroup(id)
        groupNames.delete(id)
    }

    function setGroupOrder(ids: string[]) {
        const known = preview.instances.groups.value
        const next = ids.filter((id) => id !== FAVORITES_GROUP_ID && known.includes(id))
        for (const id of known) if (!next.includes(id)) next.push(id)
        preview.instances.groups.value = next
    }

    function updateMemberships(updates: InstanceGroupMembershipChange[]) {
        for (const update of updates) {
            const instance = preview.instances.get(update.instance_id)
            if (!instance) continue
            const next = instance.groups.filter((id) => !update.remove_group_ids.includes(id))
            for (const id of update.add_group_ids) if (!next.includes(id)) next.push(id)
            preview.instances.setGroups(instance.id, next)
        }
    }

    // Processes (helpers/instance.ts `run` / `kill`)

    /**
     * The shared simulation advances a process's stage on its plain object, which
     * Vue does not observe, so readers re-check the stages on the clock while a
     * launch or a stop is in flight (the app gets `process_listener` events).
     */
    const processTick = ref(0)
    let stopTicking: (() => void) | null = null

    function inFlight() {
        return (
            preview.processes.pendingInstanceId.value !== null ||
            preview.processes.processes.value.some((entry) => entry.stage !== 'running')
        )
    }

    function trackProcesses() {
        if (stopTicking || !inFlight()) return
        stopTicking = preview.clock.every(() => {
            processTick.value += 1
            if (!inFlight()) {
                stopTicking?.()
                stopTicking = null
            }
        })
    }

    function processOf(instanceId: string) {
        void processTick.value
        return preview.processes.processes.value.find((entry) => entry.instance.id === instanceId)
    }

    /** The app's `process_listener` running set: launched and not yet finished. */
    function isRunning(instanceId: string) {
        const entry = processOf(instanceId)
        return Boolean(entry && (entry.stage === 'running' || entry.stage === 'stopping'))
    }

    /** `run()` has not returned yet. */
    function isLaunching(instanceId: string) {
        if (startingInstanceIds.value.has(instanceId)) return true
        if (preview.processes.pendingInstanceId.value === instanceId) return true
        const entry = processOf(instanceId)
        return Boolean(entry && LAUNCH_STAGES.has(entry.stage))
    }

    /**
     * Starts an instance. The app launches instances side by side; the shared
     * simulation prepares one launch at a time, so a second launch waits (with
     * its spinner) until the first one is running.
     */
    function launch(instance: GameInstance) {
        if (isRunning(instance.id) || isLaunching(instance.id)) return
        if (preview.processes.launch(instance)) {
            trackProcesses()
            return
        }
        startingInstanceIds.value = new Set([...startingInstanceIds.value, instance.id])
    }

    /** Ids of the instances whose game is running (the app's `runningInstanceIds`). */
    function runningIds() {
        void processTick.value
        return new Set(
            preview.processes.processes.value
                .filter((entry) => entry.stage === 'running' || entry.stage === 'stopping')
                .map((entry) => entry.instance.id),
        )
    }

    function stop(instanceId: string) {
        const entry = processOf(instanceId)
        if (entry) {
            preview.processes.stop(entry.uuid)
            trackProcesses()
        } else if (startingInstanceIds.value.has(instanceId)) {
            const next = new Set(startingInstanceIds.value)
            next.delete(instanceId)
            startingInstanceIds.value = next
        }
    }

    scope.run(() => {
        watch(
            () => [preview.processes.processes.value, preview.processes.pendingInstanceId.value],
            trackProcesses,
            { immediate: true },
        )
        watch(
            () => [preview.processes.pendingInstanceId.value, startingInstanceIds.value] as const,
            ([pending, starting]) => {
                if (pending || starting.size === 0) return
                const [next] = starting
                const rest = new Set(starting)
                rest.delete(next)
                startingInstanceIds.value = rest
                const instance = preview.instances.get(next)
                if (instance) launch(instance)
            },
        )
    })

    return {
        value: {
            gridState,
            displayMode,
            groupNames,
            listGroups,
            createGroup,
            renameGroup,
            deleteGroup,
            setGroupOrder,
            updateMemberships,
            isRunning,
            isLaunching,
            runningIds,
            launch,
            stop,
        },
        dispose: () => {
            stopTicking?.()
            scope.stop()
        },
    }
}

export type LibraryState = ReturnType<typeof createLibraryState>['value']

export function useLibrary(): LibraryState {
    return usePreviewDomain('library', createLibraryState)
}
