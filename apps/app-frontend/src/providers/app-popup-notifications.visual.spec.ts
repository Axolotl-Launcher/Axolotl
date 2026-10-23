import { beforeEach, expect, it } from 'vitest'

import { AppPopupNotificationManager } from './app-popup-notifications'

const prompt = {
    title: 'Nested archives detected',
    text: 'Unpack this archive to analyze?',
    type: 'info' as const,
    autoCloseMs: null,
    buttons: [{ label: 'Continue', action: () => {} }],
}

beforeEach(() => localStorage.clear())

it('shows a new drop confirmation after dismissal, including after restarting', () => {
    let manager = new AppPopupNotificationManager()
    const first = manager.addPopupNotification(prompt, { resurface: true })
    manager.removeNotification(first.id)
    expect(manager.getNotifications()).toHaveLength(0)

    manager = new AppPopupNotificationManager()
    const next = manager.addPopupNotification(prompt, { resurface: true })
    expect(manager.getNotifications()).toHaveLength(1)
    manager.removeNotification(next.id)
    manager.addPopupNotification(prompt, { resurface: true })
    expect(manager.getNotifications()).toHaveLength(1)
})

it('keeps ordinary dismissed notifications hidden when resurfacing a drop prompt', () => {
    const manager = new AppPopupNotificationManager()
    const ordinary = { ...prompt, title: 'Background notice' }
    manager.removeNotification(manager.addPopupNotification(ordinary).id)
    manager.removeNotification(manager.addPopupNotification(prompt).id)
    manager.addPopupNotification(prompt, { resurface: true })
    manager.addPopupNotification(ordinary)
    expect(manager.getNotifications().map((notification) => notification.title)).toEqual([
        prompt.title,
    ])
})

it('resurfaces confirmations stored by the legacy dismissal format', () => {
    localStorage.setItem(
        'axolotl:dismissed-popup-notifications',
        JSON.stringify([JSON.stringify([prompt.title, prompt.text, prompt.type])]),
    )
    const manager = new AppPopupNotificationManager()
    manager.addPopupNotification(prompt)
    expect(manager.getNotifications()).toHaveLength(0)
    manager.addPopupNotification(prompt, { resurface: true })
    expect(manager.getNotifications()).toHaveLength(1)
})
