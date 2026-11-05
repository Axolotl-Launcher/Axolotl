import { offlineUuid } from './offline-uuid'
import type { Handlers } from './types'

/**
 * An offline account as the launcher derives it from the name alone, so no real
 * identity is shown. Microsoft and third-party sign-in need the desktop app.
 */
function offlineAccount(name: string) {
    const id = offlineUuid(name)
    const dashed = id.replace(/^(.{8})(.{4})(.{4})(.{4})/, '$1-$2-$3-$4-')
    return {
        account_id: `offline:${dashed}`,
        account_type: 'offline',
        profile: { id, name },
        access_token: '',
        refresh_token: '',
        expires: '2099-01-01T00:00:00Z',
        active: true,
    }
}

/** The demo player, Steve unless demo-data.json names another. */
let account = offlineAccount('Steve')

export function useAccountName(name: string) {
    account = offlineAccount(name)
}

export const authHandlers: Handlers = {
    'plugin:auth|get_users': () => [structuredClone(account)],
    'plugin:auth|get_default_user': () => account.account_id,
    'plugin:auth|get_anti_piracy_status': () => ({
        region: 'non_cn',
        restricted: false,
        revision: 1,
    }),
    'plugin:auth|check_mojang_services': () =>
        ['session', 'services', 'textures'].map((service) => ({ service, reachable: true })),
    'plugin:auth|network_scope': () => 'global',
    'plugin:auth|check_reachable': () => null,
}
