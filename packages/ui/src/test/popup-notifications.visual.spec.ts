import { expect, it } from 'vitest'
import { ref } from 'vue'

import PopupNotificationPanel from '../components/nav/PopupNotificationPanel.vue'
import { I18N_INJECTION_KEY } from '../providers/i18n'
import { mountThemed } from './visual-harness'

it('announces popup notifications through a polite live region', async () => {
    const manager = {
        getNotifications: () => [],
        stopNotificationTimer: () => {},
        setNotificationTimer: () => {},
        removeNotification: () => {},
    }
    const panel = await mountThemed(PopupNotificationPanel, { hasSidebar: false }, 'dark', {
        attachTo: document.body,
        global: {
            provide: {
                [I18N_INJECTION_KEY as symbol]: {
                    locale: ref('en-US'),
                    t: (key: string) => key,
                    setLocale: () => {},
                },
                [Symbol.for('modrinth:popupNotificationManager')]: manager,
            },
            directives: { tooltip: {} },
        },
    })

    try {
        const group = panel.get<HTMLElement>('.popup-notification-group').element
        expect(group.getAttribute('role')).toBe('log')
        expect(group.getAttribute('aria-live')).toBe('polite')
        expect(group.getAttribute('aria-relevant')).toBe('additions text')
    } finally {
        panel.unmount()
    }
})
