import recorded from '../data/account.json'
import { offlineUuid } from './offline-uuid'
import { recordedCape, recordedSkin, useAccountAppearance } from './skins'
import type { Handlers } from './types'

export type DemoAccount =
    { name: string; type?: 'offline' } | { name: string; type: 'microsoft'; uuid: string }

/** An offline account as the launcher derives it from the name alone. */
function offlineAccount(name: string) {
    const id = offlineUuid(name)
    const dashed = id.replace(/^(.{8})(.{4})(.{4})(.{4})/, '$1-$2-$3-$4-')
    return {
        account_id: `offline:${dashed}`,
        account_type: 'offline' as const,
        profile: { id, name },
        access_token: '',
        refresh_token: '',
        expires: '2099-01-01T00:00:00Z',
        active: true,
    }
}

/** Public profile data only; signing in and launching still require the desktop app. */
function microsoftAccount() {
    const dashed = recorded.id.replace(/^(.{8})(.{4})(.{4})(.{4})/, '$1-$2-$3-$4-')
    return {
        ...offlineAccount(recorded.name),
        account_id: `microsoft:${dashed}`,
        account_type: 'microsoft' as const,
        profile: {
            id: recorded.id,
            name: recorded.name,
            skins: [
                {
                    state: 'ACTIVE',
                    url: recordedSkin.texture,
                    variant: recordedSkin.variant,
                    textureKey: recordedSkin.texture_key,
                },
            ],
        },
    }
}

let account: ReturnType<typeof offlineAccount> | ReturnType<typeof microsoftAccount> =
    microsoftAccount()

export function useAccount(value: DemoAccount): boolean {
    if (value.type === 'microsoft') {
        if (
            value.uuid.replaceAll('-', '').toLowerCase() !== recorded.id ||
            value.name !== recorded.name
        )
            return false
        account = microsoftAccount()
        useAccountAppearance(recordedSkin, recordedCape)
    } else {
        account = offlineAccount(value.name)
        useAccountAppearance()
    }
    return true
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
