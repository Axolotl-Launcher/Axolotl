import { formatPreviewMessage } from '../i18n'
import { desktopAppMessages } from '../messages'
import { currentPreview } from '../state'

/** `@tauri-apps/plugin-opener` `openUrl` for the preview: the link opens in a new tab. */
export async function openUrl(url: string): Promise<void> {
    window.open(url, '_blank', 'noopener,noreferrer')
}

function desktopNotice(message: { id: string; defaultMessage?: string }) {
    currentPreview().notifications.addNotification({
        type: 'info',
        title: formatPreviewMessage(message),
    })
}

/**
 * `highlightInFolder` / `showInFolder`: Finder belongs to the desktop app, so
 * the preview says so instead of pretending a folder opened.
 */
export function revealInFolder(_path: string) {
    desktopNotice(desktopAppMessages.openFolder)
}

/** `openPath` on a file: opening it with its default app is the desktop's. */
export function openInDefaultApp(_path: string) {
    desktopNotice(desktopAppMessages.openFile)
}

/** `createInstanceShortcut*`: shortcuts are files the desktop app writes. */
export function createShortcutNotice() {
    desktopNotice(desktopAppMessages.shortcut)
}
