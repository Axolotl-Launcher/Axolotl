import { injectNotificationManager } from '@modrinth/ui'
import { computed, onScopeDispose, ref } from 'vue'

import { favorite_groups_listener } from '@/helpers/events'
import {
    create_favorite_group,
    delete_favorite_group,
    type FavoriteGroupDefinition,
    type FavoriteGroupMembership,
    list_favorite_groups,
    rename_favorite_group,
    update_favorite_group_memberships,
} from '@/helpers/favorite-groups'

export { type FavoriteGroupDefinition, type FavoriteGroupMembership }

function membershipMap(memberships: FavoriteGroupMembership[]) {
    return new Map(memberships.map((membership) => [membership.favoriteKey, membership.groupIds]))
}

export function useFavoriteGroups() {
    const { handleError } = injectNotificationManager()
    const groups = ref<FavoriteGroupDefinition[]>([])
    const memberships = ref(new Map<string, string[]>())
    const loaded = ref(false)
    let disposed = false
    let fetchGeneration = 0
    let stopListening: (() => void) | undefined

    async function fetchGroups() {
        const generation = ++fetchGeneration
        const snapshot = await list_favorite_groups()
        if (disposed || generation !== fetchGeneration) return
        groups.value = snapshot.groups
        memberships.value = membershipMap(snapshot.memberships)
        loaded.value = true
    }

    void fetchGroups().catch(handleError)

    void favorite_groups_listener(() => {
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

    const groupsByKey = computed(() => memberships.value)

    function groupsFor(favoriteKey: string): string[] {
        return memberships.value.get(favoriteKey) ?? []
    }

    function isInGroup(favoriteKey: string, groupId: string): boolean {
        return groupsFor(favoriteKey).includes(groupId)
    }

    async function createGroup(name: string): Promise<FavoriteGroupDefinition> {
        const group = await create_favorite_group(name)
        groups.value = [group, ...groups.value.filter((candidate) => candidate.id !== group.id)]
        return group
    }

    async function renameGroup(id: string, name: string): Promise<FavoriteGroupDefinition> {
        const group = await rename_favorite_group(id, name)
        groups.value = groups.value.map((candidate) => (candidate.id === id ? group : candidate))
        return group
    }

    async function deleteGroup(id: string): Promise<void> {
        await delete_favorite_group(id)
        groups.value = groups.value.filter((group) => group.id !== id)
        const next = new Map(memberships.value)
        for (const [favoriteKey, groupIds] of next) {
            const remaining = groupIds.filter((groupId) => groupId !== id)
            if (remaining.length) next.set(favoriteKey, remaining)
            else next.delete(favoriteKey)
        }
        memberships.value = next
    }

    async function updateFavoriteGroups(
        favoriteKey: string,
        addGroupIds: string[],
        removeGroupIds: string[],
    ): Promise<void> {
        const previous = groupsFor(favoriteKey)
        const nextIds = [
            ...previous.filter((groupId) => !removeGroupIds.includes(groupId)),
            ...addGroupIds.filter((groupId) => !previous.includes(groupId)),
        ]
        const optimistic = new Map(memberships.value)
        if (nextIds.length) optimistic.set(favoriteKey, nextIds)
        else optimistic.delete(favoriteKey)
        memberships.value = optimistic

        try {
            await update_favorite_group_memberships([{ favoriteKey, addGroupIds, removeGroupIds }])
        } catch (error) {
            const rolledBack = new Map(memberships.value)
            if (previous.length) rolledBack.set(favoriteKey, previous)
            else rolledBack.delete(favoriteKey)
            memberships.value = rolledBack
            handleError(error)
            throw error
        }
    }

    return {
        groups,
        memberships,
        groupsByKey,
        loaded,
        groupsFor,
        isInGroup,
        createGroup,
        renameGroup,
        deleteGroup,
        updateFavoriteGroups,
        refresh: fetchGroups,
    }
}
