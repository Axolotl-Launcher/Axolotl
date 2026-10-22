<script setup lang="ts">
import {
    ChevronDownIcon,
    ChevronRightIcon,
    CodeIcon,
    CopyIcon,
    FileCodeIcon,
    FilePlusIcon,
    FolderIcon,
    FolderOpenIcon,
    RefreshCwIcon,
    SaveIcon,
    TrashIcon,
} from '@modrinth/assets'
import {
    Button,
    commonMessages,
    defineMessages,
    injectNotificationManager,
    NewModal,
    type OverflowMenuOption,
    PointMenu,
    StyledInput,
    useVIntl,
} from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { join } from '@tauri-apps/api/path'
import {
    copyFile,
    mkdir,
    readDir,
    readFile,
    readTextFile,
    remove,
    rename,
    watch as watchFiles,
    writeFile,
    writeTextFile,
} from '@tauri-apps/plugin-fs'
import { NbtFile, NbtTag } from 'deepslate/nbt'
import { computed, nextTick, onBeforeUnmount, onMounted, onScopeDispose, ref, watch } from 'vue'
import { onBeforeRouteLeave, useRouter } from 'vue-router'

import NbtEditor from '@/components/instance/studio/NbtEditor.vue'
import StudioEditor from '@/components/instance/studio/StudioEditor.vue'
import StudioEditorTabs from '@/components/instance/studio/StudioEditorTabs.vue'
import {
    type StudioDocument,
    useStudioDocuments,
} from '@/components/instance/studio/useStudioDocuments'
import { type StudioTreeNode, useStudioTree } from '@/components/instance/studio/useStudioTree'
import { useStudioWatcher } from '@/components/instance/studio/useStudioWatcher'
import type { ServerView } from '@/composables/useServers'
import { get_full_path } from '@/helpers/instance'
import {
    listenStudioFilesChanged,
    readStudioBinary,
    readStudioText,
    registerStudioWatcher,
    trashStudioFile,
    unregisterStudioWatcher,
    writeStudioBinary,
} from '@/helpers/studio'
import type { GameInstance } from '@/helpers/types'
import { highlightInFolder, openPath } from '@/helpers/utils'

const props = defineProps<{
    instance?: GameInstance
    server?: ServerView
}>()

const messages = defineMessages({
    title: {
        id: 'instance.files.studio.title',
        defaultMessage: 'Studio',
    },
    files: {
        id: 'instance.files.studio.files',
        defaultMessage: 'Explorer',
    },
    backToFiles: {
        id: 'instance.files.studio.back-to-files',
        defaultMessage: 'Exit',
    },
    format: {
        id: 'instance.files.studio.format',
        defaultMessage: 'Format',
    },
    emptyTitle: {
        id: 'instance.files.studio.empty-title',
        defaultMessage: 'Select a configuration file',
    },
    emptyDescription: {
        id: 'instance.files.studio.empty-description',
        defaultMessage: 'Open any text file from the explorer.',
    },
    loadDirectoryFailed: {
        id: 'instance.files.studio.load-directory-failed',
        defaultMessage: 'Could not load directory',
    },
    loadFileFailed: {
        id: 'instance.files.studio.load-file-failed',
        defaultMessage: 'Could not open file',
    },
    nbtLoadFailed: {
        id: 'instance.files.studio.nbt-load-failed',
        defaultMessage: 'Could not parse NBT file',
    },
    notTextFile: {
        id: 'instance.files.studio.not-text-file',
        defaultMessage: 'This file is not a text file and cannot be opened in Studio.',
    },
    saveFailed: {
        id: 'instance.files.studio.save-failed',
        defaultMessage: 'Could not save file',
    },
    loadingFile: {
        id: 'instance.files.studio.loading-file',
        defaultMessage: 'Loading file...',
    },
    copy: { id: 'instance.files.studio.copy', defaultMessage: 'Copy' },
    cut: { id: 'instance.files.studio.cut', defaultMessage: 'Cut' },
    paste: { id: 'instance.files.studio.paste', defaultMessage: 'Paste' },
    newFile: { id: 'instance.files.studio.new-file', defaultMessage: 'New file' },
    newFolder: { id: 'instance.files.studio.new-folder', defaultMessage: 'New folder' },
    delete: { id: 'instance.files.studio.delete', defaultMessage: 'Move to trash' },
    createItem: { id: 'instance.files.studio.create-item', defaultMessage: 'Create item' },
    itemName: { id: 'instance.files.studio.item-name', defaultMessage: 'Name' },
    create: { id: 'instance.files.studio.create', defaultMessage: 'Create' },
    operationFailed: {
        id: 'instance.files.studio.operation-failed',
        defaultMessage: 'File operation failed',
    },
    nonTextFile: {
        id: 'instance.files.studio.non-text-file',
        defaultMessage: 'This file is not a text file and cannot be opened.',
    },
    openInSystem: {
        id: 'instance.files.studio.open-in-system',
        defaultMessage: 'Open in system',
    },
    openPath: {
        id: 'instance.files.studio.open-path',
        defaultMessage: 'Open path',
    },
    refresh: {
        id: 'instance.files.studio.refresh',
        defaultMessage: 'Refresh files',
    },
})

const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()
const router = useRouter()
const instanceRoot = ref('')
const fileLoading = ref(false)
const treeScrollElement = ref<HTMLElement | null>(null)
const contextMenu = ref<{ node: StudioTreeNode; x: number; y: number } | null>(null)
const contextMenuElement = ref<InstanceType<typeof PointMenu>>()
const fileClipboard = ref<{ mode: 'copy' | 'cut'; node: StudioTreeNode } | null>(null)
const createModal = ref<InstanceType<typeof NewModal> | null>(null)
const createParentPath = ref('')
const createType = ref<'file' | 'directory'>('file')
const createName = ref('')
const studioEditor = ref<InstanceType<typeof StudioEditor> | InstanceType<typeof NbtEditor> | null>(
    null,
)
const nbtFiles = new Map<string, NbtFile>()

const {
    documents,
    activeDocument,
    activePath,
    hasAnyUnsavedChanges,
    activate: activateDocument,
    open: openDocument,
    close: closeDocument,
    deletePath: deleteDocumentPath,
    isDeleting,
    pathVersion,
    canOpen,
    saveActive: saveActiveDocument,
    saveAll: saveAllDocuments,
    updateActiveContent,
    reset: resetDocuments,
} = useStudioDocuments(
    async (document, content) => {
        if (document.kind === 'nbt') {
            const nbtFile = nbtFiles.get(document.path)
            if (!nbtFile) throw new Error('NBT document metadata is unavailable')
            const root = NbtTag.fromString(content)
            if (!root.isCompound()) throw new Error('NBT root must be a compound')
            nbtFile.root = root
            return writeBinary(document.path, nbtFile.write())
        }
        return writeTextFile(await resolvePath(document.path), content)
    },
    (error) => {
        addNotification({
            title: formatMessage(messages.saveFailed),
            text: error instanceof Error ? error.message : String(error),
            type: 'error',
        })
    },
)

const {
    treeLoading,
    visibleNodes,
    listDirectory,
    loadRoot,
    refreshTree: refreshDirectoryTree,
    toggleDirectory,
    reset: resetTree,
} = useStudioTree(
    async (path) => readDir(await resolvePath(path)),
    (error) =>
        addNotification({
            title: formatMessage(messages.loadDirectoryFailed),
            text: error instanceof Error ? error.message : String(error),
            type: 'error',
        }),
)
async function refreshTree() {
    const scrollTop = treeScrollElement.value?.scrollTop ?? 0
    await refreshDirectoryTree()
    await nextTick()
    if (treeScrollElement.value) treeScrollElement.value.scrollTop = scrollTop
}

async function saveActiveFile() {
    const focusedElement = document.activeElement
    if (focusedElement instanceof HTMLElement) focusedElement.blur()
    return saveActiveDocument()
}

const selectedName = computed(() => activeDocument.value?.name ?? '')
const selectedFilePath = computed(() => activeDocument.value?.path ?? '')
const deleteLabel = computed(() =>
    formatMessage(props.instance ? messages.delete : commonMessages.deleteLabel),
)
const editorLanguage = computed(() => {
    const extension = selectedName.value.split('.').pop()?.toLowerCase()
    if (extension === 'dat' || extension === 'nbt') return 'json'
    if (extension === 'yml') return 'yaml'
    return extension ?? 'plaintext'
})
const breadcrumbSegments = computed(() => selectedFilePath.value.split('/').filter(Boolean))
const visibleBreadcrumbSegments = ref<string[]>([])
const breadcrumbOuter = ref<HTMLElement | null>(null)
const breadcrumbMeasure = ref<HTMLElement | null>(null)
let breadcrumbObserver: ResizeObserver | null = null

function previewKind(name: string): StudioDocument['kind'] | null {
    if (/\.(png|jpe?g|gif|webp|svg|bmp|ico|avif)$/i.test(name)) return 'image'
    if (/\.(mp4|webm|ogv|mov|m4v)$/i.test(name)) return 'video'
    return null
}

const previewUrl = ref('')

async function resolvePath(relativePath: string): Promise<string> {
    return relativePath ? join(instanceRoot.value, ...relativePath.split('/')) : instanceRoot.value
}

async function readText(path: string): Promise<string> {
    if (props.instance) return readStudioText(props.instance.id, path)
    return readTextFile(await resolvePath(path))
}

async function readBinary(path: string): Promise<Uint8Array> {
    if (props.instance) return readStudioBinary(props.instance.id, path)
    return readFile(await resolvePath(path))
}

async function writeBinary(path: string, bytes: Uint8Array): Promise<void> {
    if (props.instance) return writeStudioBinary(props.instance.id, path, bytes)
    return writeFile(await resolvePath(path), bytes)
}

function exitStudio() {
    if (props.server) {
        void router.push({
            name: 'MultiplayerServerDetail',
            params: { id: props.server.id },
            query: { tab: 'files' },
        })
        return
    }
    if (props.instance) {
        void router.push({ name: 'Files', params: { id: props.instance.id } })
    }
}

function readNbtContent(bytes: Uint8Array, path: string): string {
    const nbtFile = NbtFile.read(bytes)
    nbtFiles.set(path, nbtFile)
    return nbtFile.root.toPrettyString()
}

function joinPath(parent: string, name: string): string {
    return parent ? `${parent}/${name}` : name
}

function contextDestination(node: StudioTreeNode): string {
    return node.type === 'directory' ? node.path : node.path.split('/').slice(0, -1).join('/')
}

function showContextMenu(event: MouseEvent, node: StudioTreeNode) {
    contextMenu.value = { node, x: event.clientX, y: event.clientY }
    contextMenuElement.value?.show(event)
}
function hideContextMenu() {
    contextMenuElement.value?.close()
    contextMenu.value = null
}
const contextActions = computed<OverflowMenuOption[]>(() => [
    {
        id: 'copy',
        label: formatMessage(messages.copy),
        icon: CopyIcon,
        action: () => copyToClipboard('copy'),
    },
    {
        id: 'cut',
        label: formatMessage(messages.cut),
        icon: CopyIcon,
        action: () => copyToClipboard('cut'),
    },
    {
        id: 'paste',
        label: formatMessage(messages.paste),
        icon: CopyIcon,
        shown: !!fileClipboard.value,
        action: pasteClipboard,
    },
    { divider: true },
    {
        id: 'new-file',
        label: formatMessage(messages.newFile),
        icon: FilePlusIcon,
        action: () => showCreateModal('file'),
    },
    {
        id: 'new-folder',
        label: formatMessage(messages.newFolder),
        icon: FolderOpenIcon,
        action: () => showCreateModal('directory'),
    },
    { divider: true },
    {
        id: 'open-folder',
        label: formatMessage(messages.openPath),
        icon: FolderOpenIcon,
        action: revealContextMenuItem,
    },
    { divider: true },
    { id: 'delete', label: deleteLabel.value, icon: TrashIcon, color: 'red', action: deleteItem },
])

function copyToClipboard(mode: 'copy' | 'cut') {
    if (!contextMenu.value) return
    fileClipboard.value = { mode, node: contextMenu.value.node }
    hideContextMenu()
}

async function copyItem(source: StudioTreeNode, destination: string): Promise<void> {
    if (source.type === 'file') {
        await copyFile(await resolvePath(source.path), await resolvePath(destination))
        return
    }

    await mkdir(await resolvePath(destination), { recursive: true })
    const children = await listDirectory(source.path, source.depth + 1)
    await Promise.all(children.map((child) => copyItem(child, joinPath(destination, child.name))))
}

async function pasteClipboard() {
    if (!contextMenu.value || !fileClipboard.value) return
    const menuTarget = contextMenu.value
    const destinationParent = contextDestination(menuTarget.node)
    const { mode, node } = fileClipboard.value
    const destination = joinPath(destinationParent, node.name)
    if (node.path === destination || destination.startsWith(`${node.path}/`)) return

    try {
        if (mode === 'copy') await copyItem(node, destination)
        else {
            await rename(await resolvePath(node.path), await resolvePath(destination))
            fileClipboard.value = null
        }
        await refreshTree()
        if (contextMenu.value === menuTarget) hideContextMenu()
    } catch (error) {
        addNotification({
            title: formatMessage(messages.operationFailed),
            text: error instanceof Error ? error.message : String(error),
            type: 'error',
        })
    }
}

async function deleteItem() {
    if (!contextMenu.value) return
    const menuTarget = contextMenu.value
    const path = menuTarget.node.path
    const instanceId = props.instance?.id
    const root = instanceRoot.value
    const version = pathVersion(path)
    try {
        if (
            !(await deleteDocumentPath(path, async () => {
                if (instanceId) await trashStudioFile(instanceId, path)
                else await remove(await join(root, ...path.split('/')), { recursive: true })
            }))
        )
            return
        if (instanceRoot.value !== root || pathVersion(path).generation !== version.generation)
            return
        for (const nbtPath of nbtFiles.keys()) {
            if (nbtPath === path || nbtPath.startsWith(`${path}/`)) nbtFiles.delete(nbtPath)
        }
        await refreshTree()
        if (contextMenu.value === menuTarget) hideContextMenu()
    } catch (error) {
        addNotification({
            title: formatMessage(messages.operationFailed),
            text: error instanceof Error ? error.message : String(error),
            type: 'error',
        })
    }
}

function showCreateModal(type: 'file' | 'directory') {
    if (!contextMenu.value) return
    createParentPath.value = contextDestination(contextMenu.value.node)
    createType.value = type
    createName.value = ''
    hideContextMenu()
    createModal.value?.show()
}

async function createItem() {
    const name = createName.value.trim()
    if (!name || /[\\/]/.test(name)) return
    const path = joinPath(createParentPath.value, name)
    try {
        if (createType.value === 'directory') await mkdir(await resolvePath(path))
        else await writeTextFile(await resolvePath(path), '')
        await refreshTree()
        createModal.value?.hide()
    } catch (error) {
        addNotification({
            title: formatMessage(messages.operationFailed),
            text: error instanceof Error ? error.message : String(error),
            type: 'error',
        })
    }
}

function updateVisibleBreadcrumbs() {
    const segments = breadcrumbSegments.value
    if (!breadcrumbOuter.value || !breadcrumbMeasure.value) {
        visibleBreadcrumbSegments.value = segments
        return
    }

    let start = 0
    while (start < Math.max(segments.length - 2, 0)) {
        const visible = start === 0 ? segments : ['...', ...segments.slice(start)]
        breadcrumbMeasure.value.textContent = visible.join(' > ')
        if (breadcrumbMeasure.value.scrollWidth <= breadcrumbOuter.value.clientWidth) {
            visibleBreadcrumbSegments.value = visible
            return
        }
        start += 1
    }

    visibleBreadcrumbSegments.value = segments.length > 1 ? ['...', segments.at(-1)!] : segments
}

async function reloadCleanDocument(document: StudioDocument) {
    const version = pathVersion(document.path)
    if (
        (document.kind !== 'text' && document.kind !== 'nbt') ||
        document.content !== document.savedContent ||
        document.saving ||
        isDeleting(document.path)
    )
        return
    try {
        const loaded =
            document.kind === 'nbt'
                ? await readBinary(document.path)
                : await readTextFile(await resolvePath(document.path))
        if (
            document.content !== document.savedContent ||
            document.saving ||
            !canOpen(document.path, version) ||
            !documents.value.includes(document)
        )
            return
        const nextContent =
            document.kind === 'nbt'
                ? readNbtContent(loaded as Uint8Array, document.path)
                : (loaded as string)
        document.content = nextContent
        document.savedContent = nextContent
    } catch {
        // The path may have been removed or be between the two sides of an atomic rename.
    }
}

async function processFileChanges(changedPaths: string[]) {
    const paths = new Set(changedPaths)
    const openDocuments = [...documents.value]
    try {
        await refreshTree()
    } catch (error) {
        addNotification({
            title: formatMessage(messages.loadDirectoryFailed),
            text: error instanceof Error ? error.message : String(error),
            type: 'error',
        })
    }

    await Promise.all(
        openDocuments
            .filter((document) => paths.has(document.path))
            .map((document) => reloadCleanDocument(document)),
    )
}

const studioWatcher = useStudioWatcher(
    {
        listen: listenStudioFilesChanged,
        register: registerStudioWatcher,
        unregister: unregisterStudioWatcher,
        watch: (path, handler) =>
            watchFiles(path, (event) => handler(event.paths), { recursive: true, delayMs: 150 }),
    },
    processFileChanges,
    (error) => console.warn('Failed to watch Studio files', error),
)
function startStudioWatcher() {
    if (props.server) return studioWatcher.start({ kind: 'workspace', path: instanceRoot.value })
    if (props.instance) return studioWatcher.start({ kind: 'instance', id: props.instance.id })
    return studioWatcher.stop()
}

watch(activePath, async (path) => {
    if (!path) return
    await nextTick()
    const node = Array.from(
        treeScrollElement.value?.querySelectorAll<HTMLElement>('[data-studio-path]') ?? [],
    ).find((element) => element.dataset.studioPath === path)
    node?.scrollIntoView({ block: 'nearest' })
})

watch(breadcrumbSegments, () => nextTick(updateVisibleBreadcrumbs), { immediate: true })

watch(
    [activeDocument, instanceRoot],
    async () => {
        previewUrl.value = activeDocument.value
            ? convertFileSrc(await resolvePath(activeDocument.value.path))
            : ''
    },
    { immediate: true },
)

onMounted(() => {
    breadcrumbObserver = new ResizeObserver(updateVisibleBreadcrumbs)
    if (breadcrumbOuter.value) breadcrumbObserver.observe(breadcrumbOuter.value)
})

onBeforeUnmount(() => {
    breadcrumbObserver?.disconnect()
})

async function openFile(node: StudioTreeNode) {
    if (node.type !== 'file' || fileLoading.value || isDeleting(node.path)) return
    const version = pathVersion(node.path)

    const existingDocument = documents.value.find((document) => document.path === node.path)
    if (existingDocument) {
        if (existingDocument.path === activePath.value) return
        await activateDocument(existingDocument.path)
        return
    }

    const mediaKind = previewKind(node.name)
    if (mediaKind) {
        await openDocument(
            {
                kind: mediaKind,
                path: node.path,
                name: node.name,
                content: '',
                savedContent: '',
                saving: false,
            },
            version,
        )
        return
    }
    if (/\.jar$/i.test(node.name)) {
        await openDocument(
            {
                kind: 'unsupported',
                path: node.path,
                name: node.name,
                content: '',
                savedContent: '',
                saving: false,
            },
            version,
        )
        return
    }

    fileLoading.value = true
    try {
        if (/\.(dat|nbt)$/i.test(node.name)) {
            let nextContent: string
            try {
                const bytes = await readBinary(node.path)
                if (!canOpen(node.path, version)) return
                nextContent = readNbtContent(bytes, node.path)
            } catch (error) {
                if (!canOpen(node.path, version)) return
                addNotification({
                    title: formatMessage(messages.nbtLoadFailed),
                    text: error instanceof Error ? error.message : String(error),
                    type: 'error',
                })
                return
            }
            await openDocument(
                {
                    kind: 'nbt',
                    path: node.path,
                    name: node.name,
                    content: nextContent,
                    savedContent: nextContent,
                    saving: false,
                },
                version,
            )
            return
        }
        const nextContent = await readText(node.path)
        const document: StudioDocument = {
            kind: 'text',
            path: node.path,
            name: node.name,
            content: nextContent,
            savedContent: nextContent,
            saving: false,
        }
        await openDocument(document, version)
    } catch {
        await openDocument(
            {
                kind: 'unsupported',
                path: node.path,
                name: node.name,
                content: '',
                savedContent: '',
                saving: false,
            },
            version,
        )
    } finally {
        fileLoading.value = false
    }
}

async function activateTab(path: string) {
    await activateDocument(path)
}

async function formatActiveDocument() {
    await studioEditor.value?.formatDocument()
}

async function openInSystem(path: string) {
    await openPath(await resolvePath(path))
}

async function revealInSystem(path: string) {
    await highlightInFolder(await resolvePath(path))
}

async function revealContextMenuItem() {
    if (!contextMenu.value) return
    const menuTarget = contextMenu.value
    await revealInSystem(menuTarget.node.path)
    if (contextMenu.value === menuTarget) hideContextMenu()
}

let initializationGeneration = 0
onScopeDispose(() => initializationGeneration++)
async function initialize() {
    const generation = ++initializationGeneration
    const serverPath = props.server?.path
    const instanceId = props.instance?.id
    await studioWatcher.stop()
    if (generation !== initializationGeneration) return false
    resetTree()
    const root = serverPath ?? (instanceId ? await get_full_path(instanceId) : '')
    if (generation !== initializationGeneration) return false
    instanceRoot.value = root
    resetDocuments()
    nbtFiles.clear()
    await loadRoot()
    return generation === initializationGeneration
}

if (await initialize()) await startStudioWatcher()

watch(
    () => props.instance?.id ?? props.server?.id,
    async () => {
        if (await initialize()) await startStudioWatcher()
    },
)

function handleBeforeUnload(event: BeforeUnloadEvent) {
    if (!hasAnyUnsavedChanges.value) return
    event.preventDefault()
}

window.addEventListener('beforeunload', handleBeforeUnload)
onBeforeUnmount(() => {
    window.removeEventListener('beforeunload', handleBeforeUnload)
})

onBeforeRouteLeave(() => {
    return saveAllDocuments()
})
</script>

<template>
    <div class="flex h-full min-h-0 flex-col">
        <section
            class="grid min-h-0 min-w-0 flex-1 grid-cols-[minmax(13rem,22rem)_minmax(0,1fr)] overflow-hidden rounded-[20px] border border-solid border-surface-4 bg-surface-1 shadow-sm"
        >
            <NewModal
                ref="createModal"
                :header="formatMessage(messages.createItem)"
                max-width="420px"
            >
                <label
                    class="flex flex-col gap-2 text-sm font-semibold text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.itemName) }}
                    <StyledInput v-model="createName" :input-attrs="{ autofocus: true }" />
                </label>
                <template #actions>
                    <Button type="colored" color="brand" @click="createItem"
                        >{{ formatMessage(messages.create) }}
                    </Button>
                </template>
            </NewModal>
            <aside
                class="flex min-h-0 min-w-0 flex-col border-0 border-r border-solid border-surface-4 bg-surface-2"
            >
                <header
                    class="flex h-12 shrink-0 items-center gap-2 border-0 border-b border-solid border-surface-4 bg-surface-3 px-3"
                >
                    <CodeIcon class="size-5 text-brand" />
                    <h1
                        class="m-0 min-w-0 truncate text-sm font-bold text-[var(--color-text-primary)]"
                    >
                        {{ formatMessage(messages.title) }}
                    </h1>
                    <div class="ml-auto flex shrink-0 items-center gap-1">
                        <Button
                            v-tooltip="formatMessage(messages.refresh)"
                            type="quiet"
                            size="2xs"
                            circular
                            icon-only
                            :aria-label="formatMessage(messages.refresh)"
                            @click="refreshTree"
                            ><RefreshCwIcon class="size-4" />
                        </Button>
                        <Button
                            v-if="activeDocument"
                            type="colored"
                            color="brand"
                            size="2xs"
                            :disabled="activeDocument.saving"
                            @click="saveActiveFile"
                            ><SaveIcon class="size-4" />
                            {{ formatMessage(commonMessages.saveButton) }}
                        </Button>
                        <Button
                            v-if="activeDocument?.kind === 'text' || activeDocument?.kind === 'nbt'"
                            type="outlined"
                            size="2xs"
                            @click="formatActiveDocument"
                            >{{ formatMessage(messages.format) }}
                        </Button>
                        <Button type="outlined" size="2xs" @click="exitStudio"
                            >{{ formatMessage(messages.backToFiles) }}
                        </Button>
                    </div>
                </header>
                <div
                    class="flex h-9 shrink-0 items-center px-3 text-xs font-bold uppercase tracking-wide text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.files) }}
                </div>
                <div
                    ref="treeScrollElement"
                    class="min-h-0 flex-1 overflow-y-auto pb-3"
                    role="tree"
                >
                    <div
                        v-if="treeLoading"
                        class="px-4 py-3 text-sm text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.loadingFile) }}
                    </div>
                    <button
                        v-for="node in visibleNodes"
                        :key="node.path"
                        type="button"
                        role="treeitem"
                        :data-studio-path="node.path"
                        :aria-expanded="node.type === 'directory' ? node.expanded : undefined"
                        class="flex h-8 w-full cursor-pointer items-center gap-1 border-0 bg-transparent pr-3 text-left text-sm text-[var(--color-text-default)] hover:bg-surface-3"
                        :class="{
                            'bg-brand-highlight !text-[var(--color-text-primary)]':
                                node.path === selectedFilePath,
                            '!bg-surface-3':
                                node.path === contextMenu?.node.path &&
                                node.path !== selectedFilePath,
                        }"
                        :style="{ paddingLeft: `${node.depth * 16 + 8}px` }"
                        @click="node.type === 'directory' ? toggleDirectory(node) : openFile(node)"
                        @contextmenu.prevent.stop="showContextMenu($event, node)"
                    >
                        <template v-if="node.type === 'directory'">
                            <ChevronDownIcon v-if="node.expanded" class="size-4 shrink-0" />
                            <ChevronRightIcon v-else class="size-4 shrink-0" />
                        </template>
                        <span v-else class="size-4 shrink-0" />
                        <FolderIcon
                            v-if="node.type === 'directory'"
                            class="size-4 shrink-0 text-[var(--color-text-tertiary)]"
                        />
                        <FileCodeIcon
                            v-else
                            class="size-4 shrink-0 text-[var(--color-text-tertiary)]"
                        />
                        <span class="truncate">{{ node.name }}</span>
                    </button>
                </div>
            </aside>

            <div class="flex min-h-0 min-w-0 flex-col bg-surface-2">
                <header
                    class="flex h-12 shrink-0 items-center overflow-hidden border-0 border-b border-solid border-surface-4 bg-surface-3"
                >
                    <StudioEditorTabs
                        :documents="documents"
                        :active-path="activePath"
                        @activate="activateTab"
                        @close="closeDocument"
                    />
                </header>
                <nav
                    v-if="selectedFilePath"
                    class="flex h-8 shrink-0 items-center gap-2 border-0 border-b border-solid border-surface-4 bg-surface-2 px-3 text-xs text-[var(--color-text-tertiary)]"
                    :aria-label="selectedFilePath"
                >
                    <div
                        ref="breadcrumbOuter"
                        class="flex min-w-0 flex-1 items-center gap-1 overflow-hidden"
                    >
                        <template
                            v-for="(segment, index) in visibleBreadcrumbSegments"
                            :key="`${segment}-${index}`"
                        >
                            <ChevronRightIcon v-if="index > 0" class="size-3.5 shrink-0" />
                            <FolderIcon
                                v-if="
                                    segment !== '...' &&
                                    index < visibleBreadcrumbSegments.length - 1
                                "
                                class="size-3.5 shrink-0"
                            />
                            <FileCodeIcon v-else class="size-3.5 shrink-0" />
                            <span
                                class="min-w-0 truncate whitespace-nowrap"
                                :class="{
                                    'text-[var(--color-text-primary)]':
                                        index === visibleBreadcrumbSegments.length - 1,
                                }"
                            >
                                {{ segment }}
                            </span>
                        </template>
                    </div>
                    <Button type="quiet" size="2xs" @click="revealInSystem(selectedFilePath)"
                        >{{ formatMessage(messages.openPath) }}
                    </Button>
                    <span
                        ref="breadcrumbMeasure"
                        class="pointer-events-none absolute whitespace-nowrap opacity-0"
                        aria-hidden="true"
                    />
                </nav>

                <div class="relative min-h-0 min-w-0 flex-1">
                    <div
                        v-if="fileLoading"
                        class="absolute inset-0 z-[2] flex items-center justify-center bg-surface-2 text-sm text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.loadingFile) }}
                    </div>
                    <div
                        v-if="activeDocument?.kind === 'image'"
                        class="flex size-full items-center justify-center overflow-auto bg-surface-1 p-6"
                    >
                        <img
                            :src="previewUrl"
                            :alt="activeDocument.name"
                            class="max-h-full max-w-full object-contain"
                        />
                    </div>
                    <div
                        v-else-if="activeDocument?.kind === 'video'"
                        class="flex size-full items-center justify-center bg-surface-1 p-6"
                    >
                        <video :src="previewUrl" controls class="max-h-full max-w-full" />
                    </div>
                    <div
                        v-else-if="activeDocument?.kind === 'unsupported'"
                        class="flex size-full items-center justify-center p-8 text-center"
                    >
                        <div class="flex flex-col items-center gap-3">
                            <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                                {{ formatMessage(messages.nonTextFile) }}
                            </p>
                            <Button type="outlined" @click="openInSystem(activeDocument.path)"
                                >{{ formatMessage(messages.openInSystem) }}
                            </Button>
                        </div>
                    </div>
                    <StudioEditor
                        v-if="activeDocument?.kind === 'text'"
                        ref="studioEditor"
                        :key="activeDocument.path"
                        :file-path="activeDocument.path"
                        :content="activeDocument.content"
                        :language="editorLanguage"
                        :read-only="activeDocument.saving || isDeleting(activeDocument.path)"
                        @update:content="updateActiveContent"
                        @save="saveActiveFile"
                        @blur="saveActiveFile"
                    />
                    <NbtEditor
                        v-else-if="activeDocument?.kind === 'nbt'"
                        ref="studioEditor"
                        :key="activeDocument.path"
                        :file-path="activeDocument.path"
                        :content="activeDocument.content"
                        :read-only="activeDocument.saving || isDeleting(activeDocument.path)"
                        @update:content="updateActiveContent"
                        @save="saveActiveFile"
                    />
                    <div v-else class="flex size-full items-center justify-center p-8 text-center">
                        <div class="flex max-w-md flex-col items-center gap-3">
                            <CodeIcon class="size-14 text-[var(--color-text-tertiary)]" />
                            <h2 class="m-0 text-xl font-bold text-[var(--color-text-primary)]">
                                {{ formatMessage(messages.emptyTitle) }}
                            </h2>
                            <p class="m-0 text-sm leading-6 text-[var(--color-text-tertiary)]">
                                {{ formatMessage(messages.emptyDescription) }}
                            </p>
                        </div>
                    </div>
                </div>
            </div>
            <PointMenu
                ref="contextMenuElement"
                :options="contextActions"
                @close="contextMenu = null"
            />
        </section>
    </div>
</template>
