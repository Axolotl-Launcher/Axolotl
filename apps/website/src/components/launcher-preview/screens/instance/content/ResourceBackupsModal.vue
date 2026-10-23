<script setup lang="ts">
import FileIcon from '@modrinth/assets/icons/file.svg?component'
import RefreshCwIcon from '@modrinth/assets/icons/refresh-cw.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
/**
 * Port of `apps/app-frontend/src/components/ui/ResourceBackupsModal.vue` on the
 * in-window modal, over the preview's `.old` files (`./resource-backups`).
 */
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useFormatBytes } from '@modrinth/ui/src/composables/format-bytes.ts'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, onBeforeUnmount, ref } from 'vue'

import NewModal from '../../../overlays/PreviewModal.vue'
import { resourceOperationError } from './resource-backups.ts'
import {
    type ResourceBackupFile,
    resourceExecuteHistoryCleanup,
    type ResourceHistoryCleanupPlan,
    resourcePrepareHistoryCleanup,
    resourceScanBackups,
} from './resource-backups.ts'

const emit = defineEmits<{ cleaned: [] }>()
const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()
const messages = defineMessages({
    title: { id: 'app.resources.backups.title', defaultMessage: 'Old files and rollback backups' },
    description: {
        id: 'app.resources.backups.description',
        defaultMessage:
            'Review old files before removing them. Verified backups can restore a previous installation.',
    },
    search: { id: 'app.resources.backups.search', defaultMessage: 'Search file names or paths…' },
    all: { id: 'app.resources.backups.filter.all', defaultMessage: 'All files' },
    rollback: { id: 'app.resources.backups.filter.rollback', defaultMessage: 'Rollback backups' },
    old: { id: 'app.resources.backups.filter.old', defaultMessage: 'Other old files' },
    selectVisible: {
        id: 'app.resources.backups.select-visible',
        defaultMessage: 'Select available files',
    },
    noMatches: {
        id: 'app.resources.backups.no-matches',
        defaultMessage: 'No files match your search or filter.',
    },
    selectedSize: {
        id: 'app.resources.backups.selected-size',
        defaultMessage: '{count, number} selected · {size}',
    },
    reviewTitle: {
        id: 'app.resources.backups.review-title',
        defaultMessage: 'Confirm backup cleanup',
    },
    reviewSize: {
        id: 'app.resources.backups.review-size',
        defaultMessage: '{count, plural, one {Delete # file} other {Delete # files}} · {size}',
    },
    scan: { id: 'app.resources.backups.scan', defaultMessage: 'Scan again' },
    loading: { id: 'app.resources.backups.loading', defaultMessage: 'Checking old files…' },
    empty: {
        id: 'app.resources.backups.empty',
        defaultMessage: 'No old files or rollback backups found.',
    },
    valid: { id: 'app.resources.backups.valid', defaultMessage: 'Verified rollback backup' },
    orphan: {
        id: 'app.resources.backups.orphan',
        defaultMessage: 'Old file without an available rollback',
    },
    invalid: {
        id: 'app.resources.backups.invalid',
        defaultMessage: 'Recorded backup unavailable for rollback',
    },
    fileSize: { id: 'app.resources.backups.file-size', defaultMessage: '{size, number} bytes' },
    unknown: {
        id: 'app.resources.backups.unknown',
        defaultMessage: 'Unknown old file — cleanup unavailable',
    },
    missing: { id: 'app.resources.backups.missing', defaultMessage: 'File missing' },
    summary: {
        id: 'app.resources.backups.summary',
        defaultMessage: '{count, number} selected · {size, number} bytes',
    },
    review: { id: 'app.resources.backups.review', defaultMessage: 'Review selected cleanup' },
    warning: {
        id: 'app.resources.backups.warning',
        defaultMessage:
            'Deleting these files permanently removes {count, number} available rollback backups. Current installed files are kept.',
    },
    confirm: { id: 'app.resources.backups.confirm', defaultMessage: 'Delete these backups' },
    back: { id: 'app.resources.backups.back', defaultMessage: 'Back to selection' },
    close: { id: 'app.resources.backups.close', defaultMessage: 'Close' },
    removed: {
        id: 'app.resources.backups.removed',
        defaultMessage: '{count, number} files deleted.',
    },
    failed: {
        id: 'app.resources.backups.failed',
        defaultMessage:
            '{count, number} files could not be deleted. Scan again to check their state.',
    },
})
const modal = ref<InstanceType<typeof NewModal> | null>(null)
const files = ref<ResourceBackupFile[]>([])
const selected = ref<string[]>([])
const plan = ref<ResourceHistoryCleanupPlan | null>(null)
const busy = ref(false)
const cleaning = ref(false)
const error = ref<string | null>(null)
const resultText = ref<string | null>(null)
const search = ref('')
const filter = ref<'all' | 'rollback' | 'old'>('all')
const visibleFiles = computed(() =>
    files.value.filter(
        (file) =>
            (!search.value.trim() ||
                `${fileName(file)} ${file.relativePath}`
                    .toLowerCase()
                    .includes(search.value.trim().toLowerCase())) &&
            (filter.value === 'all' ||
                (filter.value === 'rollback' ? file.rollbackAvailable : !file.rollbackAvailable)),
    ),
)
const selectable = computed(() =>
    visibleFiles.value.filter((file) => file.recognized && file.exists),
)
const visibleSelected = computed(
    () => selectable.value.filter((file) => selected.value.includes(file.id)).length,
)
function selectVisible(checked: boolean) {
    const visible = new Set(selectable.value.map((file) => file.id))
    selected.value = checked
        ? [...new Set([...selected.value, ...visible])]
        : selected.value.filter((id) => !visible.has(id))
}
function fileName(file: ResourceBackupFile) {
    return file.originalFileName ?? file.relativePath.split('/').pop() ?? file.relativePath
}
const size = computed(() =>
    files.value
        .filter((file) => selected.value.includes(file.id))
        .reduce((sum, file) => sum + (file.integrity.size ?? 0), 0),
)
let instanceId = ''
let generation = 0
function hide() {
    generation++
}
function show(id: string) {
    generation++
    instanceId = id
    files.value = []
    selected.value = []
    plan.value = null
    resultText.value = null
    search.value = ''
    filter.value = 'all'
    modal.value?.show()
    void scan()
}
async function scan() {
    const current = generation
    busy.value = true
    error.value = null
    plan.value = null
    try {
        const value = await resourceScanBackups(instanceId)
        if (current !== generation) return
        files.value = value
        selected.value = selected.value.filter((id) =>
            value.some((file) => file.id === id && file.exists && file.recognized),
        )
    } catch (failure) {
        if (current === generation) error.value = resourceOperationError(failure, formatMessage)
    } finally {
        if (current === generation) busy.value = false
    }
}
function select(id: string, checked: boolean) {
    selected.value = checked
        ? [...new Set([...selected.value, id])]
        : selected.value.filter((value) => value !== id)
    plan.value = null
}
async function review() {
    if (busy.value || !selected.value.length) return
    busy.value = true
    error.value = null
    const current = generation
    try {
        const value = await resourcePrepareHistoryCleanup(instanceId, [...selected.value])
        if (current === generation) plan.value = value
    } catch (failure) {
        if (current === generation) error.value = resourceOperationError(failure, formatMessage)
    } finally {
        if (current === generation) busy.value = false
    }
}
async function cleanup() {
    if (busy.value || !plan.value) return
    busy.value = true
    error.value = null
    const current = generation
    const reviewed = plan.value
    cleaning.value = true
    try {
        const result = await resourceExecuteHistoryCleanup(reviewed.id, reviewed.revision)
        if (current !== generation) return
        resultText.value = formatMessage(
            result.failures.length ? messages.failed : messages.removed,
            {
                count:
                    result.failures.length ||
                    reviewed.files.filter((file) => result.removed.includes(file.id)).length,
            },
        )
        if (result.failures.length)
            error.value = result.failures
                .map(([, failure]) => resourceOperationError(failure, formatMessage))
                .join('\n')
        const affectedPaths = new Set(reviewed.files.map((file) => file.relativePath))
        const affected = new Set(
            files.value
                .filter((file) => affectedPaths.has(file.relativePath))
                .map((file) => file.id),
        )
        selected.value = selected.value.filter((id) => !affected.has(id))
        files.value = files.value.map((file) =>
            affected.has(file.id) ? { ...file, exists: false, rollbackAvailable: false } : file,
        )
        plan.value = null
        emit('cleaned')
    } catch (failure) {
        if (current === generation) error.value = resourceOperationError(failure, formatMessage)
    } finally {
        if (current === generation) {
            busy.value = false
            cleaning.value = false
        }
    }
}
onBeforeUnmount(hide)
defineExpose({ show })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(plan ? messages.reviewTitle : messages.title)"
        :on-hide="hide"
        :disable-close="cleaning"
        scrollable
        width="min(48rem, 1152px)"
    >
        <div class="flex min-w-0 flex-col gap-4">
            <p v-if="!plan" class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.description) }}
            </p>
            <p v-if="error" class="m-0 whitespace-pre-wrap text-red" role="alert">{{ error }}</p>
            <p v-if="resultText" class="m-0 text-brand" role="status">{{ resultText }}</p>
            <template v-if="plan">
                <Admonition
                    type="warning"
                    :header="
                        formatMessage(messages.reviewSize, {
                            count: plan.files.length,
                            size: formatBytes(
                                plan.files.reduce(
                                    (total, file) => total + (file.integrity.size ?? 0),
                                    0,
                                ),
                            ),
                        })
                    "
                >
                    {{
                        formatMessage(messages.warning, {
                            count: plan.files.filter((file) => file.rollbackAvailable).length,
                        })
                    }}
                </Admonition>
                <div class="divide-y divide-divider rounded-xl border border-solid border-divider">
                    <div
                        v-for="file in plan.files"
                        :key="file.id"
                        class="flex min-w-0 items-center gap-3 px-4 py-3"
                    >
                        <FileIcon class="size-5 shrink-0 text-[var(--color-text-tertiary)]" />
                        <div class="min-w-0 flex-1">
                            <p class="m-0 break-all font-semibold">{{ fileName(file) }}</p>
                            <p class="m-0 break-all text-xs text-[var(--color-text-tertiary)]">
                                {{ file.relativePath }}
                            </p>
                        </div>
                        <span class="shrink-0 text-sm tabular-nums">{{
                            formatBytes(file.integrity.size ?? 0)
                        }}</span>
                    </div>
                </div>
            </template>
            <template v-else>
                <div class="flex flex-wrap items-center gap-2">
                    <StyledInput
                        v-model="search"
                        :icon="SearchIcon"
                        :placeholder="formatMessage(messages.search)"
                        clearable
                        wrapper-class="min-w-[10rem] flex-1"
                    />
                    <Combobox
                        v-model="filter"
                        width="content"
                        :options="
                            (['all', 'rollback', 'old'] as const).map((value) => ({
                                value,
                                label: formatMessage(messages[value]),
                            }))
                        "
                    />
                    <Button :disabled="busy" type="outlined" @click="scan()"
                        ><RefreshCwIcon class="size-4" />{{ formatMessage(messages.scan) }}</Button
                    >
                </div>
                <p
                    v-if="busy && !files.length"
                    class="m-0 py-6 text-center text-[var(--color-text-tertiary)]"
                    role="status"
                >
                    {{ formatMessage(messages.loading) }}
                </p>
                <p
                    v-else-if="!files.length && !error"
                    class="m-0 py-6 text-center text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.empty) }}
                </p>
                <template v-else-if="visibleFiles.length">
                    <Checkbox
                        :model-value="
                            selectable.length > 0 && visibleSelected === selectable.length
                        "
                        :indeterminate="visibleSelected > 0 && visibleSelected < selectable.length"
                        :disabled="busy || !selectable.length"
                        :label="formatMessage(messages.selectVisible)"
                        @update:model-value="selectVisible"
                    />
                    <div
                        class="divide-y divide-divider rounded-xl border border-solid border-divider"
                    >
                        <div
                            v-for="file in visibleFiles"
                            :key="file.id"
                            class="flex min-w-0 items-center gap-3 px-4 py-3 transition-colors hover:bg-surface-3"
                            :class="{ 'bg-surface-3': selected.includes(file.id) }"
                        >
                            <Checkbox
                                :model-value="selected.includes(file.id)"
                                :disabled="busy || !file.recognized || !file.exists"
                                :description="file.relativePath"
                                @update:model-value="select(file.id, $event)"
                            />
                            <div class="min-w-0 flex-1">
                                <p class="m-0 break-all font-semibold">{{ fileName(file) }}</p>
                                <p class="m-0 break-all text-xs text-[var(--color-text-tertiary)]">
                                    {{ file.relativePath }}
                                </p>
                                <p
                                    class="mb-0 mt-1 text-xs"
                                    :class="
                                        file.rollbackAvailable
                                            ? 'text-brand-orange'
                                            : 'text-[var(--color-text-tertiary)]'
                                    "
                                >
                                    {{
                                        formatMessage(
                                            !file.exists
                                                ? messages.missing
                                                : !file.recognized
                                                  ? messages.unknown
                                                  : file.rollbackAvailable
                                                    ? messages.valid
                                                    : file.historyId
                                                      ? messages.invalid
                                                      : messages.orphan,
                                        )
                                    }}
                                </p>
                            </div>
                            <span v-if="file.exists" class="shrink-0 text-sm tabular-nums">{{
                                formatBytes(file.integrity.size ?? 0)
                            }}</span>
                        </div>
                    </div>
                </template>
                <p
                    v-else-if="files.length"
                    class="m-0 py-6 text-center text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.noMatches) }}
                </p>
            </template>
        </div>
        <template #actions>
            <div class="flex w-full flex-wrap items-center justify-between gap-3">
                <span class="text-sm text-[var(--color-text-tertiary)]">{{
                    formatMessage(messages.selectedSize, {
                        count: selected.length,
                        size: formatBytes(size),
                    })
                }}</span>
                <div class="ml-auto flex flex-wrap gap-2">
                    <Button :disabled="cleaning" @click="plan ? (plan = null) : modal?.hide()">{{
                        formatMessage(plan ? messages.back : messages.close)
                    }}</Button>
                    <Button
                        v-if="plan"
                        type="colored"
                        color="red"
                        :loading="cleaning"
                        :disabled="busy"
                        @click="cleanup()"
                        ><TrashIcon class="size-4" />{{ formatMessage(messages.confirm) }}</Button
                    >
                    <Button
                        v-else
                        type="colored"
                        color="brand"
                        :disabled="busy || !selected.length"
                        @click="review()"
                        >{{ formatMessage(messages.review) }}</Button
                    >
                </div>
            </div>
        </template>
    </NewModal>
</template>
