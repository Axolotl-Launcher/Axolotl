import { invoke } from '@tauri-apps/api/core'

export type FavoriteGroupDefinition = {
    id: string
    name: string
}

export type FavoriteGroupMembership = {
    favoriteKey: string
    groupIds: string[]
}

export type FavoriteGroupsSnapshot = {
    groups: FavoriteGroupDefinition[]
    memberships: FavoriteGroupMembership[]
}

export type FavoriteGroupMembershipChange = {
    favoriteKey: string
    addGroupIds: string[]
    removeGroupIds: string[]
}

export async function list_favorite_groups(): Promise<FavoriteGroupsSnapshot> {
    return await invoke('plugin:axolotl-resources|resource_favorite_groups')
}

export async function create_favorite_group(name: string): Promise<FavoriteGroupDefinition> {
    return await invoke('plugin:axolotl-resources|resource_favorite_group_create', { name })
}

export async function rename_favorite_group(
    id: string,
    newName: string,
): Promise<FavoriteGroupDefinition> {
    return await invoke('plugin:axolotl-resources|resource_favorite_group_rename', {
        id,
        newName,
    })
}

export async function delete_favorite_group(id: string): Promise<void> {
    return await invoke('plugin:axolotl-resources|resource_favorite_group_delete', { id })
}

export async function update_favorite_group_memberships(
    updates: FavoriteGroupMembershipChange[],
): Promise<void> {
    return await invoke('plugin:axolotl-resources|resource_favorite_group_update_memberships', {
        updates,
    })
}
