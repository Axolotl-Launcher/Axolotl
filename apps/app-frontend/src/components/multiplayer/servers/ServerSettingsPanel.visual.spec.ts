import '@modrinth/ui/directives/tooltip-styles.ts'

import { I18N_INJECTION_KEY } from '@modrinth/ui'
import { tooltipDirective } from '@modrinth/ui/directives/tooltip.ts'
import { afterEach, expect, it, vi } from 'vitest'
import { ref } from 'vue'

import type { ServerView } from '@/composables/useServers'
import { mountThemed } from '@/test/visual-harness'

import ServerSettingsPanel from './ServerSettingsPanel.vue'

const api = vi.hoisted(() => ({
    update: vi.fn().mockResolvedValue({}),
    refresh: vi.fn(),
    error: vi.fn(),
}))
vi.mock('@/helpers/servers', () => ({ servers: { updateSettings: api.update } }))
vi.mock('@/helpers/jre', () => ({ get_jre: async () => null }))
vi.mock('@/composables/useServers', () => ({
    useServers: () => ({ deleteServer: vi.fn(), refresh: api.refresh }),
}))
vi.mock('@/components/ui/JavaSelector.vue', () => ({ default: { render: () => null } }))
vi.mock('./ServerPropertiesEditor.vue', () => ({
    default: {
        setup: (_: unknown, { expose }: { expose: (value: unknown) => void }) => {
            expose({ isDirty: false, save: async () => true, cancel: vi.fn() })
            return () => null
        },
    },
}))

let wrapper: Awaited<ReturnType<typeof mountThemed>> | undefined
afterEach(() => {
    wrapper?.unmount()
    wrapper = undefined
    document.body.replaceChildren()
    vi.clearAllMocks()
    api.update.mockReset().mockResolvedValue({})
})
async function settings(running = false) {
    const teleports = document.createElement('div')
    teleports.id = 'teleports'
    document.body.append(teleports)
    wrapper = await mountThemed(
        ServerSettingsPanel,
        {
            server: {
                id: 'settings',
                name: 'Server',
                serverType: 'fabric',
                gameVersion: '1.21.1',
                jvmArgs: [],
                running,
            } as unknown as ServerView,
        },
        'dark',
        {
            global: {
                directives: { tooltip: tooltipDirective },
                provide: {
                    [I18N_INJECTION_KEY as symbol]: {
                        locale: ref('en-US'),
                        t: (key: string) => key,
                        setLocale: () => {},
                    },
                    [Symbol.for('modrinth:notificationManager')]: {
                        handleError: api.error,
                        addNotification: vi.fn(),
                    },
                    [Symbol.for('modrinth:FilePickerContext')]: {},
                },
            },
        },
    )
    return wrapper
}
function button(label: string) {
    return wrapper!.findAll('button').find((item) => item.text() === label)!
}

it('defaults older servers to stop, saves a custom command, and cancels uncommitted changes', async () => {
    await settings()
    const field = wrapper!.get<HTMLInputElement>('#server-settings-stop-command')
    expect(field.element.value).toBe('stop')
    await field.setValue('shutdown now')
    await button('Save changes').trigger('click')
    await vi.waitFor(() =>
        expect(api.update).toHaveBeenCalledWith(
            'settings',
            expect.objectContaining({ stopCommand: 'shutdown now' }),
        ),
    )
    await field.setValue('another command')
    await button('Cancel').trigger('click')
    expect(field.element.value).toBe('shutdown now')
    await field.setValue('')
    await button('Save changes').trigger('click')
    await vi.waitFor(() =>
        expect(api.update).toHaveBeenLastCalledWith(
            'settings',
            expect.objectContaining({ stopCommand: 'stop' }),
        ),
    )
})

it('allows updating only the stop command while running and preserves input on failure', async () => {
    await settings(true)
    const field = wrapper!.get<HTMLInputElement>('#server-settings-stop-command')
    await field.setValue(' ^c ')
    await button('Save changes').trigger('click')
    await vi.waitFor(() =>
        expect(api.update).toHaveBeenCalledWith('settings', { stopCommand: '^C' }),
    )
    await field.setValue('halt')
    api.update.mockRejectedValueOnce(new Error('save failed'))
    await button('Save changes').trigger('click')
    await vi.waitFor(() => expect(api.error).toHaveBeenCalledOnce())
    expect(field.element.value).toBe('halt')
})

it('locks the submitted stop command until saving finishes', async () => {
    await settings(true)
    const field = wrapper!.get<HTMLInputElement>('#server-settings-stop-command')
    let finish!: () => void
    api.update.mockImplementationOnce(
        () =>
            new Promise<void>((resolve) => {
                finish = resolve
            }),
    )
    await field.setValue('^c')
    await button('Save changes').trigger('click')
    expect(field.element.disabled).toBe(true)
    finish()
    await vi.waitFor(() => expect(field.element.disabled).toBe(false))
    expect(field.element.value).toBe('^C')
})
