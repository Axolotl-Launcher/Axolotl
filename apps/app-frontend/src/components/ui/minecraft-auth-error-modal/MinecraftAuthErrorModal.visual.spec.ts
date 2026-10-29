import { afterEach, expect, it, vi } from 'vitest'
import { ref } from 'vue'

import chineseMessages from '@/locales/zh-CN/index.json'
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
    IntlFormatted: (
        await import('../../../../../../packages/ui/src/components/base/IntlFormatted.vue')
    ).default,
    NewModal: (await import('../../../../../../packages/ui/src/components/modal/NewModal.vue'))
        .default,
    defineMessages: (messages: unknown) => messages,
    useVIntl: (await import('../../../../../../packages/ui/src/composables/i18n')).useVIntl,
}))

const cleanup: (() => void)[] = []
afterEach(() =>
    cleanup
        .splice(0)
        .reverse()
        .forEach((stop) => stop()),
)

it.each(['en-US', 'zh-CN'])(
    'offers purchase and profile setup help for missing Java ownership in %s and resets for other errors',
    async (locale) => {
        const translations: Record<string, { message: string }> =
            locale === 'zh-CN' ? chineseMessages : {}
        const wrapper = await mountThemed(MinecraftAuthErrorModal, {}, 'dark', {
            global: {
                directives: { tooltip: {} },
                provide: {
                    [I18N_INJECTION_KEY as symbol]: {
                        locale: ref(locale),
                        t: (key: string) => translations[key]?.message ?? key,
                        setLocale: () => {},
                    },
                },
            },
        })
        cleanup.push(() => wrapper.unmount())
        const message =
            'Minecraft authentication error: The Microsoft account does not own Minecraft: Java Edition'
        wrapper.vm.show({ message })
        await waitFor(() => !!document.querySelector('.modal-container.shown'))
        const body = document.querySelector<HTMLElement>('.modal-body')!
        expect(body.textContent).toContain(
            locale === 'zh-CN'
                ? '我们无法从该微软账户中寻找到 Minecraft Java 正版档案：'
                : "We couldn't find a Minecraft: Java Edition profile for this Microsoft account.",
        )
        expect(body.querySelectorAll('ul li')).toHaveLength(2)
        const storeLink = body.querySelector<HTMLAnchorElement>('a[href*="/games/store/"]')!
        expect(storeLink.href).toBe(
            'https://www.xbox.com/zh-CN/games/store/minecraft-java-bedrock-edition-for-pc/9nxp44l49shj',
        )
        expect(storeLink.textContent?.trim()).toBe(locale === 'zh-CN' ? '这里' : 'here')
        const profileLink = body.querySelector<HTMLAnchorElement>(
            'a[href="https://minecraft.net"]',
        )!
        expect(profileLink.textContent?.trim()).toBe(
            locale === 'zh-CN' ? 'Minecraft 官网' : 'official Minecraft website',
        )
        expect(body.textContent).not.toContain(
            locale === 'zh-CN'
                ? '账号限制或地区限制'
                : 'account restrictions or regional limitations',
        )
        expect(body.textContent).not.toContain(locale === 'zh-CN' ? '未知错误' : 'Unknown error')
        const debugButton = [...body.querySelectorAll('button')].find((button) =>
            button.textContent?.includes(locale === 'zh-CN' ? '调试信息' : 'Debug information'),
        )!
        debugButton.click()
        await wrapper.vm.$nextTick()
        expect(body.textContent).toContain(message)

        wrapper.vm.show({ message: 'Minecraft authentication error: unrecognized failure' })
        await wrapper.vm.$nextTick()
        expect(body.textContent).toContain(locale === 'zh-CN' ? '未知错误' : 'Unknown error')
        expect(body.querySelector('a[href*="/games/store/"]')).toBeNull()
    },
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
