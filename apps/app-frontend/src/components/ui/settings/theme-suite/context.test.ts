import { beforeEach, describe, expect, it, vi } from 'vitest'
import { shallowRef } from 'vue'

import { createThemeSuiteContext } from './context'

const mocks = vi.hoisted(() => ({
    getSettings: vi.fn(),
    updateSettings: vi.fn(),
    listThemePacks: vi.fn(),
    getThemePackLimits: vi.fn(),
    installThemePack: vi.fn(),
    removeThemePack: vi.fn(),
    notifiable: vi.fn(),
    applyThemeRuntime: vi.fn(),
    collectCurrentAppearanceSnapshot: vi.fn(),
    applyCustomThemeColors: vi.fn(),
    save: vi.fn(),
}))

vi.mock('@modrinth/ui', () => ({
    createContext: () => [vi.fn(), vi.fn()],
    defineMessages: (messages: unknown) => messages,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage?: string }) => message.defaultMessage ?? '',
        locale: { value: 'en-US' },
    }),
}))
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: mocks.save }))
vi.mock('@/helpers/settings', () => ({
    get: mocks.getSettings,
    update: mocks.updateSettings,
}))
vi.mock('@/helpers/theme-packs', () => ({
    listThemePacks: mocks.listThemePacks,
    getThemePackLimits: mocks.getThemePackLimits,
    installThemePack: mocks.installThemePack,
    removeThemePack: mocks.removeThemePack,
    notifyThemeSourceChanged: mocks.notifiable,
}))
vi.mock('@/helpers/theme-pack-runtime', () => ({
    applyThemeRuntime: mocks.applyThemeRuntime,
    collectCurrentAppearanceSnapshot: mocks.collectCurrentAppearanceSnapshot,
}))
vi.mock('@/store/theme', () => ({ applyCustomThemeColors: mocks.applyCustomThemeColors }))

function settings(source: string) {
    return {
        theme: 'dark',
        theme_source: source,
        custom_theme_colors: null,
        ui_font: null,
        accent_color: null,
    }
}

function deferred<T>() {
    let resolve!: (value: T) => void
    const promise = new Promise<T>((done) => {
        resolve = done
    })
    return { promise, resolve }
}

beforeEach(() => {
    vi.clearAllMocks()
    mocks.getThemePackLimits.mockResolvedValue({ archiveBytes: 1, unpackedBytes: 1, entries: 1 })
    mocks.listThemePacks.mockResolvedValue([])
    mocks.save.mockResolvedValue(null)
})

describe('theme suite refresh', () => {
    it('does not let a refresh that started earlier overwrite a new selection', async () => {
        const stale = deferred<ReturnType<typeof settings>>()
        mocks.getSettings.mockReturnValueOnce(stale.promise)
        mocks.updateSettings.mockResolvedValue(settings('pack:chosen'))

        const context = createThemeSuiteContext(shallowRef(null))
        const refresh = context.refresh()
        await context.selectSource('pack:chosen')

        stale.resolve(settings('builtin'))
        await refresh

        expect(context.settings.value?.theme_source).toBe('pack:chosen')
        expect(context.loading.value).toBe(false)
    })

    it('does not let a refresh that started during a selection overwrite it', async () => {
        const stale = deferred<ReturnType<typeof settings>>()
        const selection = deferred<ReturnType<typeof settings>>()
        mocks.getSettings.mockReturnValueOnce(stale.promise)
        mocks.updateSettings.mockReturnValueOnce(selection.promise)

        const context = createThemeSuiteContext(shallowRef(null))
        const selecting = context.selectSource('pack:chosen')
        const refresh = context.refresh()

        selection.resolve(settings('pack:chosen'))
        await selecting

        stale.resolve(settings('builtin'))
        await refresh

        expect(context.settings.value?.theme_source).toBe('pack:chosen')
        expect(context.loading.value).toBe(false)
    })

    it('applies the newest refresh result', async () => {
        mocks.getSettings.mockResolvedValue(settings('pack:chosen'))

        const context = createThemeSuiteContext(shallowRef(null))
        await context.refresh()

        expect(context.settings.value?.theme_source).toBe('pack:chosen')
        expect(context.selectedPackId.value).toBe('chosen')
        expect(context.loading.value).toBe(false)
    })
})
