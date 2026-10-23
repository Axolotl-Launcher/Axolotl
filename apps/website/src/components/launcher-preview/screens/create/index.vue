<script setup lang="ts">
import FolderOpenIcon from '@modrinth/assets/icons/folder-open.svg?component'
import LeftArrowIcon from '@modrinth/assets/icons/left-arrow.svg?component'
import SparklesIcon from '@modrinth/assets/icons/sparkles.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import BigOptionButton from '@modrinth/ui/src/components/base/BigOptionButton.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { onBeforeUnmount, onMounted, useTemplateRef } from 'vue'
import { useRouter } from 'vue-router'

import { injectPreviewWindow } from '../../lib/window'
import ConfirmDropTypeModal from './components/ConfirmDropTypeModal.vue'
import CreationFlowModal from './components/CreationFlowModal.vue'
import FolderPanel from './components/FolderPanel.vue'
import GenericContentInstallModal from './components/GenericContentInstallModal.vue'
import InstanceIconPickerModal from './components/InstanceIconPickerModal.vue'
import LauncherImportModal from './components/LauncherImportModal.vue'
import SymlinkMethodCards from './components/SymlinkMethodCards.vue'
import UnknownPackWarningModal from './components/UnknownPackWarningModal.vue'
import type { ImportFlowBindings } from './import-flow'
import { HOME } from './mac-fs'
import { dropNotificationMessages, pageMessages as messages } from './messages'
import { useCreateArea } from './state'

/**
 * Port of `apps/app-frontend/src/pages/Create.vue`. The creation flow modal and the
 * import modals live in the app's `App.vue`; only this page opens them, so the
 * page hosts them here and binds them to the area's flow.
 */
const { formatMessage } = useVIntl()
const router = useRouter()
const { layer } = injectPreviewWindow()
const area = useCreateArea()
const importFlow = area.importFlow

const iconPicker = useTemplateRef<InstanceType<typeof InstanceIconPickerModal>>('iconPicker')
const confirmDropModal =
    useTemplateRef<InstanceType<typeof ConfirmDropTypeModal>>('confirmDropModal')
const launcherImportModal =
    useTemplateRef<InstanceType<typeof LauncherImportModal>>('launcherImportModal')
const symlinkCardsModal =
    useTemplateRef<InstanceType<typeof SymlinkMethodCards>>('symlinkCardsModal')
const unknownPackWarningModal =
    useTemplateRef<InstanceType<typeof UnknownPackWarningModal>>('unknownPackWarningModal')
const genericInstallModal =
    useTemplateRef<InstanceType<typeof GenericContentInstallModal>>('genericInstallModal')
const folderPanel = useTemplateRef<InstanceType<typeof FolderPanel>>('folderPanel')
const fileInput = useTemplateRef<HTMLInputElement>('fileInput')

const skipUnknownPackWarning = area.skipUnknownPackWarning

function navigateBack() {
    void router.push('/library')
}

function handleStartFresh() {
    void area.flow.show({
        skipSetupType: true,
        initialMode: 'custom',
        onBack: () => router.push('/create'),
    })
}

function handleImportExisting() {
    void area.flow.show({
        skipSetupType: true,
        initialMode: 'import',
        onBack: () => router.push('/create'),
    })
}

/** The native file dialog of `pickFile()`, here the browser's file picker. */
let resolveFile: ((picked: { file: File; path: string } | null) => void) | null = null

function pickFile(): Promise<{ file: File; path: string } | null> {
    resolveFile?.(null)
    return new Promise((resolve) => {
        resolveFile = resolve
        const input = fileInput.value
        if (!input) {
            resolve(null)
            return
        }
        input.value = ''
        input.click()
    })
}

function onFilePicked(event: Event) {
    const input = event.target as HTMLInputElement
    const file = input.files?.[0]
    const resolve = resolveFile
    resolveFile = null
    resolve?.(file ? { file, path: `${HOME}/Downloads/${file.name}` } : null)
}

function onFileCancelled() {
    const resolve = resolveFile
    resolveFile = null
    resolve?.(null)
}

const bindings: ImportFlowBindings = {
    formatMessage,
    router,
    confirmDrop: {
        show: () => confirmDropModal.value?.show(),
        hide: () => void confirmDropModal.value?.hide(),
    },
    launcherImport: {
        show: (results) => launcherImportModal.value?.show(results),
        hide: () => launcherImportModal.value?.hide(),
    },
    symlinkCards: {
        show: (options) => symlinkCardsModal.value?.show(options),
        hide: () => symlinkCardsModal.value?.hide(),
    },
    unknownPack: {
        show: (onProceed, fileName) => unknownPackWarningModal.value?.show(onProceed, fileName),
    },
    genericInstall: {
        show: (options) => genericInstallModal.value?.show(options),
        hide: () => genericInstallModal.value?.hide(),
    },
    folderPanel: {
        pick: () => folderPanel.value?.pick() ?? Promise.resolve(null),
        browse: (path) => folderPanel.value?.browse(path),
    },
    filePicker: { pick: pickFile },
}

const iconPickerApi = {
    show: () => iconPicker.value?.show() ?? Promise.resolve(null),
}

function handleGenericInstallCancel() {
    genericInstallModal.value?.hide()
    importFlow.handleGenericInstallCancel()
}

onMounted(() => {
    importFlow.bind(bindings)
    area.iconPicker.value = iconPickerApi
    void area.ensureData()
})

onBeforeUnmount(() => {
    importFlow.unbind(bindings)
    if (area.iconPicker.value === iconPickerApi) area.iconPicker.value = null
    resolveFile?.(null)
    resolveFile = null
})
</script>

<template>
    <div class="flex h-full w-full flex-col items-center justify-center p-6">
        <div class="flex w-full max-w-2xl flex-col gap-6">
            <div class="flex flex-col gap-2">
                <h1 class="m-0 text-2xl font-bold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.title) }}
                </h1>
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.subtitle) }}
                </p>
            </div>

            <div data-onboarding-id="creation-methods" class="flex flex-col gap-4 sm:flex-row">
                <BigOptionButton
                    data-onboarding-id="creation-method-custom"
                    :icon="SparklesIcon"
                    :title="formatMessage(messages.newTitle)"
                    :description="formatMessage(messages.newDescription)"
                    no-icon-box
                    @click="handleStartFresh"
                />

                <BigOptionButton
                    data-onboarding-id="creation-method-import"
                    :icon="FolderOpenIcon"
                    :title="formatMessage(messages.importTitle)"
                    :description="formatMessage(messages.importDescription)"
                    no-icon-box
                    @click="handleImportExisting"
                />
            </div>

            <Button type="quiet" class="self-start" @click="navigateBack">
                <LeftArrowIcon class="size-4" stroke-width="2" />
                {{ formatMessage(messages.back) }}
            </Button>
        </div>

        <InstanceIconPickerModal ref="iconPicker" />
        <CreationFlowModal />
        <ConfirmDropTypeModal
            ref="confirmDropModal"
            :classification="importFlow.dropClassification.value"
            :file-name="importFlow.dropFileName.value"
            @confirm="importFlow.handleConfirmDropConfirm"
            @cancel="importFlow.handleConfirmDropCancel"
            @help="importFlow.handleConfirmDropHelp"
        />
        <GenericContentInstallModal
            ref="genericInstallModal"
            @install="importFlow.handleGenericInstall"
            @cancel="handleGenericInstallCancel"
            @navigate-create="importFlow.handleGenericInstallNavigateCreate"
        />
        <LauncherImportModal
            ref="launcherImportModal"
            @confirm="importFlow.onImportSelected"
            @cancel="importFlow.onLauncherImportCancelled"
        />
        <SymlinkMethodCards
            ref="symlinkCardsModal"
            @confirm="importFlow.onSymlinkMethodConfirmed"
            @cancel="importFlow.onSymlinkMethodCancelled"
        />
        <UnknownPackWarningModal
            ref="unknownPackWarningModal"
            v-model:skip-warning="skipUnknownPackWarning"
            @closed="importFlow.onUnknownPackClosed"
        />
        <FolderPanel ref="folderPanel" />

        <input
            ref="fileInput"
            type="file"
            class="hidden"
            @change="onFilePicked"
            @cancel="onFileCancelled"
        />

        <!-- Processing overlay (App.vue) while launcher instances are scanned -->
        <Teleport :to="layer" :disabled="!layer">
            <div
                v-if="importFlow.scanningInstances.value"
                class="scan-overlay fixed inset-0 flex items-center justify-center bg-black/20"
            >
                <div class="flex flex-col items-center gap-3">
                    <SpinnerIcon class="h-10 w-10 animate-spin text-[var(--color-text-primary)]" />
                    <span class="text-sm text-[var(--color-text-tertiary)]"
                        >{{ formatMessage(dropNotificationMessages.scanning) }}…</span
                    >
                </div>
            </div>
        </Teleport>
    </div>
</template>

<style scoped>
.scan-overlay {
    z-index: 9999;
}
</style>
