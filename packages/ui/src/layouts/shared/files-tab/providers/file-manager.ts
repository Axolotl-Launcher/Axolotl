import type { ComputedRef, Ref } from 'vue'

import { createContext } from '#ui/providers/create-context'

import type {
    EditingFile,
    ExtractDryRunResult,
    FileContextMenuOption,
    FileItem,
    FileOperation,
} from '../types'

export interface FileManagerContext {
    items: Ref<FileItem[]>
    loading: Ref<boolean>
    error: Ref<Error | null>

    currentPath: Ref<string>
    navigateTo: (path: string) => void

    editingFile: Ref<EditingFile | null>
    startEditing: (file: EditingFile) => void
    stopEditing: () => void

    createItem: (name: string, type: 'file' | 'directory') => Promise<void>
    renameItem: (path: string, newName: string) => Promise<void>
    moveItem: (source: string, destination: string) => Promise<void>
    deleteItem: (path: string, recursive: boolean) => Promise<void>

    readFile: (path: string) => Promise<string>
    readFileAsBlob: (path: string) => Promise<Blob>
    writeFile: (path: string, content: string) => Promise<void>
    downloadFile: (path: string, fileName: string) => Promise<void>

    refresh: () => void

    isBusy?: Ref<boolean> | ComputedRef<boolean>
    busyTooltip?: Ref<string | undefined> | ComputedRef<string | undefined>
    busyWarning?: Ref<string | null> | ComputedRef<string | null>

    extractFile?: (
        path: string,
        override: boolean,
        dry: boolean,
    ) => Promise<ExtractDryRunResult | void>
    activeOperations?: Ref<FileOperation[]> | ComputedRef<FileOperation[]>
    dismissOperation?: (id: string, action: 'dismiss' | 'cancel') => void

    prefetchDirectory?: (path: string) => void
    prefetchFile?: (path: string) => void

    basePath?: Ref<string> | ComputedRef<string>
    openInFolder?: (path: string) => void
    getAdditionalMenuOptions?: (
        item: Pick<FileItem, 'name' | 'type' | 'path'>,
    ) => FileContextMenuOption[]

    downloadButtonLabel?: string
    uploadingLabel?: (completed: number, total: number) => string

    canRestart?: boolean
    restartServer?: () => Promise<void>
    canShareLog?: boolean
    shareLogs?: (content: string) => Promise<void>

    symlinkTarget?: Ref<string | null | undefined> | ComputedRef<string | null | undefined>
}

export const [injectFileManager, provideFileManager] = createContext<FileManagerContext>(
    'FilePageLayout',
    'fileManagerContext',
)
