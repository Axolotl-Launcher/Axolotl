import { beforeEach, describe, expect, it, vi } from 'vitest'

import {
    dispatchThemePackRequest,
    parseThemePackRequest,
    THEME_PAGE_SANDBOX,
} from '@/helpers/theme-pack-bridge'
import type { ThemePackManifest } from '@/helpers/theme-packs'

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))

vi.mock('@tauri-apps/api/core', () => ({ invoke }))

beforeEach(() => {
    invoke.mockReset()
    invoke.mockResolvedValue(undefined)
})

function manifest(overrides: Partial<ThemePackManifest> = {}): ThemePackManifest {
    return {
        schemaVersion: 1,
        id: 'example',
        name: { 'en-US': 'Example' },
        version: '1.0.0',
        description: {},
        engines: { axolotl: '*' },
        capabilities: [],
        tokens: { light: {}, dark: {} },
        styles: [],
        background: null,
        fonts: { ui: null },
        permissions: [],
        network: { allow: [] },
        pages: [],
        ...overrides,
    }
}

describe('theme pack bridge', () => {
    it('accepts well-formed requests and rejects everything else', () => {
        expect(
            parseThemePackRequest({
                type: 'axolotl-theme-request',
                id: '1',
                method: 'storage.list',
                params: {},
            }),
        ).toEqual({
            type: 'axolotl-theme-request',
            id: '1',
            method: 'storage.list',
            params: {},
        })
        expect(parseThemePackRequest(null)).toBeNull()
        expect(parseThemePackRequest({ type: 'other', id: '1', method: 'x' })).toBeNull()
        expect(
            parseThemePackRequest({ type: 'axolotl-theme-request', id: '', method: 'x' }),
        ).toBeNull()
        expect(
            parseThemePackRequest({
                type: 'axolotl-theme-request',
                id: '1',
                method: 'x',
                params: 'nope',
            }),
        ).toBeNull()
    })

    it('refuses methods the manifest did not permit', async () => {
        const response = await dispatchThemePackRequest('example', manifest(), {
            type: 'axolotl-theme-request',
            id: '1',
            method: 'storage.list',
        })
        expect(response.ok).toBe(false)
        expect(response.error).toContain('storage')
    })

    it('refuses methods outside the fixed table even with permissions', async () => {
        const response = await dispatchThemePackRequest(
            'example',
            manifest({ permissions: ['storage'] }),
            {
                type: 'axolotl-theme-request',
                id: '1',
                method: 'settings.patch',
            },
        )
        expect(response.ok).toBe(false)
        expect(response.error).toContain('not available')
    })

    it('validates storage keys before reaching a command', async () => {
        const response = await dispatchThemePackRequest(
            'example',
            manifest({ permissions: ['storage'] }),
            {
                type: 'axolotl-theme-request',
                id: '1',
                method: 'storage.read',
                params: { key: '../secret' },
            },
        )
        expect(response.ok).toBe(false)
        expect(response.error).toContain('key')
    })

    it('forwards a permitted request to the matching command', async () => {
        invoke.mockResolvedValue('contents')
        const response = await dispatchThemePackRequest(
            'example',
            manifest({ permissions: ['storage'] }),
            {
                type: 'axolotl-theme-request',
                id: '1',
                method: 'storage.read',
                params: { key: 'settings' },
            },
        )
        expect(response).toMatchObject({ id: '1', ok: true, data: 'contents' })
        expect(invoke).toHaveBeenCalledWith('theme_pack_storage_read', {
            packId: 'example',
            key: 'settings',
        })
    })

    it('sandboxes pack pages without same-origin access', () => {
        expect(THEME_PAGE_SANDBOX).toBe('allow-scripts')
        expect(THEME_PAGE_SANDBOX).not.toContain('allow-same-origin')
    })
})
