<script setup lang="ts">
import FileArchiveIcon from '@modrinth/assets/icons/file-archive.svg?component'
import FolderSearchIcon from '@modrinth/assets/icons/folder-search.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import EmptyState from '@modrinth/ui/src/components/base/EmptyState.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { ref } from 'vue'
import { useRoute } from 'vue-router'

import zhCN from '~/locales/launcher-preview/lab-a.zh-CN'

import { formatPreviewMessage, registerPreviewMessages } from '../../lib/i18n'
import { desktopAppMessages } from '../../lib/messages'
import DesktopAppLink from '../../shell/DesktopAppLink.vue'

/**
 * The opening screen of `pages/LabSchematicPreview.vue`. The workshop reads the
 * schematic and edits it in a 3D scene built on deepslate, which the desktop app
 * ships; choosing a schematic here, from a file or an instance (or from the
 * Files tab's "Open in Schematic Workshop"), leads to the desktop app instead.
 */
registerPreviewMessages('zh-CN', zhCN)

const { formatMessage } = useVIntl()
const route = useRoute()

const messages = defineMessages({
    emptyTitle: {
        id: 'app.lab.schematic-preview.empty.title',
        defaultMessage: 'Edit a Minecraft schematic',
    },
    emptyDescription: {
        id: 'app.lab.schematic-preview.empty.description',
        defaultMessage:
            'Open a Litematica or Sponge schematic to inspect and edit it locally in 3D.',
    },
    openFile: { id: 'app.lab.schematic-preview.open-file', defaultMessage: 'Open file' },
    fromInstance: {
        id: 'app.lab.schematic-preview.from-instance',
        defaultMessage: 'From instance',
    },
})

const fileInput = ref<HTMLInputElement | null>(null)
/** The schematic the visitor chose, which only the desktop app can open. */
const chosen = ref(
    typeof route.query.path === 'string' ? (route.query.path.split('/').pop() ?? '') : '',
)
const needsDesktop = ref(!!chosen.value)

function chooseLocalFile() {
    fileInput.value?.click()
}

function onPicked(event: Event) {
    const target = event.target as HTMLInputElement
    const file = target.files?.[0]
    target.value = ''
    if (!file) return
    chosen.value = file.name
    needsDesktop.value = true
}

function chooseFromInstance() {
    chosen.value = ''
    needsDesktop.value = true
}
</script>

<template>
    <main class="schematic-page">
        <section class="schematic-empty">
            <input
                ref="fileInput"
                type="file"
                accept=".litematic,.schem"
                class="hidden"
                :aria-label="formatMessage(messages.openFile)"
                @change="onPicked"
            />
            <EmptyState
                type="no-documents"
                :heading="formatMessage(messages.emptyTitle)"
                :description="formatMessage(messages.emptyDescription)"
            >
                <template #actions>
                    <Button type="colored" color="brand" @click="chooseLocalFile"
                        ><FileArchiveIcon />{{ formatMessage(messages.openFile) }}
                    </Button>
                    <Button type="outlined" @click="chooseFromInstance"
                        ><FolderSearchIcon />{{ formatMessage(messages.fromInstance) }}
                    </Button>
                </template>
            </EmptyState>
            <Admonition
                v-if="needsDesktop"
                type="info"
                class="w-[min(36rem,100%)]"
                :header="formatPreviewMessage(desktopAppMessages.schematic)"
                :body="chosen"
            >
                <template #actions><DesktopAppLink /></template>
            </Admonition>
        </section>
    </main>
</template>

<style scoped>
/* The app's `calc(100vh - 4rem)`, against the 800px window. */
.schematic-page {
    position: relative;
    display: flex;
    height: calc(800px - 4rem);
    min-height: 36rem;
    width: 100%;
    flex-direction: column;
    overflow: hidden;
    background: var(--color-bg);
}

.schematic-empty {
    display: flex;
    min-height: 22rem;
    flex: 1;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1rem;
    padding: 2rem;
}
</style>
