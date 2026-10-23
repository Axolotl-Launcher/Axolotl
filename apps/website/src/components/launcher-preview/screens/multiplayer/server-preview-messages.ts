import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

import zhCN from '~/locales/launcher-preview/multiplayer.zh-CN'

import { registerPreviewMessages } from '../../lib/i18n'

registerPreviewMessages('zh-CN', zhCN)

/** What the preview's servers leave to the desktop app. */
export const serverPreviewMessages = defineMessages({
    serverDownload: {
        id: 'launcher-preview.servers.server-download',
        defaultMessage:
            'The web preview does not download server files. Download Axolotl to create and run this server.',
    },
    contentUnavailable: {
        id: 'launcher-preview.servers.content-unavailable',
        defaultMessage: "This preview doesn't include this file's contents.",
    },
    openFolder: {
        id: 'launcher-preview.servers.open-folder',
        defaultMessage: 'Opening folders needs the desktop app.',
    },
    roomsDesktopOnly: {
        id: 'launcher-preview.rooms.desktop-only',
        defaultMessage:
            'Online rooms need the desktop app, which runs Terracotta or RedStone Online 2 on your computer.',
    },
    recordedSession: {
        id: 'launcher-preview.servers.recorded-session',
        defaultMessage:
            'This output was recorded from the real server. Commands without a recording run in the desktop app.',
    },
    unrecordedCommand: {
        id: 'launcher-preview.servers.unrecorded-command',
        defaultMessage:
            'The web preview has no recording of “{command}”. Download Axolotl to run it.',
    },
})
