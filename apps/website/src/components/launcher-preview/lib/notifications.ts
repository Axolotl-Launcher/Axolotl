import {
    AbstractPopupNotificationManager,
    type PopupNotification,
} from '@modrinth/ui/src/providers/popup-notifications.ts'
import {
    AbstractWebNotificationManager,
    type NotificationPanelLocation,
    type WebNotification,
} from '@modrinth/ui/src/providers/web-notifications.ts'
import { type Ref, ref } from 'vue'

/**
 * The desktop app's notification manager without its persistence: toasts live
 * for the session of the preview window and are cleared when it closes.
 */
export class PreviewNotificationManager extends AbstractWebNotificationManager {
    private readonly state: Ref<WebNotification[]> = ref([])
    private readonly location: Ref<NotificationPanelLocation> = ref('right')

    getNotifications(): WebNotification[] {
        return this.state.value
    }

    getNotificationLocation(): NotificationPanelLocation {
        return this.location.value
    }

    setNotificationLocation(location: NotificationPanelLocation): void {
        this.location.value = location
    }

    protected addNotificationToStorage(notification: WebNotification): void {
        this.state.value.unshift(notification)
    }

    protected removeNotificationFromStorage(id: string | number): void {
        const index = this.state.value.findIndex((item) => item.id === id)
        if (index > -1) this.state.value.splice(index, 1)
    }

    protected removeNotificationFromStorageByIndex(index: number): void {
        this.state.value.splice(index, 1)
    }

    protected clearAllNotificationsFromStorage(): void {
        this.state.value.splice(0)
    }
}

export class PreviewPopupNotificationManager extends AbstractPopupNotificationManager {
    private readonly state: Ref<PopupNotification[]> = ref([])

    getNotifications(): PopupNotification[] {
        return this.state.value
    }

    protected addNotificationToStorage(notification: PopupNotification): void {
        this.state.value.unshift(notification)
    }

    protected removeNotificationFromStorage(id: string | number): void {
        const index = this.state.value.findIndex((item) => item.id === id)
        if (index > -1) this.state.value.splice(index, 1)
    }

    protected clearAllNotificationsFromStorage(): void {
        this.state.value.splice(0)
    }
}
