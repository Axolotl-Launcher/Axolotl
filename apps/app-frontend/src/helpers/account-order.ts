import { compareMinecraftAccounts, minecraftAccountId } from './accounts'
import type { MinecraftCredential } from './auth-types'

/**
 * Where the user's account order is remembered between starts. The order is
 * display state, like the browse display mode, so it lives in local storage
 * rather than in the account store.
 */
export const ACCOUNT_ORDER_STORAGE_KEY = 'axolotl-account-order'

/**
 * The account ids in the order the user arranged them, most preferred first.
 * Anything unreadable is treated as "no custom order": the default ordering is
 * always safe to show, and a bad value must not keep the account card from
 * rendering.
 */
export function readAccountOrder(): string[] {
    try {
        const raw = globalThis.localStorage?.getItem(ACCOUNT_ORDER_STORAGE_KEY)
        const parsed: unknown = JSON.parse(raw ?? '[]')
        if (!Array.isArray(parsed)) return []
        return [
            ...new Set(
                parsed.filter((id): id is string => typeof id === 'string' && id.length > 0),
            ),
        ]
    } catch {
        return []
    }
}

export function writeAccountOrder(ids: readonly string[]): void {
    globalThis.localStorage?.setItem(ACCOUNT_ORDER_STORAGE_KEY, JSON.stringify(ids))
}

/**
 * Sorts accounts the way the user arranged them. Accounts the saved order does
 * not name (new accounts, or a first launch with no saved order) keep the
 * default type/name ordering and follow the named ones.
 */
export function sortMinecraftAccountsWithOrder<T extends MinecraftCredential>(
    accounts: readonly T[],
    order: readonly string[],
): T[] {
    if (order.length === 0) return [...accounts].sort(compareMinecraftAccounts)

    const rank = new Map<string, number>()
    order.forEach((id, index) => rank.set(id, index))
    return [...accounts].sort((left, right) => {
        const leftRank = rank.get(minecraftAccountId(left))
        const rightRank = rank.get(minecraftAccountId(right))
        if (leftRank !== undefined && rightRank !== undefined) return leftRank - rightRank
        if (leftRank !== undefined) return -1
        if (rightRank !== undefined) return 1
        return compareMinecraftAccounts(left, right)
    })
}
