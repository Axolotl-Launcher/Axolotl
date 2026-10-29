import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
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
        await new Promise(requestAnimationFrame)
        await flushPromises()
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
