export type ReleaseChannel = 'release' | 'beta'

export interface ReleaseAsset {
    name: string
    relative_path: string
    sha256?: string
}

export interface LauncherRelease {
    version: string
    tag_name: string
    assets: ReleaseAsset[]
}

interface DownloadSourceDefinition {
    id: string
    label: { id: string; defaultMessage: string }
    description: { id: string; defaultMessage: string }
    assetUrl: (release: LauncherRelease, asset: ReleaseAsset) => string
}

export const UPDATE_SERVER_BASE_URL = 'https://update.axlmc.org'
export const UPDATE_ARTIFACT_BASE_URL = 'https://dist.axlmc.org'
export const GITHUB_RELEASES_BASE_URL = 'https://github.com/Mystic-Stars/Axolotl/releases'

function encodePath(value: string) {
    return value.split('/').map(encodeURIComponent).join('/')
}

// The first source is the recommended default. Settings are derived from this registry.
export const downloadSources = [
    {
        id: 'update-server',
        label: {
            id: 'axolotl-settings.download-source.update-server',
            defaultMessage: 'Update Server',
        },
        description: {
            id: 'axolotl-settings.download-source.update-server.description',
            defaultMessage: 'Download through the official Axolotl update service.',
        },
        assetUrl: (_release, asset) =>
            UPDATE_ARTIFACT_BASE_URL + '/' + encodePath(asset.relative_path),
    },
    {
        id: 'github',
        label: { id: 'axolotl-settings.download-source.github', defaultMessage: 'GitHub' },
        description: {
            id: 'axolotl-settings.download-source.github.description',
            defaultMessage: 'Download from the official GitHub release.',
        },
        assetUrl: (release, asset) =>
            GITHUB_RELEASES_BASE_URL +
            '/download/' +
            encodeURIComponent(release.tag_name) +
            '/' +
            encodeURIComponent(asset.name),
    },
    {
        id: 'gitlink',
        label: { id: 'axolotl-settings.download-source.gitlink', defaultMessage: 'GitLink' },
        description: {
            id: 'axolotl-settings.download-source.gitlink.description',
            defaultMessage: 'Download from the GitLink mirror.',
        },
        assetUrl: (release, asset) =>
            'https://www.gitlink.org.cn/xinvxueyuan/Axolotl/releases/download/' +
            encodeURIComponent(release.tag_name) +
            '/' +
            encodeURIComponent(asset.name),
    },
] as const satisfies readonly DownloadSourceDefinition[]

export type DownloadSourceId = (typeof downloadSources)[number]['id']
export type DownloadSource = 'auto' | DownloadSourceId

export function isDownloadSource(value: unknown): value is DownloadSource {
    return value === 'auto' || downloadSources.some((source) => source.id === value)
}

export function getDownloadSource(id: DownloadSource) {
    const source =
        id === 'auto' ? downloadSources[0] : downloadSources.find((source) => source.id === id)
    if (!source) throw new Error('Unknown download source: ' + id)
    return source
}

export function getDownloadCatalogUrl(channel: ReleaseChannel) {
    if (channel !== 'release' && channel !== 'beta')
        throw new Error('Unknown release channel: ' + channel)
    return UPDATE_SERVER_BASE_URL + '/api/downloads/latest?channel=' + channel
}

function record(value: unknown): value is Record<string, unknown> {
    return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function unsafeComponent(value: string) {
    return Array.from(value).some((character) => {
        const code = character.charCodeAt(0)
        return code < 32 || code === 127 || code === 92
    })
}

function relativePath(download: Record<string, unknown>, version: string, filename: string) {
    let value = download.relative_path
    // The deployed catalog also exposes this authoritative path in its absolute URL.
    if (value === undefined && typeof download.url === 'string') {
        const url = new URL(download.url, UPDATE_SERVER_BASE_URL)
        if (
            ![UPDATE_SERVER_BASE_URL, UPDATE_ARTIFACT_BASE_URL].includes(url.origin) ||
            url.username ||
            url.password ||
            url.search ||
            url.hash
        ) {
            throw new Error('Invalid Update Server URL for ' + filename)
        }
        value = url.pathname.split('/').map(decodeURIComponent).join('/')
    }
    if (typeof value !== 'string') throw new Error('Missing relative_path for ' + filename)
    const normalized = value.startsWith('/') ? value.slice(1) : value
    const segments = normalized.split('/')
    if (
        segments.some((part) => !part || part === '.' || part === '..' || unsafeComponent(part)) ||
        segments[0] !== 'dist' ||
        segments[1] !== version ||
        segments.at(-1) !== filename
    ) {
        throw new Error('Invalid relative_path for ' + filename)
    }
    return normalized
}

export function normalizeDownloadCatalog(payload: unknown): LauncherRelease | null {
    // HTTP 204 means that the channel has no public release.
    if (payload === null || payload === undefined || payload === '') return null
    const versionPattern =
        /^[0-9]+[.][0-9]+[.][0-9]+(?:-[0-9A-Za-z-]+(?:[.][0-9A-Za-z-]+)*)?(?:[+][0-9A-Za-z-]+(?:[.][0-9A-Za-z-]+)*)?$/
    if (
        !record(payload) ||
        typeof payload.version !== 'string' ||
        payload.version !== payload.version.trim() ||
        !versionPattern.test(payload.version) ||
        !Array.isArray(payload.downloads)
    ) {
        throw new Error('Invalid Update Server download catalog')
    }
    const version = payload.version
    const tag = payload.release_tag ?? payload.tag_name ?? payload.tag ?? 'v' + version
    if (tag !== 'v' + version) throw new Error('Update Server version and tag do not match')
    const names = new Set<string>()
    const assets = payload.downloads.map((download: unknown): ReleaseAsset => {
        if (
            !record(download) ||
            typeof download.filename !== 'string' ||
            !download.filename ||
            download.filename.includes('/') ||
            unsafeComponent(download.filename) ||
            ['.', '..'].includes(download.filename) ||
            names.has(download.filename)
        ) {
            throw new Error('Invalid or duplicate Update Server attachment name')
        }
        names.add(download.filename)
        if (
            download.sha256 !== undefined &&
            (typeof download.sha256 !== 'string' || !/^[a-f0-9]{64}$/i.test(download.sha256))
        ) {
            throw new Error('Invalid SHA-256 for ' + download.filename)
        }
        return {
            name: download.filename,
            relative_path: relativePath(download, version, download.filename),
            ...(typeof download.sha256 === 'string' ? { sha256: download.sha256 } : {}),
        }
    })
    return { version, tag_name: tag, assets }
}
