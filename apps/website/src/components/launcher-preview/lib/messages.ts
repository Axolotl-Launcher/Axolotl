import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

import zhCN from '~/locales/launcher-preview.zh-CN'

import { registerPreviewMessages } from './i18n'

registerPreviewMessages('zh-CN', zhCN)

/**
 * Strings of the launcher shell. Ids and English text are the desktop app's
 * own (apps/app-frontend); `launcher-preview.*` ids belong to the web window
 * frame, which the desktop app leaves to the operating system.
 */
export const desktopAppMessages = defineMessages({
    download: {
        id: 'launcher-preview.desktop-app.download',
        defaultMessage: 'Download Axolotl',
    },
    onlineService: {
        id: 'launcher-preview.desktop-app.online-service',
        defaultMessage: 'Uploading and downloading from online services needs the desktop app.',
    },
    openFolder: {
        id: 'launcher-preview.desktop-app.open-folder',
        defaultMessage: 'Opening folders in Finder needs the desktop app.',
    },
    shortcut: {
        id: 'launcher-preview.desktop-app.shortcut',
        defaultMessage: 'Creating shortcuts needs the desktop app.',
    },
    exportPack: {
        id: 'launcher-preview.desktop-app.export-pack',
        defaultMessage: 'Saving the exported pack needs the desktop app.',
    },
    openFile: {
        id: 'launcher-preview.desktop-app.open-file',
        defaultMessage: 'Opening files in other apps needs the desktop app.',
    },
    nbt: {
        id: 'launcher-preview.desktop-app.nbt',
        defaultMessage: 'Editing NBT files needs the desktop app.',
    },
    modAnalysis: {
        id: 'launcher-preview.desktop-app.mod-analysis',
        defaultMessage: 'Analyzing mod files needs the desktop app.',
    },
    modTranslation: {
        id: 'launcher-preview.desktop-app.mod-translation',
        defaultMessage: 'Translating mods with AI needs the desktop app.',
    },
    schematic: {
        id: 'launcher-preview.desktop-app.schematic',
        defaultMessage: 'Opening schematics in the 3D workshop needs the desktop app.',
    },
})

export const shellMessages = defineMessages({
    windowControls: {
        id: 'launcher-preview.window.controls',
        defaultMessage: 'Window controls',
    },
    windowClose: {
        id: 'app.window-controls.close',
        defaultMessage: 'Close',
    },
    windowMinimize: {
        id: 'app.window-controls.minimize',
        defaultMessage: 'Minimize',
    },
    windowFullScreen: {
        id: 'launcher-preview.window.full-screen',
        defaultMessage: 'Enter full screen',
    },
    windowExitFullScreen: {
        id: 'launcher-preview.window.exit-full-screen',
        defaultMessage: 'Exit full screen',
    },
    windowReopen: {
        id: 'launcher-preview.window.reopen',
        defaultMessage: 'Open Axolotl Launcher',
    },
    forward: {
        id: 'app.navigation.forward',
        defaultMessage: 'Forward',
    },
    createInstance: {
        id: 'app.navigation.create-instance',
        defaultMessage: 'Create new instance',
    },
    playingAs: {
        id: 'app.minecraft.playing-as',
        defaultMessage: 'Playing as',
    },
    collapseSidebar: {
        id: 'app.sidebar.collapse',
        defaultMessage: 'Collapse sidebar',
    },
    expandSidebar: {
        id: 'app.sidebar.expand',
        defaultMessage: 'Expand sidebar',
    },
    closeLauncherTitle: {
        id: 'app.close-launcher.title',
        defaultMessage: 'Choose how to close Axolotl Launcher',
    },
    closeLauncherDirect: {
        id: 'app.close-launcher.direct',
        defaultMessage: 'Close directly',
    },
    closeLauncherTray: {
        id: 'app.close-launcher.tray',
        defaultMessage: 'Hide to tray',
    },
    closeLauncherRemember: {
        id: 'app.close-launcher.remember',
        defaultMessage: 'Remember my choice',
    },
    exportErrorLogs: {
        id: 'app.notification.export-error-logs',
        defaultMessage: 'Export error logs',
    },
})

/**
 * Strings of the titlebar action bar and its download center
 * (apps/app-frontend/src/components/ui/AppActionBar.vue, DownloadCenterButton.vue).
 */
export const actionBarMessages = defineMessages({
    notifications: { id: 'app.action-bar.notifications', defaultMessage: 'Notifications' },
    clearNotifications: { id: 'app.action-bar.notifications.clear', defaultMessage: 'Clear all' },
    noNotifications: {
        id: 'app.action-bar.notifications.empty',
        defaultMessage: 'No notifications',
    },
    dismissNotification: {
        id: 'app.action-bar.notifications.dismiss',
        defaultMessage: 'Dismiss notification',
    },
    announcements: { id: 'app.action-bar.announcements', defaultMessage: 'Announcements' },
    offline: { id: 'app.action-bar.offline', defaultMessage: 'Offline' },
    noInstancesRunning: {
        id: 'app.action-bar.no-instances-running',
        defaultMessage: 'No instances running',
    },
    viewInstance: { id: 'app.action-bar.view-instance', defaultMessage: 'View instance' },
    stopInstance: { id: 'app.action-bar.stop-instance', defaultMessage: 'Stop instance' },
    viewLogs: { id: 'app.action-bar.view-logs', defaultMessage: 'View logs' },
    primaryInstance: { id: 'app.action-bar.primary-instance', defaultMessage: 'Primary instance' },
    makePrimaryInstance: {
        id: 'app.action-bar.make-primary-instance',
        defaultMessage: 'Make primary instance',
    },
    showMoreRunningInstances: {
        id: 'app.action-bar.show-more-running-instances',
        defaultMessage: 'Show more running instances',
    },
    hideMoreRunningInstances: {
        id: 'app.action-bar.hide-more-running-instances',
        defaultMessage: 'Hide more running instances',
    },
    downloads: { id: 'app.action-bar.downloads', defaultMessage: 'Downloads' },
    cancel: { id: 'app.action-bar.downloads.cancel', defaultMessage: 'Cancel download' },
    viewDetails: { id: 'app.action-bar.downloads.view-details', defaultMessage: 'View details' },
    noActiveDownloads: {
        id: 'app.action-bar.downloads.empty',
        defaultMessage: 'No active downloads',
    },
    viewHistory: {
        id: 'app.action-bar.downloads.view-history',
        defaultMessage: 'View download history',
    },
    unknownPhase: { id: 'app.action-bar.downloads.phase.unknown', defaultMessage: 'Working' },
    preparingInstance: {
        id: 'app.action-bar.downloads.phase.preparing-instance',
        defaultMessage: 'Preparing instance',
    },
    resolvingPack: {
        id: 'app.action-bar.downloads.phase.resolving-pack',
        defaultMessage: 'Resolving modpack',
    },
    downloadingPack: {
        id: 'app.action-bar.downloads.phase.downloading-pack',
        defaultMessage: 'Downloading modpack',
    },
    downloadingContent: {
        id: 'app.action-bar.downloads.phase.downloading-content',
        defaultMessage: 'Downloading content',
    },
    downloadingMinecraft: {
        id: 'app.action-bar.downloads.phase.downloading-minecraft',
        defaultMessage: 'Downloading Minecraft',
    },
    preparingJava: {
        id: 'app.action-bar.downloads.phase.preparing-java',
        defaultMessage: 'Preparing Java',
    },
    verifying: { id: 'app.action-bar.downloads.phase.verifying', defaultMessage: 'Verifying' },
    finalizing: { id: 'app.action-bar.downloads.phase.finalizing', defaultMessage: 'Finalizing' },
})

/** The remote announcement center (apps/app-frontend/src/components/ui/RemoteAnnouncements.vue). */
export const announcementMessages = defineMessages({
    unread: { id: 'app.remote-announcements.unread', defaultMessage: 'Unread' },
    readAll: {
        id: 'app.remote-announcements.read-all',
        defaultMessage: 'Mark all announcements as read',
    },
    centerTitle: { id: 'app.remote-announcements.center-title', defaultMessage: 'Announcements' },
    empty: { id: 'app.remote-announcements.empty', defaultMessage: 'No announcements' },
})

/** Static breadcrumb labels (apps/app-frontend/src/components/ui/Breadcrumbs.vue). */
export const breadcrumbMessages = defineMessages({
    home: { id: 'app.navigation.home', defaultMessage: 'Home' },
    worlds: { id: 'app.navigation.worlds', defaultMessage: 'Worlds' },
    discoverContent: {
        id: 'app.navigation.discover-content',
        defaultMessage: 'Discover content',
    },
    skinSelector: { id: 'app.navigation.skin-selector', defaultMessage: 'Skin selector' },
    multiplayer: { id: 'app.navigation.multiplayer', defaultMessage: 'Multiplayer' },
    library: { id: 'app.navigation.library', defaultMessage: 'Library' },
    downloads: { id: 'app.navigation.downloads', defaultMessage: 'Downloads' },
    lab: { id: 'app.navigation.lab', defaultMessage: 'Lab' },
    gradientText: {
        id: 'app.lab.gradient-text.title',
        defaultMessage: 'Gradient text generator',
    },
    seedMap: { id: 'app.lab.seed-map.title', defaultMessage: 'Seed map' },
    schematicWorkshop: {
        id: 'app.lab.schematic-preview.title',
        defaultMessage: 'Schematic workshop',
    },
    modTranslation: {
        id: 'app.lab.mod-translation.title',
        defaultMessage: 'Mod translation',
    },
    skinEditor: { id: 'app.lab.skin-editor.title', defaultMessage: 'Skin editor' },
    content: { id: 'app.instance.tabs.content', defaultMessage: 'Content' },
    files: { id: 'app.instance.tabs.files', defaultMessage: 'Files' },
    logs: { id: 'app.instance.tabs.logs', defaultMessage: 'Logs' },
    screenshots: { id: 'app.navigation.screenshots', defaultMessage: 'Screenshots' },
    editWorld: { id: 'app.navigation.edit-world', defaultMessage: 'Edit world' },
    upgradeInstance: { id: 'app.instance.upgrade-instance', defaultMessage: 'Upgrade instance' },
    studio: { id: 'instance.files.studio.title', defaultMessage: 'Studio' },
})
