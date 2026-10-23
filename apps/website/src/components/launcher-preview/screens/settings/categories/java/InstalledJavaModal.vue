<script setup lang="ts">
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import IconButton from '@modrinth/ui/src/components/base/buttons/IconButton.vue'
import Table from '@modrinth/ui/src/components/base/Table.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, ref } from 'vue'

import PreviewModal from '../../../../overlays/PreviewModal.vue'
import { messages } from '../../messages'
import { type JavaVersion, useJre } from './jre'

/**
 * "View installed Java" (apps/app-frontend/src/components/ui/settings/
 * InstalledJavaModal.vue): every registered Java installation, newest major first,
 * each removable from the launcher's list (the files stay on disk).
 */
const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const jre = useJre()

const emit = defineEmits<{ changed: [] }>()
const modal = ref<InstanceType<typeof PreviewModal> | null>(null)
const loading = ref(false)
const javaVersions = ref<JavaVersion[]>([])

const columns = [
    {
        key: 'parsed_version',
        label: formatMessage(messages.appSettingsJavaTableVersion),
        width: '8rem',
    },
    { key: 'distribution', label: formatMessage(messages.appSettingsJavaTableDistribution) },
    { key: 'path', label: formatMessage(messages.appSettingsJavaTablePath) },
    {
        key: 'actions',
        label: formatMessage(messages.appSettingsJavaTableActions),
        align: 'right' as const,
        width: '3rem',
    },
]

const tableData = computed(() =>
    javaVersions.value
        .map((javaVersion) => ({
            ...javaVersion,
            distribution: javaVersion.distribution || null,
        }))
        .sort((a, b) => b.parsed_version - a.parsed_version || a.path.localeCompare(b.path)),
)

async function reload() {
    loading.value = true
    const versions = await jre.getJavaVersions().catch(handleError)
    if (versions) javaVersions.value = versions
    loading.value = false
}

async function show() {
    modal.value?.show()
    await reload()
}

async function removeEntry(javaVersion: JavaVersion) {
    const removed = await jre
        .removeJavaVersion(javaVersion.path)
        .then(() => true)
        .catch((error) => {
            handleError(error)
            return false
        })
    if (!removed) return

    javaVersions.value = javaVersions.value.filter((item) => item.path !== javaVersion.path)
    emit('changed')
}

defineExpose({ show })
</script>

<template>
    <PreviewModal
        ref="modal"
        :header="formatMessage(messages.appSettingsJavaInstalledTitle)"
        max-width="860px"
        width="860px"
        max-content-height="min(34rem, 560px)"
        scrollable
        actions-divider
    >
        <div
            v-if="loading"
            class="flex min-h-40 items-center justify-center gap-2 text-[var(--color-text-tertiary)]"
        >
            <SpinnerIcon class="size-4 animate-spin" aria-hidden="true" />
            {{ formatMessage(messages.appSettingsJavaInstalledLoading) }}
        </div>
        <Table v-else :columns="columns" :data="tableData" row-key="path">
            <template #cell-parsed_version="{ value }">
                <span class="font-semibold tabular-nums">Java {{ value }}</span>
            </template>
            <template #cell-distribution="{ value }">
                <span class="text-sm">{{ value || '-' }}</span>
            </template>
            <template #cell-path="{ value }">
                <span v-tooltip="value" class="block max-w-96 truncate font-mono text-xs">
                    {{ value }}
                </span>
            </template>
            <template #cell-actions="{ row }">
                <IconButton
                    v-tooltip="formatMessage(messages.appSettingsJavaInstalledRemove)"
                    type="quiet"
                    color="red"
                    interaction="filled"
                    :label="formatMessage(messages.appSettingsJavaInstalledRemove)"
                    native-type="button"
                    @click="removeEntry(row as JavaVersion)"
                >
                    <TrashIcon aria-hidden="true" />
                </IconButton>
            </template>
            <template #empty-state>
                <div class="py-8 text-center text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.appSettingsJavaInstalledEmpty) }}
                </div>
            </template>
        </Table>

        <template #actions>
            <div class="flex justify-end">
                <Button type="outlined" native-type="button" @click="modal?.hide()">
                    {{ formatMessage(commonMessages.closeButton) }}
                </Button>
            </div>
        </template>
    </PreviewModal>
</template>
