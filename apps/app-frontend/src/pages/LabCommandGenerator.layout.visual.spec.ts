import '@modrinth/ui/directives/tooltip-styles.ts'
import '@/test/visual-styles.scss'

import { Combobox, provideI18n } from '@modrinth/ui'
import { tooltipDirective } from '@modrinth/ui/directives/tooltip.ts'
import { flushPromises, mount } from '@vue/test-utils'
import IntlMessageFormat from 'intl-messageformat'
import { expect, it, vi } from 'vitest'
import { page } from 'vitest/browser'
import { defineComponent, h, ref } from 'vue'

import chinese from '@/locales/zh-CN/index.json'
import { applyTheme, waitFor } from '@/test/visual-harness'

import LabCommandGenerator from './LabCommandGenerator.vue'

vi.mock('@/components/lab/command-generator/CommandGeneratorItemPickerModal.vue', () => ({
    default: { render: () => null },
}))

it('keeps the command workspace readable across window widths and themes', async () => {
    localStorage.removeItem('axolotl:command-generator:user-presets')
    const teleports = document.createElement('div')
    teleports.id = 'teleports'
    document.body.append(teleports)
    const host = defineComponent({
        setup() {
            provideI18n({
                locale: ref('zh-CN'),
                setLocale: () => {},
                t: (key, values) => {
                    const message = (chinese as Record<string, { message: string }>)[key]?.message
                    return message
                        ? String(new IntlMessageFormat(message, 'zh-CN').format(values))
                        : key
                },
            })
            return () => h(LabCommandGenerator)
        },
    })
    const wrapper = mount(host, {
        attachTo: document.body,
        global: { directives: { tooltip: tooltipDirective } },
    })
    try {
        await flushPromises()
        applyTheme('dark')
        await page.viewport(1280, 1400)
        expect(wrapper.text()).toContain('选择一条命令开始')
        wrapper.findAllComponents(Combobox)[3].vm.$emit('update:modelValue', 'give')
        await flushPromises()
        const fields = wrapper.findAll('.argument-card input')
        await fields[0].setValue('@s')
        await fields[1].setValue('minecraft:diamond_sword')
        await wrapper.get('.action-bar button').trigger('click')
        for (const [width, theme] of [
            [1280, 'dark'],
            [1280, 'light'],
            [1024, 'dark'],
            [650, 'dark'],
        ] as const) {
            applyTheme(theme)
            await page.viewport(width, 1400)
            await flushPromises()
            const root = wrapper.element as HTMLElement
            expect(root.scrollWidth).toBeLessThanOrEqual(root.clientWidth + 1)
            const editor = wrapper.get('.parameters-section').element.getBoundingClientRect()
            const result = wrapper.get('aside').element.getBoundingClientRect()
            if (width >= 1024) expect(result.left).toBeGreaterThan(editor.right)
            else expect(result.top).toBeGreaterThan(editor.bottom)
            if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
                await page.screenshot({
                    element: root,
                    path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/command-${width}-${theme}.png`,
                })
        }
        await page.viewport(1280, 1400)
        wrapper.findAllComponents(Combobox)[3].vm.$emit('update:modelValue', 'execute')
        await flushPromises()
        const picker = wrapper
            .findAll('details')
            .find((details) => details.get('summary').text() === '可用子命令')!
        await picker.get('summary').trigger('click')
        const add =
            wrapper.findAll('button').find((button) => button.text() === 'as（以实体身份）') ??
            wrapper.findAll('button').find((button) => button.text().startsWith('as'))
        expect(add).toBeDefined()
        await add!.trigger('click')
        const remove = wrapper.get('button[aria-label="移除子命令"]')
        expect(remove.attributes('title')).toBeUndefined()
        await remove.trigger('mouseenter')
        await waitFor(() => document.querySelector('.tooltip-popper')?.textContent === '移除子命令')
        expect(
            document.querySelector('.tooltip-popper')!.getBoundingClientRect().width,
        ).toBeGreaterThan(0)
        await remove.trigger('mouseleave')
        if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
            await page.screenshot({
                element: wrapper.element,
                path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/command-execute.png`,
            })
        expect(wrapper.findAll('[title]')).toHaveLength(0)
        wrapper.findAllComponents(Combobox)[3].vm.$emit('update:modelValue', 'give')
        await flushPromises()
        await wrapper.get('.action-bar button').trigger('click')
        expect(wrapper.get('[role="alert"]').text()).toContain('请填写目标。')
        expect(wrapper.get('[role="alert"]').text()).not.toContain('app.lab.')
        const nbt = wrapper.get('.arguments-grid details')
        await nbt.get('summary').trigger('click')
        await nbt.get('input.text-input').setValue('一把用于探索世界的剑')
        await page.viewport(650, 1400)
        await flushPromises()
        expect(wrapper.element.scrollWidth).toBeLessThanOrEqual(wrapper.element.clientWidth + 1)
        if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
            await page.screenshot({
                element: wrapper.element,
                path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/command-nbt-narrow.png`,
            })
    } finally {
        wrapper.unmount()
        teleports.remove()
    }
})
