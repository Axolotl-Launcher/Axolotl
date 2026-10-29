import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'

import { analyzeMod } from '@/lab/mod-translation/backend'
import type { ModTranslationAnalysis } from '@/lab/mod-translation/types.ts'
import { useModTranslationStore } from '@/store/modTranslation'

import LabModTranslation from './LabModTranslation.vue'

const handleError = vi.fn()
vi.mock('@/lab/mod-translation/backend', () => ({
    analyzeMod: vi.fn(),
    cancelModTranslation: vi.fn(),
    dismissModTranslationTask: vi.fn(),
    listenToModTranslationTasks: vi.fn(async () => () => {}),
    listModTranslationTasks: vi.fn(async () => []),
    translateMod: vi.fn(),
}))
vi.mock('@tauri-apps/plugin-opener', () => ({ revealItemInDir: vi.fn() }))
vi.mock('@/components/lab/mod-translation/ModTranslationFilePicker.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/lab/mod-translation/ModTranslationJobList.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/lab/mod-translation/ModTranslationSettingsPanel.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@modrinth/ui', () => ({
    Admonition: defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('aside', slots.default?.()),
    }),
    Button: defineComponent({
        setup:
            (_, { attrs, slots }) =>
            () =>
                h('button', attrs, slots.default?.()),
    }),
    defineMessages: (messages: unknown) => messages,
    injectNotificationManager: () => ({ handleError }),
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))

function deferred<T>() {
    let resolve!: (value: T) => void
    let reject!: (error: unknown) => void
    const promise = new Promise<T>((done, fail) => {
        resolve = done
        reject = fail
    })
    return { promise, resolve, reject }
}

const analysisFor = (path: string) =>
    ({
        analysisId: path,
        inputHash: path,
        loader: 'fabric',
        languageEntries: 1,
        languageCharacters: 1,
        classCandidates: [],
        languageSources: [],
        warnings: [],
        signed: false,
    }) as unknown as ModTranslationAnalysis

it('does not attach the analysis or error of one file to the file picked after it', async () => {
    setActivePinia(createPinia())
    const store = useModTranslationStore()
    store.inputPath = 'C:/mods/a.jar'
    const first = deferred<ModTranslationAnalysis>()
    const second = deferred<ModTranslationAnalysis>()
    vi.mocked(analyzeMod).mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)
    const wrapper = mount(LabModTranslation)
    try {
        const analyze = () =>
            wrapper.findAll('button').find((button) => button.text().includes('Analy'))!
        await analyze().trigger('click')
        expect(analyzeMod).toHaveBeenLastCalledWith(
            'C:/mods/a.jar',
            expect.any(AbortSignal),
            expect.any(Function),
        )

        store.inputPath = 'C:/mods/b.jar'
        await flushPromises()
        first.resolve(analysisFor('C:/mods/a.jar'))
        await flushPromises()
        expect(store.analysis).toBeNull()

        await analyze().trigger('click')
        expect(analyzeMod).toHaveBeenLastCalledWith(
            'C:/mods/b.jar',
            expect.any(AbortSignal),
            expect.any(Function),
        )
        const bSignal = vi.mocked(analyzeMod).mock.calls[1][1]!
        store.inputPath = 'C:/mods/c.jar'
        await flushPromises()
        expect(bSignal.aborted).toBe(true)
        second.reject(new Error('b.jar is broken'))
        await flushPromises()
        expect(handleError).not.toHaveBeenCalled()
        expect(store.analysis).toBeNull()
    } finally {
        wrapper.unmount()
    }
})
