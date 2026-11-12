import { beforeEach, describe, expect, it, vi } from 'vitest'

import {
    applyThemeRuntime,
    collectCurrentAppearanceSnapshot,
    parseThemeSource,
    reapplyThemeRuntime,
    resetThemeRuntime,
    resolveColorBucket,
    selectPackTokens,
    type ThemeRuntimeSelection,
} from '@/helpers/theme-pack-runtime'
import type { InstalledThemePack, ThemePackManifest } from '@/helpers/theme-packs'

const { applyCustomThemeColors, getThemePack } = vi.hoisted(() => ({
    applyCustomThemeColors: vi.fn(),
    getThemePack: vi.fn(),
}))

vi.mock('@/store/theme', () => ({ applyCustomThemeColors }))
vi.mock('@/helpers/theme-packs', () => ({
    getThemePack,
    themePackAssetUrl: (packId: string, path: string) => `theme://${packId}/${path}`,
}))

const manifest = {
    id: 'aurora',
    capabilities: ['tokens', 'styles'],
    tokens: {
        light: { '--surface-1': '#ffffff', '--surface-3': '#f1f1f1' },
        dark: { '--surface-1': '#000000', '--color-text-primary': '#ffffff' },
    },
    styles: ['assets/theme.css'],
    background: 'assets/background.png',
    fonts: { ui: 'assets/Inter.woff2' },
} as unknown as ThemePackManifest

const installedPack = { manifest } as unknown as InstalledThemePack

function packSelection(): ThemeRuntimeSelection {
    return {
        themeSource: 'pack:aurora',
        customThemeColors: null,
        uiFont: null,
    }
}

beforeEach(() => {
    resetThemeRuntime()
    vi.clearAllMocks()
    getThemePack.mockResolvedValue(installedPack)
})

describe('parseThemeSource', () => {
    it('recognizes built-in, custom and pack sources', () => {
        expect(parseThemeSource('builtin')).toEqual({ kind: 'builtin', packId: null })
        expect(parseThemeSource(undefined)).toEqual({ kind: 'builtin', packId: null })
        expect(parseThemeSource('custom')).toEqual({ kind: 'custom', packId: null })
        expect(parseThemeSource('pack:aurora')).toEqual({ kind: 'pack', packId: 'aurora' })
    })

    it('treats an empty pack id as built-in', () => {
        expect(parseThemeSource('pack:')).toEqual({ kind: 'builtin', packId: null })
    })
})

describe('collectCurrentAppearanceSnapshot', () => {
    it('uses the installed pack manifest tokens and records its source', async () => {
        const snapshot = await collectCurrentAppearanceSnapshot(
            'pack:aurora',
            'dark',
            packSelection(),
            '#123456',
        )

        expect(snapshot.source).toBe('pack:aurora')
        expect(snapshot.packId).toBe('aurora')
        expect(snapshot.accentColor).toBe('#123456')
        expect(snapshot.tokensLight['--surface-1']).toBe('#ffffff')
        expect(snapshot.tokensLight['--surface-3']).toBe('#f1f1f1')
        // The pack omits required tokens, so the documented defaults fill in.
        expect(snapshot.tokensLight['--color-text-primary']).toBe('#1a202c')
        expect(snapshot.tokensDark['--surface-1']).toBe('#000000')
        expect(snapshot.tokensDark['--color-text-primary']).toBe('#ffffff')
    })

    it('ignores tokens from a pack that did not declare the tokens capability', async () => {
        getThemePack.mockResolvedValue({
            manifest: { ...manifest, capabilities: ['pages'] },
        } as unknown as InstalledThemePack)
        const snapshot = await collectCurrentAppearanceSnapshot(
            'pack:aurora',
            'light',
            packSelection(),
        )

        expect(snapshot.tokensLight['--surface-3']).toBe('#f8f8f8')
        expect(snapshot.tokensDark['--surface-1']).toBe('#101215')
    })

    it('captures both buckets for a built-in appearance', async () => {
        const snapshot = await collectCurrentAppearanceSnapshot('builtin', 'light', {
            themeSource: 'builtin',
        })

        expect(snapshot.source).toBe('builtin')
        expect(snapshot.packId).toBeNull()
        expect(snapshot.tokensLight['--surface-1']).toBe('#ebebeb')
        expect(snapshot.tokensLight['--color-text-primary']).toBe('#1a202c')
        expect(snapshot.tokensDark['--surface-1']).toBe('#101215')
        expect(snapshot.tokensDark['--color-text-primary']).toBe('#ffffff')
    })
})

describe('resolveColorBucket', () => {
    it('maps explicit themes to their bucket', () => {
        expect(resolveColorBucket('light', true)).toBe('light')
        expect(resolveColorBucket('dark', false)).toBe('dark')
        expect(resolveColorBucket('oled', false)).toBe('dark')
    })

    it('follows the system preference for the system theme', () => {
        expect(resolveColorBucket('system', true)).toBe('dark')
        expect(resolveColorBucket('system', false)).toBe('light')
    })
})

describe('selectPackTokens', () => {
    it('returns the requested bucket only', () => {
        expect(selectPackTokens(manifest, 'light')).toEqual([
            ['--surface-1', '#ffffff'],
            ['--surface-3', '#f1f1f1'],
        ])
        expect(selectPackTokens(manifest, 'dark')).toEqual([
            ['--surface-1', '#000000'],
            ['--color-text-primary', '#ffffff'],
        ])
    })

    it('drops non-variable names and blank values', () => {
        const tokens = selectPackTokens(
            {
                tokens: {
                    light: {},
                    dark: { bad: 'red', '--empty': '   ', '--ok': '#123456' },
                },
            },
            'dark',
        )
        expect(tokens).toEqual([['--ok', '#123456']])
    })
})

describe('applyThemeRuntime', () => {
    it('applies custom colors for the custom source', async () => {
        const colors = { dark: { background: '#000000', surface: '#111111', text: '#ffffff' } }
        const result = await applyThemeRuntime('dark', {
            themeSource: 'custom',
            customThemeColors: colors,
        })

        expect(result).toEqual({ missingPackId: null })
        expect(getThemePack).not.toHaveBeenCalled()
    })

    it('clears custom colors for the built-in source', async () => {
        const result = await applyThemeRuntime('light', { themeSource: 'builtin' })

        expect(result).toEqual({ missingPackId: null })
        expect(getThemePack).not.toHaveBeenCalled()
    })

    it('loads the selected pack manifest', async () => {
        const result = await applyThemeRuntime('light', packSelection())

        expect(result).toEqual({ missingPackId: null })
        expect(getThemePack).toHaveBeenCalledWith('aurora')
    })

    it('falls back to built-in when the pack is missing', async () => {
        getThemePack.mockResolvedValue(null)

        const result = await applyThemeRuntime('dark', packSelection())

        expect(result).toEqual({ missingPackId: 'aurora' })
    })

    it('falls back to built-in when reading the pack fails', async () => {
        getThemePack.mockRejectedValue(new Error('backend unavailable'))

        const result = await applyThemeRuntime('dark', packSelection())

        expect(result).toEqual({ missingPackId: 'aurora' })
    })

    it('drops a stale pack result when a newer selection started', async () => {
        let resolveFirst: (value: InstalledThemePack | null) => void = () => {}
        getThemePack.mockReturnValueOnce(
            new Promise((resolve) => {
                resolveFirst = resolve
            }),
        )

        const first = applyThemeRuntime('dark', packSelection())
        const second = applyThemeRuntime('dark', { themeSource: 'builtin' })
        resolveFirst(installedPack)

        await expect(first).resolves.toEqual({ missingPackId: null })
        await expect(second).resolves.toEqual({ missingPackId: null })
    })
})

describe('reapplyThemeRuntime', () => {
    it('reuses the cached manifest instead of reading the pack again', async () => {
        await applyThemeRuntime('dark', packSelection())
        expect(getThemePack).toHaveBeenCalledTimes(1)

        await reapplyThemeRuntime('light')

        expect(getThemePack).toHaveBeenCalledTimes(1)
    })

    it('does nothing before a selection was applied', async () => {
        await expect(reapplyThemeRuntime('light')).resolves.toEqual({ missingPackId: null })
        expect(getThemePack).not.toHaveBeenCalled()
    })
})
