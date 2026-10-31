export type MinecraftAccountSortable = {
    account_type?: string
    profile?: {
        id?: string
        name?: string
    }
}

const ACCOUNT_TYPE_ORDER: Record<string, number> = {
    microsoft: 0,
    yggdrasil: 1,
    offline: 2,
}

export function compareMinecraftAccounts(
    left: MinecraftAccountSortable,
    right: MinecraftAccountSortable,
): number {
    const typeCmp =
        (ACCOUNT_TYPE_ORDER[left.account_type ?? ''] ?? 3) -
        (ACCOUNT_TYPE_ORDER[right.account_type ?? ''] ?? 3)
    if (typeCmp !== 0) return typeCmp

    const nameCmp = (left.profile?.name ?? '').localeCompare(right.profile?.name ?? '')
    if (nameCmp !== 0) return nameCmp

    const leftId = left.profile?.id ?? ''
    const rightId = right.profile?.id ?? ''
    return leftId < rightId ? -1 : leftId > rightId ? 1 : 0
}

export function sortMinecraftAccounts<T extends MinecraftAccountSortable>(
    accounts: readonly T[],
): T[] {
    return [...accounts].sort(compareMinecraftAccounts)
}
