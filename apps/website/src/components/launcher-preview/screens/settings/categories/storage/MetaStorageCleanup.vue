<script setup lang="ts">
import ChevronRightIcon from '@modrinth/assets/icons/chevron-right.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import { useFormatBytes } from '@modrinth/ui/src/composables/format-bytes.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, nextTick, ref } from 'vue'

import PreviewModal from '../../../../overlays/PreviewModal.vue'
import { useStorageBackend } from './storageBackend'
import type { MetaAnalysis, MetaCleanupResult, MetaGroup, MetaWarning } from './storageData'
import StorageDisclosure from './StorageDisclosure.vue'
import { storageLabels, storageMessages } from './storageMessages'

/**
 * components/ui/settings/storage/MetaStorageCleanup.vue: checks which shared
 * game files (`meta/`) no instance needs, then deletes the selected ones after
 * a confirmation, and refreshes the storage totals.
 */
const props = defineProps<{ refreshStorage: () => Promise<boolean> }>()
const emit = defineEmits<{ cleaned: [result: MetaCleanupResult] }>()
const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()
const { analyzeMeta, cleanMeta } = useStorageBackend()
const analysis = ref<MetaAnalysis | null>(null)
const selected = ref<string[]>([])
const analyzing = ref(false)
const cleaning = ref(false)
const closing = ref(false)
const refreshingTotals = ref(false)
const error = ref<'analyzeFailed' | 'cleanFailed' | 'refreshFailed' | null>(null)
const result = ref<MetaCleanupResult | null>(null)
const errorDetail = ref('')
const modal = ref<InstanceType<typeof PreviewModal> | null>(null)
const review = ref<{ id: string; groups: string[]; size: number; names: string[] } | null>(null)

const categories = computed(() => {
    const groups = new Map<
        string,
        {
            category: string
            total: number
            used: number
            unused: number
            unknown: number
            items: MetaGroup[]
        }
    >()
    for (const item of analysis.value?.groups ?? []) {
        const current = groups.get(item.category) ?? {
            category: item.category,
            total: 0,
            used: 0,
            unused: 0,
            unknown: 0,
            items: [],
        }
        current.used += item.used
        current.unused += item.unused
        current.unknown += item.unknown
        current.total += item.used + item.unused + item.unknown
        current.items.push(item)
        groups.set(item.category, current)
    }
    return [...groups.values()].sort((a, b) => b.unused - a.unused || b.total - a.total)
})
const unusedSize = computed(() =>
    categories.value.reduce((sum, category) => sum + category.unused, 0),
)
const selectedSize = computed(
    () =>
        analysis.value?.groups.reduce(
            (sum, group) => sum + (selected.value.includes(group.id) ? group.unused : 0),
            0,
        ) ?? 0,
)
const warningGroups = computed(() => {
    const groups = new Map<MetaWarning['kind'], Set<string>>()
    for (const warning of analysis.value?.warnings ?? []) {
        const subjects = groups.get(warning.kind) ?? new Set<string>()
        if (warning.subject) subjects.add(warning.subject)
        groups.set(warning.kind, subjects)
    }
    return [...groups].map(([kind, subjects]) => ({ kind, subjects: [...subjects].join(', ') }))
})
const warningMessages = {
    version_unavailable: storageMessages.versionUnavailable,
    processor_unresolved: storageMessages.processorUnresolved,
    database_unavailable: storageMessages.databaseUnavailable,
    active_operation: storageMessages.activeOperation,
    custom_arguments: storageMessages.customArguments,
    external_metadata: storageMessages.externalMetadata,
    unreadable_path: storageMessages.unreadablePath,
}
const descriptions: Record<string, (typeof storageMessages)['metaHelp']> = {
    versions: storageMessages.versionsHelp,
    libraries: storageMessages.librariesHelp,
    assets: storageMessages.assetsHelp,
    natives: storageMessages.nativesHelp,
    java_versions: storageMessages.javaHelp,
    game_locales: storageMessages.localesHelp,
    log_configs: storageMessages.logConfigsHelp,
    resources: storageMessages.legacyHelp,
}

function categoryLabel(category: string) {
    return formatMessage(
        storageLabels[category as keyof typeof storageLabels] ?? storageMessages.other,
    )
}
function groupLabel(group: MetaGroup) {
    return group.name || categoryLabel(group.category)
}
function select(items: MetaGroup[], checked: boolean) {
    const ids = items.filter((item) => item.unused > 0).map((item) => item.id)
    selected.value = checked
        ? [...new Set([...selected.value, ...ids])]
        : selected.value.filter((id) => !ids.includes(id))
}
function categorySelected(items: MetaGroup[]) {
    const unused = items.filter((item) => item.unused > 0)
    return unused.length > 0 && unused.every((item) => selected.value.includes(item.id))
}

async function analyze() {
    if (analyzing.value || cleaning.value) return
    analyzing.value = true
    error.value = null
    try {
        analysis.value = await analyzeMeta()
        selected.value = []
    } catch (cause) {
        error.value = 'analyzeFailed'
        console.warn('[storage] Metadata analysis failed', cause)
    } finally {
        analyzing.value = false
    }
}

function showReview(event: MouseEvent) {
    if (!analysis.value || !selectedSize.value) return
    review.value = {
        id: analysis.value.id,
        groups: [...selected.value],
        size: selectedSize.value,
        names: analysis.value.groups
            .filter((group) => selected.value.includes(group.id))
            .map(groupLabel),
    }
    error.value = null
    errorDetail.value = ''
    modal.value?.show(event)
}

async function refreshTotals() {
    if (error.value === 'refreshFailed') error.value = null
    refreshingTotals.value = true
    try {
        if (!(await props.refreshStorage())) error.value = 'refreshFailed'
    } catch {
        error.value = 'refreshFailed'
    } finally {
        refreshingTotals.value = false
    }
}

async function clean() {
    if (!review.value || cleaning.value || closing.value) return
    cleaning.value = true
    error.value = null
    errorDetail.value = ''
    try {
        const outcome = await cleanMeta(review.value.id, review.value.groups)
        if (analysis.value) {
            analysis.value = {
                ...analysis.value,
                groups: analysis.value.groups
                    .map((group) => {
                        const removed = outcome.removed_groups[group.id]
                        return removed
                            ? {
                                  ...group,
                                  unused: Math.max(0, group.unused - removed.bytes),
                                  files: Math.max(0, group.files - removed.files),
                              }
                            : group
                    })
                    .filter((group) => group.used + group.unused + group.unknown > 0),
            }
        }
        selected.value = []
        emit('cleaned', outcome)
        closing.value = true
        cleaning.value = false
        await nextTick()
        await modal.value?.hide()
        result.value = outcome
        review.value = null
        void refreshTotals()
        void analyze()
    } catch (cause) {
        error.value = 'cleanFailed'
        errorDetail.value = cause instanceof Error ? cause.message : String(cause)
        console.warn('[storage] Metadata cleanup failed', cause)
    } finally {
        cleaning.value = false
        closing.value = false
    }
}
</script>

<template>
    <div class="flex flex-col gap-4">
        <div class="flex flex-wrap items-center justify-between gap-3">
            <div class="flex min-w-0 flex-col gap-1">
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(storageMessages.metaHelp) }}
                </p>
                <span v-if="analysis" class="text-sm font-semibold">{{
                    formatMessage(storageMessages.availableSpace, { size: formatBytes(unusedSize) })
                }}</span>
            </div>
            <Button :loading="analyzing" :disabled="cleaning" @click="analyze">{{
                formatMessage(
                    analyzing ? storageMessages.analyzingMeta : storageMessages.analyzeMeta,
                )
            }}</Button>
        </div>
        <div
            v-if="result"
            role="status"
            class="flex flex-col gap-2 rounded-lg bg-surface-4 p-3 text-sm"
        >
            <span>{{
                formatMessage(storageMessages.cleanResult, {
                    count: result.removed_files,
                    size: formatBytes(result.removed_bytes),
                    skipped: result.skipped_files,
                })
            }}</span>
            <span v-if="refreshingTotals" class="text-[var(--color-text-tertiary)]">{{
                formatMessage(storageMessages.refreshingTotals)
            }}</span>
            <details v-if="result.failures.length">
                <summary>{{ formatMessage(storageMessages.cleanFailures) }}</summary>
                <ul>
                    <li v-for="failure in result.failures" :key="failure" class="break-all">
                        {{ failure }}
                    </li>
                </ul>
            </details>
        </div>
        <div
            v-if="error && error !== 'cleanFailed'"
            class="flex flex-wrap items-center gap-3 text-sm"
            role="alert"
        >
            <span>{{ formatMessage(storageMessages[error]) }}</span>
            <Button
                v-if="error === 'refreshFailed'"
                :loading="refreshingTotals"
                @click="refreshTotals"
                >{{ formatMessage(storageMessages.retry) }}</Button
            >
        </div>
        <template v-if="analysis">
            <div
                class="overflow-hidden rounded-lg border border-solid border-surface-4"
                :aria-busy="analyzing || undefined"
            >
                <div
                    class="grid grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-4 bg-surface-4 px-3 py-2 text-xs font-semibold text-[var(--color-text-tertiary)]"
                >
                    <span>{{ formatMessage(storageMessages.dataType) }}</span>
                    <span class="w-20 text-right">{{
                        formatMessage(storageMessages.fileSize)
                    }}</span>
                    <span class="w-20 text-right">{{ formatMessage(storageMessages.unused) }}</span>
                </div>
                <div
                    v-for="category in categories"
                    :key="category.category"
                    class="border-0 border-t border-solid border-surface-4"
                >
                    <StorageDisclosure :expandable="category.items.length > 1">
                        <template #summary="{ open }">
                            <div
                                class="flex list-none items-center gap-3 px-3 py-3 [&::-webkit-details-marker]:hidden"
                                :class="
                                    category.items.length > 1
                                        ? 'cursor-pointer hover:bg-surface-4'
                                        : ''
                                "
                            >
                                <Checkbox
                                    v-if="category.unused > 0"
                                    :model-value="categorySelected(category.items)"
                                    :disabled="analyzing || cleaning"
                                    :description="
                                        formatMessage(storageMessages.selectGroup, {
                                            name: categoryLabel(category.category),
                                        })
                                    "
                                    @click.stop
                                    @update:model-value="select(category.items, $event)"
                                />
                                <span v-else class="w-5 shrink-0" />
                                <div class="min-w-0 flex-1">
                                    <span class="text-sm font-semibold">{{
                                        categoryLabel(category.category)
                                    }}</span>
                                    <ChevronRightIcon
                                        v-if="category.items.length > 1"
                                        class="ml-1 inline size-3 text-[var(--color-text-tertiary)] transition-transform duration-150 motion-reduce:transition-none"
                                        :class="open ? 'rotate-90' : ''"
                                    />
                                    <span
                                        v-if="category.items.length > 1"
                                        class="ml-2 text-xs text-[var(--color-text-tertiary)]"
                                        >{{
                                            formatMessage(storageMessages.metaItemCount, {
                                                count: category.items.length,
                                            })
                                        }}</span
                                    >
                                    <p
                                        v-if="descriptions[category.category]"
                                        class="m-0 mt-1 text-xs leading-relaxed text-[var(--color-text-tertiary)]"
                                    >
                                        {{ formatMessage(descriptions[category.category]) }}
                                    </p>
                                    <p
                                        v-if="category.unknown > 0"
                                        class="m-0 mt-1 text-xs text-[var(--color-text-tertiary)]"
                                    >
                                        {{
                                            formatMessage(storageMessages.retainedAmount, {
                                                size: formatBytes(category.unknown),
                                            })
                                        }}
                                    </p>
                                </div>
                                <span class="w-20 shrink-0 text-right text-sm tabular-nums">{{
                                    formatBytes(category.total)
                                }}</span>
                                <span
                                    class="w-20 shrink-0 text-right text-sm tabular-nums"
                                    :class="
                                        category.unused > 0
                                            ? 'font-semibold text-brand'
                                            : 'text-[var(--color-text-tertiary)]'
                                    "
                                    >{{
                                        category.unused > 0 ? formatBytes(category.unused) : '—'
                                    }}</span
                                >
                            </div>
                        </template>
                        <div
                            v-if="category.items.length > 1"
                            class="border-0 border-t border-solid border-surface-4 bg-surface-2"
                        >
                            <div
                                v-for="item in category.items"
                                :key="item.id"
                                class="flex items-center gap-3 px-3 py-2 pl-6 text-sm"
                            >
                                <Checkbox
                                    v-if="item.unused > 0"
                                    :model-value="selected.includes(item.id)"
                                    :disabled="analyzing || cleaning"
                                    :description="
                                        formatMessage(storageMessages.selectGroup, {
                                            name: groupLabel(item),
                                        })
                                    "
                                    @update:model-value="select([item], $event)"
                                />
                                <span v-else class="w-5 shrink-0" />
                                <span class="min-w-0 flex-1 break-words">{{
                                    groupLabel(item)
                                }}</span>
                                <span class="w-20 shrink-0 text-right tabular-nums">{{
                                    formatBytes(item.used + item.unused + item.unknown)
                                }}</span>
                                <span
                                    class="w-20 shrink-0 text-right tabular-nums text-[var(--color-text-tertiary)]"
                                    >{{
                                        item.unused > 0
                                            ? formatBytes(item.unused)
                                            : formatMessage(
                                                  item.used > 0
                                                      ? storageMessages.used
                                                      : storageMessages.unknown,
                                              )
                                    }}</span
                                >
                            </div>
                        </div>
                    </StorageDisclosure>
                </div>
            </div>
            <div v-if="selectedSize > 0" class="flex flex-wrap items-center justify-between gap-3">
                <span class="text-sm">{{
                    formatMessage(storageMessages.selectedSize, {
                        count: selected.length,
                        size: formatBytes(selectedSize),
                    })
                }}</span>
                <Button
                    type="colored"
                    color="red"
                    :disabled="analyzing || cleaning"
                    @click="showReview"
                    >{{ formatMessage(storageMessages.cleanSelected) }}</Button
                >
            </div>
            <p v-else-if="!unusedSize" class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(storageMessages.noUnused) }}
            </p>
            <details v-if="warningGroups.length" class="text-sm text-[var(--color-text-tertiary)]">
                <summary class="cursor-pointer">
                    {{ formatMessage(storageMessages.metaWarnings) }}
                </summary>
                <ul class="mb-0 flex flex-col gap-2 pl-5">
                    <li v-for="warning in warningGroups" :key="warning.kind">
                        {{
                            formatMessage(warningMessages[warning.kind], {
                                names: warning.subjects,
                            })
                        }}
                    </li>
                </ul>
            </details>
        </template>
    </div>
    <PreviewModal
        ref="modal"
        :header="formatMessage(storageMessages.cleanTitle)"
        :disable-close="cleaning"
        width="min(36rem, 1152px)"
        fade="danger"
    >
        <div class="flex flex-col gap-3">
            <p class="m-0">
                {{
                    formatMessage(storageMessages.cleanConfirm, {
                        size: formatBytes(review?.size ?? 0),
                    })
                }}
            </p>
            <ul class="m-0 max-h-40 overflow-y-auto pl-5 text-sm text-[var(--color-text-tertiary)]">
                <li v-for="name in review?.names" :key="name">{{ name }}</li>
            </ul>
            <p v-if="cleaning" role="status" class="m-0 text-sm">
                {{ formatMessage(storageMessages.cleaning) }}
            </p>
            <div
                v-if="error === 'cleanFailed'"
                role="alert"
                class="flex flex-col gap-2 text-sm text-red"
            >
                <span>{{ formatMessage(storageMessages.cleanFailed) }}</span
                ><span v-if="errorDetail" class="break-words">{{ errorDetail }}</span>
            </div>
        </div>
        <template #actions>
            <div class="flex flex-wrap justify-end gap-3">
                <Button :disabled="cleaning || closing" @click="modal?.hide()">{{
                    formatMessage(storageMessages.cancel)
                }}</Button>
                <Button
                    type="colored"
                    color="red"
                    :loading="cleaning"
                    :disabled="closing"
                    @click="clean"
                    >{{
                        formatMessage(
                            cleaning ? storageMessages.cleaning : storageMessages.cleanSelected,
                        )
                    }}</Button
                >
            </div>
        </template>
    </PreviewModal>
</template>
