import type { SkinModel } from './skins'

export type MinecraftCredential = {
    credential_persistence?: 'keyring' | 'session_only' | 'requires_login' | 'not_required' | null
    account_id: string
    account_type: 'microsoft' | 'offline' | 'yggdrasil'
    profile: {
        id: string
        name: string
        skins?: Array<{
            state: string
            url: string
            variant: SkinModel
            textureKey?: string
        }>
    }
    yggdrasil?: {
        api_root: string
        server_name: string
        login: string
    }
}
