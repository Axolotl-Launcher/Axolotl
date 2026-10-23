<script setup lang="ts">
import CodeIcon from '@modrinth/assets/icons/code.svg?component'
import FileArchiveIcon from '@modrinth/assets/icons/file-archive.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import ReadyTransition from '@modrinth/ui/src/components/base/ReadyTransition.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { provideFileManager } from '@modrinth/ui/src/layouts/shared/files-tab/providers/file-manager.ts'
import type {
    EditingFile,
    FileContextMenuOption,
    FileItem,
} from '@modrinth/ui/src/layouts/shared/files-tab/types.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import { revealInFolder } from '../../lib/platform/opener'
import { usePreview } from '../../lib/state'
import type { GameInstance } from '../../lib/types'
import FilePageLayout from '../../shared/files-tab/layout.vue'
import { type FsDir, useInstanceFiles } from './files/instance-files'
import { createInstanceFs, parentOf } from './files/instance-fs'
import { filesMessages as messages, filesPreviewMessages } from './messages'

/**
 * Port of `apps/app-frontend/src/pages/instance/Files.vue` over the instance
 * folder of the preview's launcher disk (see `files/instance-files.ts`).
 */
const props = defineProps<{
    instance: GameInstance
    offline?: boolean
    playing?: boolean
    installed?: boolean
    isServerInstance?: boolean
}>()

const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()
const router = useRouter()
const { treeFor } = useInstanceFiles()
const { downloads } = usePreview()

const instanceRoot = computed(() => props.instance.path)
const items = ref<FileItem[]>([])
/** True until the first directory read for the current instance finishes. */
const firstPaintPending = ref(true)
const loading = ref(true)
const error = ref<Error | null>(null)
const currentPath = ref('')
const editingFile = ref<EditingFile | null>(null)

function tree(): FsDir {
    return treeFor(props.instance)
}

const fs = createInstanceFs(tree, {
    folder: () => formatMessage(filesPreviewMessages.folderUnavailable),
    content: () => formatMessage(filesPreviewMessages.contentUnavailable),
})

function seconds(time: number) {
    return Math.floor(time / 1000)
}

function listDirectory(dirPath: string): FileItem[] {
    return fs.directoryAt(dirPath).children.map((child) => {
        const item: FileItem = {
            name: child.name,
            type: child.kind === 'dir' ? 'directory' : 'file',
            path: dirPath ? `${dirPath}/${child.name}` : child.name,
            modified: seconds(child.modified),
            created: seconds(child.created),
        }
        if (child.kind === 'file') item.size = child.size
        else item.count = child.children?.length ?? 0
        return item
    })
}

async function refresh() {
    loading.value = true
    error.value = null
    try {
        items.value = listDirectory(currentPath.value)
    } catch (e) {
        error.value = e instanceof Error ? e : new Error(String(e))
        items.value = []
    } finally {
        loading.value = false
        firstPaintPending.value = false
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

function notifyFailure(title: string, e: unknown) {
    addNotification({
        title,
        text: e instanceof Error ? e.message : '',
        type: 'error',
    })
}

async function handleCreateItem(name: string, type: 'file' | 'directory') {
    try {
        fs.create(currentPath.value ? `${currentPath.value}/${name}` : name, type)
        await refresh()
    } catch (e) {
        notifyFailure(formatMessage(commonMessages.createFailedLabel), e)
    }
}

async function handleRenameItem(path: string, newName: string) {
    const { parent } = parentOf(path)
    try {
        fs.move(path, parent ? `${parent}/${newName}` : newName)
        await refresh()
    } catch (e) {
        notifyFailure(formatMessage(commonMessages.renameFailedLabel), e)
    }
}

async function handleMoveItem(source: string, destination: string) {
    try {
        fs.move(source, destination)
        await refresh()
    } catch (e) {
        notifyFailure(formatMessage(commonMessages.moveFailedLabel), e)
    }
}

async function handleDeleteItem(path: string, _recursive: boolean) {
    try {
        fs.detach(path)
        await refresh()
    } catch (e) {
        notifyFailure(formatMessage(commonMessages.deleteFailedLabel), e)
    }
}

async function handleReadFile(path: string): Promise<string> {
    return fs.readText(path)
}

async function handleReadFileAsBlob(path: string): Promise<Blob> {
    return fs.readBlob(path)
}

async function handleWriteFile(path: string, content: string) {
    fs.writeText(path, content)
}

/** The app's native "Save as" dialog; the browser saves the file to its downloads instead. */
async function handleDownloadFile(path: string, fileName: string) {
    const blob = await handleReadFileAsBlob(path)
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = fileName
    link.click()
    setTimeout(() => URL.revokeObjectURL(url), 0)
}

async function handleExtractFile(_path: string, _override: boolean, _dry: boolean) {
    addNotification({
        title: formatMessage(commonMessages.extractFailedLabel),
        text: formatMessage(filesPreviewMessages.contentUnavailable),
        type: 'error',
    })
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

watch(
    () => props.instance.id,
    () => {
        firstPaintPending.value = true
        currentPath.value = ''
        void refresh()
    },
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
    extractFile: handleExtractFile,
    refresh: () => void refresh(),
    basePath: instanceRoot,
    openInFolder: (path: string) => revealInFolder(path),
    getAdditionalMenuOptions,
    downloadButtonLabel: formatMessage(messages.saveAs),
    uploadingLabel: (completed: number, total: number) =>
        formatMessage(messages.addingFiles, { completed, total }),
    symlinkTarget: computed(() => props.instance.symlink_target),
})

// The folder lists the instance's installed files, which load with the content list.
void downloads.ensureContents().then(refresh)
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
