import type { Handlers } from './types'

/**
 * The demo player: an offline account as the launcher derives it from the name
 * alone (`OfflinePlayer:Steve` → name-based UUID), so no real identity is shown.
 * Microsoft and third-party sign-in need the desktop app.
 */
const STEVE = '5627dd98e6be3c21b8a8e92344183641'
const account = {
    account_id: `offline:5627dd98-e6be-3c21-b8a8-e92344183641`,
    account_type: 'offline',
    profile: { id: STEVE, name: 'Steve' },
    access_token: '',
    refresh_token: '',
    expires: '2099-01-01T00:00:00Z',
    active: true,
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
    'plugin:minecraft-skins|get_available_skins': () => [],
    'plugin:minecraft-skins|get_available_capes': () => [],
}
