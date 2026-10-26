import type { Ref } from 'vue'

import {
    getDownloadCatalogUrl,
    type LauncherRelease,
    normalizeDownloadCatalog,
    type ReleaseChannel,
} from '../utils/download-sources.ts'
import { resolveLauncherDownloads } from '../utils/launcher-downloads.ts'
import { useDownloadSource } from './use-download-source.ts'

export function useLauncherDownloads(channel: Ref<ReleaseChannel>) {
    const { resolvedSource } = useDownloadSource()
    const catalogUrl = computed(() => getDownloadCatalogUrl(channel.value))
    const { data, status, refresh } = useFetch<LauncherRelease | null>(catalogUrl, {
        server: false,
        timeout: 10000,
        retry: 1,
        retryDelay: 500,
        transform: normalizeDownloadCatalog,
    })
    const release = computed(() => (status.value === 'success' ? data.value : null))
    const links = computed(() => resolveLauncherDownloads(release.value, resolvedSource.value.id))

    return { release, links, status, refresh }
}
