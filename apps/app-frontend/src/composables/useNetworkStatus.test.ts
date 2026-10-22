import assert from 'node:assert/strict'
import test from 'node:test'

import { createNetworkStatus } from './useNetworkStatus.ts'

test('starts online unless the browser reports being offline', () => {
    assert.equal(createNetworkStatus().offline.value, false)
    assert.equal(createNetworkStatus(true).offline.value, true)
})

test('browser connectivity changes update global offline mode', () => {
    const status = createNetworkStatus()
    status.setBrowserOffline(true)
    assert.equal(status.offline.value, true)
    status.markBrowserOnline()
    assert.equal(status.browserOffline.value, false)
    assert.equal(status.offline.value, false)
})
