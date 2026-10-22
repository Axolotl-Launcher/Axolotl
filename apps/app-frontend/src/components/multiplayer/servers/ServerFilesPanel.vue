<script setup lang="ts">
import { CodeIcon } from '@modrinth/assets'
import type { EditingFile, FileItem } from '@modrinth/ui'
import {
    Button,
    commonMessages,
    defineMessages,
    FilePageLayout,
    injectNotificationManager,
    provideFileManager,
    useVIntl,
} from '@modrinth/ui'
import { join } from '@tauri-apps/api/path'
import { save } from '@tauri-apps/plugin-dialog'
import {
    copyFile,
    mkdir,
    readDir,
    readFile as readFileBytes,
    readTextFile,
    remove,
    rename,
    stat,
    writeTextFile,
} from '@tauri-apps/plugin-fs'
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import type { ServerView } from '@/composables/useServers'
import { createRequestGeneration } from '@/helpers/request-generation'
import { highlightInFolder } from '@/helpers/utils'

const props = defineProps<{
    server: ServerView
}>()

const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()
const router = useRouter()

const messages = defineMessages({
    saveAs: {
        id: 'app.servers.files.save-as',
        defaultMessage: 'Save as...',
    },
    busyTooltip: {
        id: 'app.servers.files.busy-tooltip',
        defaultMessage: 'Stop the server to modify files',
    },
    openStudio: {
        id: 'app.servers.files.open-studio',
        defaultMessage: 'Open Studio',
    },
})

const items = ref<FileItem[]>([])
const loading = ref(true)
const error = ref<Error | null>(null)
const currentPath = ref('')
const editingFile = ref<EditingFile | null>(null)
const refreshRequests = createRequestGeneration()

const serverRoot = computed(() => props.server.path)
const isBusy = computed(() => props.server.running)

async function resolvePath(relativePath: string): Promise<string> {
    const clean = relativePath.startsWith('/') ? relativePath.slice(1) : relativePath
    return clean ? join(serverRoot.value, ...clean.split('/')) : serverRoot.value
}

async function listDirectory(dirPath: string): Promise<FileItem[]> {
    const absPath = await resolvePath(dirPath)
    const entries = await readDir(absPath)

    const results = await Promise.all(
        entries.map(async (entry) => {
            const entryAbsPath = await join(absPath, entry.name)
            let metadata
            try {
                metadata = await stat(entryAbsPath)
            } catch {
                return null
            }
            const item: FileItem = {
                name: entry.name,
                type: entry.isDirectory ? 'directory' : 'file',
                path: dirPath ? `${dirPath}/${entry.name}` : entry.name,
                modified: metadata.mtime ? Math.floor(metadata.mtime.getTime() / 1000) : 0,
                created: metadata.birthtime ? Math.floor(metadata.birthtime.getTime() / 1000) : 0,
            }
            if (!entry.isDirectory) {
                item.size = metadata.size
            }
            if (entry.isDirectory) {
                try {
                    const children = await readDir(entryAbsPath)
                    item.count = children.length
                } catch {
                    item.count = 0
                }
            }
            return item
        }),
    )
    return results.filter((item): item is FileItem => item !== null)
}

async function refresh() {
    const request = refreshRequests.begin(`${serverRoot.value}\0${currentPath.value}`)
    const requestedRoot = serverRoot.value
    const requestedPath = currentPath.value
    loading.value = true
    error.value = null
    try {
        const nextItems = await listDirectory(requestedPath)
        if (
            !refreshRequests.isCurrent(request, `${serverRoot.value}\0${currentPath.value}`) ||
            requestedRoot !== serverRoot.value ||
            requestedPath !== currentPath.value
        )
            return
        items.value = nextItems
    } catch (e) {
        if (
            !refreshRequests.isCurrent(request, `${serverRoot.value}\0${currentPath.value}`) ||
            requestedRoot !== serverRoot.value ||
            requestedPath !== currentPath.value
        )
            return
        error.value = e instanceof Error ? e : new Error(String(e))
        items.value = []
    } finally {
        if (refreshRequests.isCurrent(request, `${serverRoot.value}\0${currentPath.value}`))
            loading.value = false
    }
}

function navigateTo(path: string) {
    currentPath.value = path.startsWith('/') ? path.slice(1) : path
    void refresh()
}

function startEditing(file: EditingFile) {
    editingFile.value = file
}

function stopEditing() {
    editingFile.value = null
}

function notifyFailure(label: string, e: unknown) {
    addNotification({
        title: label,
        text: e instanceof Error ? e.message : '',
        type: 'error',
    })
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
        notifyFailure(formatMessage(commonMessages.createFailedLabel), e)
    }
}

async function handleRenameItem(path: string, newName: string) {
    const oldAbs = await resolvePath(path)
    const parentDir = path.includes('/') ? path.substring(0, path.lastIndexOf('/')) : ''
    const newPath = parentDir ? `${parentDir}/${newName}` : newName
    try {
        await rename(oldAbs, await resolvePath(newPath))
        await refresh()
    } catch (e) {
        notifyFailure(formatMessage(commonMessages.renameFailedLabel), e)
    }
}

async function handleMoveItem(source: string, destination: string) {
    try {
        await rename(await resolvePath(source), await resolvePath(destination))
        await refresh()
    } catch (e) {
        notifyFailure(formatMessage(commonMessages.moveFailedLabel), e)
    }
}

async function handleDeleteItem(path: string, recursive: boolean) {
    try {
        await remove(await resolvePath(path), { recursive })
        await refresh()
    } catch (e) {
        notifyFailure(formatMessage(commonMessages.deleteFailedLabel), e)
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

async function handleDownloadFile(path: string, fileName: string) {
    const outputPath = await save({ defaultPath: fileName })
    if (!outputPath) return
    await copyFile(await resolvePath(path), outputPath)
}

watch(
    () => props.server.path,
    async () => {
        refreshRequests.invalidate()
        currentPath.value = ''
        await refresh()
    },
    { immediate: true },
)

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
    refresh,
    isBusy,
    busyTooltip: computed(() => (isBusy.value ? formatMessage(messages.busyTooltip) : undefined)),
    basePath: serverRoot,
    openInFolder: (path: string) => highlightInFolder(path),
    downloadButtonLabel: formatMessage(messages.saveAs),
})
</script>

<template>
    <div class="min-h-0 w-full">
        <FilePageLayout :show-refresh-button="true">
            <template #before-refresh>
                <Button
                    v-tooltip="isBusy ? formatMessage(messages.busyTooltip) : undefined"
                    type="colored"
                    color="brand"
                    class="!h-10"
                    :disabled="isBusy"
                    @click="
                        router.push({
                            name: 'MultiplayerServerFileStudio',
                            params: { id: server.id },
                        })
                    "
                    ><CodeIcon class="size-5" />
                    <span class="inline-flex items-center gap-1">
                        {{ formatMessage(messages.openStudio) }}
                        <span
                            class="rounded bg-orange px-1.5 py-0.5 text-[10px] font-bold uppercase leading-none text-[var(--color-text-primary)]"
                        >
                            Beta
                        </span>
                    </span>
                </Button>
            </template>
        </FilePageLayout>
    </div>
</template>
