import type { SkinModel } from './skins'

export type MinecraftCredential = {
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
