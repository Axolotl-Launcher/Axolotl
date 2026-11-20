<script setup lang="ts">
import { CodeIcon, FileArchiveIcon } from '@modrinth/assets'
import type {
    EditingFile,
    ExtractDryRunResult,
    FileContextMenuOption,
    FileItem,
} from '@modrinth/ui'
import {
    Button,
    commonMessages,
    defineMessages,
    FilePageLayout,
    injectNotificationManager,
    provideFileManager,
    ReadyTransition,
    useDebugLogger,
    useVIntl,
} from '@modrinth/ui'
import { hashKey, replaceEqualDeep, useQueryClient } from '@tanstack/vue-query'
import { join } from '@tauri-apps/api/path'
import {
    mkdir,
    readFile as readFileBytes,
    readTextFile,
    remove,
    rename,
    writeTextFile,
} from '@tauri-apps/plugin-fs'
import { computed, nextTick, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import { useScopedEventListeners } from '@/composables/use-scoped-event-listeners'
import { instance_listener } from '@/helpers/events'
import { get_full_path } from '@/helpers/instance'
import { invoke } from '@/helpers/invoke'
import { createRequestGeneration } from '@/helpers/request-generation'
import type { GameInstance } from '@/helpers/types'
import { highlightInFolder } from '@/helpers/utils'

import { readInstanceDirectory, readInstanceDirectoryCounts } from './files-directory'
import { instanceDirectoryScope, retainInstanceView } from './instance-view-cache'

const props = defineProps<{
    instance: GameInstance
    options: unknown
    offline: boolean
    playing: boolean
    installed: boolean
    isServerInstance: boolean
}>()

const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()
const debug = useDebugLogger('Files')
const router = useRouter()
const queryClient = useQueryClient()

const messages = defineMessages({
    saveAs: {
        id: 'instance.files.save-as',
        defaultMessage: 'Save as...',
    },
    addingFiles: {
        id: 'instance.files.adding-files',
        defaultMessage: 'Adding files ({completed}/{total})',
    },
    openInSchematicWorkshop: {
        id: 'instance.files.open-in-schematic-workshop',
        defaultMessage: 'Open in schematic workshop',
    },
    openStudio: {
        id: 'instance.files.open-studio',
        defaultMessage: 'Open Studio',
    },
})

const rootKey = computed(
    () => ['instance-file-root', instanceDirectoryScope(props.instance)] as const,
)
const rootScope = computed(() => hashKey(rootKey.value))
const directoryKey = (root: string, path: string) =>
    ['instance-directory', props.instance.id, root, path] as const
const instanceRoot = ref(queryClient.getQueryData<string>(rootKey.value) ?? '')
const initialItems = instanceRoot.value
    ? queryClient.getQueryData<FileItem[]>(directoryKey(instanceRoot.value, ''))
    : undefined
const items = ref<FileItem[]>(initialItems?.map((item) => ({ ...item })) ?? [])
const firstPaintPending = ref(!initialItems)
const loading = ref(!initialItems)
const error = ref<Error | null>(null)
const currentPath = ref('')
const editingFile = ref<EditingFile | null>(null)
const refreshRequests = createRequestGeneration()
const rootRequests = createRequestGeneration()
const { subscribe, isDisposed } = useScopedEventListeners()
retainInstanceView(rootKey)
retainInstanceView(computed(() => directoryKey(instanceRoot.value, currentPath.value)))

let displayedDirectory = initialItems ? hashKey(directoryKey(instanceRoot.value, '')) : undefined
let directorySubscription: Promise<void> | undefined

async function resolvePath(relativePath: string): Promise<string> {
    return relativePath ? join(instanceRoot.value, ...relativePath.split('/')) : instanceRoot.value
}

async function refresh() {
    if (isDisposed()) return
    if (!instanceRoot.value) {
        await readRoot()
        return
    }
    const requestedRoot = instanceRoot.value
    const requestedPath = currentPath.value
    const key = directoryKey(requestedRoot, requestedPath)
    const scope = hashKey(key)
    const request = refreshRequests.begin(scope)
    const isCurrent = () =>
        !isDisposed() &&
        refreshRequests.isCurrent(
            request,
            hashKey(directoryKey(instanceRoot.value, currentPath.value)),
        )
    debug('refresh: called, currentPath =', currentPath.value, 'instanceRoot =', instanceRoot.value)
    if (displayedDirectory !== scope) {
        const cached = queryClient.getQueryData<FileItem[]>(key)
        items.value = cached?.map((item) => ({ ...item })) ?? []
        displayedDirectory = cached ? scope : undefined
    }
    loading.value = displayedDirectory !== scope
    error.value = null
    try {
        const absolutePath = requestedPath
            ? await join(requestedRoot, ...requestedPath.split('/'))
            : requestedRoot
        if (!isCurrent()) return
        const nextItems = await readInstanceDirectory(absolutePath, requestedPath, isCurrent)
        if (!isCurrent()) return
        const previousByPath = new Map(items.value.map((item) => [item.path, item]))
        const reconciled = nextItems.map((item) => {
            const previous = previousByPath.get(item.path)
            return replaceEqualDeep(
                previous,
                previous?.count === undefined || item.type !== 'directory'
                    ? item
                    : { ...item, count: previous.count },
            )
        })
        if (
            reconciled.length !== items.value.length ||
            reconciled.some((item, index) => item !== items.value[index])
        ) {
            items.value = reconciled
        }
        displayedDirectory = scope
        queryClient.setQueryData(
            key,
            items.value.map((item) => ({ ...item })),
        )
        debug('refresh: success, items =', items.value.length)
        void loadDirectoryCounts(absolutePath, items.value, isCurrent).then(() => {
            if (isCurrent())
                queryClient.setQueryData(
                    key,
                    items.value.map((item) => ({ ...item })),
                )
        })
    } catch (e) {
        debug('refresh: error =', e)
        if (!isCurrent()) return
        error.value = e instanceof Error ? e : new Error(String(e))
    } finally {
        if (isCurrent()) {
            loading.value = false
            firstPaintPending.value = false
        }
    }
}

async function loadDirectoryCounts(
    absolutePath: string,
    directoryItems: FileItem[],
    isCurrent: () => boolean,
) {
    await nextTick()
    // Let the new list paint before serializing entries from large child folders.
    await new Promise<void>((resolve) => requestAnimationFrame(() => setTimeout(resolve, 0)))
    if (isCurrent()) await readInstanceDirectoryCounts(absolutePath, directoryItems, isCurrent)
}

function navigateTo(path: string) {
    debug('navigateTo:', path)
    currentPath.value = path.startsWith('/') ? path.slice(1) : path
    void refresh()
}

function startEditing(file: EditingFile) {
    editingFile.value = file
}

function stopEditing() {
    editingFile.value = null
}

async function handleCreateItem(name: string, type: 'file' | 'directory') {
    const targetPath = currentPath.value ? `${currentPath.value}/${name}` : name
    const absPath = await resolvePath(targetPath)
    try {
        if (type === 'directory') {
            await mkdir(absPath)
        } else {
            await writeTextFile(absPath, '')
        }
        await refresh()
    } catch (e) {
        addNotification({
            title: formatMessage(commonMessages.createFailedLabel),
            text: e instanceof Error ? e.message : '',
            type: 'error',
        })
    }
}

async function handleRenameItem(path: string, newName: string) {
    const oldAbs = await resolvePath(path)
    const parentDir = path.includes('/') ? path.substring(0, path.lastIndexOf('/')) : ''
    const newPath = parentDir ? `${parentDir}/${newName}` : newName
    const newAbs = await resolvePath(newPath)
    try {
        await rename(oldAbs, newAbs)
        await refresh()
    } catch (e) {
        addNotification({
            title: formatMessage(commonMessages.renameFailedLabel),
            text: e instanceof Error ? e.message : '',
            type: 'error',
        })
    }
}

async function handleMoveItem(source: string, destination: string) {
    try {
        await rename(await resolvePath(source), await resolvePath(destination))
        await refresh()
    } catch (e) {
        addNotification({
            title: formatMessage(commonMessages.moveFailedLabel),
            text: e instanceof Error ? e.message : '',
            type: 'error',
        })
    }
}

async function handleDeleteItem(path: string, recursive: boolean) {
    try {
        await remove(await resolvePath(path), { recursive })
        await refresh()
    } catch (e) {
        addNotification({
            title: formatMessage(commonMessages.deleteFailedLabel),
            text: e instanceof Error ? e.message : '',
            type: 'error',
        })
    }
}

async function handleReadFile(path: string): Promise<string> {
    return await readTextFile(await resolvePath(path))
}

async function handleReadFileAsBlob(path: string): Promise<Blob> {
    const bytes = await readFileBytes(await resolvePath(path))
    return new Blob([bytes])
}

async function handleWriteFile(path: string, content: string) {
    await writeTextFile(await resolvePath(path), content)
}

async function handleDownloadFile(path: string, _fileName: string) {
    await invoke('plugin:files|file_save_as', {
        instanceId: props.instance.id,
        filePath: path,
    })
}

async function handleExtractFile(path: string, override: boolean, dry: boolean) {
    try {
        return (
            (await invoke<ExtractDryRunResult | null>('plugin:files|file_extract_zip', {
                instanceId: props.instance.id,
                filePath: path,
                overrideConflicts: override,
                dryRun: dry,
            })) ?? undefined
        )
    } catch (e) {
        addNotification({
            title: formatMessage(commonMessages.extractFailedLabel),
            text: e instanceof Error ? e.message : '',
            type: 'error',
        })
    }
}

function getAdditionalMenuOptions(
    item: Pick<FileItem, 'name' | 'type' | 'path'>,
): FileContextMenuOption[] {
    if (item.type !== 'file' || !/\.(litematic|schem)$/i.test(item.name)) return []

    return [
        {
            id: 'open-in-schematic-workshop',
            label: formatMessage(messages.openInSchematicWorkshop),
            icon: FileArchiveIcon,
            action: () => {
                void router.push({
                    name: 'SchematicWorkshop',
                    query: { instance: props.instance.id, path: item.path },
                })
            },
        },
    ]
}

async function readRoot() {
    const key = rootKey.value
    const scope = rootScope.value
    const request = rootRequests.begin(scope)
    const valid = () => !isDisposed() && rootRequests.isCurrent(request, rootScope.value)
    try {
        const root = await get_full_path(props.instance.id)
        if (!valid()) return
        directorySubscription ??= subscribe(
            instance_listener,
            async (event: { event: string; instance_id: string }) => {
                if (
                    instanceRoot.value &&
                    event.instance_id === props.instance.id &&
                    event.event === 'synced'
                )
                    await refresh()
            },
        ).catch((cause) => debug('Could not listen for directory changes', cause))
        await directorySubscription
        if (!valid()) return
        instanceRoot.value = root
        queryClient.setQueryData(key, root)
        await refresh()
    } catch (cause) {
        if (!valid()) return
        error.value = cause instanceof Error ? cause : new Error(String(cause))
        firstPaintPending.value = false
        loading.value = false
    }
}

watch(rootScope, () => {
    firstPaintPending.value = true
    loading.value = true
    refreshRequests.invalidate()
    instanceRoot.value = ''
    currentPath.value = ''
    items.value = []
    displayedDirectory = undefined
    void readRoot()
})

provideFileManager({
    items,
    loading,
    error,
    currentPath,
    navigateTo,
    editingFile,
    startEditing,
    stopEditing,
    createItem: handleCreateItem,
    renameItem: handleRenameItem,
    moveItem: handleMoveItem,
    deleteItem: handleDeleteItem,
    readFile: handleReadFile,
    readFileAsBlob: handleReadFileAsBlob,
    writeFile: handleWriteFile,
    downloadFile: handleDownloadFile,
    extractFile: handleExtractFile,
    refresh,
    basePath: instanceRoot,
    openInFolder: (path: string) => highlightInFolder(path),
    getAdditionalMenuOptions,
    downloadButtonLabel: formatMessage(messages.saveAs),
    uploadingLabel: (completed: number, total: number) =>
        formatMessage(messages.addingFiles, { completed, total }),
    symlinkTarget: computed(() => props.instance.symlink_target),
})

void readRoot()
</script>

<template>
    <ReadyTransition :pending="firstPaintPending">
        <div class="flex flex-col gap-4">
            <FilePageLayout :show-refresh-button="true">
                <template #before-refresh>
                    <Button
                        type="colored"
                        color="brand"
                        class="!h-10"
                        @click="router.push({ name: 'FileStudio', params: { id: instance.id } })"
                        ><CodeIcon class="size-5" />
                        <span class="inline-flex items-center gap-1">
                            {{ formatMessage(messages.openStudio) }}
                            <span
                                class="rounded bg-orange px-1.5 py-0.5 text-[10px] font-bold uppercase leading-none text-[var(--color-text-primary)]"
                            >
                                {{ formatMessage(commonMessages.beta) }}
                            </span>
                        </span>
                    </Button>
                </template>
            </FilePageLayout>
        </div>
    </ReadyTransition>
</template>
