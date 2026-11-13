import { computed, onMounted, readonly, ref, watch } from 'vue'

import steveSkinTexture from '@/assets/skins/steve.png?inline'
import { useNetworkStatus } from '@/composables/useNetworkStatus'
import { accountSelectionRevision } from '@/helpers/account-state'
import { compareMinecraftAccounts } from '@/helpers/accounts'
import { get_default_user, users } from '@/helpers/auth'
import {
    getPlayerHeadUrl,
    retainPlayerHeadTextures,
} from '@/helpers/rendering/batch-skin-renderer.ts'
import { createSkinHeadDataUrl, getAccountSkin } from '@/helpers/rendering/skin-head'

export type OobeAccount = {
    account_id: string
    account_type: 'microsoft' | 'offline' | 'yggdrasil'
    profile: { id: string; name: string; skins?: Array<{ state: string; url: string }> }
    yggdrasil?: { server_name: string }
}

/** Steve's head, cut from the bundled skin: what an account without a skin shows. */
export const steveHead = createSkinHeadDataUrl(steveSkinTexture)

/**
 * The account the launcher holds as default, shared by the step and its artwork so a save
 * shows in both at once.
 */
const defaultAccount = ref<string>()
/** An account picked in this step whose save has not settled: shown as selected meanwhile. */
const picked = ref<string>()
/** Numbers every read of the accounts, so an older read never undoes a newer one or a save. */
let reads = 0
/** The newest read, or save, that `defaultAccount` reflects. */
let defaultRead = 0

/**
 * Shows account `id` as selected at once, so the list and the art answer the click rather
 * than the save. `undefined` goes back to the account the launcher holds.
 */
export function showPicked(id: string | undefined) {
    picked.value = id
}

/**
 * The launcher now holds `id`: it becomes the default everywhere and the pick gives way to
 * it, so later changes to the default show again. Reads begun before the save are ignored.
 */
export function pickSaved(id: string) {
    defaultAccount.value = id
    defaultRead = reads
    picked.value = undefined
}

/**
 * The accounts on this device, the selected one and their rendered heads. It follows
 * `accountSelectionRevision`, so the step and its artwork each keep their own list in
 * step without depending on each other.
 */
export function useOobeAccounts() {
    const { offline } = useNetworkStatus()
    const accounts = ref<OobeAccount[]>([])
    let listRead = 0
    /**
     * The account to show as selected: a pick still saving, or the one the launcher holds.
     * It can name an account this list has not loaded yet, just after a sign-in.
     */
    const selected = computed(() => picked.value ?? defaultAccount.value)
    /** Rendered heads by skin texture; an account shows Steve until its head is ready. */
    const heads = ref(new Map<string, string>())

    async function refresh() {
        const read = ++reads
        try {
            const [list, held] = await Promise.all([
                users(offline.value, true) as unknown as Promise<OobeAccount[]>,
                get_default_user(offline.value) as Promise<string | undefined>,
            ])
            if (read > listRead) {
                listRead = read
                accounts.value = [...(list ?? [])].sort(compareMinecraftAccounts)
            }
            if (read > defaultRead) {
                defaultRead = read
                defaultAccount.value = held ?? undefined
            }
        } catch (error) {
            console.warn('Failed to list Minecraft accounts', error)
        }
    }

    watch(accountSelectionRevision, () => void refresh())
    onMounted(() => void refresh())

    watch(
        accounts,
        (list, _, onCleanup) => {
            const skins = list.flatMap((account) => getAccountSkin(account) ?? [])
            onCleanup(retainPlayerHeadTextures(skins.map((skin) => skin.texture_key)))
            for (const skin of skins) {
                if (heads.value.has(skin.texture_key)) continue
                getPlayerHeadUrl(skin)
                    .then((url) => {
                        heads.value = new Map(heads.value).set(skin.texture_key, url)
                    })
                    .catch((error) => console.warn('Failed to render an account head', error))
            }
        },
        { immediate: true },
    )

    function headOf(account: OobeAccount | undefined) {
        const skin = getAccountSkin(account)
        return (skin && heads.value.get(skin.texture_key)) ?? steveHead
    }

    return { accounts, defaultAccount: readonly(defaultAccount), selected, refresh, headOf }
}
