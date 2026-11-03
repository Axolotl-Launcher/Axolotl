import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { type ComputedRef, defineComponent, h, type Ref, Suspense } from 'vue'

import Logs from './Logs.vue'

type Manager = {
    loading: ComputedRef<boolean>
    shareDisabled: ComputedRef<boolean>
    activeLogSourceIndex: Ref<number>
}
const mocks = vi.hoisted(() => ({
    manager: null as Manager | null,
    getHistoricalLogs: vi.fn(),
    getOutput: vi.fn(),
    clear: vi.fn(),
    append: vi.fn(async () => {}),
    processListener: null as ((event: { instance_id: string; event: string }) => void) | null,
}))

vi.mock('vue-router', () => ({ useRoute: () => ({ params: { id: 'instance' } }) }))
vi.mock('@/composables/useCrashAnalysis', () => ({
    useCrashAnalysis: () => ({
        analysis: null,
        loading: false,
        refresh: vi.fn(async () => {}),
        clear: vi.fn(),
    }),
}))
vi.mock('@/composables/useInstanceConsole', async () => {
    const { shallowRef } = await import('vue')
    const state = () => ({
        output: shallowRef([]),
        clear: mocks.clear,
        addLegacyLog: mocks.append,
    })
    const live = state()
    const historical = state()
    return {
        useInstanceConsole: () => ({
            liveConsole: live,
            historicalConsole: historical,
            hydrate: async () => {},
            getHistoricalLogs: mocks.getHistoricalLogs,
            getHistoricalContent: () => undefined,
            invalidate: vi.fn(),
            clearLive: vi.fn(),
        }),
    }
})
vi.mock('@/helpers/events.js', () => ({
    log_listener: async () => () => {},
    process_listener: async (listener: (event: { instance_id: string; event: string }) => void) => {
        mocks.processListener = listener
        return () => {}
    },
}))
vi.mock('@/helpers/logs.js', () => ({
    delete_logs_by_filename: vi.fn(),
    export_crash_context: vi.fn(),
    get_output_by_filename: mocks.getOutput,
}))
vi.mock('@modrinth/ui', () => ({
    ConsolePageLayout: { render: () => null },
    defineMessages: (value: unknown) => value,
    injectNotificationManager: () => ({ handleError: vi.fn() }),
    provideConsoleManager: (manager: Manager) => {
        mocks.manager = manager
    },
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))

function deferred<T>() {
    let resolve!: (value: T) => void
    const promise = new Promise<T>((done) => {
        resolve = done
    })
    return { promise, resolve }
}
const log = (filename: string) => ({ filename, log_type: 'InfoLog', output: null })

async function mountLogs() {
    const wrapper = mount(
        defineComponent({
            setup: () => () =>
                h(Suspense, {}, { default: () => h(Logs, { instance: { id: 'instance' } }) }),
        }),
    )
    await flushPromises()
    return wrapper
}

it('lets the log list and the selected log finish their own loading when they interleave', async () => {
    vi.clearAllMocks()
    mocks.getHistoricalLogs.mockResolvedValueOnce([log('a.log'), log('b.log')])
    const wrapper = await mountLogs()
    try {
        const manager = mocks.manager!
        const text = deferred<string>()
        mocks.getOutput.mockReturnValueOnce(text.promise)
        manager.activeLogSourceIndex.value = 1
        await flushPromises()
        expect(manager.loading.value).toBe(true)

        const list = deferred<unknown[]>()
        mocks.getHistoricalLogs.mockReturnValueOnce(list.promise)
        mocks.processListener?.({ instance_id: 'instance', event: 'finished' })
        await flushPromises()

        text.resolve('first line')
        await flushPromises()
        expect(mocks.append).toHaveBeenCalledWith('first line')
        expect(manager.loading.value).toBe(true)

        list.resolve([log('a.log'), log('b.log')])
        await flushPromises()
        expect(manager.loading.value).toBe(false)
    } finally {
        wrapper.unmount()
    }
})

it('ends the pending log request when switching back to the live log', async () => {
    vi.clearAllMocks()
    mocks.getHistoricalLogs.mockResolvedValueOnce([log('a.log')])
    const wrapper = await mountLogs()
    try {
        const manager = mocks.manager!
        const text = deferred<string>()
        mocks.getOutput.mockReturnValueOnce(text.promise)
        manager.activeLogSourceIndex.value = 1
        await flushPromises()
        expect(manager.loading.value).toBe(true)
        expect(mocks.clear).not.toHaveBeenCalled()
        manager.activeLogSourceIndex.value = 0
        await flushPromises()
        text.resolve('late text')
        await flushPromises()
        expect(mocks.append).not.toHaveBeenCalled()
        expect(mocks.clear).not.toHaveBeenCalled()
        expect(manager.loading.value).toBe(false)
    } finally {
        wrapper.unmount()
    }
})

it('does not offer to share the previous log while the selected one is loading', async () => {
    vi.clearAllMocks()
    mocks.getHistoricalLogs.mockResolvedValueOnce([log('a.log'), log('b.log')])
    const wrapper = await mountLogs()
    try {
        const manager = mocks.manager!
        mocks.getOutput.mockResolvedValueOnce('first log')
        manager.activeLogSourceIndex.value = 1
        await flushPromises()
        expect(manager.shareDisabled.value).toBe(false)
        const text = deferred<string>()
        mocks.getOutput.mockReturnValueOnce(text.promise)
        manager.activeLogSourceIndex.value = 2
        await flushPromises()
        expect(manager.shareDisabled.value).toBe(true)
        text.resolve('second log')
        await flushPromises()
        expect(manager.shareDisabled.value).toBe(false)
    } finally {
        wrapper.unmount()
    }
})

it('loads the text of the log that a list refresh puts at the selected position', async () => {
    vi.clearAllMocks()
    mocks.getHistoricalLogs.mockResolvedValueOnce([log('a.log'), log('b.log')])
    const wrapper = await mountLogs()
    try {
        const manager = mocks.manager!
        mocks.getOutput.mockResolvedValueOnce('a text')
        manager.activeLogSourceIndex.value = 1
        await flushPromises()
        expect(mocks.getOutput).toHaveBeenLastCalledWith('instance', 'InfoLog', 'a.log')
        mocks.getHistoricalLogs.mockResolvedValueOnce([log('b.log')])
        mocks.getOutput.mockResolvedValueOnce('b text')
        mocks.processListener?.({ instance_id: 'instance', event: 'finished' })
        await flushPromises()
        expect(mocks.getOutput).toHaveBeenLastCalledWith('instance', 'InfoLog', 'b.log')
        expect(mocks.append).toHaveBeenLastCalledWith('b text')
    } finally {
        wrapper.unmount()
    }
})
