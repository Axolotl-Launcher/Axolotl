import { describe, expect, it } from 'vitest'

import {
    DEFAULT_SKIN_EDITOR_PREFERENCES,
    loadSkinEditorPreferences,
    normalizeExportFilename,
    normalizeSkinEditorPreferences,
    parseSkinEditorMessage,
    saveSkinEditorPreferences,
    shouldKeepRecoveryDraftAfterExport,
    shouldPromptBeforeLeave,
} from './session'

function memoryStorage(initial?: string) {
    let value = initial
    return {
        getItem: () => value ?? null,
        setItem: (_key: string, next: string) => {
            value = next
        },
    }
}

describe('skin editor preferences', () => {
    it('uses defaults for missing or malformed storage', () => {
        expect(loadSkinEditorPreferences(memoryStorage('{'))).toEqual(
            DEFAULT_SKIN_EDITOR_PREFERENCES,
        )
        expect(normalizeSkinEditorPreferences({ autoSaveIntervalSeconds: Number.NaN })).toEqual(
            DEFAULT_SKIN_EDITOR_PREFERENCES,
        )
    })

    it('clamps the interval and forces leave warnings when auto-save is off', () => {
        expect(
            saveSkinEditorPreferences(
                {
                    autoSaveEnabled: false,
                    autoSaveIntervalSeconds: 1,
                    warnBeforeLeave: false,
                },
                memoryStorage(),
            ),
        ).toEqual({
            autoSaveEnabled: false,
            autoSaveIntervalSeconds: 5,
            warnBeforeLeave: true,
        })
    })

    it('only skips the leave prompt when auto-save is on and warnings are off', () => {
        expect(shouldPromptBeforeLeave(DEFAULT_SKIN_EDITOR_PREFERENCES, false)).toBe(false)
        expect(shouldPromptBeforeLeave(DEFAULT_SKIN_EDITOR_PREFERENCES, true)).toBe(true)
        expect(
            shouldPromptBeforeLeave(
                {
                    autoSaveEnabled: true,
                    autoSaveIntervalSeconds: 30,
                    warnBeforeLeave: false,
                },
                true,
            ),
        ).toBe(false)
        expect(
            shouldPromptBeforeLeave(
                {
                    autoSaveEnabled: false,
                    autoSaveIntervalSeconds: 30,
                    warnBeforeLeave: false,
                },
                true,
            ),
        ).toBe(true)
    })

    it('keeps the recovery draft when edits happened after export started', () => {
        expect(shouldKeepRecoveryDraftAfterExport(2, 2)).toBe(false)
        expect(shouldKeepRecoveryDraftAfterExport(2, 3)).toBe(true)
    })

    it('always suggests a local PNG filename for export', () => {
        expect(normalizeExportFilename('Axolotl')).toBe('Axolotl.png')
        expect(normalizeExportFilename('Axolotl.PNG')).toBe('Axolotl.PNG')
        expect(normalizeExportFilename('   ')).toBe('skin.png')
    })
})

describe('skin editor messages', () => {
    it('accepts recovery metadata and rejects malformed timestamps', () => {
        expect(
            parseSkinEditorMessage({
                type: 'axolotl-skin-theme-ready',
                recovery: { updatedAt: 42, name: 'Axolotl.png' },
            }),
        ).toEqual({
            type: 'axolotl-skin-theme-ready',
            recovery: { updatedAt: 42, name: 'Axolotl.png' },
        })
        expect(
            parseSkinEditorMessage({
                type: 'axolotl-skin-theme-ready',
                recovery: { updatedAt: '42' },
            }),
        ).toBeNull()
    })

    it('validates export requests and editor state', () => {
        expect(
            parseSkinEditorMessage({
                type: 'axolotl-skin-export-request',
                requestId: 'request',
                name: 'skin.png',
                dataUrl: 'data:image/png;base64,iVBORw0KGgo=',
                reason: 'manual',
            }),
        ).toMatchObject({ type: 'axolotl-skin-export-request', requestId: 'request' })
        expect(
            parseSkinEditorMessage({
                type: 'axolotl-skin-state',
                dirty: true,
                autoSave: 'saved',
                lastSavedAt: 42,
            }),
        ).toEqual({
            type: 'axolotl-skin-state',
            dirty: true,
            autoSave: 'saved',
            lastSavedAt: 42,
        })
        expect(
            parseSkinEditorMessage({
                type: 'axolotl-skin-state',
                dirty: true,
                autoSave: 'unknown',
            }),
        ).toBeNull()
        expect(
            parseSkinEditorMessage({
                type: 'axolotl-skin-export-request',
                requestId: 'request',
                name: 'skin.png',
                dataUrl: 'https://example.com/skin.png',
                reason: 'manual',
            }),
        ).toBeNull()
        expect(
            parseSkinEditorMessage({
                type: 'axolotl-skin-export-request',
                requestId: 'request',
                name: 'skin.png',
                dataUrl: 'data:image/png;base64,AA==',
                reason: 'manual',
            }),
        ).toBeNull()
    })
})
