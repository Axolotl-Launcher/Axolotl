import { injectNotificationManager } from '@modrinth/ui'
import { computed, onScopeDispose, type Ref, ref, watch } from 'vue'

import { instance_groups_listener } from '@/helpers/events'
import {
    create_group,
    delete_group,
    type InstanceGroupDefinition,
    type InstanceGroupMembershipChange,
    list_groups,
    MAX_INSTANCE_GROUP_NAME_LENGTH,
    rename_group,
    set_group_order,
    update_group_memberships,
} from '@/helpers/instance-groups'

export const FAVORITES_GROUP_ID = 'group:favorites'

export const UNGROUPED_GROUP_ID = 'group:none'

export { MAX_INSTANCE_GROUP_NAME_LENGTH }

export { type InstanceGroupDefinition, type InstanceGroupMembershipChange }

export interface InstanceGroupWithMembership {
    key: string
    name: string
    instances: string[]
}

export function useInstanceGroups(instances: Ref<{ id: string; groups: string[] }[]>) {
    const { handleError } = injectNotificationManager()
    const libraryGroups = ref<InstanceGroupDefinition[]>([])
    const libraryGroupsLoaded = ref(false)
    let disposed = false
    let stopListening: (() => void) | undefined

    async function fetchGroups() {
        const groups = await list_groups()
        if (disposed) return
        libraryGroups.value = groups
        libraryGroupsLoaded.value = true
    }

    void fetchGroups().catch(handleError)

    void instance_groups_listener(() => {
        if (!disposed) void fetchGroups().catch(handleError)
    })
        .then((unlisten) => {
            if (disposed) unlisten()
            else stopListening = unlisten
        })
        .catch(handleError)

    onScopeDispose(() => {
        disposed = true
        stopListening?.()
        stopListening = undefined
    })

    const orderedLibraryGroupIds = ref<string[]>([])

    watch(
        libraryGroups,
        (groups) => {
            orderedLibraryGroupIds.value = groups.map((g) => g.id)
        },
        { immediate: true },
    )

    const instanceGroups = computed(() => {
        if (!libraryGroupsLoaded.value) {
            return []
        }

        const instanceGroupMap = new Map<string, string[]>()

        for (const instance of instances.value) {
            const groups = instance.groups ?? []
            for (const groupId of groups) {
                if (!instanceGroupMap.has(groupId)) {
                    instanceGroupMap.set(groupId, [])
                }
                instanceGroupMap.get(groupId)!.push(instance.id)
            }
        }

        const allGroupIds = new Set<string>()
        const groupMap = new Map<string, string[]>()

        for (const groupId of orderedLibraryGroupIds.value) {
            allGroupIds.add(groupId)
            groupMap.set(groupId, instanceGroupMap.get(groupId) ?? [])
        }

        const ungroupedIds: string[] = []
        for (const instance of instances.value) {
            const instanceGroups = instance.groups ?? []
            const isInAnyGroup = instanceGroups.some((g) => allGroupIds.has(g))
            if (!isInAnyGroup) {
                ungroupedIds.push(instance.id)
            }
        }

        if (ungroupedIds.length > 0) {
            groupMap.set(UNGROUPED_GROUP_ID, ungroupedIds)
        }

        return [...groupMap.entries()].map(([key, instances]) => ({
            key,
            name:
                key === UNGROUPED_GROUP_ID
                    ? 'Ungrouped'
                    : (libraryGroups.value.find((g) => g.id === key)?.name ?? key),
            instances,
        }))
    })

    const creatingGroup = ref(false)
    const reorderingGroups = ref(false)

    async function createNewGroup(name: string): Promise<string | null> {
        creatingGroup.value = true
        try {
            const group = await create_group(name)
            libraryGroups.value = [...libraryGroups.value, group]
            orderedLibraryGroupIds.value = [...orderedLibraryGroupIds.value, group.id]
            return group.id
        } finally {
            creatingGroup.value = false
        }
    }

    async function renameGroupById(id: string, newName: string): Promise<boolean> {
        if (id === FAVORITES_GROUP_ID) return false
        const result = await rename_group(id, newName)
        const idx = libraryGroups.value.findIndex((g) => g.id === id)
        if (idx !== -1) {
            libraryGroups.value = [
                ...libraryGroups.value.slice(0, idx),
                result,
                ...libraryGroups.value.slice(idx + 1),
            ]
        }
        return true
    }

    async function deleteGroupById(id: string): Promise<boolean> {
        if (id === FAVORITES_GROUP_ID) return false
        await delete_group(id)
        libraryGroups.value = libraryGroups.value.filter((g) => g.id !== id)
        orderedLibraryGroupIds.value = orderedLibraryGroupIds.value.filter((gid) => gid !== id)
        return true
    }

    async function reorderGroups(groupIds: string[]): Promise<boolean> {
        reorderingGroups.value = true
        try {
            await set_group_order(groupIds)
            // Favorites is rendered separately from draggable sections. Keep it in
            // the local order after a drag even though the backend accepts only the
            // reorderable group ids; otherwise it appears again as an ordinary
            // section until the next full group refresh.
            orderedLibraryGroupIds.value = [
                FAVORITES_GROUP_ID,
                ...groupIds.filter((id) => id !== FAVORITES_GROUP_ID),
            ]
            return true
        } finally {
            reorderingGroups.value = false
        }
    }

    async function updateMemberships(updates: InstanceGroupMembershipChange[]): Promise<boolean> {
        await update_group_memberships(updates)
        return true
    }

    function groupNameById(id: string): string | undefined {
        if (id === UNGROUPED_GROUP_ID) return 'Ungrouped'
        return libraryGroups.value.find((g) => g.id === id)?.name
    }

    return {
        libraryGroups,
        libraryGroupsLoaded,
        orderedLibraryGroupIds,
        instanceGroups,
        creatingGroup,
        reorderingGroups,
        createGroup: createNewGroup,
        renameGroupById,
        deleteGroupById,
        reorderGroups,
        updateMemberships,
        groupNameById,
    }
}
