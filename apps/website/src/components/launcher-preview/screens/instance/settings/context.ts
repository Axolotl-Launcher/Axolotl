import { createContext } from '@modrinth/ui/src/providers/create-context.ts'
import type { ComputedRef, Ref } from 'vue'

import type { GameInstance } from '../../../lib/types'

/** Port of `apps/app-frontend/src/providers/instance-settings.ts`. */
export interface InstanceSettingsContext {
    instance: ComputedRef<GameInstance>
    offline?: boolean
    isMinecraftServer: Ref<boolean>
    onUnlinked: () => void
    onInstanceUpdated: (instance: GameInstance) => void
    closeModal?: (onAfterClose?: () => void) => void
}

export const [injectInstanceSettings, provideInstanceSettings] =
    createContext<InstanceSettingsContext>('InstanceSettingsModal', 'instanceSettings')
