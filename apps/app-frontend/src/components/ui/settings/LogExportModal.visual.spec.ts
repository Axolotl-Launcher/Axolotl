import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'

import LogExportModal from './LogExportModal.vue'

const calls = vi.hoisted(() => ({
    invoke: vi.fn(),
    save: vi.fn(),
    hide: vi.fn(),
    popup: vi.fn(),
    handleError: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ invoke: calls.invoke }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: calls.save }))
vi.mock('@/helpers/instance', () => ({ get_full_path: vi.fn(), get_mod_full_path: vi.fn() }))
vi.mock('@modrinth/ui', async () => ({
    Button: (await import('../../../../../../packages/ui/src/components/base/buttons/Button.vue'))
        .default,
    Checkbox: (await import('../../../../../../packages/ui/src/components/base/Checkbox.vue'))
        .default,
    Combobox: (await import('../../../../../../packages/ui/src/components/base/Combobox.vue'))
        .default,
    NewModal: defineComponent({
        name: 'ExportModalStub',
        props: { closable: Boolean, closeOnClickOutside: Boolean },
        setup(_, { slots, expose }) {
            expose({ show: vi.fn(), hide: calls.hide })
            return () => h('div', [slots.default?.(), slots.actions?.()])
        },
    }),
    defineMessages: (messages: unknown) => messages,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
    }),
    commonMessages: { openInFolderButton: { defaultMessage: 'Open in folder' } },
    injectNotificationManager: () => ({ handleError: calls.handleError }),
    injectPopupNotificationManager: () => ({ addPopupNotification: calls.popup }),
}))

const wrappers: ReturnType<typeof mount>[] = []
afterEach(() => {
    wrappers.splice(0).forEach((wrapper) => wrapper.unmount())
    vi.resetAllMocks()
})

function render() {
    if (!document.querySelector('#teleports')) {
        const teleports = document.createElement('div')
        teleports.id = 'teleports'
        document.body.append(teleports)
    }
    calls.save.mockResolvedValue('C:/logs.zip')
    calls.invoke.mockResolvedValue(undefined)
    const wrapper = mount(LogExportModal)
    wrappers.push(wrapper)
    return wrapper
}

function exportButton(wrapper: ReturnType<typeof mount>) {
    return wrapper.findAll('button').find((button) => button.text() === 'Export')!
}

it('leaves network tests off by default and forwards that choice to the native export', async () => {
    const wrapper = render()
    expect(wrapper.get('[aria-label="Network connections"]').attributes('aria-checked')).toBe(
        'false',
    )
    await exportButton(wrapper).trigger('click')
    await flushPromises()
    expect(calls.invoke).toHaveBeenCalledExactlyOnceWith('plugin:utils|export_launcher_logs', {
        outputPath: 'C:/logs.zip',
        range: 'last2Hours',
        level: 'all',
        includeSystemInfo: true,
        includeInstanceLogs: false,
        includeCrashAnalysis: false,
        includeNetworkConnections: false,
    })
})

it('requests fresh network tests and stays busy until the native export finishes', async () => {
    const wrapper = render()
    let finish!: () => void
    calls.invoke.mockImplementation(() => new Promise<void>((resolve) => (finish = resolve)))
    await wrapper.get('[aria-label="Network connections"]').trigger('click')
    const button = exportButton(wrapper)
    await button.trigger('click')
    await flushPromises()

    expect(calls.invoke.mock.calls[0][1].includeNetworkConnections).toBe(true)
    expect(button.attributes('aria-busy')).toBe('true')
    expect(wrapper.text()).toContain('Exporting…')
    expect(wrapper.getComponent({ name: 'ExportModalStub' }).props('closable')).toBe(false)
    expect(wrapper.getComponent({ name: 'ExportModalStub' }).props('closeOnClickOutside')).toBe(
        false,
    )
    expect(
        wrapper
            .findAll('[role="checkbox"]')
            .every((checkbox) => checkbox.attributes('disabled') !== undefined),
    ).toBe(true)
    expect(calls.hide).not.toHaveBeenCalled()
    await button.trigger('click')
    expect(calls.invoke).toHaveBeenCalledTimes(1)

    finish()
    await flushPromises()
    expect(calls.hide).toHaveBeenCalledOnce()
    expect(calls.popup).toHaveBeenCalledOnce()
    expect(button.attributes('aria-busy')).toBeUndefined()
})

it('keeps the network option selected after failure so the export can be retried', async () => {
    const wrapper = render()
    await wrapper.get('[aria-label="Network connections"]').trigger('click')
    calls.invoke.mockRejectedValueOnce(new Error('Export failed'))
    await exportButton(wrapper).trigger('click')
    await flushPromises()
    expect(calls.handleError).toHaveBeenCalledOnce()
    expect(calls.hide).not.toHaveBeenCalled()
    expect(wrapper.get('[aria-label="Network connections"]').attributes('aria-checked')).toBe(
        'true',
    )
    await exportButton(wrapper).trigger('click')
    await flushPromises()
    expect(calls.invoke).toHaveBeenCalledTimes(2)
    expect(calls.hide).toHaveBeenCalledOnce()
})

it('does not start network tests or export when the save dialog is cancelled', async () => {
    const wrapper = render()
    await wrapper.get('[aria-label="Network connections"]').trigger('click')
    calls.save.mockResolvedValue(null)
    await exportButton(wrapper).trigger('click')
    await flushPromises()
    expect(calls.invoke).not.toHaveBeenCalled()
    expect(calls.hide).not.toHaveBeenCalled()
    expect(exportButton(wrapper).attributes('aria-busy')).toBeUndefined()
})
