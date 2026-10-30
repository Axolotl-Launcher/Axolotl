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
    const config = useRuntimeConfig()
    const endpoints = {
        updateServerUrl: config.public.updateServerUrl,
        updateArtifactUrl: config.public.updateArtifactUrl,
    }
    const { resolvedSource, sources } = useDownloadSource()
    const catalogUrl = computed(() => getDownloadCatalogUrl(channel.value, endpoints))
    const { data, status, refresh } = useFetch<LauncherRelease | null>(catalogUrl, {
        server: false,
        timeout: 10000,
        retry: 1,
        retryDelay: 500,
        transform: (payload) => normalizeDownloadCatalog(payload, endpoints),
    })
    const release = computed(() => (status.value === 'success' ? data.value : null))
    const links = computed(() =>
        resolveLauncherDownloads(release.value, resolvedSource.value.id, sources),
    )

    return { release, links, status, refresh }
}
