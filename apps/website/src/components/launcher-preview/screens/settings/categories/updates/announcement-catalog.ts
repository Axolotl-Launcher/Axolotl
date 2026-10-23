/**
 * The launcher's bundled release announcements (apps/app-frontend/src/announcements/catalog.ts).
 *
 * The website keeps the same entries, field for field and in the same order, in
 * `apps/website/releases/catalog.json` (written from the app catalog by
 * scripts/axolotl/create-website-release-catalog.mjs), so the preview reads that
 * file instead of carrying a second copy. It is imported lazily: only the
 * Updates category needs it.
 */
export type AnnouncementLocale = 'en-US' | 'zh-CN'

export type AnnouncementChangeType =
    'added' | 'changed' | 'deprecated' | 'removed' | 'fixed' | 'security'

export type LocalizedAnnouncementText = Readonly<Record<AnnouncementLocale, string>>

export type LauncherAnnouncement = {
    readonly id: string
    readonly version: string
    readonly publishedAt: string
    readonly title: LocalizedAnnouncementText
    readonly changes: Readonly<
        Partial<Record<AnnouncementChangeType, readonly LocalizedAnnouncementText[]>>
    >
    readonly notes?: LocalizedAnnouncementText
    readonly externalUrl?: string
}

export const ANNOUNCEMENT_CHANGE_TYPES: readonly AnnouncementChangeType[] = [
    'added',
    'changed',
    'deprecated',
    'removed',
    'fixed',
    'security',
]

/** `getAnnouncements()`: every bundled announcement, newest first. */
export async function loadAnnouncements(): Promise<readonly LauncherAnnouncement[]> {
    const catalog = await import('../../../../../../../releases/catalog.json')
    return catalog.default.announcements as readonly LauncherAnnouncement[]
}

/** `getAnnouncementByVersion()`. */
export function getAnnouncementByVersion(
    announcements: readonly LauncherAnnouncement[],
    version: string | null | undefined,
) {
    if (!version) return undefined
    return announcements.find((announcement) => announcement.version === version)
}

/** `getLocalizedAnnouncementText()`: Simplified Chinese, or English for every other locale. */
export function getLocalizedAnnouncementText(
    text: LocalizedAnnouncementText,
    locale: string,
): string {
    return locale === 'zh-CN' ? text['zh-CN'] : text['en-US']
}
