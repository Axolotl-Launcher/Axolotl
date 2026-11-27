import '@/test/visual-styles.scss'

import { provideI18n } from '@modrinth/ui'
import { flushPromises, mount } from '@vue/test-utils'
import IntlMessageFormat from 'intl-messageformat'
import { expect, it, vi } from 'vitest'
import { defineComponent, h, ref, Suspense } from 'vue'

import LanguageSettings from './LanguageSettings.vue'

vi.mock('@/helpers/settings.ts', () => ({
    get: async () => ({ locale: 'en-US' }),
    update: vi.fn(),
}))
vi.mock('@/i18n.config', () => ({
    default: { global: { locale: { value: 'en-US' } } },
    getSystemResolvedLocale: () => 'en-US',
    isFollowingSystemLocale: () => false,
    setAppLocale: vi.fn(),
    setFollowSystemLocale: vi.fn(),
}))

it('aligns the localized language guidance and the fallback note in one text column', async () => {
    const warning =
        'Beim Ändern der Sprache können Inhalte ohne Übersetzung weiterhin auf Englisch angezeigt werden.'
    const wrapper = mount(
        defineComponent({
            setup() {
                provideI18n({
                    locale: ref('de-DE'),
                    t: (key, values) => {
                        if (key === 'settings.language.description')
                            return 'Wähle die gewünschte Sprache für die {platform}. Freiwillige übersetzen <crowdin-link>auf Crowdin</crowdin-link>.'
                        if (key === 'settings.language.platform.app') return 'App'
                        if (key === 'settings.language.warning')
                            return new IntlMessageFormat(warning, 'de-DE').format(values) as string
                        return key
                    },
                    setLocale: () => {},
                })
                return () => h(Suspense, {}, () => h(LanguageSettings))
            },
        }),
        {
            attachTo: document.body,
            global: {
                directives: { tooltip: () => {} },
                stubs: { Combobox: true, RouterLink: true },
            },
        },
    )
    try {
        await flushPromises()
        const container = wrapper.get('.settings-section-card').element as HTMLElement
        const link = wrapper.get<HTMLAnchorElement>('a[href="https://translate.modrinth.com"]')
        const description = link.element.closest('p')!
        const note = Array.from(container.querySelectorAll('p, div')).find(
            (element) => element.textContent?.trim() === warning && element.childElementCount === 0,
        )!
        expect(note).toBeDefined()
        expect(description.textContent).toContain('Sprache für die App')
        expect(link.text()).toBe('auf Crowdin')

        for (const width of [600, 360]) {
            container.style.width = `${width}px`
            expect(description.getBoundingClientRect().left).toBeCloseTo(
                note.getBoundingClientRect().left,
                1,
            )
            expect(description.getBoundingClientRect().width).toBeCloseTo(
                note.getBoundingClientRect().width,
                1,
            )
            expect(getComputedStyle(description).fontSize).toBe(getComputedStyle(note).fontSize)
            expect(getComputedStyle(description).lineHeight).toBe(getComputedStyle(note).lineHeight)
        }
    } finally {
        wrapper.unmount()
    }
})
