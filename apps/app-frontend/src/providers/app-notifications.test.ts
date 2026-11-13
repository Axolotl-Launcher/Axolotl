import { afterEach, beforeEach, expect, it, vi } from 'vitest'

import { AppNotificationManager } from './app-notifications.ts'

// The UI barrel pulls in Vue components the node environment cannot load; the manager only
// needs its base class.
vi.mock('@modrinth/ui', async () => ({
    AbstractWebNotificationManager: (
        await import('../../../../packages/ui/src/providers/web-notifications.ts')
    ).AbstractWebNotificationManager,
}))

const originalStorageDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'localStorage')

beforeEach(() => {
    const values = new Map<string, string>()
    Object.defineProperty(globalThis, 'localStorage', {
        configurable: true,
        value: {
            getItem: (key: string) => values.get(key) ?? null,
            setItem: (key: string, value: string) => values.set(key, value),
            removeItem: (key: string) => values.delete(key),
        },
    })
    vi.useFakeTimers()
})

afterEach(() => {
    vi.useRealTimers()
    if (originalStorageDescriptor)
        Object.defineProperty(globalThis, 'localStorage', originalStorageDescriptor)
    else Reflect.deleteProperty(globalThis, 'localStorage')
})

function visible(manager: AppNotificationManager) {
    return manager
        .getNotifications()
        .filter((notification) => !notification.collapsed)
        .map((notification) => notification.title)
}

it('holds what the app raises during the OOBE and shows it as fresh toasts afterwards', () => {
    const manager = new AppNotificationManager()
    manager.holdNotifications()
    manager.addNotification({ title: 'loader tags', type: 'error' })
    manager.handleError(new Error('offline'))
    expect(visible(manager)).toEqual([])
    expect(manager.getNotifications()).toHaveLength(2)

    // Longer than a toast lasts: held toasts must not have spent their time while hidden.
    vi.advanceTimersByTime(60_000)
    manager.releaseNotifications()
    expect(visible(manager)).toHaveLength(2)

    vi.advanceTimersByTime(31_000)
    expect(visible(manager)).toEqual([])
})

it('shows what the OOBE itself raises at once, even while the rest is held', () => {
    const manager = new AppNotificationManager()
    const oobe = manager.immediate()
    manager.holdNotifications()
    oobe.addNotification({ title: 'sign-in failed', type: 'error' })
    oobe.handleError(new Error('yggdrasil'))
    manager.addNotification({ title: 'background', type: 'error' })
    expect(visible(manager)).toHaveLength(2)
    expect(visible(manager)).not.toContain('background')

    manager.releaseNotifications()
    expect(visible(manager)).toContain('background')
})

it('does not resurrect a held notification the app removed meanwhile', () => {
    const manager = new AppNotificationManager()
    manager.holdNotifications()
    const progress = manager.addNotification({ title: 'scanning', type: 'info' })
    manager.removeNotification(progress.id)
    manager.releaseNotifications()
    expect(manager.getNotifications()).toEqual([])
})
