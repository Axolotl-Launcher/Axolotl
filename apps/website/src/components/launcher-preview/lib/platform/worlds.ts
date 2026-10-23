import type { HomeWorld } from '../../screens/home/home-utils'
import { useHomeState } from '../../screens/home/state'

/**
 * The app's `helpers/worlds.ts` for the preview, over the world list the Home
 * screen keeps for every instance.
 */
export type World = HomeWorld
export type SingleplayerWorld = Extract<HomeWorld, { type: 'singleplayer' }>

export function sortWorlds(worlds: World[]) {
    worlds.sort((a, b) => {
        if (!a.last_played) return 1
        if (!b.last_played) return -1
        return Date.parse(b.last_played) - Date.parse(a.last_played)
    })
}

export function isSingleplayerWorld(world: World): world is SingleplayerWorld {
    return world.type === 'singleplayer'
}

/** Call from a component's setup; the returned functions work in event handlers. */
export function useWorldsApi() {
    const home = useHomeState()
    return {
        get_instance_worlds: (instanceId: string) => home.getInstanceWorlds(instanceId),
    }
}
