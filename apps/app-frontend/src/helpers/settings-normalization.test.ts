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
    expect(settings.force_multiconnection_downloads).toBe(false)
    expect(settings.forced_max_concurrent_downloads).toBe(72)
    expect(settings.feature_flags.advanced_download_info).toBe(false)
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

it.each([
    [128, 128],
    [256, 256],
    [0, 1],
    [900, 256],
    [42.8, 42],
])('normalizes experimental connection limit %s to %s', async (configured, expected) => {
    vi.mocked(invoke).mockResolvedValueOnce({
        locale: 'en-US',
        feature_flags: {},
        forced_max_concurrent_downloads: configured,
    })
    expect((await get()).forced_max_concurrent_downloads).toBe(expected)
})
