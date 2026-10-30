import { expect, it } from 'vitest'

import { createNetworkStatus } from './useNetworkStatus.ts'

it('starts online unless the browser reports being offline', () => {
    expect(createNetworkStatus().offline.value).toBe(false)
    expect(createNetworkStatus(true).offline.value).toBe(true)
})

it('browser connectivity changes update global offline mode', () => {
    const status = createNetworkStatus()
    status.setBrowserOffline(true)
    expect(status.offline.value).toBe(true)
    status.markBrowserOnline()
    expect(status.browserOffline.value).toBe(false)
    expect(status.offline.value).toBe(false)
})
