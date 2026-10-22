import { expect, it, vi } from 'vitest'

import { accountSelectionRevision } from './account-state'
import { set_default_user } from './auth.js'
const backend = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: backend.invoke }))
it('publishes default-account changes from every caller only after persistence succeeds', async () => {
    const initial = accountSelectionRevision.value
    backend.invoke.mockResolvedValueOnce(undefined)
    await set_default_user('new-account')
    expect(accountSelectionRevision.value).toBe(initial + 1)
    backend.invoke.mockRejectedValueOnce(new Error('save failed'))
    await expect(set_default_user('other-account')).rejects.toThrow('save failed')
    expect(accountSelectionRevision.value).toBe(initial + 1)
})
