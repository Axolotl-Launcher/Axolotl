import {
    createContext,
    injectNotificationManager,
    injectPopupNotificationManager,
    type MultiStageModal,
    useVIntl,
} from '@modrinth/ui'
import { save } from '@tauri-apps/plugin-dialog'
import { computed, nextTick, onBeforeUnmount, ref, type ShallowRef } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'

import { resourceOperationError } from '@/helpers/resource-operation-logic'
import {
    type ExportCategory,
    type ExportRefreshProgress,
    type ExportResolutionSession,
    resourceExportClose,
    type ResourceExportFormat,
    resourceExportQueue,
    resourceExportRefresh,
    resourceExportRefreshRetry,
    type ResourceExportScan,
    resourceExportScan,
} from '@/helpers/resources'

import { categoryMessages, exportMessages } from './messages'
import { pendingFailures } from './resolution'
import {
    categoryLeaves,
    defaultCategories,
    type ExportSelectionDraft,
    reviseCategorySelection,
    reviseFileSelection,
    selectedFiles,
} from './selection'

export function createExportContext(
    modal: ShallowRef<ComponentExposed<typeof MultiStageModal> | null>,
    instance: () => { id: string; name: string },
) {
    const { formatMessage } = useVIntl()
    const { handleError } = injectNotificationManager()
    const notifications = injectPopupNotificationManager()
    const snapshot = ref<ResourceExportScan | null>(null)
    const scanning = ref(false)
    const error = ref('')
    const exporting = ref(false)
    const refreshing = ref(false)
    const resolution = ref<ExportResolutionSession | null>(null)
    const refreshProgress = ref<ExportRefreshProgress | null>(null)
    const refreshError = ref('')
    const embedded = ref<string[]>([])
    let refreshController: AbortController | undefined
    let refreshGeneration = 0
    let revision = 0
    const draft = ref<ExportSelectionDraft>({ categories: [], included: [], excluded: [] })
    const currentPath = ref('')
    const search = ref('')
    const selectedOnly = ref(false)
    const packageInfo = ref({ name: '', version: '1.0.0', description: '', author: '' })
    const format = ref<ResourceExportFormat>('modrinth')
    const exportError = ref('')
    let controller: AbortController | undefined
    let generation = 0
    const files = computed(() => snapshot.value?.files ?? [])
    const selection = computed({
        get: () => selectedFiles(files.value, draft.value),
        set: (paths: string[]) => {
            draft.value = reviseFileSelection(files.value, draft.value, paths)
        },
    })
    const selectedSize = computed(() => {
        const paths = new Set(selection.value)
        const frozen = new Map(
            resolution.value?.files.map((file) => [
                file.snapshot.relativePath,
                file.snapshot.integrity.size,
            ]) ?? [],
        )
        return files.value.reduce(
            (size, file) =>
                size +
                (paths.has(file.relativePath)
                    ? (frozen.get(file.relativePath) ?? file.integrity.size ?? 0)
                    : 0),
            0,
        )
    })
    const selectedPathSet = computed(() => new Set(selection.value))
    const canExport = computed(
        () =>
            !!snapshot.value &&
            !scanning.value &&
            !exporting.value &&
            !resolution.value?.files.some(
                (file) => selectedPathSet.value.has(file.snapshot.relativePath) && file.localError,
            ),
    )
    const failedFiles = computed(
        () =>
            resolution.value?.files.filter(
                (file) =>
                    selectedPathSet.value.has(file.snapshot.relativePath) &&
                    (file.localError || file.resolutions.some((r) => r.kind === 'failed')),
            ) ?? [],
    )
    function cancelRefresh() {
        refreshGeneration++
        refreshController?.abort()
        refreshing.value = false
    }
    async function refresh(retry = false) {
        if (!snapshot.value || exporting.value) return
        cancelRefresh()
        refreshController = new AbortController()
        const active = refreshController
        const generation = refreshGeneration
        const candidates = files.value
        const paths = new Set(candidates.map((file) => file.relativePath))
        const session = snapshot.value.sessionId
        const currentRevision = ++revision
        refreshing.value = true
        refreshError.value = ''
        refreshProgress.value = null
        embedded.value = []
        try {
            const onProgress = (progress: ExportRefreshProgress) => {
                if (
                    generation === refreshGeneration &&
                    progress.sessionId === session &&
                    progress.selectionRevision === currentRevision
                )
                    refreshProgress.value = progress
            }
            const failed =
                resolution.value?.files.flatMap((file) =>
                    file.resolutions
                        .filter((r) => r.kind === 'failed')
                        .map((r) => ({ snapshot: file.snapshot, source: r.source })),
                ) ?? []
            const sameVerificationScope =
                resolution.value?.selectionRevision === currentRevision - 1 &&
                resolution.value.files.length === candidates.length &&
                resolution.value.files.every((file) => paths.has(file.snapshot.relativePath))
            const result =
                retry &&
                sameVerificationScope &&
                failed.length &&
                !resolution.value?.files.some((file) => file.localError)
                    ? await resourceExportRefreshRetry(
                          session,
                          failed,
                          currentRevision,
                          onProgress,
                          active.signal,
                      )
                    : await resourceExportRefresh(
                          snapshot.value.instanceId,
                          session,
                          candidates,
                          currentRevision,
                          onProgress,
                          active.signal,
                      )
            if (generation !== refreshGeneration) return
            resolution.value = result
        } catch (failure) {
            if (generation === refreshGeneration && !active.signal.aborted)
                refreshError.value = resourceOperationError(failure, formatMessage)
        } finally {
            if (generation === refreshGeneration) refreshing.value = false
        }
    }
    function embed(path: string, accepted: boolean) {
        embedded.value = accepted
            ? [...new Set([...embedded.value, path])]
            : embedded.value.filter((value) => value !== path)
    }
    function close() {
        cancelScan()
        cancelRefresh()
        if (snapshot.value) void resourceExportClose(snapshot.value.sessionId).catch(() => {})
        snapshot.value = null
        resolution.value = null
    }
    const categories = computed(
        () =>
            snapshot.value?.categories.filter(
                (category) => categoryFiles(category).length > 0 || category.children.length > 0,
            ) ?? [],
    )
    const treeItems = computed(() => [
        ...(snapshot.value?.directories.map((directory) => ({
            path: directory.relativePath,
            type: 'directory' as const,
            modified: directory.modifiedAt === null ? undefined : directory.modifiedAt / 1000,
        })) ?? []),
        ...files.value.map((file) => ({
            path: file.relativePath,
            type: 'file' as const,
            size: file.integrity.size ?? undefined,
            modified: file.modifiedAt === null ? undefined : file.modifiedAt / 1000,
        })),
    ])

    function categoryFiles(category: ExportCategory) {
        const ids = new Set(categoryLeaves(category).map((leaf) => leaf.id))
        return files.value.filter((file) => ids.has(file.categoryId))
    }
    function categoryState(category: ExportCategory) {
        const ids = categoryLeaves(category).map((leaf) => leaf.id)
        const selected = new Set(draft.value.categories)
        const count = ids.filter((id) => selected.has(id)).length
        return { checked: count === ids.length, indeterminate: count > 0 && count < ids.length }
    }
    function categoryTitle(category: ExportCategory) {
        return (
            category.title ??
            (categoryMessages[category.id as keyof typeof categoryMessages]
                ? formatMessage(categoryMessages[category.id as keyof typeof categoryMessages])
                : category.paths[0])
        )
    }
    function setCategory(category: ExportCategory, selected: boolean) {
        draft.value = reviseCategorySelection(draft.value, category, selected)
    }
    function restore() {
        draft.value = { ...draft.value, included: [], excluded: [] }
    }
    function cancelScan() {
        generation++
        controller?.abort()
        scanning.value = false
    }
    async function scan() {
        cancelScan()
        controller = new AbortController()
        const activeController = controller
        const request = generation
        scanning.value = true
        error.value = ''
        try {
            const result = await resourceExportScan(instance().id, activeController.signal)
            if (generation !== request) {
                await resourceExportClose(result.sessionId).catch(() => {})
                return
            }
            snapshot.value = result
            draft.value = {
                categories: defaultCategories(result.categories),
                included: [],
                excluded: [],
            }
            void refresh()
        } catch (failure) {
            if (generation === request && !activeController.signal.aborted)
                error.value = resourceOperationError(failure, formatMessage)
        } finally {
            if (generation === request) scanning.value = false
        }
    }
    function show() {
        if (exporting.value) return
        close()
        snapshot.value = null
        draft.value = { categories: [], included: [], excluded: [] }
        currentPath.value = ''
        search.value = ''
        selectedOnly.value = false
        packageInfo.value = { name: instance().name, version: '1.0.0', description: '', author: '' }
        format.value = 'modrinth'
        embedded.value = []
        revision = 0
        refreshError.value = ''
        exportError.value = ''
        refreshProgress.value = null
        modal.value?.setStage('categories')
        modal.value?.show()
        void scan()
    }
    async function exportPack() {
        if (exporting.value || !snapshot.value || !canExport.value) return
        exporting.value = true
        exportError.value = ''
        let queued = false
        const activeGeneration = generation
        try {
            const extension = format.value === 'modrinth' ? 'mrpack' : 'zip'
            const output = await save({
                defaultPath: `${packageInfo.value.name} ${packageInfo.value.version}.${extension}`,
                filters: [
                    { name: formatMessage(exportMessages[format.value]), extensions: [extension] },
                ],
            })
            if (!output || generation !== activeGeneration || !snapshot.value) return
            cancelRefresh()
            const selected = new Set(selection.value)
            await resourceExportQueue(
                snapshot.value.sessionId,
                files.value.filter((file) => selected.has(file.relativePath)),
                format.value,
                {
                    name: packageInfo.value.name,
                    version: packageInfo.value.version,
                    summary: packageInfo.value.description,
                    author: packageInfo.value.author.trim() || null,
                },
                pendingFailures(
                    resolution.value
                        ? {
                              ...resolution.value,
                              files: resolution.value.files.filter((file) =>
                                  selected.has(file.snapshot.relativePath),
                              ),
                          }
                        : null,
                    embedded.value,
                ),
                output,
            )
            queued = true
            if (generation !== activeGeneration) return
            notifications.addPopupNotification({
                title: formatMessage(exportMessages.queued),
                text: formatMessage(exportMessages.queuedDescription, {
                    name: output.split(/[\\/]/).pop() ?? output,
                }),
                type: 'success',
            })
            exporting.value = false
            await nextTick()
            modal.value?.hide()
        } catch (failure) {
            if (generation !== activeGeneration) return
            exportError.value = resourceOperationError(failure, formatMessage)
            handleError(failure)
        } finally {
            exporting.value = false
            if (!queued && snapshot.value && !refreshing.value && !resolution.value) void refresh()
        }
    }
    onBeforeUnmount(close)
    return {
        modal,
        snapshot,
        scanning,
        error,
        exporting,
        draft,
        currentPath,
        search,
        selectedOnly,
        packageInfo,
        format,
        exportError,
        files,
        categories,
        treeItems,
        selection,
        selectedSize,
        categoryFiles,
        categoryState,
        categoryTitle,
        setCategory,
        restore,
        scan,
        cancelScan,
        show,
        exportPack,
        formatMessage,
        refreshing,
        resolution,
        refreshProgress,
        refreshError,
        embedded,
        canExport,
        failedFiles,
        refresh,
        cancelRefresh,
        embed,
        close,
    }
}

export type ExportContext = ReturnType<typeof createExportContext>
export const [injectExportContext, provideExportContext] =
    createContext<ExportContext>('ExportModal')
