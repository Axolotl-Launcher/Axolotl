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

import { createAnalysisTask } from '@/helpers/analysis-task'
import { resourceOperationError } from '@/helpers/resource-operation-logic'
import {
    type ExportCategory,
    type ExportFileSnapshot,
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

type RefreshInput = {
    instanceId: string
    sessionId: string
    files: ExportFileSnapshot[]
    revision: number
    /** Only the sources that failed last time, when the selection did not change. */
    failed: Parameters<typeof resourceExportRefreshRetry>[1] | null
}

export function createExportContext(
    modal: ShallowRef<ComponentExposed<typeof MultiStageModal> | null>,
    instance: () => { id: string; name: string },
) {
    const { formatMessage } = useVIntl()
    const { handleError } = injectNotificationManager()
    const notifications = injectPopupNotificationManager()
    const snapshot = ref<ResourceExportScan | null>(null)
    const scanTask = createAnalysisTask<string, never, ResourceExportScan>({
        label: 'export-scan',
        run: (instanceId, { signal }) => resourceExportScan(instanceId, signal),
        // A scan that finishes after the modal moved on still opened a session.
        discard: (late) => void resourceExportClose(late.sessionId).catch(() => {}),
    })
    const refreshTask = createAnalysisTask<
        RefreshInput,
        ExportRefreshProgress,
        ExportResolutionSession
    >({
        label: 'export-refresh',
        run(input, { signal, progress }) {
            const onProgress = (value: ExportRefreshProgress) => {
                if (
                    value.sessionId === input.sessionId &&
                    value.selectionRevision === input.revision
                )
                    progress(value)
            }
            return input.failed
                ? resourceExportRefreshRetry(
                      input.sessionId,
                      input.failed,
                      input.revision,
                      onProgress,
                      signal,
                  )
                : resourceExportRefresh(
                      input.instanceId,
                      input.sessionId,
                      input.files,
                      input.revision,
                      onProgress,
                      signal,
                  )
        },
    })
    const scanning = computed(() => scanTask.running.value)
    const error = computed(() =>
        scanTask.status.value === 'error'
            ? resourceOperationError(scanTask.error.value, formatMessage)
            : '',
    )
    const exporting = ref(false)
    const refreshing = computed(() => refreshTask.running.value)
    /** Kept across verification runs, so sizes and blockers stay while a new one runs. */
    const resolution = ref<ExportResolutionSession | null>(null)
    const refreshProgress = computed(() => refreshTask.progress.value)
    const refreshError = computed(() =>
        refreshTask.status.value === 'error'
            ? resourceOperationError(refreshTask.error.value, formatMessage)
            : '',
    )
    const embedded = ref<string[]>([])
    let revision = 0
    /** Changes whenever the modal is closed or reopened, so a pending export can tell. */
    let session = 0
    const draft = ref<ExportSelectionDraft>({ categories: [], included: [], excluded: [] })
    const currentPath = ref('')
    const search = ref('')
    const selectedOnly = ref(false)
    const packageInfo = ref({ name: '', version: '1.0.0', description: '', author: '' })
    const format = ref<ResourceExportFormat>('modrinth')
    const exportError = ref('')
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
    async function refresh(retry = false) {
        if (!snapshot.value || exporting.value) return
        const candidates = files.value
        const paths = new Set(candidates.map((file) => file.relativePath))
        // Every run gets a new revision, so a retry is a new start, not task.retry().
        const currentRevision = ++revision
        embedded.value = []
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
        const input: RefreshInput = {
            instanceId: snapshot.value.instanceId,
            sessionId: snapshot.value.sessionId,
            files: candidates,
            revision: currentRevision,
            failed:
                retry &&
                sameVerificationScope &&
                failed.length &&
                !resolution.value?.files.some((file) => file.localError)
                    ? failed
                    : null,
        }
        const result = await refreshTask.start(input)
        if (result && refreshTask.input.value === input) resolution.value = result
    }
    function embed(path: string, accepted: boolean) {
        embedded.value = accepted
            ? [...new Set([...embedded.value, path])]
            : embedded.value.filter((value) => value !== path)
    }
    function close() {
        session++
        scanTask.reset()
        refreshTask.reset()
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
    async function scan() {
        const result = await scanTask.start(instance().id)
        if (!result) return
        // The modal closed between the scan finishing and this continuation.
        if (scanTask.result.value !== result) {
            void resourceExportClose(result.sessionId).catch(() => {})
            return
        }
        snapshot.value = result
        draft.value = {
            categories: defaultCategories(result.categories),
            included: [],
            excluded: [],
        }
        void refresh()
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
        exportError.value = ''
        modal.value?.setStage('categories')
        modal.value?.show()
        void scan()
    }
    async function exportPack() {
        if (exporting.value || !snapshot.value || !canExport.value) return
        exporting.value = true
        exportError.value = ''
        let queued = false
        const activeSession = session
        try {
            const extension = format.value === 'modrinth' ? 'mrpack' : 'zip'
            const output = await save({
                defaultPath: `${packageInfo.value.name} ${packageInfo.value.version}.${extension}`,
                filters: [
                    { name: formatMessage(exportMessages[format.value]), extensions: [extension] },
                ],
            })
            if (!output || session !== activeSession || !snapshot.value) return
            void refreshTask.cancel()
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
            if (session !== activeSession) return
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
            if (session !== activeSession) return
            exportError.value = resourceOperationError(failure, formatMessage)
            handleError(failure)
        } finally {
            exporting.value = false
            if (!queued && snapshot.value && !refreshing.value && !resolution.value) void refresh()
        }
    }
    onBeforeUnmount(() => {
        close()
        scanTask.dispose()
        refreshTask.dispose()
    })
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
        embed,
        close,
    }
}

export type ExportContext = ReturnType<typeof createExportContext>
export const [injectExportContext, provideExportContext] =
    createContext<ExportContext>('ExportModal')
