import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import type { ImportPlanSnapshot } from '#ui/providers'

import SymlinkMethodCards from './SymlinkMethodCards.vue'

const plans = vi.hoisted(() => ({
    listen: vi.fn(),
    start: vi.fn(),
    cancel: vi.fn(),
    getLoaderVersions: vi.fn(),
}))
vi.mock('#ui/providers', async () => ({
    ...(await import('../../../providers/instance-import')),
    injectTags: () => ({ gameVersions: ref([{ version: '1.21.1' }]) }),
    injectInstanceImport: () => ({
        listenImportPlan: plans.listen,
        startImportPlan: plans.start,
        cancelImportPlan: plans.cancel,
        getLoaderVersions: plans.getLoaderVersions,
    }),
}))
vi.mock('#ui/components/modal/NewModal.vue', () => ({
    default: defineComponent({
        setup: (_, { slots, expose }) => {
            expose({ show: vi.fn(), hide: vi.fn() })
            return () => h('section', [slots.default?.(), slots.actions?.()])
        },
    }),
}))
vi.mock('#ui/composables/i18n', async (importOriginal) => ({
    ...(await importOriginal<object>()),
    defineMessages: (value: unknown) => value,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
}))

beforeEach(() => {
    vi.resetAllMocks()
    plans.cancel.mockResolvedValue(undefined)
    plans.getLoaderVersions.mockResolvedValue(['0.16.9'])
})

afterEach(() => vi.unstubAllGlobals())

it('confirms copying with the detected versions without offering or requiring an import method', async () => {
    let receive!: (snapshot: ImportPlanSnapshot) => void
    plans.listen.mockImplementation(async (callback) => {
        receive = callback
        return vi.fn()
    })
    plans.start.mockImplementation(async (request) => {
        receive({
            requestId: request.requestId,
            stage: 'done',
            gameVersion: '1.21.1',
            loader: 'fabric',
            loaderVersion: '0.16.9',
            importPath: request.instancePath,
            minecraftRoot: 'D:\\Minecraft\\.minecraft',
            modCount: 1,
            cache: { bytes: 0, files: 0 },
            local: { bytes: 0, files: 0 },
            network: { bytes: 0, files: 0 },
            migrate: { bytes: 4, files: 1 },
        })
        return request.requestId
    })
    const wrapper = mount(SymlinkMethodCards, {
        global: {
            directives: { tooltip: {} },
            stubs: { Combobox: true, Chips: true, StyledInput: true },
        },
    })
    try {
        const path = 'D:\\Minecraft\\.minecraft\\versions\\1.21.1'
        wrapper.vm.show({ instances: [{ name: '1.21.1', path, launcherType: 'Generic' }] })
        await flushPromises()
        await vi.waitFor(() => expect(plans.start).toHaveBeenCalledOnce())
        // Scan snapshots are applied on the next animation frame.
        await vi.waitFor(() =>
            expect(
                wrapper
                    .findAll('button')
                    .find((button) => button.text() === 'Confirm')!
                    .attributes('disabled'),
            ).toBeUndefined(),
        )
        expect(wrapper.text()).toContain('Import instances')
        expect(wrapper.text()).not.toContain('Symbolic link')
        expect(wrapper.text()).not.toContain('Import method')
        expect(wrapper.text()).not.toContain('Game directory')
        await wrapper
            .findAll('button')
            .find((button) => button.text() === 'Confirm')!
            .trigger('click')
        expect(wrapper.emitted('confirm')).toEqual([
            [
                [
                    {
                        instanceName: '1.21.1',
                        instancePath: path,
                        symlink: false,
                        gameVersion: '1.21.1',
                        loader: 'fabric',
                        loaderVersion: '0.16.9',
                        gameDirOverride: null,
                    },
                ],
            ],
        ])
    } finally {
        wrapper.unmount()
    }
})

function snapshot(
    requestId: string,
    path: string,
    stage: ImportPlanSnapshot['stage'] = 'done',
): ImportPlanSnapshot {
    return {
        requestId,
        stage,
        gameVersion: '1.21.1',
        loader: 'fabric',
        loaderVersion: '0.16.9',
        importPath: path,
        minecraftRoot: 'root',
        modCount: 1,
        cache: { bytes: 0, files: 0 },
        local: { bytes: 0, files: 0 },
        network: { bytes: 0, files: 0 },
        migrate: { bytes: 4, files: 1 },
    }
}

function holdSnapshots() {
    const frames: FrameRequestCallback[] = []
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) =>
        frames.push(callback),
    )
    return async () => {
        for (const callback of frames.splice(0)) callback(0)
        await flushPromises()
    }
}

function mountCards() {
    return mount(SymlinkMethodCards, {
        global: {
            directives: { tooltip: {} },
            stubs: { Combobox: true, Chips: true, StyledInput: true },
        },
    })
}

it('keeps Confirm locked before the queued snapshot is applied and rejects direct submissions', async () => {
    const applyFrame = holdSnapshots()
    let receive!: (value: ImportPlanSnapshot) => void
    plans.listen.mockImplementation(async (callback) => {
        receive = callback
        return vi.fn()
    })
    plans.start.mockImplementation(async (request) => {
        receive(snapshot(request.requestId, request.instancePath))
        return request.requestId
    })
    const wrapper = mountCards()
    try {
        wrapper.vm.show({ instances: [{ name: 'fabric', path: 'version' }] })
        await flushPromises()
        const confirm = () =>
            wrapper.findAll('button').find((button) => button.text() === 'Confirm')!
        expect(confirm().attributes('disabled')).toBeDefined()
        await confirm().trigger('click')
        ;(wrapper.vm.$.setupState as unknown as { handleConfirm: () => void }).handleConfirm()
        expect(wrapper.emitted('confirm')).toBeUndefined()
        await applyFrame()
        expect(confirm().attributes('disabled')).toBeUndefined()
        await confirm().trigger('click')
        expect(wrapper.emitted('confirm')?.[0][0]).toMatchObject([
            { gameVersion: '1.21.1', loader: 'fabric', loaderVersion: '0.16.9' },
        ])
    } finally {
        wrapper.unmount()
    }
})

it('requires a successful current plan and ignores snapshots from replaced requests', async () => {
    const applyFrame = holdSnapshots()
    let receive!: (value: ImportPlanSnapshot) => void
    plans.listen.mockImplementation(async (callback) => {
        receive = callback
        return vi.fn()
    })
    plans.start.mockImplementation(async (request) => request.requestId)
    const wrapper = mountCards()
    try {
        wrapper.vm.show({ instances: [{ name: 'fabric', path: 'version' }] })
        await flushPromises()
        const first = plans.start.mock.calls[0][0]
        receive(snapshot(first.requestId, 'version', 'error'))
        await applyFrame()
        const confirm = () =>
            wrapper.findAll('button').find((button) => button.text() === 'Confirm')!
        expect(confirm().attributes('disabled')).toBeDefined()
        wrapper.vm.hide()
        wrapper.vm.show({ instances: [{ name: 'fabric', path: 'version' }] })
        await flushPromises()
        const second = plans.start.mock.calls[1][0]
        receive(snapshot(first.requestId, 'version'))
        await applyFrame()
        expect(confirm().attributes('disabled')).toBeDefined()
        receive(snapshot(second.requestId, 'version', 'scanning'))
        await applyFrame()
        expect(confirm().attributes('disabled')).toBeDefined()
        receive(snapshot(second.requestId, 'version'))
        await applyFrame()
        expect(confirm().attributes('disabled')).toBeUndefined()
    } finally {
        wrapper.unmount()
    }
})

it('locks confirmation immediately during the rescan debounce after editing detected versions', async () => {
    const applyFrame = holdSnapshots()
    let receive!: (value: ImportPlanSnapshot) => void
    plans.listen.mockImplementation(async (callback) => {
        receive = callback
        return vi.fn()
    })
    plans.start.mockImplementation(async (request) => {
        receive(snapshot(request.requestId, request.instancePath))
        return request.requestId
    })
    const wrapper = mountCards()
    try {
        wrapper.vm.show({ instances: [{ name: 'fabric', path: 'version' }] })
        await flushPromises()
        await applyFrame()
        wrapper.findAllComponents({ name: 'Combobox' })[0].vm.$emit('update:modelValue', '1.21.2')
        await flushPromises()
        const confirm = wrapper.findAll('button').find((button) => button.text() === 'Confirm')!
        expect(confirm.attributes('disabled')).toBeDefined()
        expect(plans.cancel).toHaveBeenCalled()
        expect(plans.start).toHaveBeenCalledTimes(1)
    } finally {
        wrapper.unmount()
    }
})

it('does not submit a batch until every instance has a ready plan', async () => {
    const applyFrame = holdSnapshots()
    let receive!: (value: ImportPlanSnapshot) => void
    plans.listen.mockImplementation(async (callback) => {
        receive = callback
        return vi.fn()
    })
    plans.start.mockImplementation(async (request) => request.requestId)
    const wrapper = mountCards()
    try {
        wrapper.vm.show({
            instances: [
                { name: 'first', path: 'first' },
                { name: 'second', path: 'second' },
            ],
        })
        await flushPromises()
        const [first, second] = plans.start.mock.calls.map(([request]) => request)
        receive(snapshot(second.requestId, 'second'))
        await applyFrame()
        await wrapper
            .findAll('button')
            .find(
                (button) =>
                    button.attributes('aria-label') === 'Switch to {name}' &&
                    button.attributes('data-active') === 'false',
            )!
            .trigger('click')
        await vi.waitFor(() =>
            expect(
                wrapper.findAll('button').find((button) => button.text() === 'Confirm'),
            ).toBeDefined(),
        )
        const confirm = () =>
            wrapper.findAll('button').find((button) => button.text() === 'Confirm')!
        expect(confirm().attributes('disabled')).toBeDefined()
        receive(snapshot(first.requestId, 'first'))
        await applyFrame()
        await vi.waitFor(() => expect(confirm().attributes('disabled')).toBeUndefined())
        await confirm().trigger('click')
        const choices = wrapper.emitted('confirm')?.[0][0] as {
            loader: string
            loaderVersion: string
        }[]
        expect(choices).toHaveLength(2)
        expect(
            choices.every(
                (choice) => choice.loader === 'fabric' && choice.loaderVersion === '0.16.9',
            ),
        ).toBe(true)
    } finally {
        wrapper.unmount()
    }
})

it('keeps each instance rescan scheduled when edits span a page change', async () => {
    const applyFrame = holdSnapshots()
    let receive!: (value: ImportPlanSnapshot) => void
    plans.listen.mockImplementation(async (callback) => {
        receive = callback
        return vi.fn()
    })
    plans.start.mockImplementation(async (request) => {
        receive(snapshot(request.requestId, request.instancePath))
        return request.requestId
    })
    const wrapper = mountCards()
    try {
        wrapper.vm.show({
            instances: [
                { name: 'first', path: 'first' },
                { name: 'second', path: 'second' },
            ],
        })
        await flushPromises()
        await applyFrame()
        wrapper.findAllComponents({ name: 'Combobox' })[0].vm.$emit('update:modelValue', '1.21.2')
        await flushPromises()
        await wrapper
            .findAll('button')
            .find((button) => button.attributes('data-active') === 'false')!
            .trigger('click')
        await vi.waitFor(() =>
            expect(
                wrapper.findAll('button').find((button) => button.text() === 'Confirm'),
            ).toBeDefined(),
        )
        wrapper.findAllComponents({ name: 'Combobox' })[0].vm.$emit('update:modelValue', '1.21.3')
        await flushPromises()
        await vi.waitFor(() => expect(plans.start).toHaveBeenCalledTimes(4))
        expect(
            plans.start.mock.calls
                .slice(2)
                .map(([request]) => request.instancePath)
                .sort(),
        ).toEqual(['first', 'second'])
        await applyFrame()
        const confirm = () =>
            wrapper.findAll('button').find((button) => button.text() === 'Confirm')!
        await vi.waitFor(() => expect(confirm().attributes('disabled')).toBeUndefined())
        await confirm().trigger('click')
        expect(
            (wrapper.emitted('confirm')?.[0][0] as { gameVersion: string }[]).map(
                (choice) => choice.gameVersion,
            ),
        ).toEqual(['1.21.2', '1.21.3'])
    } finally {
        wrapper.unmount()
    }
})

it('releases a listener that arrives after replacement without detaching the current listener', async () => {
    const oldRelease = vi.fn()
    const currentRelease = vi.fn()
    let finishOld!: (release: () => void) => void
    plans.listen
        .mockImplementationOnce(
            () =>
                new Promise((resolve) => {
                    finishOld = resolve
                }),
        )
        .mockResolvedValue(currentRelease)
    plans.start.mockImplementation(async (request) => request.requestId)
    const wrapper = mountCards()
    try {
        wrapper.vm.show({ instances: [{ name: 'current', path: 'current' }] })
        await flushPromises()
        expect(plans.start).toHaveBeenCalledTimes(1)
        finishOld(oldRelease)
        await flushPromises()
        expect(oldRelease).toHaveBeenCalledOnce()
        expect(currentRelease).not.toHaveBeenCalled()
        wrapper.vm.hide()
        expect(currentRelease).toHaveBeenCalledOnce()
    } finally {
        wrapper.unmount()
    }
})

it('does not resume the old batch after a different import view has opened', async () => {
    const applyFrame = holdSnapshots()
    let finishOld!: (id: string) => void
    let receive!: (value: ImportPlanSnapshot) => void
    plans.listen.mockImplementation(async (callback) => {
        receive = callback
        return vi.fn()
    })
    plans.start
        .mockImplementationOnce(
            () =>
                new Promise((resolve) => {
                    finishOld = resolve
                }),
        )
        .mockImplementation(async (request) => {
            receive(snapshot(request.requestId, request.instancePath))
            return request.requestId
        })
    const wrapper = mountCards()
    try {
        wrapper.vm.show({
            instances: [
                { name: 'old-first', path: 'old-first' },
                { name: 'old-second', path: 'old-second' },
            ],
        })
        await flushPromises()
        const old = plans.start.mock.calls[0][0]
        wrapper.vm.hide()
        wrapper.vm.show({ instances: [{ name: 'current', path: 'current' }] })
        await flushPromises()
        finishOld(old.requestId)
        await flushPromises()
        await applyFrame()
        expect(plans.start).toHaveBeenCalledTimes(2)
        expect(plans.cancel).toHaveBeenCalledWith(old.requestId)
        const confirm = wrapper.findAll('button').find((button) => button.text() === 'Confirm')!
        expect(confirm.attributes('disabled')).toBeUndefined()
        await confirm.trigger('click')
        expect(wrapper.emitted('confirm')?.[0][0]).toMatchObject([
            { instanceName: 'current', loader: 'fabric' },
        ])
    } finally {
        wrapper.unmount()
    }
})

it('does not start a native plan when its snapshot listener failed to register', async () => {
    plans.listen.mockRejectedValue(new Error('listener unavailable'))
    const wrapper = mountCards()
    try {
        wrapper.vm.show({ instances: [{ name: 'current', path: 'current' }] })
        await flushPromises()
        expect(plans.start).not.toHaveBeenCalled()
        expect(wrapper.text()).toContain('listener unavailable')
        expect(
            wrapper
                .findAll('button')
                .find((button) => button.text() === 'Confirm')!
                .attributes('disabled'),
        ).toBeDefined()
    } finally {
        wrapper.unmount()
    }
})
