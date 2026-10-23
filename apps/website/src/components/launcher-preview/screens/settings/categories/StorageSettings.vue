<script setup lang="ts">
import RefreshCwIcon from '@modrinth/assets/icons/refresh-cw.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import ProgressBar from '@modrinth/ui/src/components/base/ProgressBar.vue'
import { useFormatBytes } from '@modrinth/ui/src/composables/format-bytes.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'

import SettingsSection from '../components/SettingsSection.vue'
import CacheCleanupButton from './storage/CacheCleanupButton.vue'
import MetaStorageCleanup from './storage/MetaStorageCleanup.vue'
import { useStorageBackend } from './storage/storageBackend'
import type {
    MetaCleanupResult,
    StorageNode,
    StorageScanEvent,
    StorageSize,
    StorageTree,
} from './storage/storageData'
import { storageLabels, storageMessages } from './storage/storageMessages'
import StorageTreeNode from './storage/StorageTreeNode.vue'
import StorageUsageChart from './storage/StorageUsageChart.vue'

/**
 * Port of the app's StorageSettings (apps/app-frontend/src/components/ui/
 * settings/StorageSettings.vue) over the preview's simulated launcher disk.
 */
const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()
const router = useRouter()
const { scanStorage, previewCacheUsage, openStoragePaths } = useStorageBackend()

const tree = ref<StorageTree | null>(null)
const loading = ref(true)
const lastUpdated = ref<Date | null>(null)
const scanError = ref(false)
const previewBytes = ref<number | null>(null)
const scanProgress = ref<Extract<StorageScanEvent, { kind: 'progress' }> | null>(null)
const showingCache = ref(false)
let requestId = 0

const categories = computed(() => tree.value?.categories ?? [])

const mainCategories = computed<StorageNode[]>(() => {
    const list = categories.value.filter(
        (category) => category.type !== 'other' && category.size.actual + category.size.symlink > 0,
    )
    const rootOther = tree.value?.rootOther
    if (rootOther && rootOther.size.actual + rootOther.size.symlink > 0) {
        list.push(rootOther)
    }
    return list
})

const overviewSize = computed(() => tree.value?.total.actual ?? 0)
function sizeTotal(size: StorageSize) {
    return size.actual + size.symlink
}

async function loadStorage(force: boolean) {
    const current = ++requestId
    loading.value = true
    scanError.value = false
    scanProgress.value = null
    showingCache.value = false
    try {
        const [snapshot, previews] = await Promise.all([
            scanStorage(force, (event) => {
                if (current !== requestId) return
                if (event.kind === 'cached') {
                    tree.value = event.tree
                    showingCache.value = true
                    lastUpdated.value = event.tree.scannedAt ? new Date(event.tree.scannedAt) : null
                } else scanProgress.value = event
            }),
            previewCacheUsage().catch(() => null),
        ])
        if (current !== requestId) return false
        previewBytes.value = previews
        tree.value = snapshot
        showingCache.value = false
        lastUpdated.value = snapshot.scannedAt ? new Date(snapshot.scannedAt) : new Date()
        return true
    } catch (error) {
        if (current !== requestId) return false
        scanError.value = true
        console.warn('[storage] Scan failed', error)
        return false
    } finally {
        if (current === requestId) loading.value = false
    }
}

function applyCleanup(result: MetaCleanupResult) {
    if (!tree.value) return
    const removedByType = new Map<string, number>()
    for (const [group, removed] of Object.entries(result.removed_groups)) {
        const category = group.split('/')[0].replaceAll('-', '_')
        removedByType.set(category, (removedByType.get(category) ?? 0) + removed.bytes)
    }
    const categories = tree.value.categories.map((category) => {
        if (category.type !== 'meta') return category
        return {
            ...category,
            size: {
                ...category.size,
                actual: Math.max(0, category.size.actual - result.removed_bytes),
            },
            children: category.children?.map((child) => ({
                ...child,
                size: {
                    ...child.size,
                    actual: Math.max(0, child.size.actual - (removedByType.get(child.type) ?? 0)),
                },
            })),
        }
    })
    tree.value = {
        ...tree.value,
        categories,
        total: {
            ...tree.value.total,
            actual: Math.max(0, tree.value.total.actual - result.removed_bytes),
        },
    }
}

async function updateStorage() {
    return loadStorage(true)
}

onMounted(() => void loadStorage(false))
onUnmounted(() => {
    requestId += 1
})

function launcherRouteFor(node: StorageNode, instanceId: string): string | null {
    const encodedId = encodeURIComponent(instanceId)
    switch (node.type) {
        case 'instance':
        case 'mods':
            return `/instance/${encodedId}`
        case 'saves':
        case 'world':
            return `/instance/${encodedId}/worlds`
        case 'screenshots':
            return `/instance/${encodedId}/screenshots`
        default:
            return null
    }
}

async function tryNavigate(path: string): Promise<boolean> {
    try {
        await router.push(path)
        return true
    } catch (error) {
        console.warn('[storage] Launcher navigation failed, falling back to filesystem', error)
        return false
    }
}

async function openNodePaths(node: StorageNode) {
    if (node.paths.length === 0) return
    try {
        const result = await openStoragePaths(node.paths)
        for (const failure of result.failed) {
            console.warn(`[storage] Failed to open path: ${failure.path}`, failure.reason)
        }
    } catch (error) {
        console.warn('[storage] Failed to open storage paths', error)
    }
}

async function handleAction(node: StorageNode) {
    const instanceId = node.instance_id
    const route = instanceId ? launcherRouteFor(node, instanceId) : null

    if (route && (await tryNavigate(route))) return

    await openNodePaths(node)
}

function formatDateTime(date: Date) {
    return new Intl.DateTimeFormat(undefined, {
        dateStyle: 'medium',
        timeStyle: 'short',
    }).format(date)
}
</script>

<template>
    <div
        id="settings-target-storage-overview"
        tabindex="-1"
        class="flex w-full flex-col gap-3 text-[var(--color-text-primary)]"
    >
        <div v-if="loading" class="flex flex-col gap-2" aria-busy="true">
            <div
                class="flex flex-wrap justify-between gap-2 text-sm text-[var(--color-text-tertiary)]"
            >
                <span
                    >{{ formatMessage(storageMessages.scanning)
                    }}<template v-if="scanProgress">
                        · {{ formatMessage(storageLabels[scanProgress.category]) }}</template
                    ></span
                >
                <span v-if="scanProgress" class="tabular-nums">{{
                    formatMessage(storageMessages.scanSections, {
                        completed: scanProgress.completed,
                        total: scanProgress.total,
                    })
                }}</span>
            </div>
            <ProgressBar
                :progress="scanProgress?.completed ?? 0"
                :max="scanProgress?.total ?? 1"
                full-width
                :gradient-border="false"
                :aria-label="formatMessage(storageMessages.scanning)"
            />
            <span v-if="showingCache" class="text-xs text-[var(--color-text-tertiary)]">{{
                formatMessage(storageMessages.cachedScanHelp)
            }}</span>
        </div>
        <div v-if="loading && !tree" class="flex flex-col gap-4" aria-hidden="true">
            <div
                class="flex items-center justify-between gap-4 border-0 border-b border-solid border-surface-4 pb-4"
            >
                <div class="flex flex-col gap-2">
                    <div class="h-4 w-16 rounded bg-surface-3" />
                    <div class="h-8 w-32 rounded bg-surface-3" />
                </div>
                <div class="size-28 shrink-0 rounded-full bg-surface-3" />
            </div>
            <SettingsSection>
                <div
                    class="flex justify-between border-0 border-b border-solid border-surface-4 px-3 py-2 text-xs text-[var(--color-text-tertiary)]"
                >
                    <span>{{ formatMessage(storageMessages.dataType) }}</span>
                    <span>{{ formatMessage(storageMessages.fileSize) }}</span>
                </div>
                <div
                    v-for="row in 6"
                    :key="row"
                    class="flex min-h-10 items-center justify-between px-3 py-2"
                >
                    <div class="h-4 w-24 rounded bg-surface-4" />
                    <div class="h-4 w-16 rounded bg-surface-4" />
                </div>
            </SettingsSection>
        </div>
        <div v-if="scanError" class="flex items-center justify-between gap-3" role="alert">
            <span>{{ formatMessage(storageMessages.scanFailed) }}</span>
            <Button :loading="loading" @click="updateStorage">{{
                formatMessage(storageMessages.retry)
            }}</Button>
        </div>

        <template v-if="tree">
            <!-- 顶部面板：总览 + 更新按钮 + 饼图 -->
            <section
                class="flex flex-wrap items-center justify-between gap-4 border-0 border-b border-solid border-surface-4 pb-4"
            >
                <div class="flex flex-col gap-1">
                    <span class="text-sm text-[var(--color-text-tertiary)]">{{
                        formatMessage(storageMessages.total)
                    }}</span>
                    <strong class="text-3xl font-semibold tabular-nums leading-tight">{{
                        formatBytes(overviewSize)
                    }}</strong>
                    <span
                        v-if="tree.total.symlink > 0"
                        class="text-sm text-[var(--color-text-tertiary)]"
                        >{{
                            formatMessage(storageMessages.linkedSize, {
                                size: formatBytes(tree.total.symlink),
                            })
                        }}</span
                    >
                </div>
                <StorageUsageChart :categories="mainCategories" />
                <div class="flex flex-col items-end gap-2">
                    <Button :disabled="loading" @click="updateStorage"
                        ><RefreshCwIcon />{{
                            formatMessage(
                                loading ? storageMessages.updating : storageMessages.update,
                            )
                        }}</Button
                    >
                    <span v-if="lastUpdated" class="text-xs text-[var(--color-text-tertiary)]">{{
                        formatDateTime(lastUpdated)
                    }}</span>
                </div>
            </section>
            <SettingsSection>
                <div
                    class="flex items-center justify-between border-0 border-b border-solid border-surface-4 px-3 py-2 text-xs font-semibold text-[var(--color-text-tertiary)]"
                >
                    <span>{{ formatMessage(storageMessages.dataType) }}</span>
                    <span>{{ formatMessage(storageMessages.fileSize) }}</span>
                </div>
                <div class="divide-y divide-solid divide-surface-4 [&>*]:border-x-0">
                    <StorageTreeNode
                        v-for="category in mainCategories"
                        :key="category.id"
                        :node="{ ...category, name: undefined }"
                        :depth="0"
                        :parent-total="sizeTotal(tree.total)"
                        @action="handleAction"
                        @open-folder="openNodePaths"
                        ><template v-if="category.type === 'cache'" #actions
                            ><CacheCleanupButton compact @cleared="updateStorage" /></template
                    ></StorageTreeNode>
                    <div
                        v-if="previewBytes !== null"
                        v-tooltip="formatMessage(storageMessages.previewCacheHelp)"
                        class="flex min-h-10 items-center justify-between gap-3 px-3 py-2 text-sm"
                    >
                        <span>{{ formatMessage(storageMessages.previewCache) }}</span>
                        <span class="shrink-0 tabular-nums">{{ formatBytes(previewBytes) }}</span>
                    </div>
                </div>
            </SettingsSection>
            <details class="text-xs text-[var(--color-text-tertiary)]">
                <summary class="cursor-pointer">
                    {{
                        formatMessage(
                            tree.warnings.length
                                ? storageMessages.incompleteScan
                                : storageMessages.measurementTitle,
                        )
                    }}
                </summary>
                <p>{{ formatMessage(storageMessages.measurementHelp) }}</p>
                <ul v-if="tree.warnings.length">
                    <li v-for="warning in tree.warnings" :key="warning" class="break-all">
                        {{ warning }}
                    </li>
                </ul>
            </details>
            <SettingsSection>
                <template #header
                    ><h2 class="m-0 text-lg font-semibold">
                        {{ formatMessage(storageMessages.metaData) }}
                    </h2></template
                >
                <div class="p-4">
                    <MetaStorageCleanup :refresh-storage="updateStorage" @cleaned="applyCleanup" />
                </div>
            </SettingsSection>
            <SettingsSection v-if="tree.external.length" class="mt-6">
                <template #header>{{ formatMessage(storageMessages.externalData) }}</template>
                <div class="flex flex-col gap-3 p-4">
                    <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(storageMessages.externalHelp) }}
                    </p>
                    <StorageTreeNode
                        v-for="node in tree.external"
                        :key="node.id"
                        :node="node"
                        :depth="0"
                        :parent-total="sizeTotal(node.size)"
                        @action="handleAction"
                    />
                </div>
            </SettingsSection>
        </template>
    </div>
</template>
