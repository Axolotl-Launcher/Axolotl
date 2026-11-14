import { computed, inject, type InjectionKey, type Ref } from 'vue'

import type { GameInstance } from '@/helpers/types'

import { resolveHomeLaunchInstance } from './home-utils'

/**
 * What the Home page shares with every launch surface on it (the minimal stage
 * and the launch widget): the instance chosen for Home, and how to change it.
 */
export type HomeLaunchContext = {
    selectedInstanceId: Readonly<Ref<string | null>>
    /** Opens the instance picker. */
    choose: () => void
    /** Makes this instance the Home instance. */
    select: (instance: GameInstance) => void
    create: () => void
}

export const HOME_LAUNCH_KEY: InjectionKey<HomeLaunchContext> = Symbol('home-launch')

/** The instance Home offers to launch, for widgets that point the next step at it. */
export function useHomeLaunchTarget(instances: () => readonly GameInstance[]) {
    const launch = inject(HOME_LAUNCH_KEY, null)
    return computed(
        () =>
            resolveHomeLaunchInstance(instances(), launch?.selectedInstanceId.value)?.instance ??
            null,
    )
}
