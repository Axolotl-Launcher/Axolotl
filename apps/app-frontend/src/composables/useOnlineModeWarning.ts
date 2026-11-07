import { useTemplateRef } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'

import type OnlineModeWarningModal from '@/components/multiplayer/servers/OnlineModeWarningModal.vue'
import { get_default_user, users } from '@/helpers/auth.js'

/**
 * Warns before launching with an offline account against a server that has
 * `online-mode` enabled, since the server will refuse that connection.
 *
 * Bind the returned `onlineModeWarningModal` ref to an
 * `<OnlineModeWarningModal ref="onlineModeWarningModal" />` in the template.
 */
export function useOnlineModeWarning() {
    const onlineModeWarningModal =
        useTemplateRef<ComponentExposed<typeof OnlineModeWarningModal>>('onlineModeWarningModal')

    /** Name of the active account when it is an offline account, else null. */
    async function offlineAccountName(): Promise<string | null> {
        try {
            const accountId = await get_default_user(false)
            if (!accountId) return null
            const account = (await users(false)).find(
                (candidate: { account_id: string }) => candidate.account_id === accountId,
            )
            if (account?.account_type !== 'offline') return null
            return account.profile?.name || 'offline'
        } catch {
            // No account selected yet; nothing to warn about.
            return null
        }
    }

    /**
     * Returns whether the launch should continue. Servers that turn off
     * `online-mode`, non-offline accounts, and missing modals all continue.
     */
    async function confirmOnlineModeLaunch(server: {
        name: string
        onlineMode?: boolean | null
    }): Promise<boolean> {
        if (server.onlineMode === false) return true
        const accountName = await offlineAccountName()
        if (accountName === null) return true
        const modal = onlineModeWarningModal.value
        if (!modal) return true
        return modal.show({ serverName: server.name, accountName })
    }

    return { onlineModeWarningModal, confirmOnlineModeLaunch }
}
