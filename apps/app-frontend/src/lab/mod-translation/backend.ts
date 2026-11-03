import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import { abortable } from '../../helpers/abortable.ts'
import type {
    ModTranslationAnalysis,
    ModTranslationOptions,
    ModTranslationTaskEvent,
    ModTranslationTaskSnapshot,
} from './types.ts'
export { modTranslationPercent } from './job-state.ts'

const TASK_EVENT = 'mod-translation-task-event'

/**
 * Analyzes a mod JAR. Aborting stops the native analysis at its next
 * checkpoint; `onStop` receives the acknowledgement of that stop request.
 */
export async function analyzeMod(
    inputPath: string,
    signal?: AbortSignal,
    onStop?: (confirmation: Promise<unknown>) => void,
): Promise<ModTranslationAnalysis> {
    signal?.throwIfAborted()
    const requestId = crypto.randomUUID()
    const request = invoke<ModTranslationAnalysis>(
        'plugin:mod-translation|mod_translation_analyze',
        { inputPath, requestId },
    )
    return await abortable(request, signal, () => {
        const stopped = invoke('plugin:mod-translation|mod_translation_cancel_analysis', {
            requestId,
        })
        onStop?.(stopped)
        void stopped.catch(() => {})
    })
}

export async function translateMod(request: {
    inputPath: string
    outputPath: string
    providerId: string
    modelId: string
    analysisId?: string
    inputHash?: string
    options: ModTranslationOptions
}): Promise<ModTranslationTaskSnapshot> {
    return await invoke<ModTranslationTaskSnapshot>(
        'plugin:mod-translation|mod_translation_translate',
        {
            inputPath: request.inputPath,
            outputPath: request.outputPath,
            providerId: request.providerId,
            modelId: request.modelId,
            analysisId: request.analysisId,
            inputHash: request.inputHash,
            options: request.options,
        },
    )
}

export async function cancelModTranslation(taskId: string): Promise<void> {
    await invoke('plugin:mod-translation|mod_translation_cancel', { taskId })
}

export async function listModTranslationTasks(): Promise<ModTranslationTaskSnapshot[]> {
    return await invoke<ModTranslationTaskSnapshot[]>(
        'plugin:mod-translation|mod_translation_list_tasks',
    )
}

export async function getModTranslationTask(
    taskId: string,
): Promise<ModTranslationTaskSnapshot | null> {
    return await invoke<ModTranslationTaskSnapshot | null>(
        'plugin:mod-translation|mod_translation_get_task',
        { taskId },
    )
}

export async function dismissModTranslationTask(taskId: string): Promise<void> {
    await invoke('plugin:mod-translation|mod_translation_dismiss_task', { taskId })
}

export async function listenToModTranslationTasks(
    handler: (event: ModTranslationTaskEvent) => void,
): Promise<UnlistenFn> {
    return await listen<ModTranslationTaskEvent>(TASK_EVENT, (event) => handler(event.payload))
}
