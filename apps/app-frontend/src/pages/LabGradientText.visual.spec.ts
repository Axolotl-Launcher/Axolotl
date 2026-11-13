import '@modrinth/ui/directives/tooltip-styles.ts'
import '@/test/visual-styles.scss'

import { provideI18n, provideNotificationManager } from '@modrinth/ui'
import { tooltipDirective } from '@modrinth/ui/directives/tooltip.ts'
import { flushPromises, mount } from '@vue/test-utils'
import IntlMessageFormat from 'intl-messageformat'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import chinese from '@/locales/zh-CN/index.json'
import { applyTheme } from '@/test/visual-harness'

import LabGradientText from './LabGradientText.vue'

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: vi.fn() }))
vi.mock('@tauri-apps/plugin-fs', () => ({ readTextFile: vi.fn(), writeTextFile: vi.fn() }))

it('announces the add-colour trigger as a dialog toggle', async () => {
    const host = defineComponent({
        setup() {
            provideI18n({
                locale: ref('zh-CN'),
                setLocale: () => {},
                t: (key, values) => {
                    const message = (
                        chinese as Record<string, { message?: string; defaultMessage?: string }>
                    )[key]?.message
                    return message
                        ? String(new IntlMessageFormat(message, 'zh-CN').format(values))
                        : key
                },
            })
            provideNotificationManager({
                addNotification: vi.fn(),
                handleError: vi.fn(),
            } as unknown as Parameters<typeof provideNotificationManager>[0])
            return () => h(LabGradientText)
        },
    })
    const wrapper = mount(host, {
        attachTo: document.body,
        global: { directives: { tooltip: tooltipDirective } },
    })

    try {
        applyTheme('dark')
        await flushPromises()

        // The page draws its own trigger inside the picker's slot, so the
        // picker cannot set these attributes for it.
        const add = wrapper.findAll('button').find((button) => button.text() === '添加颜色')!
        expect(add.attributes('aria-haspopup')).toBe('dialog')
        expect(add.attributes('aria-expanded')).toBe('false')

        await add.trigger('click')
        await flushPromises()
        expect(add.attributes('aria-expanded')).toBe('true')
    } finally {
        wrapper.unmount()
    }
})
