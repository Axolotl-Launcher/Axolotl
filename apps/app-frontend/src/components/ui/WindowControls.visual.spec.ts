import { provideI18n } from '@modrinth/ui'
import { createPinia } from 'pinia'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import { mountThemed, waitFor } from '@/test/visual-harness'

import WindowControls from './WindowControls.vue'

const native = vi.hoisted(() => ({ close: vi.fn(async () => {}) }))

vi.mock('@tauri-apps/api/window', () => ({
    getCurrentWindow: () => ({
        close: native.close,
        setDecorations: async () => {},
        isMaximized: async () => false,
        onResized: async () => () => {},
    }),
}))
vi.mock('@tauri-apps/plugin-window-state', () => ({
    saveWindowState: () => new Promise(() => {}),
    StateFlags: { ALL: 63 },
}))
vi.mock('@/helpers/settings.ts', () => ({ get: async () => ({ native_decorations: false }) }))
vi.mock('@/helpers/utils.js', () => ({ getOS: async () => 'Windows' }))

it('requests native close without waiting for a window-state IPC', async () => {
    const host = defineComponent({
        setup() {
            provideI18n({ locale: ref('en-US'), t: (key) => key, setLocale: () => {} })
            return () => h(WindowControls)
        },
    })
    const wrapper = await mountThemed(host, {}, 'light', { global: { plugins: [createPinia()] } })
    try {
        await waitFor(() => wrapper.find('button.close-button').exists())
        await wrapper.get('button.close-button').trigger('click')
        expect(native.close).toHaveBeenCalledTimes(1)
    } finally {
        wrapper.unmount()
    }
})
