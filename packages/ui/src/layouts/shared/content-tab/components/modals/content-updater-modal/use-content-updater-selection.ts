import type { Labrinth } from '@modrinth/api-client'
import type { Ref } from 'vue'
import { ref, watch } from 'vue'

type UseContentUpdaterSelectionOptions = {
    versions: Readonly<Ref<Labrinth.Versions.v2.Version[]>>
    /** True while more versions may still arrive. */
    loading?: Readonly<Ref<boolean>>
    /** Set when loading stopped early, so the list may be missing versions. */
    loadingError?: Readonly<Ref<string | undefined>>
    currentVersionId: Readonly<Ref<string>>
    onVersionSelect: (version: Labrinth.Versions.v2.Version) => void
    debug: (message: string, data?: Record<string, unknown>) => void
}

export function useContentUpdaterSelection({
    versions,
    loading,
    loadingError,
    currentVersionId,
    onVersionSelect,
    debug,
}: UseContentUpdaterSelectionOptions) {
    const selectedVersion = ref<Labrinth.Versions.v2.Version | null>(null)
    const pendingInitialVersionId = ref<string | undefined>(undefined)
    const pinnedInitialVersionId = ref<string | undefined>(undefined)

    watch(
        [versions, () => (loading?.value ?? false) || !!loadingError?.value],
        ([newVersions, incomplete]) => {
            if (selectedVersion.value) {
                const updatedVersion = newVersions.find((v) => v.id === selectedVersion.value?.id)
                if (updatedVersion && updatedVersion !== selectedVersion.value) {
                    selectedVersion.value = updatedVersion
                }
            }

            if (newVersions.length > 0 && !selectedVersion.value && pendingInitialVersionId.value) {
                const pendingFound = newVersions.find((v) => v.id === pendingInitialVersionId.value)
                debug('versions watcher: initial selection', {
                    pendingInitialVersionId: pendingInitialVersionId.value,
                    foundPending: !!pendingFound,
                    currentVersionId: currentVersionId.value,
                    currentInList: newVersions.some((v) => v.id === currentVersionId.value),
                    totalVersions: newVersions.length,
                    loaderDistribution: [...new Set(newVersions.flatMap((v) => v.loaders))],
                    gameVersionDistribution: [
                        ...new Set(newVersions.flatMap((v) => v.game_versions)),
                    ].slice(0, 10),
                })
                // The requested version may be on a page that has not arrived yet,
                // or on one that failed and will arrive after a retry.
                if (!pendingFound && incomplete) return
                const version = pendingFound ?? newVersions[0]
                selectedVersion.value = version
                if (version) {
                    onVersionSelect(version)
                }
                pendingInitialVersionId.value = undefined
            }
        },
        { deep: true },
    )

    function selectVersion(version: Labrinth.Versions.v2.Version) {
        selectedVersion.value = version
        onVersionSelect(version)
    }

    function resetInitialSelection(initialVersionId?: string) {
        pinnedInitialVersionId.value = initialVersionId
        const incomplete = (loading?.value ?? false) || !!loadingError?.value
        const requestedListed =
            !initialVersionId || versions.value.some((v) => v.id === initialVersionId)

        if (versions.value.length > 0 && (requestedListed || !incomplete)) {
            selectedVersion.value = initialVersionId
                ? (versions.value.find((v) => v.id === initialVersionId) ?? versions.value[0])
                : versions.value[0]
            pendingInitialVersionId.value = undefined
            if (selectedVersion.value) {
                onVersionSelect(selectedVersion.value)
            }
        } else {
            // The requested version may still be on a page that has not arrived.
            selectedVersion.value = null
            pendingInitialVersionId.value = initialVersionId
            debug('show(): requested version not listed yet, deferring selection', {
                pendingInitialVersionId: initialVersionId,
            })
        }
    }

    return {
        selectedVersion,
        pinnedInitialVersionId,
        selectVersion,
        resetInitialSelection,
    }
}
