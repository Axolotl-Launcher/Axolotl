import { AbstractPopupNotificationManager, type PopupNotification } from '@modrinth/ui'
import { type Ref, ref } from 'vue'

import { loadNotificationDismissals } from './notification-dismissals.ts'
import { legacyPopupNotificationKey, popupNotificationKey } from './notification-keys.ts'

export class AppPopupNotificationManager extends AbstractPopupNotificationManager {
    private static readonly STORAGE_KEY = 'axolotl:dismissed-popup-notifications-v2'
    private static readonly LEGACY_STORAGE_KEY = 'axolotl:dismissed-popup-notifications'
    private readonly state: Ref<PopupNotification[]>
    private readonly dismissedKeys: Set<string>
    private readonly legacyDismissedKeys: Set<string>
    private dismissedBefore: number | null

    public constructor() {
        super()
        this.state = ref<PopupNotification[]>([])
        const dismissed = loadNotificationDismissals(
            [
                AppPopupNotificationManager.STORAGE_KEY,
                AppPopupNotificationManager.LEGACY_STORAGE_KEY,
            ],
            7,
            3,
        )
        this.dismissedKeys = dismissed.keys
        this.legacyDismissedKeys = dismissed.legacyKeys
        this.dismissedBefore = dismissed.clearedAt
    }

    public override addPopupNotification = (
        notification: Omit<PopupNotification, 'id' | 'timer'>,
        options?: { resurface?: boolean },
    ): PopupNotification => {
        if (options?.resurface) {
            // A new user action needs a fresh decision even if its earlier prompt was dismissed.
            const keyedNotification = { ...notification, id: 0 }
            this.dismissedKeys.delete(popupNotificationKey(keyedNotification))
            this.legacyDismissedKeys.delete(legacyPopupNotificationKey(keyedNotification))
            this.saveDismissedKeys()
        }
        return super.addPopupNotification(notification)
    }

    public getNotifications(): PopupNotification[] {
        return this.state.value
    }

    protected addNotificationToStorage(notification: PopupNotification): void {
        if (
            this.dismissedKeys.has(popupNotificationKey(notification)) ||
            this.legacyDismissedKeys.has(legacyPopupNotificationKey(notification)) ||
            (this.dismissedBefore !== null && (notification.createdAt ?? 0) <= this.dismissedBefore)
        )
            return
        this.state.value.unshift(notification)
    }

    protected removeNotificationFromStorage(id: string | number): void {
        const index = this.state.value.findIndex((n) => n.id === id)
        if (index > -1) this.state.value.splice(index, 1)
    }

    protected clearAllNotificationsFromStorage(): void {
        this.state.value.splice(0)
    }

    public override removeNotification = (id: string | number): void => {
        const notification = this.state.value.find((item) => item.id === id)
        super.removeNotification(id)
        if (notification) {
            this.dismissedKeys.add(popupNotificationKey(notification))
            this.saveDismissedKeys()
        }
    }

    public override clearAllNotifications = (): void => {
        this.dismissedBefore = Date.now()
        for (const notification of this.state.value) {
            this.dismissedKeys.add(popupNotificationKey(notification))
        }
        super.clearAllNotifications()
        this.saveDismissedKeys()
    }

    private saveDismissedKeys(): void {
        try {
            localStorage.setItem(
                AppPopupNotificationManager.STORAGE_KEY,
                JSON.stringify({
                    clearedAt: this.dismissedBefore,
                    keys: [...this.dismissedKeys].slice(-100),
                }),
            )
        } catch {
            // Popup notifications remain usable when storage is unavailable.
        }
    }
}
