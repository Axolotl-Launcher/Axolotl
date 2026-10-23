<template>
    <li
        role="button"
        :class="[containerClasses, isDragSource ? 'opacity-50' : '']"
        tabindex="0"
        :data-file-path="path"
        :data-file-type="type"
        @click="selectItem"
        @contextmenu="openContextMenu"
        @keydown="(e) => e.key === 'Enter' && selectItem()"
        @mouseenter="handleMouseEnter"
        @pointerdown="handlePointerDown"
    >
        <div class="pointer-events-none flex flex-1 items-center gap-3 truncate">
            <Checkbox
                class="pointer-events-auto"
                :model-value="selected"
                @click.stop
                @update:model-value="emit('toggle-select')"
            />
            <div class="pointer-events-none flex size-5 items-center justify-center">
                <component
                    :is="iconComponent"
                    class="size-5 group-hover:text-[var(--color-text-primary)] group-focus:text-[var(--color-text-primary)]"
                />
            </div>
            <div class="pointer-events-none flex flex-col truncate">
                <span
                    class="pointer-events-none truncate group-hover:text-[var(--color-text-primary)] group-focus:text-[var(--color-text-primary)]"
                >
                    {{ name }}
                </span>
            </div>
        </div>
        <div
            class="pointer-events-auto flex w-fit flex-shrink-0 items-center gap-4 @[800px]:gap-12"
        >
            <span
                class="hidden w-[100px] text-nowrap text-sm text-[var(--color-text-tertiary)] @[800px]:block"
            >
                {{ formattedSize }}
            </span>
            <span
                class="hidden w-[160px] text-nowrap text-sm text-[var(--color-text-tertiary)] @[800px]:block"
            >
                {{ formattedCreationDate }}
            </span>
            <span
                class="hidden w-[160px] text-nowrap text-sm text-[var(--color-text-tertiary)] @[800px]:block"
            >
                {{ formattedModifiedDate }}
            </span>
            <div class="grid min-w-[51px] shrink-0 items-center justify-items-end">
                <span
                    aria-hidden="true"
                    class="invisible col-start-1 row-start-1 text-nowrap font-semibold"
                >
                    {{ formatMessage(commonMessages.actionsLabel) }}
                </span>
                <div class="col-start-1 row-start-1 flex justify-end">
                    <TeleportOverflowMenu :options="menuOptions">
                        <MoreHorizontalIcon class="h-5 w-5 bg-transparent" />
                        <template #copy-filename
                            ><ClipboardCopyIcon />
                            {{ formatMessage(commonMessages.copyFilenameButton) }}</template
                        >
                        <template #copy-full-path
                            ><ClipboardCopyIcon />
                            {{ formatMessage(commonMessages.copyFullPathButton) }}</template
                        >
                        <template #open-in-folder
                            ><FolderOpenIcon />
                            {{ formatMessage(commonMessages.openInFolderButton) }}</template
                        >
                        <template #extract
                            ><PackageOpenIcon />
                            {{ formatMessage(commonMessages.extractButton) }}</template
                        >
                        <template #rename
                            ><EditIcon /> {{ formatMessage(commonMessages.renameButton) }}</template
                        >
                        <template #move
                            ><RightArrowIcon />
                            {{ formatMessage(commonMessages.moveButton) }}</template
                        >
                        <template #download
                            ><DownloadIcon />
                            {{
                                ctx.downloadButtonLabel ??
                                formatMessage(commonMessages.downloadButton)
                            }}</template
                        >
                        <template #delete
                            ><TrashIcon /> {{ formatMessage(commonMessages.deleteLabel) }}</template
                        >
                    </TeleportOverflowMenu>
                </div>
            </div>
        </div>
    </li>
</template>

<script setup lang="ts">
import BoxIcon from '@modrinth/assets/icons/box.svg?component'
import BracesIcon from '@modrinth/assets/icons/braces.svg?component'
import ClipboardCopyIcon from '@modrinth/assets/icons/clipboard-copy.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import EditIcon from '@modrinth/assets/icons/edit.svg?component'
import FolderCogIcon from '@modrinth/assets/icons/folder-cog.svg?component'
import FolderOpenIcon from '@modrinth/assets/icons/folder-open.svg?component'
import GlassesIcon from '@modrinth/assets/icons/glasses.svg?component'
import GlobeIcon from '@modrinth/assets/icons/globe.svg?component'
import MoreHorizontalIcon from '@modrinth/assets/icons/more-horizontal.svg?component'
import PackageOpenIcon from '@modrinth/assets/icons/package-open.svg?component'
import PaintbrushIcon from '@modrinth/assets/icons/paintbrush.svg?component'
import RightArrowIcon from '@modrinth/assets/icons/right-arrow.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import { useFormatDateTime } from '@modrinth/ui/src/composables/format-date-time.ts'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { useFormatBytes } from '@modrinth/ui/src/composables/index.ts'
import {
    fileDragActive,
    fileDragData,
    fileDragTarget,
    startFileDrag,
    wasRecentDrag,
} from '@modrinth/ui/src/layouts/shared/files-tab/composables/file-drag-state.ts'
import { injectFileManager } from '@modrinth/ui/src/layouts/shared/files-tab/providers/file-manager.ts'
import type { FileItem } from '@modrinth/ui/src/layouts/shared/files-tab/types.ts'
import { joinDisplayPath } from '@modrinth/ui/src/layouts/shared/files-tab/utils.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { getFileExtensionIcon } from '@modrinth/ui/src/utils/auto-icons.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { canOpenInFileEditor, getFileExtension } from '@modrinth/ui/src/utils/file-extensions.ts'
import { computed, ref } from 'vue'

import TeleportOverflowMenu from '../../../overlays/PreviewTeleportOverflowMenu.vue'

const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()
const ctx = injectFileManager()

const messages = defineMessages({
    itemCount: {
        id: 'files.row.item-count',
        defaultMessage: '{count, plural, one {# item} other {# items}}',
    },
})

const props = defineProps<
    FileItem & {
        index: number
        isLast: boolean
        selected: boolean
        writeDisabled?: boolean
        writeDisabledTooltip?: string
    }
>()

const emit = defineEmits<{
    (
        e: 'rename' | 'move' | 'download' | 'delete' | 'edit' | 'extract' | 'hover' | 'navigate',
        item: Pick<FileItem, 'name' | 'type' | 'path'>,
    ): void
    (
        e: 'moveDirectTo',
        item: Pick<FileItem, 'name' | 'type' | 'path'> & { destination: string },
    ): void
    (e: 'contextmenu', x: number, y: number): void
    (e: 'toggle-select'): void
}>()

const isDropTarget = computed(
    () => fileDragActive.value && fileDragTarget.value === props.path && props.type === 'directory',
)
const isDragSource = computed(() => fileDragActive.value && fileDragData.value?.path === props.path)

const formatDateTime = useFormatDateTime({
    year: '2-digit',
    month: '2-digit',
    day: '2-digit',
    hour: 'numeric',
    minute: 'numeric',
})
const formatBytes = useFormatBytes()

const containerClasses = computed(() => {
    const dropTarget = isDropTarget.value
    return [
        'group m-0 flex w-full select-none items-center justify-between overflow-hidden border-0 border-t border-solid border-surface-4 pl-3 pr-4 py-3 focus:!outline-none',
        dropTarget
            ? '!bg-brand-highlight'
            : props.selected
              ? 'bg-surface-2.5'
              : props.index % 2 === 0
                ? 'bg-surface-2'
                : 'bg-surface-1.5',
        props.isLast ? 'rounded-b-[20px]' : '',
        isEditableFile.value || props.type === 'directory'
            ? 'cursor-pointer hover:bg-surface-2.5'
            : '',
        'transition-colors duration-100 focus:!outline-none',
    ]
})

const fileExtension = computed(() => getFileExtension(props.name))

const isZip = computed(() => fileExtension.value === 'zip')

function getFullPath() {
    return joinDisplayPath(ctx.basePath?.value, props.path)
}

const menuOptions = computed(() => {
    const item = { name: props.name, type: props.type, path: props.path }
    const wd = props.writeDisabled
    const wdTooltip = props.writeDisabledTooltip
    const additionalOptions = ctx.getAdditionalMenuOptions?.(item) ?? []
    const hasAdditionalOptions = additionalOptions.some((option) => option.shown !== false)
    return [
        {
            id: 'copy-filename',
            icon: ClipboardCopyIcon,
            action: () => {
                navigator.clipboard.writeText(props.name)
                addNotification({
                    title: formatMessage(commonMessages.copiedFilenameLabel),
                    type: 'success',
                })
            },
        },
        {
            id: 'copy-full-path',
            icon: ClipboardCopyIcon,
            action: () => {
                navigator.clipboard.writeText(getFullPath())
                addNotification({
                    title: formatMessage(commonMessages.copiedPathLabel),
                    type: 'success',
                })
            },
        },
        {
            id: 'open-in-folder',
            icon: FolderOpenIcon,
            shown: !!ctx.openInFolder,
            action: () => ctx.openInFolder?.(getFullPath()),
        },
        { divider: true },
        ...additionalOptions,
        { divider: true, shown: hasAdditionalOptions },
        {
            id: 'extract',
            shown: isZip.value,
            disabled: wd,
            tooltip: wd ? wdTooltip : undefined,
            action: () => emit('extract', item),
        },
        {
            divider: true,
            shown: isZip.value,
        },
        {
            id: 'rename',
            disabled: wd,
            tooltip: wd ? wdTooltip : undefined,
            action: () => emit('rename', item),
        },
        {
            id: 'move',
            disabled: wd,
            tooltip: wd ? wdTooltip : undefined,
            action: () => emit('move', item),
        },
        {
            id: 'download',
            action: () => emit('download', item),
            shown: props.type !== 'directory',
        },
        {
            id: 'delete',
            disabled: wd,
            tooltip: wd ? wdTooltip : undefined,
            action: () => emit('delete', item),
            color: 'red' as const,
        },
    ]
})

const iconComponent = computed(() => {
    if (props.type === 'directory') {
        if (props.name === 'config') return FolderCogIcon
        if (props.name === 'world' || props.name === 'saves') return GlobeIcon
        if (props.name === 'mods') return BoxIcon
        if (props.name === 'resourcepacks') return PaintbrushIcon
        if (props.name === 'shaderpacks') return GlassesIcon
        if (props.name === 'datapacks') return BracesIcon
        return FolderOpenIcon
    }

    return getFileExtensionIcon(fileExtension.value)
})

const formattedModifiedDate = computed(() => {
    const date = new Date(props.modified * 1000)
    return formatDateTime(date)
})

const formattedCreationDate = computed(() => {
    const date = new Date(props.created * 1000)
    return formatDateTime(date)
})

const isEditableFile = computed(() => {
    if (props.type === 'file') {
        return canOpenInFileEditor(props.name)
    }
    return false
})

const formattedSize = computed(() => {
    if (props.type === 'directory') {
        return formatMessage(messages.itemCount, { count: props.count ?? 0 })
    }

    if (props.size === undefined) return ''
    return formatBytes(props.size)
})

function openContextMenu(event: MouseEvent) {
    event.preventDefault()
    emit('contextmenu', event.clientX, event.clientY)
}

function handleMouseEnter() {
    emit('hover', { name: props.name, type: props.type, path: props.path })
}

const isNavigating = ref(false)

function selectItem() {
    if (isNavigating.value || wasRecentDrag()) return
    isNavigating.value = true

    const item = { name: props.name, type: props.type, path: props.path }
    if (props.type === 'directory') {
        emit('navigate', item)
    } else if (props.type === 'file' && isEditableFile.value) {
        emit('edit', item)
    }

    setTimeout(() => {
        isNavigating.value = false
    }, 500)
}

function handlePointerDown(e: PointerEvent) {
    if (e.button !== 0) return
    startFileDrag(
        { name: props.name, type: props.type, path: props.path },
        e,
        (source, destination) => {
            emit('moveDirectTo', {
                name: source.name,
                type: source.type as FileItem['type'],
                path: source.path,
                destination,
            })
        },
    )
}
</script>
