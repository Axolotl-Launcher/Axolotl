import { I18N_INJECTION_KEY } from '@modrinth/ui'
import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { ref } from 'vue'

import { applyTheme } from '@/test/visual-harness'

import ModpackInstallModal from './ModpackInstallModal.vue'

const native = vi.hoisted(() => ({ join: vi.fn(), handleError: vi.fn() }))
vi.mock('@tauri-apps/api/path', () => ({ join: native.join }))
vi.mock('@/helpers/utils', () => ({ releaseColor: () => 'brand' }))
vi.mock('@modrinth/ui', async () => {
    const [
        button,
        modal,
        i18n,
        messages,
        provider,
        avatar,
        badge,
        combobox,
        input,
        directory,
        directories,
    ] = await Promise.all([
        import('@modrinth/ui/src/components/base/buttons/Button.vue'),
        import('@modrinth/ui/src/components/modal/NewModal.vue'),
        import('@modrinth/ui/src/composables/i18n'),
        import('@modrinth/ui/src/utils/common-messages'),
        import('@modrinth/ui/src/providers/i18n'),
        import('@modrinth/ui/src/components/base/Avatar.vue'),
        import('@modrinth/ui/src/components/base/Badge.vue'),
        import('@modrinth/ui/src/components/base/Combobox.vue'),
        import('@modrinth/ui/src/components/base/StyledInput.vue'),
        import('@modrinth/ui/src/components/flows/creation-flow-modal/components/GameDirectoryPicker.vue'),
        import('@modrinth/ui/src/utils/minecraft-directories'),
    ])
    return {
        Button: button.default,
        NewModal: modal.default,
        Avatar: avatar.default,
        Badge: badge.default,
        Combobox: combobox.default,
        StyledInput: input.default,
        GameDirectoryPicker: directory.default,
        defineMessages: i18n.defineMessages,
        useVIntl: i18n.useVIntl,
        commonMessages: messages.commonMessages,
        I18N_INJECTION_KEY: provider.I18N_INJECTION_KEY,
        BUILTIN_GAME_DIRECTORY: directories.BUILTIN_GAME_DIRECTORY,
        loadMinecraftDirectories: directories.loadMinecraftDirectories,
        injectNotificationManager: () => ({ handleError: native.handleError }),
    }
})

const data = {
    project: { id: 'pack', title: 'Example Pack', icon_url: null },
    versions: [
        {
            id: 'release',
            name: 'Release',
            version_number: '1.0',
            game_versions: ['1.20.1'],
            loaders: ['fabric'],
            version_type: 'release',
        },
    ],
    existingInstancesByVersion: {},
    instancePaths: [],
} as unknown as Parameters<InstanceType<typeof ModpackInstallModal>['show']>[0]
const mounted: ReturnType<typeof mount>[] = []
afterEach(() => {
    mounted.splice(0).forEach((wrapper) => wrapper.unmount())
    document.getElementById('teleports')?.remove()
    localStorage.removeItem('axolotl-minecraft-directories')
    vi.clearAllMocks()
})
function setup(directories: unknown[] = []) {
    applyTheme('dark')
    const teleports = document.createElement('div')
    teleports.id = 'teleports'
    document.body.appendChild(teleports)
    localStorage.setItem('axolotl-minecraft-directories', JSON.stringify(directories))
    native.join.mockImplementation(async (...parts: string[]) => parts.join('/'))
    const wrapper = mount(ModpackInstallModal, {
        attachTo: document.body,
        global: {
            provide: {
                [I18N_INJECTION_KEY as symbol]: {
                    locale: ref('en-US'),
                    t: (key: string) => key,
                    setLocale: () => {},
                },
            },
            directives: { tooltip: () => {} },
        },
    })
    mounted.push(wrapper)
    wrapper.vm.show(data)
    return wrapper
}
function button(text: string) {
    const result = [...document.querySelectorAll('button')].find(
        (node) => node.textContent?.trim() === text,
    )
    expect(result).toBeDefined()
    return result!
}

it('hides the directory block without configured roots and submits the managed default', async () => {
    const wrapper = setup()
    await flushPromises()
    expect(document.body.textContent).not.toContain('Game directory')
    button('Install').click()
    await flushPromises()
    expect(wrapper.emitted('install')).toEqual([['release', 'Example Pack', null]])
    expect(native.join).not.toHaveBeenCalled()
})

it.each(['isolated', 'automatic', 'shared'])(
    'uses the configured %s root and reloads settings on every opening',
    async (mode) => {
        const root = '/games/.minecraft'
        const wrapper = setup([{ path: root, mode }])
        await flushPromises()
        expect(document.body.textContent).toContain('Game directory')
        expect(document.body.textContent).toContain('Axolotl directory')
        button(root).click()
        const input = document.querySelector('input')!
        input.value = '../Pack'
        input.dispatchEvent(new Event('input', { bubbles: true }))
        await flushPromises()
        button('Install').click()
        await flushPromises()
        expect(wrapper.emitted('install')).toEqual([
            ['release', '../Pack', mode === 'shared' ? root : root + '/versions/.._Pack'],
        ])
        localStorage.setItem('axolotl-minecraft-directories', '[]')
        wrapper.vm.show(data)
        await flushPromises()
        expect(document.body.textContent).not.toContain('Game directory')
    },
)

it('keeps form values after path resolution fails and permits retry', async () => {
    const wrapper = setup(['/games/.minecraft'])
    await flushPromises()
    button('/games/.minecraft').click()
    const error = new Error('path unavailable')
    native.join.mockRejectedValueOnce(error)
    button('Install').click()
    await flushPromises()
    expect(native.handleError).toHaveBeenCalledWith(error)
    expect(wrapper.emitted('install')).toBeUndefined()
    expect(document.querySelector('input')!.value).toBe('Example Pack')
    button('Install').click()
    await flushPromises()
    expect(wrapper.emitted('install')).toHaveLength(1)
})

it('does not let an older path resolution install or close a reopened modal', async () => {
    const wrapper = setup(['/games/.minecraft'])
    await flushPromises()
    button('/games/.minecraft').click()
    let resolve!: (path: string) => void
    native.join.mockReturnValueOnce(
        new Promise<string>((done) => {
            resolve = done
        }),
    )
    button('Install').click()
    await flushPromises()
    wrapper.vm.hide()
    wrapper.vm.show({ ...data, project: { ...data.project, title: 'New Pack' } })
    await flushPromises()
    resolve('/games/.minecraft/versions/Example Pack')
    await flushPromises()
    expect(wrapper.emitted('install')).toBeUndefined()
    expect(document.querySelector('input')!.value).toBe('New Pack')
    button('Install').click()
    await flushPromises()
    expect(wrapper.emitted('install')).toEqual([['release', 'New Pack', null]])
})

it.each(['.', '..'])('does not install the unsafe instance name %s', async (name) => {
    const wrapper = setup(['/games/.minecraft'])
    await flushPromises()
    const input = document.querySelector('input')!
    input.value = name
    input.dispatchEvent(new Event('input', { bubbles: true }))
    await flushPromises()
    expect(button('Install').disabled).toBe(true)
    button('Install').click()
    await flushPromises()
    expect(wrapper.emitted('install')).toBeUndefined()
})
