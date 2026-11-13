import type { MinecraftCredential } from './auth-types'

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

/**
 * The stable identity an account is stored under.
 *
 * `account_id` is what the backend writes today; the derivation below is the
 * one it uses for accounts saved before that field existed, so a stored order
 * keeps naming the same account either way.
 */
export function minecraftAccountId(account: MinecraftCredential): string {
    if (account.account_id) return account.account_id

    const profileId = account.profile.id
    if (account.account_type === 'yggdrasil' && account.yggdrasil) {
        return `${account.account_type}:${account.yggdrasil.api_root}:${profileId}`
    }
    return `${account.account_type}:${profileId}`
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
