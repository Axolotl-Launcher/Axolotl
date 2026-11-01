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

export function beginThirdPartyAuthorization(): Promise<{
    flowId: string
    authorizationUrl: string
}> {
    return invoke('plugin:third-party-accounts|begin_authorization')
}

export function cancelThirdPartyAuthorization(flowId: string): Promise<void> {
    return invoke('plugin:third-party-accounts|cancel_authorization', { flowId })
}

export function disconnectThirdPartyAccount(): Promise<void> {
    return invoke('plugin:third-party-accounts|disconnect')
}

export function checkThirdPartyAccount(): Promise<ThirdPartyAccount> {
    return invoke('plugin:third-party-accounts|check_connection')
}
