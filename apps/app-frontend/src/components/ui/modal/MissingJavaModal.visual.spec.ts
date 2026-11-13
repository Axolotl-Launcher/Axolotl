import { I18N_INJECTION_KEY } from '@modrinth/ui'
import { flushPromises, mount } from '@vue/test-utils'
import IntlMessageFormat from 'intl-messageformat'
import { expect, it, vi } from 'vitest'
import { nextTick, ref } from 'vue'
import { createMemoryHistory, createRouter } from 'vue-router'

import en from '@/locales/en-US/index.json'
import zh from '@/locales/zh-CN/index.json'
import { applyTheme, waitFor } from '@/test/visual-harness'

import MissingJavaModal from './MissingJavaModal.vue'

vi.mock('@modrinth/ui', async () => {
    const [button, modal, i18n, messages, provider] = await Promise.all([
        import('@modrinth/ui/src/components/base/buttons/Button.vue'),
        import('@modrinth/ui/src/components/modal/NewModal.vue'),
        import('@modrinth/ui/src/composables/i18n'),
        import('@modrinth/ui/src/utils/common-messages'),
        import('@modrinth/ui/src/providers/i18n'),
    ])
    return {
        Button: button.default,
        NewModal: modal.default,
        defineMessages: i18n.defineMessages,
        useVIntl: i18n.useVIntl,
        commonMessages: messages.commonMessages,
        I18N_INJECTION_KEY: provider.I18N_INJECTION_KEY,
    }
})

it.each([
    ['en-US', en],
    ['zh-CN', zh],
] as const)(
    'shows the required Java version and opens Java settings in %s',
    async (locale, messages) => {
        applyTheme('dark')
        const router = createRouter({
            history: createMemoryHistory(),
            routes: [
                { path: '/downloads', component: { render: () => null } },
                { path: '/settings', component: { render: () => null } },
            ],
        })
        await router.push('/downloads')
        const wrapper = mount(MissingJavaModal, {
            attachTo: document.body,
            global: {
                plugins: [router],
                provide: {
                    [I18N_INJECTION_KEY as symbol]: {
                        locale: ref(locale),
                        t: (key: string, values: Record<string, string | number>) => {
                            const message = (messages as Record<string, { message?: string }>)[key]
                                ?.message
                            return message
                                ? new IntlMessageFormat(message, locale).format(values)
                                : key
                        },
                        setLocale: () => {},
                    },
                },
                directives: { tooltip: () => {} },
            },
        })
        try {
            for (const version of [8, 17, 21, 25]) {
                wrapper.vm.show(version)
                await nextTick()
                await waitFor(() => !!document.querySelector('[role="dialog"]'))
                const dialog = document.querySelector('[role="dialog"]') as HTMLElement
                const expectedBody = new IntlMessageFormat(
                    messages['app.install.missing-java.body'].message,
                    locale,
                ).format({ version })
                expect(dialog.textContent).toContain(expectedBody)
                expect(dialog.textContent).not.toContain('{version}')
            }

            const dialog = document.querySelector('[role="dialog"]') as HTMLElement
            const settingsButton = Array.from(dialog.querySelectorAll('button')).find((button) =>
                button.textContent?.includes(
                    messages['app.install.missing-java.open-settings'].message,
                ),
            )
            expect(settingsButton).toBeDefined()
            settingsButton!.click()
            await flushPromises()
            expect(router.currentRoute.value.fullPath).toBe('/settings#java-performance')
            await waitFor(() => !document.querySelector('[role="dialog"]'))
        } finally {
            wrapper.unmount()
        }
    },
)
