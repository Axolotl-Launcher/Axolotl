import {
    type DownloadSource,
    getDownloadSource,
    isDownloadSource,
} from '../utils/download-sources.ts'

const DOWNLOAD_SOURCE_STORAGE_KEY = 'axolotl-download-source'
export function useDownloadSource() {
    const selectedSource = useState<DownloadSource>('axolotl-download-source', () => 'auto')

    const resolvedSource = computed(() => getDownloadSource(selectedSource.value))

    function setDownloadSource(source: DownloadSource) {
        selectedSource.value = source
        if (import.meta.client) {
            try {
                localStorage.setItem(DOWNLOAD_SOURCE_STORAGE_KEY, source)
            } catch {
                // Browser privacy settings can disable storage; the in-memory choice still works.
            }
        }
    }

    onMounted(() => {
        try {
            const savedSource = localStorage.getItem(DOWNLOAD_SOURCE_STORAGE_KEY)
            if (isDownloadSource(savedSource)) selectedSource.value = savedSource
        } catch {
            // Keep the recommended source when persistent storage is unavailable.
        }
    })

    return {
        selectedSource,
        resolvedSource,
        setDownloadSource,
    }
}
