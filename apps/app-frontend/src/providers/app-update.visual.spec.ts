import { expect, it } from 'vitest'

// UpdateSettings imports these from the provider. If one of them is no longer
// exported, the settings section fails to link and its lazy Suspense fallback
// (the page skeleton) stays on screen with no visible error.
it('exports the update capability consumed by the settings panel', async () => {
    const provider = await import('./app-update.ts')
    expect(provider.updateCapability).toBeDefined()
    expect(provider.resolveUpdateCapability).toBeTypeOf('function')
    expect(provider.isPackageManagedInstall).toBeTypeOf('function')
})
