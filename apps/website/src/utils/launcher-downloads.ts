export type ReleaseChannel = 'release' | 'beta'
export type DownloadStatus = 'idle' | 'pending' | 'success' | 'error'
export interface LauncherRelease {
    tag_name: string
    assets: string[]
}

export function resolveLauncherDownloads(
    release: LauncherRelease | null | undefined,
    source: 'update-server' | 'github',
) {
    function findAsset(patterns: RegExp[]) {
        if (!release) return null
        const filename = patterns
            .map((pattern) => release.assets.find((name) => pattern.test(name)))
            .find(Boolean)
        if (!filename) return null
        const tag = encodeURIComponent(release.tag_name)
        const version = encodeURIComponent(release.tag_name.replace(/^v/, ''))
        return source === 'update-server'
            ? `https://update.axlmc.org/dist/${version}/${encodeURIComponent(filename)}`
            : `https://github.com/Mystic-Stars/Axolotl/releases/download/${tag}/${encodeURIComponent(filename)}`
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
    const base = 'https://github.com/Mystic-Stars/Axolotl/releases'
    if (release?.tag_name) return `${base}/tag/${encodeURIComponent(release.tag_name)}`
    return channel === 'release' ? `${base}/latest` : base
}
