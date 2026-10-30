import { expect, it } from 'vitest'

import { applySettingChanges } from './deep-link-settings.ts'
import type { AppSettings } from './settings.ts'
import { createSettingsPatchSaver, diffSettings } from './settings-patch.ts'

it('stale drafts save only edits and preserve changes from another settings region', async () => {
    const original = { theme: 'dark', developer_mode: false, feature_flags: { a: false, b: false } }
    const backend = structuredClone(original)
    const patches: Record<string, unknown>[] = []
    const save = async (patch: Record<string, unknown>) => {
        patches.push(patch)
        Object.assign(backend, patch)
    }
    const appearance = createSettingsPatchSaver(original, save)
    const developer = createSettingsPatchSaver(original, save)
    await appearance({ ...original, theme: 'light' })
    await developer({ ...original, developer_mode: true })
    expect(backend.theme).toBe('light')
    expect(backend.developer_mode).toBe(true)
    expect(patches).toEqual([{ theme: 'light' }, { developer_mode: true }])
    expect(diffSettings(original, { ...original, feature_flags: { a: true, b: false } })).toEqual({
        feature_flags: { a: true },
    })
})

it('captures rapid saves and retries failed edits without advancing the baseline', async () => {
    let release!: () => void
    const pending = new Promise<void>((resolve) => {
        release = resolve
    })
    const patches: Record<string, unknown>[] = []
    const draft = { theme: 'dark', locale: 'en-US' }
    const saver = createSettingsPatchSaver(draft, async (patch) => {
        patches.push(patch)
        if (patches.length === 1) {
            await pending
            throw new Error('write failed')
        }
    })
    draft.theme = 'light'
    const first = saver(draft)
    draft.locale = 'zh-CN'
    const second = saver(draft)
    draft.theme = 'oled'
    release()
    await expect(first).rejects.toThrow(/write failed/)
    await second
    expect(patches).toEqual([{ theme: 'light' }, { theme: 'light', locale: 'zh-CN' }])
    await saver(draft)
    expect(patches[2]).toEqual({ theme: 'oled' })
})

it('replaces structured configuration when keys are removed and supports clearing values', () => {
    expect(
        diffSettings(
            { home_widgets: { a: 1, b: 2 }, ui_font: 'Font' },
            { home_widgets: { b: 2 }, ui_font: null },
        ),
    ).toEqual({ home_widgets: { b: 2 }, ui_font: null })
})

it('clearing a privileged launch hook survives JSON patch serialization', () => {
    const settings = {
        hooks: { pre_launch: 'old command', wrapper: 'keep wrapper' },
    } as AppSettings
    const before = structuredClone(settings)
    expect(applySettingChanges(settings, [{ key: 'hooks_pre_launch', value: '' }])).toBe(true)
    const patch = JSON.parse(JSON.stringify(diffSettings(before, settings)))
    expect(patch).toEqual({ hooks: { pre_launch: null } })
    expect(settings.hooks.wrapper).toBe('keep wrapper')
})

it('applies a privileged default game language change', () => {
    const settings = {
        default_game_language: '',
    } as AppSettings

    expect(applySettingChanges(settings, [{ key: 'default_game_language', value: 'en_us' }])).toBe(
        true,
    )
    expect(settings.default_game_language).toBe('en_us')
})
