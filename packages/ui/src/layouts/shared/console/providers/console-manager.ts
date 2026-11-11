import type { ComputedRef, Ref } from 'vue'

import { createContext } from '#ui/providers/create-context'

import type { LogLine, LogSource } from '../types'

export interface LocalCrashAnalysis {
    crashed: boolean
    sources: Array<{ filename: string; source_type: string; line_count: number }>
    findings: Array<{
        id: string
        confidence: string
        evidence: Array<{ filename: string; line: number; text: string }>
    }>
    mods: Array<{
        file_name: string
        id?: string
        name?: string
        matched_class?: string
    }>
    mod_changes: Array<{
        kind: 'added' | 'removed' | 'modified'
        filename: string
        previous_size?: number
        current_size?: number
        current_sha256?: string
        project_id?: string
        project_title?: string
        icon_url?: string
        version_id?: string
        version_number?: string
    }>
    mod_change_counts: { added: number; removed: number; modified: number }
    windows_events: Array<{
        event_id: number
        provider: string
        time_created: string
        message: string
    }>
}

export interface ConsoleManagerContext {
    logLines: Ref<LogLine[]>

    logSources?: ComputedRef<LogSource[]>
    activeLogSourceIndex?: Ref<number>

    sendCommand?: (cmd: string) => void
    showCommandInput?: boolean | Ref<boolean>
    disableCommandInput?: boolean | Ref<boolean>
    disableCommandInputTooltip?: string | Ref<string | undefined>

    loading?: Ref<boolean>

    onClear?: () => void
    clearDisabled?: Ref<boolean>
    clearDisabledTooltip?: string | Ref<string | undefined>
    onDelete?: () => Promise<void>
    onExportLog?: (() => Promise<void>) | Ref<(() => Promise<void>) | undefined>
    deleteDisabled?: Ref<boolean>
    deleteDisabledTooltip?: string | Ref<string | undefined>

    shareDisabled?: Ref<boolean>

    emptyStateType?: 'server' | 'instance'

    localCrashAnalysis?: Ref<LocalCrashAnalysis | null>
    crashAnalysisLoading?: Ref<boolean>
    onExportCrashContext?: () => Promise<void>
}

export const [injectConsoleManager, provideConsoleManager] = createContext<ConsoleManagerContext>(
    'ConsolePageLayout',
    'consoleManagerContext',
)
