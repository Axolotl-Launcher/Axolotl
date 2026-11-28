import '@/test/visual-styles.scss'
import '@/assets/stylesheets/global.scss'

import { flushPromises, mount } from '@vue/test-utils'
import { IntlMessageFormat } from 'intl-messageformat'
import { createPinia } from 'pinia'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { page, userEvent } from 'vitest/browser'
import { ref } from 'vue'

import en from '@/locales/en-US/index.json'
import zh from '@/locales/zh-CN/index.json'
import { useTheming } from '@/store/theme'
import { applyTheme } from '@/test/visual-harness'

import { I18N_INJECTION_KEY } from '../../../../packages/ui/src/providers/i18n'
import LabStronghold from './LabStronghold.vue'

// Keep the real shared controls, formatting provider and calculation Worker.
vi.mock('@modrinth/ui', async () => ({
    Button: (await import('../../../../packages/ui/src/components/base/buttons/Button.vue'))
        .default,
    StyledInput: (await import('../../../../packages/ui/src/components/base/StyledInput.vue'))
        .default,
    Combobox: (await import('../../../../packages/ui/src/components/base/Combobox.vue')).default,
    Toggle: (await import('../../../../packages/ui/src/components/base/Toggle.vue')).default,
    ...(await import('../../../../packages/ui/src/composables/i18n')),
}))

const first = '/execute in minecraft:overworld run tp @s 199.50 71.00 -63.50 60.23 -31.39'
const second = '/execute in minecraft:overworld run tp @s 196.49 69.00 -69.35 59.99 -31.52'
const wrappers: ReturnType<typeof mount>[] = []

beforeEach(() => {
    sessionStorage.removeItem('axolotl.lab.stronghold.sectors.v1')
    if (!document.getElementById('teleports')) {
        const target = document.createElement('div')
        target.id = 'teleports'
        document.body.append(target)
    }
})

afterEach(() => {
    wrappers.splice(0).forEach((wrapper) => wrapper.unmount())
    vi.restoreAllMocks()
    document.documentElement.classList.remove('has-custom-background')
})

async function render(language = 'en-US', theme: 'dark' | 'light' | 'oled' = 'dark') {
    applyTheme(theme)
    useTheming(createPinia()).setAccentColor('pink')
    document.body.style.margin = '0'
    document.body.style.position = 'static'
    document.body.style.overflow = 'auto'
    document.body.style.background = 'var(--surface-1)'
    document.body.style.color = 'var(--color-text-primary)'
    const catalog = (language === 'zh-CN' ? zh : en) as Record<string, { message: string }>
    const wrapper = mount(LabStronghold, {
        attachTo: document.body,
        global: {
            provide: {
                [I18N_INJECTION_KEY as symbol]: {
                    locale: ref(language),
                    t: (key: string, values: Record<string, unknown> = {}) =>
                        catalog[key]
                            ? String(
                                  new IntlMessageFormat(catalog[key].message, language).format(
                                      values,
                                  ),
                              )
                            : key,
                    setLocale: () => {},
                },
            },
        },
    })
    wrappers.push(wrapper)
    await flushPromises()
    return wrapper
}

async function paste(wrapper: ReturnType<typeof mount>, text: string) {
    await wrapper.get('textarea').setValue(text)
    await userEvent.click(wrapper.get('form button[type="submit"]').element)
    await vi.waitFor(() => expect(wrapper.findAll('tbody tr').length).toBeGreaterThan(0), {
        timeout: 20000,
    })
}
const fixture = [
    [9.5, 44.5, 65.1],
    [-141.15, -108.92, 59.85],
    [-402.7, -363.38, 48.45],
    [-611.82, -443.66, 42],
    [-845.81, -440.52, 36],
    [-1110.35, -443.71, 27],
    [-1368.35, -406.09, 18.75],
]
const commands = fixture
    .map(([x, z, yaw]) => `/execute in minecraft:overworld run tp @s ${x} 64 ${z} ${yaw} 0`)
    .join('\n')

it('scores real Worker sectors, selects table rows, copies X/Z and dimension-qualified teleport', async () => {
    await page.viewport(1440, 1100)
    const wrapper = await render()
    expect((wrapper.get('input[type="number"]').element as HTMLInputElement).value).toBe('5')
    await paste(wrapper, commands)
    expect(wrapper.findAll('tbody tr')).toHaveLength(10)
    expect(wrapper.text()).toContain('Support score')
    expect(wrapper.text()).not.toContain('Model probability')
    expect(wrapper.text()).not.toContain('Ninjabrain Bot · GPL-3.0')
    const panel = wrapper.get('[data-testid="stronghold-results"]').element.getBoundingClientRect()
    const selection = wrapper.get('[data-testid="selected-chunk"]').element.getBoundingClientRect()
    expect(panel.bottom - selection.bottom).toBeLessThan(32)
    const padding = getComputedStyle(wrapper.get('main').element)
    expect(parseFloat(padding.paddingBottom)).toBeGreaterThanOrEqual(
        parseFloat(padding.paddingTop) * 4,
    )
    await vi.waitFor(() =>
        expect(Number(wrapper.get('canvas').element.getAttribute('data-grid-step')) % 16).toBe(0),
    )
    const row = wrapper.findAll('tbody tr')[3]
    await row.get('button').trigger('click')
    expect(wrapper.get('[data-testid="best-chunk"]').text()).toBe(row.findAll('td')[0].text())
    const coords = row.findAll('td')[1].text()
    const write = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue()
    await page.getByRole('button', { name: 'Copy X/Z', exact: true }).click()
    expect(write).toHaveBeenLastCalledWith(coords)
    await page.getByRole('button', { name: 'Copy teleport command', exact: true }).click()
    const [x, z] = coords.split(', ')
    expect(write).toHaveBeenLastCalledWith(
        `/execute in minecraft:overworld run tp @s ${x} 100 ${z}`,
    )
    await wrapper.get('input[aria-label="Y"]').setValue('-20')
    await page.getByRole('button', { name: 'Copy teleport command', exact: true }).click()
    expect(write).toHaveBeenLastCalledWith(
        `/execute in minecraft:overworld run tp @s ${x} -20 ${z}`,
    )
    await wrapper.get('input[aria-label="Y"]').setValue('1000')
    expect(
        (
            page
                .getByRole('button', { name: 'Copy teleport command', exact: true })
                .element() as HTMLButtonElement
        ).disabled,
    ).toBe(true)
    await wrapper.get('input[aria-label="Top-K"]').setValue('25')
    expect(wrapper.findAll('tbody tr')).toHaveLength(25)
})

it('clicks map chunks, distinguishes drags, pans and zooms, and persists inputs', async () => {
    await page.viewport(1440, 1100)
    const wrapper = await render()
    await paste(wrapper, commands)
    await wrapper.findAll('tbody tr')[0].get('button').trigger('click')
    const canvas = wrapper.get('canvas')
    canvas.element.scrollIntoView({ block: 'center' })
    await new Promise((resolve) => requestAnimationFrame(resolve))
    const rect = canvas.element.getBoundingClientRect()
    const selected = wrapper.get('[data-testid="best-chunk"]').text()
    vi.spyOn(canvas.element, 'setPointerCapture').mockImplementation(() => {})
    const dispatch = async (type: string, init: PointerEventInit) => {
        canvas.element.dispatchEvent(new PointerEvent(type, { bubbles: true, ...init }))
        await flushPromises()
    }
    const event = {
        pointerId: 1,
        button: 0,
        clientX: rect.left + rect.width / 2,
        clientY: rect.top + rect.height / 2,
    }
    await dispatch('pointerdown', event)
    await dispatch('pointerup', event)
    expect(wrapper.get('[data-testid="best-chunk"]').text()).toBe(selected)
    const before = canvas.attributes('data-span')
    await canvas.trigger('keydown', { key: '+' })
    expect(Number(canvas.attributes('data-span'))).toBeLessThan(Number(before))
    await dispatch('pointerdown', event)
    await dispatch('pointermove', { ...event, clientX: event.clientX + 70 })
    await dispatch('pointerup', { ...event, clientX: event.clientX + 70 })
    expect(wrapper.get('[data-testid="best-chunk"]').text()).toBe(selected)
    const rect2 = canvas.element.getBoundingClientRect()
    const adjacent = {
        ...event,
        clientX:
            rect2.left +
            rect2.width / 2 +
            (16 * canvas.element.clientWidth) / Number(canvas.attributes('data-span')),
        clientY: rect2.top + rect2.height / 2,
    }
    await dispatch('pointerdown', adjacent)
    await dispatch('pointerup', adjacent)
    expect(wrapper.get('[data-testid="best-chunk"]').text()).not.toBe(selected)
    const saved = JSON.parse(sessionStorage.getItem('axolotl.lab.stronghold.sectors.v1')!)
    expect(saved.observations).toHaveLength(7)
    expect(saved.sigma).toBe('5')
})

it('keeps opposing target support and validates settings without stale results', async () => {
    const wrapper = await render()
    await paste(
        wrapper,
        '/execute in minecraft:overworld run tp @s 0 64 0 0 0\n/execute in minecraft:overworld run tp @s 0 64 0 180 0',
    )
    expect(Number(wrapper.get('canvas').attributes('data-candidates'))).toBeGreaterThan(100)
    await wrapper.get('input[aria-label="Sector half-angle (°)"]').setValue('0')
    await flushPromises()
    expect(wrapper.findAll('tbody tr')).toHaveLength(0)
    expect(wrapper.find('[role="alert"]').exists()).toBe(true)
    await wrapper.get('input[aria-label="Sector half-angle (°)"]').setValue('2')
    await vi.waitFor(() => expect(wrapper.findAll('tbody tr')).toHaveLength(10), { timeout: 20000 })
    await page.getByRole('button', { name: 'Clear all', exact: true }).click()
    expect(wrapper.findAll('tbody tr')).toHaveLength(0)
    expect(wrapper.find('[data-testid="selected-chunk"]').exists()).toBe(false)
})

it('rejects invalid batches atomically and duplicates', async () => {
    const wrapper = await render()
    await wrapper.get('textarea').setValue(first + '\nnot a command')
    await wrapper.get('form').trigger('submit')
    expect(wrapper.findAll('[data-testid="measurements"] li')).toHaveLength(0)
    await paste(wrapper, first + '\n' + second)
    await wrapper.get('textarea').setValue(first)
    await wrapper.get('form').trigger('submit')
    expect(wrapper.findAll('[data-testid="measurements"] li')).toHaveLength(2)
    expect(wrapper.get('[role="alert"]').text()).toContain('already')
})

for (const theme of ['dark', 'light', 'oled'] as const)
    it(`renders Chinese sector map in ${theme}`, async () => {
        await page.viewport(theme === 'oled' ? 430 : 1440, 1100)
        const wrapper = await render('zh-CN', theme)
        await paste(wrapper, commands)
        expect(wrapper.text()).toContain('支持分数')
        const canvas = wrapper.get('canvas')
        canvas.element.scrollIntoView({ block: 'center' })
        await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))
        expect(canvas.element.getBoundingClientRect().width).toBeGreaterThan(300)
        if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
            await page.screenshot({
                path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/sectors-${theme}.png`,
            })
    })
