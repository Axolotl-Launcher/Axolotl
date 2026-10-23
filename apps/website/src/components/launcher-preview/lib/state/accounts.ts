import { computed, ref } from 'vue'

import { hyphenateUuid, offlineUuid } from '../offline-uuid'
import type { MinecraftAccount } from '../types'

export interface SkinEntry {
    texture_key: string
    name: string
    section: string | null
    variant: 'CLASSIC' | 'SLIM'
    texture: string
    cape_id: string | null
    source: 'default' | 'custom'
    is_equipped: boolean
}

/** Port of the app's `compareMinecraftAccounts` (apps/app-frontend/src/helpers/accounts.ts). */
const ACCOUNT_TYPE_ORDER: Record<string, number> = { microsoft: 0, yggdrasil: 1, offline: 2 }

function compareAccounts(left: MinecraftAccount, right: MinecraftAccount) {
    const nameCmp = left.profile.name.localeCompare(right.profile.name)
    if (nameCmp !== 0) return nameCmp
    const typeCmp =
        (ACCOUNT_TYPE_ORDER[left.account_type] ?? 3) - (ACCOUNT_TYPE_ORDER[right.account_type] ?? 3)
    if (typeCmp !== 0) return typeCmp
    return left.profile.id < right.profile.id ? -1 : left.profile.id > right.profile.id ? 1 : 0
}

/** An offline account as the launcher creates it (`Credentials::offline_with_uuid`). */
function offlineAccount(name: string, id = offlineUuid(name)): MinecraftAccount {
    return {
        account_type: 'offline',
        account_id: `offline:${hyphenateUuid(id)}`,
        profile: { id, name },
    }
}

/**
 * Accounts of the preview user. Both are offline accounts the launcher derives
 * from the player name alone, so the preview shows no real player's identity;
 * Microsoft and third-party sign-in need the desktop app.
 */
function defaultAccounts(): MinecraftAccount[] {
    return [offlineAccount('Steve'), offlineAccount('Alex')].sort(compareAccounts)
}

/**
 * The account's own skins (the app's `CustomMinecraftSkin` rows): one custom
 * skin, the app's bundled Herobrine texture, which is equipped. The default
 * skins are listed by the Skins page from the app's DEFAULT_SKINS snapshot.
 */
export const defaultSkins: SkinEntry[] = [
    {
        texture_key: 'local-herobrine',
        name: 'Herobrine',
        section: null,
        variant: 'CLASSIC',
        texture: '/launcher-preview/skins/herobrine.png',
        cape_id: null,
        source: 'custom',
        is_equipped: true,
    },
]

/** The app accepts letters of any script, digits and underscores (1–16). */
const USERNAME_PATTERN = /^[\p{L}\p{N}_]{1,16}$/u

export type AddOfflineResult =
    | { status: 'ok'; account: MinecraftAccount }
    | { status: 'invalid-name' }
    | { status: 'duplicate-uuid' }

export function createAccountsState(_clock: unknown) {
    const accounts = ref<MinecraftAccount[]>(defaultAccounts())
    const selectedAccountId = ref<string | null>(
        accounts.value.find((account) => account.profile.name === 'Steve')?.account_id ?? null,
    )
    const skins = ref<SkinEntry[]>(defaultSkins.map((skin) => ({ ...skin })))

    const selected = computed(
        () =>
            accounts.value.find((account) => account.account_id === selectedAccountId.value) ??
            accounts.value[0] ??
            null,
    )

    function select(accountId: string) {
        if (accounts.value.some((account) => account.account_id === accountId)) {
            selectedAccountId.value = accountId
        }
    }

    function validateUsername(name: string) {
        return USERNAME_PATTERN.test(name)
    }

    /**
     * Port of `add_offline_user`: without a custom UUID the account is derived
     * from the name and upserted; a custom UUID must not be taken yet.
     */
    function addOffline(name: string, uuid?: string): AddOfflineResult {
        const trimmed = name.trim()
        if (!validateUsername(trimmed)) return { status: 'invalid-name' }
        const account = offlineAccount(
            trimmed,
            uuid ? uuid.replaceAll('-', '').toLowerCase() : undefined,
        )
        const existing = accounts.value.find((entry) => entry.account_id === account.account_id)
        if (uuid && existing) return { status: 'duplicate-uuid' }
        accounts.value = [
            ...accounts.value.filter((entry) => entry.account_id !== account.account_id),
            account,
        ].sort(compareAccounts)
        selectedAccountId.value = account.account_id
        return { status: 'ok', account }
    }

    function remove(accountId: string) {
        accounts.value = accounts.value.filter((account) => account.account_id !== accountId)
        if (selectedAccountId.value === accountId) {
            selectedAccountId.value = accounts.value[0]?.account_id ?? null
        }
    }

    function equipSkin(textureKey: string) {
        skins.value = skins.value.map((skin) => ({
            ...skin,
            is_equipped: skin.texture_key === textureKey,
        }))
    }

    const equippedSkin = computed(
        () => skins.value.find((skin) => skin.is_equipped) ?? skins.value[0] ?? null,
    )

    return {
        accounts,
        selected,
        selectedAccountId,
        skins,
        equippedSkin,
        select,
        validateUsername,
        addOffline,
        remove,
        equipSkin,
    }
}

export type AccountsState = ReturnType<typeof createAccountsState>
