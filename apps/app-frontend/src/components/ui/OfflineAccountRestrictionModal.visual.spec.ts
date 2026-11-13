import { I18N_INJECTION_KEY } from '@modrinth/ui'
import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { nextTick, ref } from 'vue'

import en from '@/locales/en-US/index.json'
import zh from '@/locales/zh-CN/index.json'
import { applyTheme, waitFor } from '@/test/visual-harness'

import OfflineAccountRestrictionModal from './OfflineAccountRestrictionModal.vue'

const native = vi.hoisted(() => ({ openUrl: vi.fn() }))
const addNotification = vi.fn()
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: native.openUrl }))

it.each([
    ['en-US', en],
    ['zh-CN', zh],
] as const)(
    'shows the restriction and working recovery actions in %s',
    async (locale, messages) => {
        applyTheme('dark')
        native.openUrl.mockReset().mockResolvedValue(undefined)
        addNotification.mockReset()
        const wrapper = mount(OfflineAccountRestrictionModal, {
            attachTo: document.body,
            global: {
                provide: {
                    [Symbol.for('modrinth:notificationManager')]: { addNotification },
                    [I18N_INJECTION_KEY as symbol]: {
                        locale: ref(locale),
                        t: (key: string) =>
                            (messages as Record<string, { message?: string }>)[key]?.message ?? key,
                        setLocale: () => {},
                    },
                },
                directives: { tooltip: () => {} },
            },
        })
        try {
            wrapper.vm.show()
            await nextTick()
            await waitFor(() => !!document.querySelector('[role="dialog"]'), {
                label: 'restriction dialog',
            })
            const dialog = document.querySelector('[role="dialog"]') as HTMLElement
            expect(dialog.textContent).toContain(
                messages['minecraft-account.restriction.description'].message,
            )
            expect(dialog.getBoundingClientRect().width).toBeGreaterThan(0)
            expect(dialog.scrollWidth).toBeLessThanOrEqual(dialog.clientWidth)
            const button = (key: 'buy' | 'sign-in') =>
                Array.from(dialog.querySelectorAll('button')).find((element) =>
                    element.textContent?.includes(
                        messages[`minecraft-account.restriction.${key}`].message,
                    ),
                )!
            button('buy').click()
            expect(native.openUrl).toHaveBeenCalledWith(
                'https://www.minecraft.net/en-us/store/minecraft-java-bedrock-edition-pc',
            )
            await flushPromises()
            expect(addNotification).not.toHaveBeenCalled()
            native.openUrl.mockRejectedValueOnce(new Error('Browser unavailable'))
            button('buy').click()
            await flushPromises()
            expect(addNotification).toHaveBeenCalledWith({
                title: messages['minecraft-account.restriction.open-failed'].message,
                text: messages['minecraft-account.restriction.open-failed-description'].message,
                type: 'error',
            })
            expect(button('buy').disabled).toBe(false)
            button('buy').click()
            await flushPromises()
            expect(native.openUrl).toHaveBeenCalledTimes(3)
            expect(addNotification).toHaveBeenCalledTimes(1)
            button('sign-in').click()
            expect(wrapper.emitted('signIn')).toHaveLength(1)
        } finally {
            wrapper.unmount()
        }
    },
)
