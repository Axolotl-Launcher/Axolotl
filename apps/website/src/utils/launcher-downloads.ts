import {
    type DownloadSourceId,
    getDownloadSource,
    GITHUB_RELEASES_BASE_URL,
    type LauncherRelease,
    type ReleaseChannel,
} from './download-sources.ts'

export type DownloadStatus = 'idle' | 'pending' | 'success' | 'error'

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
