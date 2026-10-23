<script setup lang="ts">
import FileArchiveIcon from '@modrinth/assets/icons/file-archive.svg?component'
import PlayIcon from '@modrinth/assets/icons/play.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, reactive } from 'vue'

import { formatPreviewMessage } from '../../lib/i18n'
import { desktopAppMessages } from '../../lib/messages'
import { usePreviewDomain } from '../../lib/state'
import { modTranslationMessages as messages } from './mod-translation/i18n'
import ModTranslationFilePicker from './mod-translation/ModTranslationFilePicker.vue'
import ModTranslationJobList from './mod-translation/ModTranslationJobList.vue'
import ModTranslationSettingsPanel from './mod-translation/ModTranslationSettingsPanel.vue'
import type { ModTranslationOptions } from './mod-translation/types'

/**
 * Port of `pages/LabModTranslation.vue`. Analysing a JAR and translating it run
 * in the desktop app (its mod-translation plugin and the AI provider), so those
 * two buttons say so; the input, the AI choice and the options work as in the
 * app and are kept for the window, as the app's store keeps them.
 */
const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()

const store = usePreviewDomain('lab:mod-translation', () => ({
    value: reactive({
        inputPath: '',
        providerId: '',
        modelId: '',
        options: {
            batchSize: 40,
            deepBatchSize: 24,
            generateModName: false,
            repairEnabled: true,
            classTextEnabled: false,
            maxClassBatch: 16,
        } as ModTranslationOptions,
    }),
}))

const canStart = computed(() => !!store.inputPath && !!store.providerId && !!store.modelId)
const startHint = computed(() => {
    if (!store.inputPath) return formatMessage(messages.startHint)
    if (!store.providerId || !store.modelId) return formatMessage(messages.aiNotConfigured)
    return formatMessage(messages.outputPath, { path: defaultOutputPath(store.inputPath) })
})

function defaultOutputPath(path: string): string {
    return path.replace(/\.jar$/i, '-zh_cn.jar')
}

function desktopOnly(message: { id: string; defaultMessage?: string }) {
    addNotification({ type: 'info', title: formatPreviewMessage(message) })
}
</script>

<template>
    <main class="mod-translation-page flex min-h-0 flex-col gap-4 p-6">
        <header class="flex min-w-0 items-start justify-between gap-4">
            <div class="min-w-0">
                <h1 class="m-0 text-2xl font-bold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.title) }}
                </h1>
                <p class="m-0 mt-1 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.description) }}
                </p>
            </div>
        </header>

        <div class="mod-translation-grid">
            <div class="flex min-w-0 flex-col gap-4">
                <section class="panel">
                    <h2 class="panel-title">{{ formatMessage(messages.inputSection) }}</h2>
                    <ModTranslationFilePicker v-model:path="store.inputPath" />
                    <div class="flex items-center gap-2">
                        <Button
                            type="colored"
                            color="brand"
                            :disabled="!store.inputPath"
                            @click="desktopOnly(desktopAppMessages.modAnalysis)"
                            ><FileArchiveIcon />
                            {{ formatMessage(messages.analyze) }}
                        </Button>
                        <span v-if="!store.inputPath" class="panel-hint">
                            {{ formatMessage(messages.selectFile) }}…
                        </span>
                    </div>
                </section>

                <section class="panel">
                    <h2 class="panel-title">{{ formatMessage(messages.aiSection) }}</h2>
                    <ModTranslationSettingsPanel
                        v-model="store.options"
                        v-model:provider-id="store.providerId"
                        v-model:model-id="store.modelId"
                    />
                    <div class="flex flex-col gap-1.5">
                        <Button
                            type="colored"
                            color="brand"
                            class="start-button"
                            :disabled="!canStart"
                            @click="desktopOnly(desktopAppMessages.modTranslation)"
                            ><PlayIcon />{{ formatMessage(messages.start) }}
                        </Button>
                        <span class="panel-hint">{{ startHint }}</span>
                    </div>
                </section>
            </div>

            <div class="flex min-w-0 flex-col">
                <section class="panel jobs-panel flex-1">
                    <h2 class="panel-title">{{ formatMessage(messages.jobsSection) }}</h2>
                    <ModTranslationJobList />
                </section>
            </div>
        </div>
    </main>
</template>

<style scoped>
/* The app's two columns from 1100px of window width; the window is 1280px wide. */
.mod-translation-grid {
    display: grid;
    grid-template-columns: minmax(0, 5fr) minmax(0, 7fr);
    gap: 1rem;
    align-items: start;
}

.panel {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-md);
    background: var(--surface-2);
    padding: 1rem;
}

.panel-title {
    margin: 0;
    font-size: 0.72rem;
    font-weight: 800;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--color-text-tertiary);
}

.panel-hint {
    font-size: 0.72rem;
    color: var(--color-text-tertiary);
}

.start-button {
    display: inline-flex;
    width: 100%;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    white-space: nowrap;
}
</style>
