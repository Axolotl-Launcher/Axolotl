import {
    type DownloadSourceId,
    getDownloadSource,
    GITHUB_RELEASES_BASE_URL,
    type LauncherRelease,
    type ReleaseChannel,
} from './download-sources.ts'

export type DownloadStatus = 'idle' | 'pending' | 'success' | 'error'

/**
 * The state the landing page's hero call to action is driven by.
 *
 * Deliberately narrower than {@link DownloadStatus}: the page collapses the
 * request's idle and pending states into one "loading" state, and treats a
 * settled request with no matching package as "ready" so the button stays usable.
 */
export type HeroDownloadState = 'loading' | 'ready' | 'error'

/**
 * Whether the hero call to action should stop waiting and degrade to a usable
 * control.
 *
 * The version service is a third party. A request that never settles used to
 * leave the button disabled and wordless for as long as it hung, with no way for
 * the visitor to act. Past the limit the button becomes clickable and explains
 * itself instead.
 */
export function shouldDegradeDownload(state: HeroDownloadState, elapsedMs: number, limitMs = 2500) {
    return state === 'loading' && elapsedMs >= limitMs
}

export function hasLauncherDownloads(release: LauncherRelease | null | undefined) {
    return Object.values(resolveLauncherDownloads(release, 'update-server')).some(Boolean)
}

export function resolveLauncherDownloads(
    release: LauncherRelease | null | undefined,
    source: DownloadSourceId,
) {
    const definition = getDownloadSource(source)
    function findAsset(patterns: RegExp[]) {
        if (!release) return null
        const asset = patterns
            .map((pattern) => release.assets.find((candidate) => pattern.test(candidate.name)))
            .find(Boolean)
        if (!asset) return null
        return definition.assetUrl(release, asset)
    }

    return {
        windows: findAsset([/x64-setup\.exe$/i, /\.exe$/i]),
        mac: findAsset([/universal\.dmg$/i, /\.dmg$/i]),
        appImage: findAsset([/(amd64|x86_64)\.AppImage$/i]),
        deb: findAsset([/_amd64\.deb$/i, /(amd64|x86_64).*\.deb$/i]),
        rpm: findAsset([/(x86_64|amd64).*\.rpm$/i]),
    }
}

export function getDownloadAvailability(status: DownloadStatus, link: string | null) {
    if (status === 'error') return 'error'
    if (status !== 'success') return 'loading'
    return link ? 'ready' : 'unavailable'
}

export function getReleasePage(channel: ReleaseChannel, release?: LauncherRelease | null) {
    const base = GITHUB_RELEASES_BASE_URL
    if (release?.tag_name) return `${base}/tag/${encodeURIComponent(release.tag_name)}`
    return channel === 'release' ? `${base}/latest` : base
}
