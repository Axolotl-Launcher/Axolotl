<script setup lang="ts">
import ChevronDownIcon from '@modrinth/assets/icons/chevron-down.svg?component'
import ChevronRightIcon from '@modrinth/assets/icons/chevron-right.svg?component'
import CodeIcon from '@modrinth/assets/icons/code.svg?component'
import CopyIcon from '@modrinth/assets/icons/copy.svg?component'
import FileCodeIcon from '@modrinth/assets/icons/file-code.svg?component'
import FilePlusIcon from '@modrinth/assets/icons/file-plus.svg?component'
import FolderIcon from '@modrinth/assets/icons/folder.svg?component'
import FolderOpenIcon from '@modrinth/assets/icons/folder-open.svg?component'
import RefreshCwIcon from '@modrinth/assets/icons/refresh-cw.svg?component'
import SaveIcon from '@modrinth/assets/icons/save.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import type { Option as OverflowMenuOption } from '@modrinth/ui/src/components/base/OverflowMenu.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, nextTick, onBeforeUnmount, onMounted, onScopeDispose, ref, watch } from 'vue'
import { onBeforeRouteLeave, useRouter } from 'vue-router'

import { formatPreviewMessage } from '../../../lib/i18n'
import { desktopAppMessages } from '../../../lib/messages'
import { openInDefaultApp, revealInFolder } from '../../../lib/platform/opener'
import type { GameInstance } from '../../../lib/types'
import PreviewModal from '../../../overlays/PreviewModal.vue'
import PointMenu from '../../../overlays/PreviewPointMenu.vue'
import DesktopAppLink from '../../../shell/DesktopAppLink.vue'
import { PreviewContentUnavailable, useInstanceFiles } from '../files/instance-files'
import { filesPreviewMessages } from '../messages'
import StudioEditor from './StudioEditor.vue'
import StudioEditorTabs from './StudioEditorTabs.vue'
import { type StudioDocument, useStudioDocuments } from './useStudioDocuments'
import { type StudioTreeNode, useStudioTree } from './useStudioTree'
import { instanceWorkspace, serverWorkspace, type StudioWorkspace } from './workspaces'

/**
 * Port of `pages/instance/FileStudio.vue`, the editor workspace of an instance
 * folder or a managed server's folder, over the preview's copies of those
 * folders (`workspaces.ts`). Nothing else writes to them while Studio is open,
 * so the app's file watcher has nothing to report here.
 */
const props = defineProps<{
    instance?: GameInstance
    server?: { id: string }
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
const { treeFor } = useInstanceFiles()
const workspace = ref<StudioWorkspace | null>(null)
const fileLoading = ref(false)
const treeScrollElement = ref<HTMLElement | null>(null)
const contextMenu = ref<{ node: StudioTreeNode; x: number; y: number } | null>(null)
const contextMenuElement = ref<InstanceType<typeof PointMenu>>()
const fileClipboard = ref<{ mode: 'copy' | 'cut'; node: StudioTreeNode } | null>(null)
const createModal = ref<InstanceType<typeof PreviewModal> | null>(null)
const createParentPath = ref('')
const createType = ref<'file' | 'directory'>('file')
const createName = ref('')
const studioEditor = ref<InstanceType<typeof StudioEditor> | null>(null)
/**
 * Why an opened file shows no editor, when it is not the app's "not a text
 * file": NBT files are parsed by the desktop app (deepslate), and files whose
 * contents the preview does not have say so.
 */
const unopenedReasons = ref(new Map<string, { text: string; desktop: boolean }>())

function requireWorkspace() {
    if (!workspace.value) throw new Error('Studio has no folder open')
    return workspace.value
}

const {
    documents,
    activeDocument,
    activePath,
    activate: activateDocument,
    open: openDocument,
    close: closeDocument,
    deletePath: deleteDocumentPath,
    isDeleting,
    pathVersion,
    saveActive: saveActiveDocument,
    saveAll: saveAllDocuments,
    updateActiveContent,
    reset: resetDocuments,
} = useStudioDocuments(
    async (document, content) => requireWorkspace().writeText(document.path, content),
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
    async (path) => requireWorkspace().readDir(path),
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

const previewUrl = computed(() =>
    activeDocument.value ? (workspace.value?.mediaUrl(activeDocument.value.path) ?? '') : '',
)

function absolutePath(path: string) {
    const root = workspace.value?.root ?? ''
    return path ? `${root}/${path}` : root
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
        await requireWorkspace().copyFile(source.path, destination)
        return
    }

    await requireWorkspace().mkdir(destination)
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
            await requireWorkspace().rename(node.path, destination)
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
    const current = workspace.value
    const version = pathVersion(path)
    try {
        if (!(await deleteDocumentPath(path, () => requireWorkspace().remove(path)))) return
        if (workspace.value !== current || pathVersion(path).generation !== version.generation)
            return
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
        if (createType.value === 'directory') await requireWorkspace().mkdir(path)
        else await requireWorkspace().writeText(path, '')
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

watch(activePath, async (path) => {
    if (!path) return
    await nextTick()
    const node = Array.from(
        treeScrollElement.value?.querySelectorAll<HTMLElement>('[data-studio-path]') ?? [],
    ).find((element) => element.dataset.studioPath === path)
    node?.scrollIntoView({ block: 'nearest' })
})

watch(breadcrumbSegments, () => nextTick(updateVisibleBreadcrumbs), { immediate: true })

onMounted(() => {
    breadcrumbObserver = new ResizeObserver(updateVisibleBreadcrumbs)
    if (breadcrumbOuter.value) breadcrumbObserver.observe(breadcrumbOuter.value)
})

onBeforeUnmount(() => {
    breadcrumbObserver?.disconnect()
})

function unopenedDocument(node: StudioTreeNode): StudioDocument {
    return {
        kind: 'unsupported',
        path: node.path,
        name: node.name,
        content: '',
        savedContent: '',
        saving: false,
    }
}

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
        await openDocument(unopenedDocument(node), version)
        return
    }
    if (/\.(dat|nbt)$/i.test(node.name)) {
        unopenedReasons.value.set(node.path, {
            text: formatPreviewMessage(desktopAppMessages.nbt),
            desktop: true,
        })
        await openDocument(unopenedDocument(node), version)
        return
    }

    fileLoading.value = true
    try {
        const nextContent = await requireWorkspace().readText(node.path)
        const document: StudioDocument = {
            kind: 'text',
            path: node.path,
            name: node.name,
            content: nextContent,
            savedContent: nextContent,
            saving: false,
        }
        await openDocument(document, version)
    } catch (error) {
        if (
            error instanceof PreviewContentUnavailable ||
            (props.server && error instanceof Error)
        ) {
            unopenedReasons.value.set(node.path, { text: error.message, desktop: false })
        }
        await openDocument(unopenedDocument(node), version)
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

function openInSystem(path: string) {
    openInDefaultApp(absolutePath(path))
}

function revealInSystem(path: string) {
    revealInFolder(absolutePath(path))
}

function revealContextMenuItem() {
    if (!contextMenu.value) return
    const menuTarget = contextMenu.value
    revealInSystem(menuTarget.node.path)
    if (contextMenu.value === menuTarget) hideContextMenu()
}

let initializationGeneration = 0
onScopeDispose(() => initializationGeneration++)
async function initialize() {
    const generation = ++initializationGeneration
    resetTree()
    workspace.value = props.server
        ? serverWorkspace(props.server.id)
        : props.instance
          ? instanceWorkspace(props.instance, () => treeFor(props.instance!))
          : null
    resetDocuments()
    unopenedReasons.value = new Map()
    await loadRoot()
    return generation === initializationGeneration
}

void initialize()

watch(
    () => props.instance?.id ?? props.server?.id,
    () => void initialize(),
)

// The app also asks before its window closes with unsaved edits; the preview's
// window lives in a web page, which keeps its own `beforeunload`.
onBeforeRouteLeave(() => {
    return saveAllDocuments()
})
</script>

<template>
    <div class="flex h-full min-h-0 flex-col">
        <section
            class="grid min-h-0 min-w-0 flex-1 grid-cols-[minmax(13rem,22rem)_minmax(0,1fr)] overflow-hidden rounded-[20px] border border-solid border-surface-4 bg-surface-1 shadow-sm"
        >
            <PreviewModal
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
            </PreviewModal>
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
                            v-if="previewUrl"
                            :src="previewUrl"
                            :alt="activeDocument.name"
                            class="max-h-full max-w-full object-contain"
                        />
                        <p v-else class="m-0 text-sm text-[var(--color-text-tertiary)]">
                            {{ formatMessage(filesPreviewMessages.contentUnavailable) }}
                        </p>
                    </div>
                    <div
                        v-else-if="activeDocument?.kind === 'video'"
                        class="flex size-full items-center justify-center bg-surface-1 p-6"
                    >
                        <video
                            v-if="previewUrl"
                            :src="previewUrl"
                            controls
                            class="max-h-full max-w-full"
                        />
                        <p v-else class="m-0 text-sm text-[var(--color-text-tertiary)]">
                            {{ formatMessage(filesPreviewMessages.contentUnavailable) }}
                        </p>
                    </div>
                    <div
                        v-else-if="activeDocument?.kind === 'unsupported'"
                        class="flex size-full items-center justify-center p-8 text-center"
                    >
                        <div class="flex flex-col items-center gap-3">
                            <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                                {{
                                    unopenedReasons.get(activeDocument.path)?.text ??
                                    formatMessage(messages.nonTextFile)
                                }}
                            </p>
                            <DesktopAppLink
                                v-if="unopenedReasons.get(activeDocument.path)?.desktop"
                            />
                            <Button
                                v-else
                                type="outlined"
                                @click="openInSystem(activeDocument.path)"
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
                    <div
                        v-else-if="!activeDocument"
                        class="flex size-full items-center justify-center p-8 text-center"
                    >
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
