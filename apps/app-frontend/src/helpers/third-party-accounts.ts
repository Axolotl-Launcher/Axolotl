import { invoke } from '@tauri-apps/api/core'

export interface ThirdPartyAccount {
    providerId: 'shimu'
    available: boolean
    status: 'disconnected' | 'connected' | 'expired'
    user: { userId: number; userName: string; nickName: string | null } | null
    expiresAt: number | null
    flowId: string | null
    authorization: 'waiting' | 'exchanging' | 'denied' | 'timedOut' | 'failed' | 'succeeded' | null
}

export const thirdPartyAccountKey = ['third-party-accounts', 'shimu'] as const

const notifiedAuthorizationFlows = new Set<string>()
let authorizationReturn: { flowId: string; route: string } | null = null

export async function consumeThirdPartyAuthorizationReturn(): Promise<string | null> {
    if (!authorizationReturn) return null
    const account = await getThirdPartyAccount()
    if (
        account.flowId !== authorizationReturn.flowId ||
        !['succeeded', 'denied', 'failed', 'timedOut'].includes(account.authorization ?? '')
    )
        return null
    const target = authorizationReturn.route
    authorizationReturn = null
    return target
}

export function claimThirdPartyAuthorizationNotification(account: ThirdPartyAccount): boolean {
    if (
        account.authorization !== 'succeeded' ||
        account.status !== 'connected' ||
        !account.flowId
    ) {
        return false
    }
    if (notifiedAuthorizationFlows.has(account.flowId)) return false
    notifiedAuthorizationFlows.add(account.flowId)
    return true
}

export function getThirdPartyAccount(): Promise<ThirdPartyAccount> {
    return invoke('plugin:third-party-accounts|get_account')
}

export async function beginThirdPartyAuthorization(returnTo?: string): Promise<{
    flowId: string
    authorizationUrl: string
}> {
    const flow = await invoke<{ flowId: string; authorizationUrl: string }>(
        'plugin:third-party-accounts|begin_authorization',
    )
    const target = returnTo ? new URL(returnTo, 'https://launcher.invalid') : null
    authorizationReturn =
        target?.origin === 'https://launcher.invalid' && target.pathname === '/browse/schematic'
            ? { flowId: flow.flowId, route: `${target.pathname}${target.search}` }
            : null
    return flow
}

export async function cancelThirdPartyAuthorization(flowId: string): Promise<void> {
    await invoke('plugin:third-party-accounts|cancel_authorization', { flowId })
    if (authorizationReturn?.flowId === flowId) authorizationReturn = null
}

export async function disconnectThirdPartyAccount(): Promise<void> {
    await invoke('plugin:third-party-accounts|disconnect')
    authorizationReturn = null
}

export function checkThirdPartyAccount(): Promise<ThirdPartyAccount> {
    return invoke('plugin:third-party-accounts|check_connection')
}
