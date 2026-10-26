import { onMounted, onUnmounted, ref } from 'vue'

import { useDebugLogger } from '#ui/composables/debug-logger'
import type { FileDropProvider, NativeFileDropEvent } from '#ui/providers/file-drop'
import { injectFileDrop } from '#ui/providers/file-drop'
import { injectLoadingState } from '#ui/providers/loading-state'

/**
 * Classification result returned by the Rust backend.
 */
export interface ClassificationResult {
    item_type:
        | 'server'
        | 'launcher'
        | 'hmcl_launcher'
        | 'mod'
        | 'modpack'
        | 'litematic'
        | 'resource_pack'
        | 'shader_pack'
        | 'world_save'
        | 'shortcut_resolved'
        | 'multiple'
        | 'unknown'
    file_path?: string
    candidates?: string[]
    choices?: ClassificationChoice[]
    launcher_type?: string
    base_path?: string
    /** For ZIP sources, the virtual folder inside the archive where launcher
     * markers matched (e.g. `.minecraft`). Scan/import against
     * `<extracted-temp>/<inner_base>`. */
    innerBase?: string
    launcher_dir?: string
    data_dir?: string
    original?: string
    resolved_to?: ClassificationResult
    reason?: string
}

export interface ClassificationChoice {
    itemType: string
    filePath?: string
    innerBase?: string
    candidates?: string[]
}

export interface UseGlobalDropOptions {
    /**
     * Function that classifies a dropped file by its filesystem path.
     * Typically wraps `classifyDroppedItem` from `@/helpers/drop`.
     */
    classifyFile: (path: string) => Promise<ClassificationResult>

    /** Called when classification begins (before the loading bar). Receives the file name. */
    onClassifyStart?: (fileName: string) => void

    /**
     * Called when a droppable item is successfully classified.
     * @param type  - 'launcher' | 'content'
     * @param data  - The (resolved) ClassificationResult
     */
    onImportStart?: (type: string, data: ClassificationResult) => void

    /** Called after processing ends (success or error). */
    onImportEnd?: () => void

    /**
     * Called when the user drops multiple files. The consumer owns the rest of
     * the batch flow and must call `finishBatch()` when it is done.
     */
    onBatchStart?: (paths: string[]) => void

    /** Called when an error occurs (unknown type, too many files, etc.). */
    onError?: (message: string) => void
}

export function useGlobalDrop(
    options: UseGlobalDropOptions,
    fileDropOverride?: FileDropProvider | null,
) {
    const fileDrop = fileDropOverride ?? injectFileDrop(null)
    const loadingState = injectLoadingState(null)
    const debug = useDebugLogger('useGlobalDrop')

    const isDragging = ref(false)
    const isProcessing = ref(false)
    const droppedFileName = ref<string | null>(null)

    let nativeFileDropUnlisten: (() => void) | null = null
    let unmounted = false

    /**
     * Walk the `shortcut_resolved` chain up to 3 levels.
     * If still a shortcut after depth 3, return the terminal node
     * (the caller will treat it as unknown).
     */
    function resolveClassification(result: ClassificationResult, depth = 0): ClassificationResult {
        if (result.item_type === 'shortcut_resolved' && result.resolved_to && depth < 3) {
            debug('resolveClassification: shortcut chain', {
                depth,
                from: result.file_path,
                to: result.resolved_to.file_path,
            })
            return resolveClassification(result.resolved_to, depth + 1)
        }
        if (depth > 0)
            debug('resolveClassification: resolved at depth', {
                depth,
                item_type: result.item_type,
                file_path: result.file_path,
            })
        return result
    }

    async function handleNativeDrop(event: NativeFileDropEvent) {
        if (event.type === 'enter' || event.type === 'over') {
            isDragging.value = true
            return
        }

        if (event.type === 'leave') {
            isDragging.value = false
            return
        }

        // ── type === 'drop' ──
        isDragging.value = false

        const { paths } = event
        if (!paths?.length) {
            debug('handleNativeDrop: drop event with empty paths — skipping')
            return
        }

        // Multiple files start a batch flow owned by the consumer.
        if (paths.length > 1) {
            console.log('[BatchDrop] native drop MULTI paths=', paths)
            if (options.onBatchStart) {
                isProcessing.value = true
                droppedFileName.value = null
                options.onBatchStart(paths)
                return
            }
            options.onError?.('multiple-files')
            return
        }

        if (isProcessing.value) {
            debug('handleNativeDrop: drop while another file is still processing — skipping')
            return
        }

        isProcessing.value = true
        droppedFileName.value = paths[0].split(/[/\\]/).pop() ?? 'file'
        options.onClassifyStart?.(droppedFileName.value)

        const loadToken = loadingState?.begin()

        try {
            const raw = await options.classifyFile(paths[0])
            debug('classifyFile raw result', {
                item_type: raw.item_type,
                file_path: raw.file_path,
                launcher_type: raw.launcher_type,
                reason: raw.reason,
            })

            const resolved = resolveClassification(raw)
            debug('resolveClassification final', {
                item_type: resolved.item_type,
                file_path: resolved.file_path,
                launcher_type: resolved.launcher_type,
            })

            if (resolved.item_type === 'unknown') {
                debug(
                    'routing: unknown type — passing to onImportStart so the consumer can decide',
                    {
                        reason: resolved.reason,
                    },
                )
                options.onImportStart?.('unknown', resolved)
                return
            }

            // If still a shortcut after max depth — treat as unknown
            if (resolved.item_type === 'shortcut_resolved') {
                debug('routing: shortcut exceeded max depth', { file_path: resolved.file_path })
                options.onError?.('shortcut-exceeded')
                return
            }

            if (resolved.item_type === 'launcher' || resolved.item_type === 'hmcl_launcher') {
                debug('routing: launcher import', {
                    launcher_type: resolved.launcher_type,
                    base_path: resolved.base_path,
                    data_dir: resolved.data_dir,
                })
                options.onImportStart?.('launcher', resolved)
                return
            }

            // Content types: mod, litematic, resource_pack, shader_pack, world_save
            debug('routing: content import', { item_type: resolved.item_type })
            options.onImportStart?.('content', resolved)
        } catch (error) {
            const message = error instanceof Error ? error.message : String(error)
            options.onError?.(message)
        } finally {
            isProcessing.value = false
            if (loadToken) loadingState?.end(loadToken)
            options.onImportEnd?.()
        }
    }

    async function setup() {
        if (!fileDrop) {
            debug('setup: fileDrop provider not available — native drops disabled')
            return
        }

        let unlisten: () => void
        try {
            unlisten = await fileDrop.listenNativeFileDrop(handleNativeDrop)
            debug('setup: native file drop listener registered successfully')
        } catch (err) {
            debug('setup: failed to register native file drop listener', err)
            return
        }

        if (unmounted) {
            debug('setup: component unmounted before listener was ready, cleaning up')
            unlisten()
            return
        }

        nativeFileDropUnlisten = unlisten
    }

    onMounted(() => {
        void setup()
    })

    onUnmounted(() => {
        unmounted = true
        isDragging.value = false
        isProcessing.value = false
        if (nativeFileDropUnlisten) {
            nativeFileDropUnlisten()
            nativeFileDropUnlisten = null
        }
    })

    /** Ends the batch flow and clears the global processing flag. */
    function finishBatch() {
        isProcessing.value = false
        options.onImportEnd?.()
    }

    return {
        isDragging,
        isProcessing,
        droppedFileName,
        finishBatch,
    }
}
