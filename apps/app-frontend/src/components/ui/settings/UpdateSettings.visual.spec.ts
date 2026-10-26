import { expect, it } from 'vitest'

// The settings page loads this component lazily. A missing export in a module
// it imports fails at link time, and Vue then leaves the Suspense fallback
// (the page skeleton) on screen forever, so guard the module graph itself.
it('loads the update settings module', async () => {
    const module = await import('./UpdateSettings.vue')
    expect(module.default).toBeTruthy()
})
