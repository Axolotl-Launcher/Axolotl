<script setup lang="ts">
import { FileIcon, RefreshCwIcon, SearchIcon, TrashIcon } from '@modrinth/assets'
import {
    Admonition,
    Button,
    Checkbox,
    Combobox,
    defineMessages,
    NewModal,
    StyledInput,
    useFormatBytes,
    useVIntl,
} from '@modrinth/ui'
import { computed, onBeforeUnmount, ref } from 'vue'

import { resourceOperationError } from '@/helpers/resource-operation-logic'
import {
    type ResourceBackupFile,
    resourceExecuteHistoryCleanup,
    type ResourceHistoryCleanupPlan,
    resourcePrepareHistoryCleanup,
    resourceScanBackups,
} from '@/helpers/resources'

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
        defaultMessage: 'Select eligible items',
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
    reviewMissing: {
        id: 'app.resources.backups.review-missing',
        defaultMessage:
            '{count, plural, one {Clear # missing backup} other {Clear # missing backups}} from this list. No files are deleted for these entries, and update history is kept.',
    },
    reviewChanged: {
        id: 'app.resources.backups.review-changed',
        defaultMessage: 'A selected backup changed. Scan again before reviewing cleanup.',
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
    confirmMissing: {
        id: 'app.resources.backups.confirm-missing',
        defaultMessage: 'Clear missing backups',
    },
    back: { id: 'app.resources.backups.back', defaultMessage: 'Back to selection' },
    close: { id: 'app.resources.backups.close', defaultMessage: 'Close' },
    removed: {
        id: 'app.resources.backups.removed',
        defaultMessage: '{count, number} files deleted.',
    },
    removedMissing: {
        id: 'app.resources.backups.removed-missing',
        defaultMessage:
            '{count, plural, one {Cleared # missing backup.} other {Cleared # missing backups.}}',
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
const resultText = ref<string[]>([])
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
const selectable = computed(() => visibleFiles.value.filter(canSelect))
const reviewedExisting = computed(() => plan.value?.files.filter((file) => file.exists) ?? [])
const reviewedMissingCount = computed(
    () => (plan.value?.files.length ?? 0) - reviewedExisting.value.length,
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
function canSelect(file: ResourceBackupFile) {
    return file.recognized && (file.exists || !!file.historyId)
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
    resultText.value = []
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
        const previous = new Map(files.value.map((file) => [file.id, file.exists]))
        selected.value = selected.value.filter((id) =>
            value.some(
                (file) => file.id === id && canSelect(file) && previous.get(id) === file.exists,
            ),
        )
        files.value = value
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
        if (current !== generation) return
        const previous = new Map(files.value.map((file) => [file.id, file.exists]))
        if (value.files.some((file) => previous.get(file.id) !== file.exists)) {
            error.value = formatMessage(messages.reviewChanged)
            return
        }
        plan.value = value
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
        const removed = new Set(result.removed)
        const completed = reviewed.files.filter((file) => removed.has(file.id))
        const deletedFiles = completed.filter((file) => file.exists).length
        const clearedMissing = completed.length - deletedFiles
        resultText.value = []
        if (deletedFiles)
            resultText.value.push(formatMessage(messages.removed, { count: deletedFiles }))
        if (clearedMissing)
            resultText.value.push(formatMessage(messages.removedMissing, { count: clearedMissing }))
        if (result.failures.length)
            resultText.value.push(formatMessage(messages.failed, { count: result.failures.length }))
        if (result.failures.length)
            error.value = result.failures
                .map(([, failure]) => resourceOperationError(failure, formatMessage))
                .join('\n')
        const affected = new Set([...selected.value, ...result.removed])
        files.value = files.value.filter((file) => !affected.has(file.id))
        selected.value = []
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
        width="min(48rem, 90vw)"
    >
        <div class="flex min-w-0 flex-col gap-4">
            <p v-if="!plan" class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.description) }}
            </p>
            <p v-if="error" class="m-0 whitespace-pre-wrap text-red" role="alert">{{ error }}</p>
            <p v-for="text in resultText" :key="text" class="m-0 text-brand" role="status">
                {{ text }}
            </p>
            <template v-if="plan">
                <Admonition
                    v-if="reviewedExisting.length"
                    type="warning"
                    :header="
                        formatMessage(messages.reviewSize, {
                            count: reviewedExisting.length,
                            size: formatBytes(
                                reviewedExisting.reduce(
                                    (total, file) => total + (file.integrity.size ?? 0),
                                    0,
                                ),
                            ),
                        })
                    "
                >
                    {{
                        formatMessage(messages.warning, {
                            count: reviewedExisting.filter((file) => file.rollbackAvailable).length,
                        })
                    }}
                </Admonition>
                <p
                    v-if="reviewedMissingCount"
                    class="m-0 text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.reviewMissing, { count: reviewedMissingCount }) }}
                </p>
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
                            file.exists
                                ? formatBytes(file.integrity.size ?? 0)
                                : formatMessage(messages.missing)
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
                                :disabled="busy || !canSelect(file)"
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
                        ><TrashIcon v-if="reviewedExisting.length" class="size-4" />{{
                            formatMessage(
                                !reviewedMissingCount
                                    ? messages.confirm
                                    : reviewedExisting.length
                                      ? messages.reviewTitle
                                      : messages.confirmMissing,
                            )
                        }}</Button
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
