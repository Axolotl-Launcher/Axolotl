<script setup lang="ts">
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import ButtonLink from '@modrinth/ui/src/components/base/buttons/ButtonLink.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'

import { desktopAppMessages } from '../lib/messages'

/**
 * Where the preview cannot do what the desktop app does, this leads to the
 * site's download section: it leaves full screen and scrolls there, or opens
 * the home page's section when the preview is shown elsewhere.
 */
withDefaults(defineProps<{ size?: 'sm' | 'md' }>(), { size: 'sm' })

const { formatMessage } = useVIntl()

async function openDownloads(event: MouseEvent) {
    const section = document.getElementById('download')
    if (!section) return
    event.preventDefault()
    if (document.fullscreenElement) await document.exitFullscreen().catch(() => undefined)
    section.scrollIntoView({ behavior: 'smooth', block: 'start' })
}
</script>

<template>
    <ButtonLink href="/#download" type="outlined" :size="size" @click="openDownloads">
        <DownloadIcon aria-hidden="true" />
        {{ formatMessage(desktopAppMessages.download) }}
    </ButtonLink>
</template>
