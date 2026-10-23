<script setup lang="ts">
import ClipboardCopyIcon from '@modrinth/assets/icons/clipboard-copy.svg?component'
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import GlobeIcon from '@modrinth/assets/icons/globe.svg?component'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { ref } from 'vue'

import PreviewContextMenu from '../../../overlays/PreviewContextMenu.vue'

/**
 * The search result right-click menu (Browse.vue `ContextMenu` with the handlers of
 * composables/browse/use-app-server-browse.ts): open the Modrinth page or copy its link.
 */
const { formatMessage } = useVIntl()
const menu = ref<InstanceType<typeof PreviewContextMenu>>()

type MenuProject = { project_id: string; slug: string | null; project_type?: string }

function projectUrl(project: MenuProject) {
    return `https://modrinth.com/${project.project_type ?? 'project'}/${project.slug ?? project.project_id}`
}

function show(event: MouseEvent, project: MenuProject) {
    menu.value?.showMenu(event, project, [{ name: 'open_link' }, { name: 'copy_link' }])
}

function handleOptionsClick(args: { item: unknown; option: string }) {
    const url = projectUrl(args.item as MenuProject)
    if (args.option === 'open_link') window.open(url, '_blank', 'noopener,noreferrer')
    else if (args.option === 'copy_link') void navigator.clipboard?.writeText(url).catch(() => {})
}

defineExpose({ show })
</script>

<template>
    <PreviewContextMenu ref="menu" @option-clicked="handleOptionsClick">
        <template #open_link>
            <GlobeIcon /> {{ formatMessage(commonMessages.openInModrinthButton) }}
            <ExternalIcon />
        </template>
        <template #copy_link>
            <ClipboardCopyIcon /> {{ formatMessage(commonMessages.copyLinkButton) }}
        </template>
    </PreviewContextMenu>
</template>
