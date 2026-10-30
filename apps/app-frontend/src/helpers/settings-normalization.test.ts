import { invoke } from '@tauri-apps/api/core'
import { expect, it, vi } from 'vitest'

import { get } from './settings'

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
}))

it('normalizes missing settings to empty defaults where the launcher expects them', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ locale: 'en-US', feature_flags: {} })

    const settings = await get()

    expect(settings.default_game_language).toBe('')
})

it('preserves a configured default game language while normalizing settings', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
        locale: 'zh-CN',
        default_game_language: 'en_us',
        feature_flags: {},
    })

    const settings = await get()

    expect(settings.default_game_language).toBe('en_us')
})
