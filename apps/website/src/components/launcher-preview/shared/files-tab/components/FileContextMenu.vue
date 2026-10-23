<script setup lang="ts">
import ClipboardCopyIcon from '@modrinth/assets/icons/clipboard-copy.svg?component'
import FolderOpenIcon from '@modrinth/assets/icons/folder-open.svg?component'
import type { MenuOption } from '@modrinth/ui/src/components/base/menu-options.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectFileManager } from '@modrinth/ui/src/layouts/shared/files-tab/providers/file-manager.ts'
import type {
    FileContextMenuOption,
    FileItem,
} from '@modrinth/ui/src/layouts/shared/files-tab/types.ts'
import { joinDisplayPath } from '@modrinth/ui/src/layouts/shared/files-tab/utils.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, ref, shallowRef } from 'vue'

import PointMenu from '../../../overlays/PreviewPointMenu.vue'

const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()
const ctx = injectFileManager()

const menu = ref<InstanceType<typeof PointMenu>>()
const currentItem = shallowRef<FileItem | null>(null)
const menuOptions = shallowRef<FileContextMenuOption[]>([])
const options = computed<MenuOption[]>(() => [
    {
        id: 'copy-filename',
        label: formatMessage(commonMessages.copyFilenameButton),
        icon: ClipboardCopyIcon,
        action: handleCopyFilename,
    },
    {
        id: 'copy-path',
        label: formatMessage(commonMessages.copyFullPathButton),
        icon: ClipboardCopyIcon,
        action: handleCopyPath,
    },
    {
        id: 'open-folder',
        label: formatMessage(commonMessages.openInFolderButton),
        icon: FolderOpenIcon,
        shown: !!ctx.openInFolder,
        action: handleOpenInFolder,
    },
    { divider: true },
    ...menuOptions.value,
])
function show(item: FileItem, x: number, y: number, actions: FileContextMenuOption[]) {
    currentItem.value = item
    menuOptions.value = actions
    menu.value?.show({ clientX: x, clientY: y, target: document.activeElement })
}
function hide() {
    menu.value?.close()
}

function handleCopyFilename() {
    if (!currentItem.value) return
    navigator.clipboard.writeText(currentItem.value.name)
    addNotification({ title: formatMessage(commonMessages.copiedFilenameLabel), type: 'success' })
    hide()
}

function getFullPath() {
    if (!currentItem.value) return ''
    return joinDisplayPath(ctx.basePath?.value, currentItem.value.path)
}

function handleCopyPath() {
    if (!currentItem.value) return
    navigator.clipboard.writeText(getFullPath())
    addNotification({ title: formatMessage(commonMessages.copiedPathLabel), type: 'success' })
    hide()
}

function handleOpenInFolder() {
    if (!currentItem.value) return
    ctx.openInFolder?.(getFullPath())
    hide()
}

defineExpose({ show, hide })
</script>

<template>
    <PointMenu ref="menu" :options="options" @close="currentItem = null">
        <template v-for="(_, name) in $slots" #[name]><slot :name="name" /></template>
    </PointMenu>
</template>
