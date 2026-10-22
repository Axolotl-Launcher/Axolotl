import {
    AbstractWebNotificationManager,
    type NotificationPanelLocation,
    type WebNotification,
} from '@modrinth/ui'
import { type Ref, ref } from 'vue'

import { loadNotificationDismissals } from './notification-dismissals.ts'
import { webNotificationKey } from './notification-keys.ts'

export class AppNotificationManager extends AbstractWebNotificationManager {
    public errorTextFormatter?: (error: unknown) => string | null
    private static readonly STORAGE_KEY = 'axolotl:active-web-notifications-v1'
    private static readonly DISMISSED_STORAGE_KEY = 'axolotl:dismissed-web-notifications-v2'
    private static readonly LEGACY_DISMISSED_STORAGE_KEY = 'axolotl:dismissed-web-notifications'
    private static readonly MAX_NOTIFICATIONS = 100
    private static readonly MAX_NOTIFICATION_AGE_MS = 30 * 24 * 60 * 60 * 1000
    private static readonly MAX_SUPPORT_DATA_BYTES = 32 * 1024
    private readonly state: Ref<WebNotification[]>
    private readonly locationState: Ref<NotificationPanelLocation>
    private readonly dismissedKeys: Set<string>
    private dismissedBefore: number | null

    public constructor() {
        super()
        const dismissed = loadNotificationDismissals(
            [
                AppNotificationManager.DISMISSED_STORAGE_KEY,
                AppNotificationManager.LEGACY_DISMISSED_STORAGE_KEY,
            ],
            4,
        )
        this.dismissedKeys = dismissed.keys
        this.dismissedBefore = dismissed.clearedAt
        const restoredNotifications = this.loadActiveNotifications().filter(
            (notification) =>
                !this.dismissedKeys.has(webNotificationKey(notification)) &&
                (this.dismissedBefore === null ||
                    (notification.createdAt ?? 0) > this.dismissedBefore),
        )
        // Persisted entries belong to the history panel after a restart. Only notifications
        // created during the current session should enter the visible toast stack.
        restoredNotifications.forEach((notification) => {
            notification.collapsed = true
        })
        this.state = ref<WebNotification[]>(restoredNotifications)
        this.locationState = ref<NotificationPanelLocation>('right')
        this.saveActiveNotifications()
    }

    public getNotificationLocation(): NotificationPanelLocation {
        return this.locationState.value
    }

    public setNotificationLocation(location: NotificationPanelLocation): void {
        this.locationState.value = location
    }

    public getNotifications(): WebNotification[] {
        return this.state.value
    }

    protected addNotificationToStorage(notification: WebNotification): void {
        if (
            this.dismissedKeys.has(webNotificationKey(notification)) ||
            (this.dismissedBefore !== null && (notification.createdAt ?? 0) <= this.dismissedBefore)
        )
            return
        this.state.value.unshift(notification)
        this.saveActiveNotifications()
    }

    protected removeNotificationFromStorage(id: string | number): void {
        const index = this.state.value.findIndex((n) => n.id === id)
        if (index > -1) {
            this.state.value.splice(index, 1)
            this.saveActiveNotifications()
        }
    }

    protected removeNotificationFromStorageByIndex(index: number): void {
        this.state.value.splice(index, 1)
        this.saveActiveNotifications()
    }

    protected clearAllNotificationsFromStorage(): void {
        for (const notification of this.state.value) {
            this.dismissedKeys.add(webNotificationKey(notification))
        }
        this.state.value.splice(0)
        this.saveActiveNotifications()
        this.saveDismissedKeys()
    }

    public override addNotification = (notification: Partial<WebNotification>): WebNotification => {
        const result = super.addNotification(notification)
        this.saveActiveNotifications()
        return result
    }

    public override handleError(error: unknown): void {
        const text = this.errorTextFormatter?.(error)
        if (text === null || text === undefined) {
            super.handleError(error)
            return
        }
        this.addNotification({
            title: this.errorTitleProvider?.(),
            text,
            type: 'error',
            supportData: { error: error instanceof Error ? error.message : error },
        })
    }

    public override collapseNotification = (id: string | number): void => {
        super.collapseNotification(id)
        this.saveActiveNotifications()
    }

    public override expandNotification = (id: string | number): void => {
        super.expandNotification(id)
        this.saveActiveNotifications()
    }

    public override markNotificationRead = (id: string | number): void => {
        super.markNotificationRead(id)
        this.saveActiveNotifications()
    }

    public override removeNotification = (id: string | number): WebNotification | undefined => {
        const existing = this.state.value.find((notification) => notification.id === id)
        const notification = super.removeNotification(id)
        if (existing && notification) {
            this.dismissedKeys.add(webNotificationKey(existing))
            this.saveDismissedKeys()
        }
        this.saveActiveNotifications()
        return notification
    }

    public override clearAllNotifications = (): void => {
        this.dismissedBefore = Date.now()
        super.clearAllNotifications()
        this.saveDismissedKeys()
    }

    private saveDismissedKeys(): void {
        try {
            localStorage.setItem(
                AppNotificationManager.DISMISSED_STORAGE_KEY,
                JSON.stringify({
                    clearedAt: this.dismissedBefore,
                    keys: [...this.dismissedKeys].slice(-100),
                }),
            )
        } catch {
            // Notification history remains usable when storage is unavailable.
        }
    }

    private loadActiveNotifications(): WebNotification[] {
        try {
            const parsed = JSON.parse(
                localStorage.getItem(AppNotificationManager.STORAGE_KEY) ?? '[]',
            )
            const value = Array.isArray(parsed) ? parsed : parsed?.notifications
            if (!Array.isArray(value)) return []
            const cutoff = Date.now() - AppNotificationManager.MAX_NOTIFICATION_AGE_MS
            return value
                .filter((notification): notification is WebNotification => {
                    return (
                        notification &&
                        (typeof notification.id === 'string' ||
                            typeof notification.id === 'number') &&
                        typeof notification.createdAt === 'number' &&
                        notification.createdAt >= cutoff &&
                        (notification.title === undefined || typeof notification.title === 'string')
                    )
                })
                .slice(0, AppNotificationManager.MAX_NOTIFICATIONS)
        } catch {
            return []
        }
    }

    private saveActiveNotifications(): void {
        try {
            const cutoff = Date.now() - AppNotificationManager.MAX_NOTIFICATION_AGE_MS
            const active = this.state.value.filter(
                (notification) => (notification.createdAt ?? 0) >= cutoff,
            )
            if (active.length !== this.state.value.length)
                this.state.value.splice(0, this.state.value.length, ...active)
            if (this.state.value.length > AppNotificationManager.MAX_NOTIFICATIONS)
                this.state.value.splice(AppNotificationManager.MAX_NOTIFICATIONS)
            const persisted = active
                .slice(0, AppNotificationManager.MAX_NOTIFICATIONS)
                .map(({ timer: _timer, supportData, ...notification }) => {
                    if (supportData === undefined) return notification
                    try {
                        const serialized = JSON.stringify(supportData)
                        const size = new TextEncoder().encode(serialized).byteLength
                        return size <= AppNotificationManager.MAX_SUPPORT_DATA_BYTES
                            ? { ...notification, supportData }
                            : notification
                    } catch {
                        return notification
                    }
                })
            localStorage.setItem(AppNotificationManager.STORAGE_KEY, JSON.stringify(persisted))
        } catch {
            // Notification history remains usable when storage is unavailable.
        }
    }
}
