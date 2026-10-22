import IntlMessageFormat from 'intl-messageformat'
import { afterEach, expect, it, vi } from 'vitest'
import { ref } from 'vue'

import { mountThemed, waitFor } from '@/test/visual-harness'

import { I18N_INJECTION_KEY } from '../../../../../../packages/ui/src/providers/i18n'
import MinecraftAuthErrorModal from './MinecraftAuthErrorModal.vue'

vi.mock('@/helpers/auth.js', () => ({ login: vi.fn(), set_default_user: vi.fn() }))
vi.mock('@/i18n.config', () => ({ default: { global: { locale: { value: 'en-US' } } } }))
vi.mock('@/store/error.js', () => ({ handleSevereError: vi.fn() }))
vi.mock('@modrinth/ui', async () => ({
    Admonition: (await import('../../../../../../packages/ui/src/components/base/Admonition.vue'))
        .default,
    Button: (await import('../../../../../../packages/ui/src/components/base/buttons/Button.vue'))
        .default,
    ButtonLink: (
        await import('../../../../../../packages/ui/src/components/base/buttons/ButtonLink.vue')
    ).default,
    Collapsible: (await import('../../../../../../packages/ui/src/components/base/Collapsible.vue'))
        .default,
    NewModal: (await import('../../../../../../packages/ui/src/components/modal/NewModal.vue'))
        .default,
    defineMessages: (messages: unknown) => messages,
    useVIntl: () => ({
        formatMessage: (message: { defaultMessage: string }) =>
            new IntlMessageFormat(message.defaultMessage, 'en-US').format(),
    }),
}))

const cleanup: (() => void)[] = []
afterEach(() =>
    cleanup
        .splice(0)
        .reverse()
        .forEach((stop) => stop()),
)

it('keeps both sign-in recovery actions compact and inside the modal at normal and narrow widths', async () => {
    const wrapper = await mountThemed(MinecraftAuthErrorModal, {}, 'dark', {
        global: {
            directives: { tooltip: {} },
            provide: {
                [I18N_INJECTION_KEY as symbol]: {
                    locale: ref('en-US'),
                    t: (key: string) => key,
                    setLocale: () => {},
                },
            },
        },
    })
    cleanup.push(() => wrapper.unmount())
    wrapper.vm.show({
        message:
            'Failed to deserialize response to JSON during step MinecraftToken: 429 Too Many Requests',
    })
    await waitFor(() => !!document.querySelector('.modal-container.shown'))
    await waitFor(() => getComputedStyle(document.querySelector('.modal-body')!).opacity === '1')
    const body = document.querySelector<HTMLElement>('.modal-body')!
    const support = body.querySelector<HTMLElement>('a[href*="/issues"]')!
    const retry = [...body.querySelectorAll<HTMLElement>('button')].find((button) =>
        button.textContent?.includes('Sign in again'),
    )!
    expect(support.textContent).toContain('Contact support')
    expect(retry).toBeTruthy()
    expect(support.getBoundingClientRect().width).toBeLessThan(body.clientWidth / 2)
    expect(retry.getBoundingClientRect().width).toBeLessThan(body.clientWidth / 2)
    const originalWidth = body.style.width
    cleanup.push(() => {
        body.style.width = originalWidth
    })
    for (const width of [548, 320]) {
        body.style.width = `${width}px`
        await new Promise((resolve) => requestAnimationFrame(resolve))
        const bounds = body.getBoundingClientRect()
        for (const action of [support, retry]) {
            const rect = action.getBoundingClientRect()
            expect(rect.left).toBeGreaterThanOrEqual(bounds.left)
            expect(rect.right).toBeLessThanOrEqual(bounds.right)
        }
        expect(body.scrollWidth).toBeLessThanOrEqual(body.clientWidth)
        const content = body.querySelector<HTMLElement>('[data-modal-content]')!
        expect(content.scrollWidth).toBeLessThanOrEqual(content.clientWidth)
    }
})
