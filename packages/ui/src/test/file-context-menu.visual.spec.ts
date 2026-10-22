import { afterEach, expect, it, vi } from 'vitest'
import { ref } from 'vue'

import FileContextMenu from '../layouts/shared/files-tab/components/FileContextMenu.vue'
import { mountThemed, waitFor } from './visual-harness'

const cleanup: (() => void)[] = []
afterEach(() => {
    cleanup
        .splice(0)
        .reverse()
        .forEach((fn) => fn())
    vi.restoreAllMocks()
})

it('uses the current file for common actions and custom slots in the shared point menu', async () => {
    const copy = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue()
    const notify = vi.fn()
    const custom = vi.fn()
    const wrapper = await mountThemed(FileContextMenu, {}, 'dark', {
        global: {
            provide: {
                [Symbol.for('modrinth:fileManagerContext')]: {
                    basePath: ref('/instance'),
                    openInFolder: vi.fn(),
                },
                [Symbol.for('modrinth:notificationManager')]: { addNotification: notify },
            },
        },
        slots: { custom: '<span>Custom action</span>' },
    })
    cleanup.push(() => wrapper.unmount())
    const file = {
        name: 'options.txt',
        type: 'file' as const,
        path: 'config/options.txt',
        modified: 0,
        created: 0,
    }
    wrapper.vm.show(file, innerWidth - 2, innerHeight - 2, [{ id: 'custom', action: custom }])
    await waitFor(() => !!document.querySelector('[role="menu"]'))
    const buttons = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')]
    expect(buttons.at(-1)?.textContent).toContain('Custom action')
    buttons[1].click()
    await waitFor(() => !document.querySelector('[role="menu"]'))
    expect(copy).toHaveBeenCalledWith('/instance/config/options.txt')
    expect(notify).toHaveBeenCalledOnce()
    wrapper.vm.show(file, 20, 20, [{ id: 'custom', action: custom }])
    await waitFor(() => !!document.querySelector('[role="menu"]'))
    ;[...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].at(-1)!.click()
    await waitFor(() => !document.querySelector('[role="menu"]'))
    expect(custom).toHaveBeenCalledOnce()
})
