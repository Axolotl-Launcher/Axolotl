import { expect, it } from 'vitest'
import { createMemoryHistory, createRouter } from 'vue-router'

import { multiplayerRoutes } from './multiplayer'

it('opens the multiplayer page on the rooms tab', async () => {
    // Rooms is the first tab in pages/Multiplayer.vue, so the index redirect has
    // to point at it: redirecting to the servers tab instead lands the sidebar
    // entry on the second tab, with the wrong one highlighted.
    const router = createRouter({ history: createMemoryHistory(), routes: multiplayerRoutes })
    await router.push('/multiplayer')
    expect(router.currentRoute.value.name).toBe('MultiplayerRooms')
    expect(router.currentRoute.value.path).toBe('/multiplayer/rooms')
})
