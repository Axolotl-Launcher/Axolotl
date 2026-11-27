<script setup>
import { SpinnerIcon, TrashIcon } from '@modrinth/assets'
import {
    Button,
    commonMessages,
    defineMessages,
    IconButton,
    injectNotificationManager,
    NewModal,
    Table,
    useVIntl,
} from '@modrinth/ui'
import { computed, ref } from 'vue'

import { get_java_versions, remove_java_version } from '@/helpers/jre'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()

const messages = defineMessages({
    title: {
        id: 'app.settings.java.installed.title',
        defaultMessage: 'Installed Java versions',
    },
    version: {
        id: 'app.settings.java.table.version',
        defaultMessage: 'Java Version',
    },
    distribution: {
        id: 'app.settings.java.table.distribution',
        defaultMessage: 'Distribution',
    },
    path: {
        id: 'app.settings.java.table.path',
        defaultMessage: 'Path',
    },
    actions: {
        id: 'app.settings.java.table.actions',
        defaultMessage: '',
    },
    loading: {
        id: 'app.settings.java.installed.loading',
        defaultMessage: 'Loading installed Java versions...',
    },
    empty: {
        id: 'app.settings.java.installed.empty',
        defaultMessage: 'No Java installations found.',
    },
    remove: {
        id: 'app.settings.java.installed.remove',
        defaultMessage: 'Remove Java installation',
    },
})

const emit = defineEmits(['changed'])
const modal = ref(null)
const loading = ref(false)
const removing = ref(null)
const javaVersions = ref([])

const columns = [
    { key: 'parsed_version', label: formatMessage(messages.version), width: '8rem' },
    { key: 'distribution', label: formatMessage(messages.distribution) },
    { key: 'path', label: formatMessage(messages.path) },
    { key: 'actions', label: formatMessage(messages.actions), align: 'right', width: '3rem' },
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
    const versions = await get_java_versions().catch(handleError)
    if (versions) javaVersions.value = versions
    loading.value = false
}

async function show() {
    modal.value?.show()
    await reload()
}

async function removeEntry(javaVersion) {
    if (removing.value) return
    removing.value = javaVersion.path
    try {
        await remove_java_version(javaVersion.path)
        await reload()
        emit('changed')
    } catch (error) {
        handleError(error)
    } finally {
        removing.value = null
    }
}

defineExpose({ show })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        max-width="860px"
        width="min(860px, calc(100vw - 2rem))"
        max-content-height="min(34rem, 70vh)"
        scrollable
        actions-divider
    >
        <div
            v-if="loading"
            class="flex min-h-40 items-center justify-center gap-2 text-[var(--color-text-tertiary)]"
        >
            <SpinnerIcon class="size-4 animate-spin" aria-hidden="true" />
            {{ formatMessage(messages.loading) }}
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
                    v-tooltip="formatMessage(messages.remove)"
                    type="quiet"
                    color="red"
                    interaction="filled"
                    :label="formatMessage(messages.remove)"
                    :disabled="!!removing"
                    native-type="button"
                    @click="removeEntry(row)"
                >
                    <SpinnerIcon
                        v-if="removing === row.path"
                        class="animate-spin"
                        aria-hidden="true"
                    />
                    <TrashIcon v-else aria-hidden="true" />
                </IconButton>
            </template>
            <template #empty-state>
                <div class="py-8 text-center text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.empty) }}
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
    </NewModal>
</template>
