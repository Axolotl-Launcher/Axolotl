<script setup lang="ts">
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Badge from '@modrinth/ui/src/components/base/Badge.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, ref } from 'vue'

import type { ProjectVersion } from '../../../data'
import PreviewModal from '../../../overlays/PreviewModal.vue'
import { injectContentInstall, type ModpackInstallModalData } from '../install'
import { messages } from '../messages'

/** Port of "Install modpack" (apps/app-frontend/src/components/ui/modal/ModpackInstallModal.vue). */
const { formatMessage } = useVIntl()
const install = injectContentInstall()
const modal = ref<InstanceType<typeof PreviewModal>>()
const data = ref<ModpackInstallModalData | null>(null)
const selectedVersionId = ref('')
const instanceName = ref('')
const submitting = ref(false)
const submitted = ref(false)

function releaseColor(type: string) {
    return type === 'release' ? 'green' : type === 'beta' ? 'orange' : type === 'alpha' ? 'red' : ''
}

const selectedVersion = computed(() =>
    data.value?.versions.find((version) => version.id === selectedVersionId.value),
)

const versionOptions = computed(() =>
    (data.value?.versions ?? []).map((version) => ({
        value: version.id,
        label: versionLabel(version),
    })),
)

const existingInstances = computed(
    () => data.value?.existingInstancesByVersion[selectedVersionId.value] ?? [],
)

const folderName = computed(() => {
    const baseName = instanceName.value.trim().replace(/[\\/?*:'"|<>!]/g, '_')
    let candidate = baseName
    let index = 1
    while (candidate && data.value?.instancePaths.includes(candidate)) {
        candidate = `${baseName} (${index++})`
    }
    return candidate
})

const hasFolderNameConflict = computed(() => folderName.value !== instanceName.value.trim())

const canInstall = computed(
    () => !!selectedVersion.value && instanceName.value.trim().length > 0 && !submitting.value,
)

function versionLabel(version: ProjectVersion) {
    return [version.version_number || version.name, version.game_versions.join(', ')]
        .filter(Boolean)
        .join(' · ')
}

function show(nextData: ModpackInstallModalData) {
    data.value = nextData
    selectedVersionId.value =
        nextData.versions.find((version) => version.id === nextData.initialVersionId)?.id ??
        nextData.versions[0]?.id ??
        ''
    instanceName.value = nextData.project.title
    submitting.value = false
    submitted.value = false
    modal.value?.show()
}

function hide() {
    modal.value?.hide()
}

function submit() {
    if (!canInstall.value) return
    submitting.value = true
    submitted.value = true
    void install.handleModpackInstall(selectedVersionId.value, instanceName.value.trim())
    modal.value?.hide()
}

function handleHide() {
    if (!submitted.value) install.handleModpackInstallCancel()
}

defineExpose({ show, hide })
</script>

<template>
    <PreviewModal
        ref="modal"
        :header="formatMessage(messages.modpackInstallTitle)"
        width="min(36rem, calc(1216px - 10rem))"
        max-width="36rem"
        :on-hide="handleHide"
    >
        <div v-if="data" class="flex min-w-0 flex-col gap-4">
            <div class="flex min-w-0 items-center gap-3">
                <Avatar
                    :src="data.project.icon_url"
                    :alt="data.project.title"
                    size="3rem"
                    :tint-by="data.project.title"
                    no-shadow
                />
                <span
                    class="min-w-0 truncate text-lg font-semibold text-[var(--color-text-primary)]"
                    >{{ data.project.title }}</span
                >
            </div>

            <label class="flex flex-col gap-2">
                <span class="font-semibold text-[var(--color-text-primary)]">{{
                    formatMessage(messages.modpackVersion)
                }}</span>
                <Combobox
                    v-model="selectedVersionId"
                    :options="versionOptions"
                    :name="formatMessage(messages.modpackVersion)"
                    :display-value="
                        selectedVersion
                            ? versionLabel(selectedVersion)
                            : formatMessage(messages.modpackSelectVersion)
                    "
                />
            </label>

            <div
                v-if="selectedVersion"
                class="flex flex-wrap items-center gap-2 text-sm text-[var(--color-text-tertiary)]"
            >
                <Badge
                    :color="releaseColor(selectedVersion.version_type)"
                    :type="selectedVersion.version_type"
                />
                <span v-if="selectedVersion.loaders.length">{{
                    selectedVersion.loaders.join(', ')
                }}</span>
                <span v-if="selectedVersion.game_versions.length">{{
                    selectedVersion.game_versions.join(', ')
                }}</span>
            </div>

            <label class="flex flex-col gap-2">
                <span class="font-semibold text-[var(--color-text-primary)]">{{
                    formatMessage(messages.modpackInstanceName)
                }}</span>
                <StyledInput
                    v-model="instanceName"
                    :placeholder="formatMessage(messages.modpackInstanceNamePlaceholder)"
                    autocomplete="off"
                    wrapper-class="w-full"
                />
            </label>

            <p v-if="folderName" class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{
                    formatMessage(
                        hasFolderNameConflict
                            ? messages.modpackFolderNameConflict
                            : messages.modpackFolderName,
                        { name: folderName },
                    )
                }}
            </p>

            <div
                v-if="existingInstances.length"
                class="flex items-start gap-2 rounded-lg border border-warning bg-warning-bg p-3"
            >
                <CheckIcon class="mt-0.5 shrink-0" />
                <div class="min-w-0">
                    <p class="m-0 font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.modpackAlreadyInstalled) }}
                    </p>
                    <p class="mb-0 mt-1 text-sm text-[var(--color-text-tertiary)]">
                        {{
                            formatMessage(messages.modpackAlreadyInstalledDescription, {
                                names: existingInstances
                                    .map((instance) => instance.name)
                                    .join(', '),
                            })
                        }}
                    </p>
                </div>
            </div>
        </div>

        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" :disabled="submitting" @click="hide"
                    ><XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button type="colored" color="brand" :disabled="!canInstall" @click="submit"
                    ><DownloadIcon />
                    {{ formatMessage(messages.modpackInstall) }}
                </Button>
            </div>
        </template>
    </PreviewModal>
</template>
