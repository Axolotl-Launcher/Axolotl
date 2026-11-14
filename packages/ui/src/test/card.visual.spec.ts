import { afterEach, expect, it } from 'vitest'
import { ref } from 'vue'

import Card from '../components/base/Card.vue'
import { I18N_INJECTION_KEY } from '../providers/i18n'
import { mountThemed } from './visual-harness'

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).forEach((fn) => fn()))

it('names the collapse control and exposes its expanded state', async () => {
    const wrapper = await mountThemed(Card, { collapsible: true }, 'dark', {
        slots: {
            header: '<span>Advanced</span>',
            default: '<p>Body</p>',
        },
    })
    cleanup.push(() => wrapper.unmount())

    const toggle = wrapper.get('button')
    expect(toggle.attributes('aria-label')).toBe('Collapse')
    expect(toggle.attributes('aria-expanded')).toBe('true')

    await toggle.trigger('click')

    expect(toggle.attributes('aria-label')).toBe('Expand')
    expect(toggle.attributes('aria-expanded')).toBe('false')
})

it('updates its labels when the locale changes', async () => {
    const locale = ref('en-US')
    const translations: Record<string, Record<string, string>> = {
        'en-US': { 'button.collapse': 'Collapse', 'button.expand': 'Expand' },
        'zh-CN': { 'button.collapse': '收起', 'button.expand': '展开' },
    }
    const wrapper = await mountThemed(Card, { collapsible: true }, 'dark', {
        slots: {
            header: '<span>Advanced</span>',
            default: '<p>Body</p>',
        },
        global: {
            provide: {
                [I18N_INJECTION_KEY as symbol]: {
                    locale,
                    t: (key: string) => translations[locale.value]?.[key] ?? key,
                    setLocale: (next: string) => {
                        locale.value = next
                    },
                },
            },
        },
    })
    cleanup.push(() => wrapper.unmount())

    const toggle = wrapper.get('button')
    expect(toggle.attributes('aria-label')).toBe('Collapse')

    locale.value = 'zh-CN'
    await wrapper.vm.$nextTick()
    expect(toggle.attributes('aria-label')).toBe('收起')
})
