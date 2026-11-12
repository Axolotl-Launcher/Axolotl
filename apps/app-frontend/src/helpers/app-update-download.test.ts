import { expect, it, vi } from 'vitest'

import { openManualAppUpdate } from './app-update-download'

it.each(['1.11.0', '1.11.0-beta.4'])(
    'opens the checked %s release without falling back to the stable channel',
    async (version) => {
        const openUrl = vi.fn().mockResolvedValue(undefined)
        await openManualAppUpdate(version, openUrl)
        expect(openUrl).toHaveBeenCalledExactlyOnceWith(
            `https://github.com/Axolotl-Launcher/Axolotl/releases/tag/v${version}`,
        )
    },
)

it('keeps version delimiters inside a single release tag path segment', async () => {
    const openUrl = vi.fn()
    await openManualAppUpdate('1.2.3+build /?#', openUrl)
    expect(openUrl).toHaveBeenCalledExactlyOnceWith(
        'https://github.com/Axolotl-Launcher/Axolotl/releases/tag/v1.2.3%2Bbuild%20%2F%3F%23',
    )
})

it('propagates browser opening failure to the existing action error handler', async () => {
    const openUrl = vi.fn().mockRejectedValue(new Error('browser unavailable'))
    await expect(openManualAppUpdate('1.11.0-beta.4', openUrl)).rejects.toThrow(
        'browser unavailable',
    )
})
