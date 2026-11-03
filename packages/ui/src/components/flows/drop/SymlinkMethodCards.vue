<template>
    <NewModal
        ref="modal"
        max-width="920px"
        :closable="true"
        :close-on-click-outside="true"
        :on-hide="onModalHide"
        hide-header
        scrollable
    >
        <div class="flex flex-col gap-4 p-6">
            <div class="flex flex-col gap-1">
                <span class="text-lg font-semibold text-[var(--color-text-primary)]">{{
                    formatMessage(messages.title)
                }}</span>
                <span class="text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.subtitle, { n: instances.length }) }}
                </span>
            </div>

            <HorizontalRule />

            <div
                class="relative grid grid-cols-1 gap-5 min-[640px]:grid-cols-[minmax(0,2fr)_minmax(0,3fr)] min-[640px]:gap-8"
            >
                <div
                    class="pointer-events-none absolute inset-y-0 left-[calc(40%+0.2rem)] z-0 hidden w-px bg-divider min-[640px]:block"
                    aria-hidden="true"
                />

                <div class="relative z-[1] flex min-w-0 flex-col gap-4">
                    <div class="flex flex-col gap-2">
                        <div class="flex items-center gap-2">
                            <span class="text-sm font-semibold text-[var(--color-text-primary)]">
                                {{ formatMessage(messages.statsLabel) }}
                            </span>
                        </div>
                        <ProgressBar v-if="statsLoading" :progress="0" waiting full-width />
                        <div class="grid grid-cols-1 gap-2 sm:grid-cols-2">
                            <div
                                v-for="row in statRows"
                                :key="row.key"
                                class="flex items-center justify-between gap-2 rounded-lg bg-surface-2 px-3 py-2"
                            >
                                <span
                                    v-tooltip="row.tooltip"
                                    class="shrink-0 whitespace-nowrap text-sm text-[var(--color-text-tertiary)]"
                                >
                                    {{ row.label }}
                                </span>
                                <div class="flex flex-col items-end gap-0.5 whitespace-nowrap">
                                    <span
                                        class="text-sm font-semibold text-[var(--color-text-primary)]"
                                        >{{ row.size }}</span
                                    >
                                    <span class="text-xs text-[var(--color-text-tertiary)]">{{
                                        row.files
                                    }}</span>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>

                <div
                    class="relative z-[1] flex min-w-0 flex-col gap-4"
                    :class="pageAnim ? 'overflow-hidden' : ''"
                >
                    <div
                        class="flex min-w-0 flex-col gap-4"
                        :class="[pageAnimationClass, pageAnim ? 'overflow-hidden' : '']"
                    >
                        <div class="flex flex-col gap-2">
                            <div class="relative flex items-center gap-2">
                                <span
                                    class="text-sm font-semibold text-[var(--color-text-primary)]"
                                >
                                    {{ formatMessage(messages.loaderLabel) }}
                                </span>
                                <TagItem
                                    v-if="
                                        warnings.loaderTypeCustom || warnings.loaderTypeUnrecognized
                                    "
                                    class="shrink-0 border !border-solid border-orange"
                                    :style="{
                                        '--_bg-color': 'var(--color-orange-bg)',
                                        '--_color': 'var(--color-orange)',
                                    }"
                                >
                                    <span
                                        v-tooltip="
                                            warnings.loaderTypeCustom
                                                ? formatMessage(messages.customTooltip)
                                                : formatMessage(messages.loaderTypeUnknownTooltip)
                                        "
                                        class="inline-flex items-center gap-1"
                                    >
                                        <CircleAlertIcon />
                                        {{
                                            warnings.loaderTypeCustom
                                                ? formatMessage(messages.custom)
                                                : formatMessage(messages.unrecognized)
                                        }}
                                    </span>
                                </TagItem>
                            </div>
                            <Chips
                                v-model="loader"
                                :items="loaderItems"
                                :format-label="formatLoader"
                                size="small"
                            />
                        </div>

                        <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
                            <div class="flex flex-col gap-2">
                                <div class="relative flex items-center gap-2">
                                    <span
                                        class="text-sm font-semibold text-[var(--color-text-primary)]"
                                    >
                                        {{ formatMessage(messages.gameVersionLabel) }}
                                    </span>
                                    <TagItem
                                        v-if="warnings.gameVersionCustom"
                                        class="absolute right-0 top-1/2 -translate-y-1/2 border !border-solid border-orange"
                                        :style="{
                                            '--_bg-color': 'var(--color-orange-bg)',
                                            '--_color': 'var(--color-orange)',
                                        }"
                                    >
                                        <span
                                            v-tooltip="formatMessage(messages.customTooltip)"
                                            class="inline-flex items-center gap-1"
                                        >
                                            <CircleAlertIcon />
                                            {{ formatMessage(messages.custom) }}
                                        </span>
                                    </TagItem>
                                </div>
                                <Combobox
                                    v-model="gameVersion"
                                    :options="gameVersionOptions"
                                    :placeholder="
                                        statsLoading
                                            ? formatMessage(messages.detecting)
                                            : formatMessage(messages.selectGameVersion)
                                    "
                                    :search-placeholder="formatMessage(messages.searchGameVersion)"
                                    searchable
                                    @search-input="handleGameVersionSearch"
                                />
                            </div>

                            <div class="flex flex-col gap-2">
                                <div class="relative flex items-center gap-2">
                                    <span
                                        class="text-sm font-semibold"
                                        :class="
                                            loader === 'vanilla'
                                                ? 'text-[var(--color-text-tertiary)]'
                                                : 'text-[var(--color-text-primary)]'
                                        "
                                    >
                                        {{ formatMessage(messages.loaderVersionLabel) }}
                                    </span>
                                    <TagItem
                                        v-if="
                                            warnings.loaderVersionCustom ||
                                            warnings.loaderVersionMissing
                                        "
                                        class="absolute right-0 top-1/2 -translate-y-1/2 border !border-solid border-orange"
                                        :style="{
                                            '--_bg-color': 'var(--color-orange-bg)',
                                            '--_color': 'var(--color-orange)',
                                        }"
                                    >
                                        <span
                                            v-tooltip="
                                                warnings.loaderVersionCustom
                                                    ? formatMessage(messages.customTooltip)
                                                    : formatMessage(
                                                          messages.loaderVersionUnknownTooltip,
                                                      )
                                            "
                                            class="inline-flex items-center gap-1"
                                        >
                                            <CircleAlertIcon />
                                            {{
                                                warnings.loaderVersionCustom
                                                    ? formatMessage(messages.custom)
                                                    : formatMessage(messages.missing)
                                            }}
                                        </span>
                                    </TagItem>
                                </div>
                                <Combobox
                                    v-model="loaderVersion"
                                    :options="loaderVersionOptions"
                                    :disabled="loader === 'vanilla'"
                                    :placeholder="formatMessage(messages.selectLoaderVersion)"
                                    :search-placeholder="
                                        formatMessage(messages.searchLoaderVersion)
                                    "
                                    searchable
                                    @search-input="handleLoaderVersionSearch"
                                />
                            </div>
                        </div>

                        <div class="flex flex-col gap-3">
                            <div class="flex flex-col gap-2">
                                <span
                                    v-tooltip="formatMessage(messages.importPathTooltip)"
                                    class="text-sm font-semibold text-[var(--color-text-primary)]"
                                >
                                    {{ formatMessage(messages.importPathLabel) }}
                                </span>
                                <StyledInput
                                    :model-value="
                                        activeInstance?.path || formatMessage(messages.missingPath)
                                    "
                                    input-class="!pr-9"
                                    wrapper-class="w-full"
                                    readonly
                                >
                                    <template #right>
                                        <span
                                            class="pointer-events-none absolute inset-y-0 right-0 flex items-center pr-1.5"
                                        >
                                            <button
                                                type="button"
                                                class="pointer-events-auto flex h-6 w-6 items-center justify-center rounded-md text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)] disabled:opacity-50"
                                                :disabled="!activeInstance?.path"
                                                @click="openVersionPath"
                                            >
                                                <FolderOpenIcon class="size-4" />
                                            </button>
                                        </span>
                                    </template>
                                </StyledInput>
                            </div>

                            <div class="flex flex-col gap-2">
                                <span
                                    v-tooltip="formatMessage(messages.runtimeRootTooltip)"
                                    class="text-sm font-semibold text-[var(--color-text-primary)]"
                                >
                                    {{ formatMessage(messages.runtimeRootLabel) }}
                                </span>
                                <StyledInput
                                    :model-value="
                                        activeSnapshot?.minecraftRoot ||
                                        (statsLoading ? formatMessage(messages.detecting) : '')
                                    "
                                    input-class="!pr-9"
                                    wrapper-class="w-full"
                                    readonly
                                >
                                    <template #right>
                                        <span
                                            class="pointer-events-none absolute inset-y-0 right-0 flex items-center pr-1.5"
                                        >
                                            <button
                                                type="button"
                                                class="pointer-events-auto flex h-6 w-6 items-center justify-center rounded-md text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)] disabled:opacity-50"
                                                :disabled="!activeSnapshot?.minecraftRoot"
                                                @click="openRootInFileManager"
                                            >
                                                <FolderOpenIcon class="size-4" />
                                            </button>
                                        </span>
                                    </template>
                                </StyledInput>
                            </div>
                        </div>

                        <div v-if="planError" class="text-xs text-danger">{{ planError }}</div>
                    </div>

                    <nav
                        v-if="instances.length > 1"
                        class="mt-auto flex justify-end"
                        :aria-label="formatMessage(messages.instance)"
                    >
                        <div
                            ref="instanceNavTrackRef"
                            class="instance-selector-track flex max-w-full items-center gap-2 overflow-x-auto"
                        >
                            <button
                                v-for="(instance, index) in instances"
                                :key="`${instance.name}-${instance.path ?? index}`"
                                type="button"
                                class="shrink-0 rounded-full transition-all duration-150"
                                :class="
                                    index === activeIndex
                                        ? `h-2 w-8 ${instanceWarnings[index] ? 'bg-orange' : 'bg-brand'}`
                                        : `size-2 ${
                                              instanceWarnings[index]
                                                  ? 'bg-orange'
                                                  : 'bg-[var(--color-text-tertiary)] hover:bg-surface-3'
                                          }`
                                "
                                :aria-pressed="index === activeIndex"
                                :aria-label="
                                    formatMessage(messages.instanceNavLabel, {
                                        name: instance.name,
                                    })
                                "
                                :data-active="index === activeIndex"
                                @click="goTo(index)"
                                @focus="scrollFocusedNavIntoView"
                            />
                        </div>
                    </nav>
                </div>
            </div>
        </div>

        <template #actions>
            <div class="flex w-full items-center justify-between p-4 pt-0">
                <div class="flex items-center gap-2">
                    <Button type="quiet" @click="handleCancel"
                        >{{ formatMessage(messages.cancel) }}
                    </Button>
                    <Button type="quiet" :disabled="!canReset" @click="resetChanges"
                        >{{ formatMessage(messages.resetChanges) }}
                    </Button>
                </div>
                <div class="flex items-center gap-2">
                    <TagItem
                        v-if="warnings.hasWarnings"
                        class="border !border-solid border-orange"
                        :style="{
                            '--_bg-color': 'var(--color-orange-bg)',
                            '--_color': 'var(--color-orange)',
                        }"
                    >
                        <CircleAlertIcon />
                        {{ formatMessage(messages.custom) }}
                    </TagItem>
                    <Button
                        class="flex items-center gap-2"
                        :disabled="!canConfirm"
                        @click="handleConfirm"
                        >{{ confirmLabel }}
                    </Button>
                </div>
            </div>
        </template>
    </NewModal>
</template>

<script setup lang="ts">
import { CircleAlertIcon, FolderOpenIcon } from '@modrinth/assets'
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'

import Button from '#ui/components/base/buttons/Button.vue'
import Chips from '#ui/components/base/Chips.vue'
import Combobox, { type ComboboxOption } from '#ui/components/base/Combobox.vue'
import HorizontalRule from '#ui/components/base/HorizontalRule.vue'
import ProgressBar from '#ui/components/base/ProgressBar.vue'
import StyledInput from '#ui/components/base/StyledInput.vue'
import TagItem from '#ui/components/base/TagItem.vue'
import NewModal from '#ui/components/modal/NewModal.vue'
import { useFormatBytes } from '#ui/composables/format-bytes.ts'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import {
    type ImportPlanCounts,
    importPlanDefaultGameVersion,
    importPlanDefaultLoader,
    importPlanDefaultLoaderVersion,
    type ImportPlanSnapshot,
    importPlanWarnings,
    injectInstanceImport,
    injectTags,
    reduceImportPlanSnapshot,
    type SymlinkMethodChoice,
    type SymlinkMethodInstance,
} from '#ui/providers'
import { formatLoaderLabel } from '#ui/utils/loaders'

const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()
const instanceImport = injectInstanceImport()
const tags = injectTags()

const messages = defineMessages({
    title: {
        id: 'drop.instance-import.title',
        defaultMessage: 'Import instances',
    },
    subtitle: {
        id: 'drop.symlink_method.subtitle',
        defaultMessage: 'Importing {n} instance(s)',
    },
    instance: {
        id: 'drop.symlink_method.instance',
        defaultMessage: 'Instance',
    },
    selectInstance: {
        id: 'drop.symlink_method.select_instance',
        defaultMessage: 'Select instance',
    },
    gameVersionLabel: {
        id: 'drop.symlink_method.game_version',
        defaultMessage: 'Game version',
    },
    selectGameVersion: {
        id: 'drop.symlink_method.select_game_version',
        defaultMessage: 'Select game version',
    },
    searchGameVersion: {
        id: 'drop.symlink_method.search_game_version',
        defaultMessage: 'Search game versions',
    },
    loaderLabel: {
        id: 'drop.symlink_method.loader',
        defaultMessage: 'Loader',
    },
    loaderVersionLabel: {
        id: 'drop.symlink_method.loader_version',
        defaultMessage: 'Loader version',
    },
    latest: {
        id: 'drop.symlink_method.latest',
        defaultMessage: 'Latest',
    },
    selectLoaderVersion: {
        id: 'drop.symlink_method.select_loader_version',
        defaultMessage: 'Select loader version',
    },
    searchLoaderVersion: {
        id: 'drop.symlink_method.search_loader_version',
        defaultMessage: 'Search loader versions',
    },
    detecting: {
        id: 'drop.symlink_method.detecting',
        defaultMessage: 'Detecting...',
    },
    importPathLabel: {
        id: 'drop.symlink_method.import_path',
        defaultMessage: 'Version path',
    },
    importPathTooltip: {
        id: 'drop.symlink_method.import_path_tooltip',
        defaultMessage: 'Import path',
    },
    runtimeRootLabel: {
        id: 'drop.symlink_method.runtime_root',
        defaultMessage: 'Root',
    },
    runtimeRootTooltip: {
        id: 'drop.symlink_method.runtime_root_tooltip',
        defaultMessage: 'Runtime root',
    },
    statsLabel: {
        id: 'drop.symlink_method.stats',
        defaultMessage: 'Estimate',
    },
    statsCache: {
        id: 'drop.symlink_method.stats_cache',
        defaultMessage: 'Cache',
    },
    statsCacheTooltip: {
        id: 'drop.symlink_method.stats_cache_tooltip',
        defaultMessage: 'Axolotl cache',
    },
    statsLocal: {
        id: 'drop.symlink_method.stats_local',
        defaultMessage: 'Reuse',
    },
    statsLocalTooltip: {
        id: 'drop.symlink_method.stats_local_tooltip',
        defaultMessage: 'Local reuse',
    },
    statsNetwork: {
        id: 'drop.symlink_method.stats_network',
        defaultMessage: 'Download',
    },
    statsNetworkTooltip: {
        id: 'drop.symlink_method.stats_network_tooltip',
        defaultMessage: 'Needs download',
    },
    statsMigrate: {
        id: 'drop.symlink_method.stats_migrate',
        defaultMessage: 'Migrate',
    },
    statsMigrateTooltip: {
        id: 'drop.symlink_method.stats_migrate_tooltip',
        defaultMessage: 'Needs migration',
    },
    statsFiles: {
        id: 'drop.symlink_method.stats_files',
        defaultMessage: '{files} files',
    },
    notAvailable: {
        id: 'drop.symlink_method.not_available',
        defaultMessage: 'Unavailable',
    },
    loaderVersionUnknownTooltip: {
        id: 'drop.symlink_method.loader_version_unknown_tooltip',
        defaultMessage: 'Loader version not detected. A custom version will be used.',
    },
    loaderTypeUnknownTooltip: {
        id: 'drop.symlink_method.loader_type_unknown_tooltip',
        defaultMessage: 'Loader not recognized. Mods may not load correctly.',
    },
    missing: {
        id: 'drop.symlink_method.missing',
        defaultMessage: 'Missing',
    },
    unrecognized: {
        id: 'drop.symlink_method.unrecognized',
        defaultMessage: 'Unrecognized',
    },
    custom: {
        id: 'drop.symlink_method.custom',
        defaultMessage: 'Custom',
    },
    customTooltip: {
        id: 'drop.symlink_method.custom_tooltip',
        defaultMessage:
            'Usually you should not customize it, unless you are sure it was detected incorrectly.',
    },
    missingPath: {
        id: 'drop.symlink_method.missing_path',
        defaultMessage: 'No import path available',
    },
    planFailed: {
        id: 'drop.symlink_method.plan_failed',
        defaultMessage: 'Import estimate failed',
    },
    cancel: {
        id: 'drop.symlink_method.cancel',
        defaultMessage: 'Cancel',
    },
    resetChanges: {
        id: 'drop.symlink_method.reset_changes',
        defaultMessage: 'Reset changes',
    },
    confirm: {
        id: 'drop.symlink_method.confirm',
        defaultMessage: 'Confirm',
    },
    next: {
        id: 'drop.symlink_method.next',
        defaultMessage: 'Next',
    },
    instanceNavLabel: {
        id: 'drop.symlink_method.instance_nav_label',
        defaultMessage: 'Switch to {name}',
    },
})

const emit = defineEmits<{
    (e: 'confirm', choices: SymlinkMethodChoice[]): void
    (e: 'cancel'): void
}>()

interface InstanceChoice {
    gameVersion: string
    loader: string
    loaderVersion: string
    touched: {
        gameVersion: boolean
        loader: boolean
        loaderVersion: boolean
    }
}

const modal = ref<InstanceType<typeof NewModal> | null>(null)
const isOpen = ref(false)
const instances = ref<SymlinkMethodInstance[]>([])
const activeIndex = ref(0)
const gameVersion = ref('')
const loader = ref('vanilla')
const loaderVersion = ref('latest')
const loaderVersions = ref<string[]>([])
let loaderVersionRequest = 0
const touched = ref({
    gameVersion: false,
    loader: false,
    loaderVersion: false,
})
const internalUpdating = ref(false)
const snapshots = ref<Record<number, ImportPlanSnapshot | null>>({})
const detectedByInstance = ref<
    Record<number, { gameVersion: string; loader: string; loaderVersion: string; modCount: number }>
>({})
const requestIds = ref<Record<number, string | null>>({})
const scanning = ref<Record<number, boolean>>({})
const planErrors = ref<Record<number, string | null>>({})
const pageAnim = ref<'out-next' | 'out-prev' | 'in-next' | 'in-prev' | null>(null)
const instanceNavTrackRef = ref<HTMLElement | null>(null)
const instanceChoices = ref<Record<number, InstanceChoice>>({})
let unlisten: (() => void) | null = null
let listeningPromise: Promise<void> | null = null
let listenerGeneration = 0
let viewGeneration = 0
const rescanTimers = new Map<number, number>()

const activeInstance = computed(() => instances.value[activeIndex.value])
const activeSnapshot = computed(() => snapshots.value[activeIndex.value] ?? null)
const statsLoading = computed(() => Object.values(scanning.value).some(Boolean))
const planError = computed(() => planErrors.value[activeIndex.value] ?? null)
const pageAnimationClass = computed(() => (pageAnim.value ? `page-${pageAnim.value}` : ''))
const loaderItems = ['vanilla', 'fabric', 'forge', 'neoforge', 'quilt', 'cleanroom']
const formatLoader = (item: string) => formatLoaderLabel(item, formatMessage)
const loaderApiName = computed(() => (loader.value === 'neoforge' ? 'neo' : loader.value))

const gameVersionOptions = computed<ComboboxOption<string>[]>(() => {
    const versions = new Set(tags.gameVersions.value.map((version) => version.version))
    if (gameVersion.value) versions.add(gameVersion.value)
    return [...versions].map((version) => ({ value: version, label: version }))
})

const loaderVersionOptions = computed<ComboboxOption<string>[]>(() => {
    const versions = new Set(['latest'])
    if (loaderVersion.value && loaderVersion.value !== 'latest') {
        versions.add(loaderVersion.value)
    }
    for (const version of loaderVersions.value) {
        versions.add(version)
    }
    return [...versions].map((version) => ({
        value: version,
        label: version === 'latest' ? formatMessage(messages.latest) : version,
    }))
})

async function loadLoaderVersions() {
    const apiLoader = loaderApiName.value
    const version = gameVersion.value
    if (loader.value === 'vanilla' || !apiLoader || !version) {
        loaderVersions.value = []
        return
    }

    const request = ++loaderVersionRequest
    try {
        const versions = await instanceImport.getLoaderVersions(apiLoader, version)
        console.debug(
            '[SymlinkMethodCards] loader versions loaded',
            apiLoader,
            version,
            versions.length,
        )
        if (request === loaderVersionRequest) {
            loaderVersions.value = versions
        }
    } catch (error) {
        console.error('[SymlinkMethodCards] load loader versions failed', error)
        if (request === loaderVersionRequest) {
            loaderVersions.value = []
        }
    }
}

const warnings = computed(() =>
    importPlanWarnings(activeSnapshot.value, {
        gameVersion: gameVersion.value,
        loader: loader.value,
        loaderVersion: loaderVersion.value,
        detectedGameVersion: detectedByInstance.value[activeIndex.value]?.gameVersion,
        detectedLoader: detectedByInstance.value[activeIndex.value]?.loader,
        detectedLoaderVersion: detectedByInstance.value[activeIndex.value]?.loaderVersion,
        detectedModCount: detectedByInstance.value[activeIndex.value]?.modCount,
        gameVersionTouched: touched.value.gameVersion,
        loaderTouched: touched.value.loader,
        loaderVersionTouched: touched.value.loaderVersion,
    }),
)

const instanceWarnings = computed(() => {
    const result: Record<number, boolean> = {}
    for (let index = 0; index < instances.value.length; index++) {
        if (index === activeIndex.value) {
            result[index] = warnings.value.hasWarnings
            continue
        }
        const snapshot = snapshots.value[index] ?? null
        const saved = instanceChoices.value[index]
        const detected = detectedByInstance.value[index]
        result[index] = importPlanWarnings(snapshot, {
            gameVersion:
                saved?.gameVersion ??
                detected?.gameVersion ??
                importPlanDefaultGameVersion(snapshot?.gameVersion),
            loader: saved?.loader ?? detected?.loader ?? importPlanDefaultLoader(snapshot?.loader),
            loaderVersion:
                saved?.loaderVersion ??
                detected?.loaderVersion ??
                importPlanDefaultLoaderVersion(snapshot?.loaderVersion),
            detectedGameVersion: detected?.gameVersion,
            detectedLoader: detected?.loader,
            detectedLoaderVersion: detected?.loaderVersion,
            detectedModCount: detected?.modCount,
        }).hasWarnings
    }
    return result
})

const canReset = computed(
    () => touched.value.gameVersion || touched.value.loader || touched.value.loaderVersion,
)

function resetChanges() {
    if (!canReset.value) return
    const snapshot = activeSnapshot.value
    const detected = detectedByInstance.value[activeIndex.value]
    internalUpdating.value = true
    touched.value = {
        gameVersion: false,
        loader: false,
        loaderVersion: false,
    }
    gameVersion.value = detected?.gameVersion ?? importPlanDefaultGameVersion(snapshot?.gameVersion)
    loader.value = detected?.loader ?? importPlanDefaultLoader(snapshot?.loader)
    loaderVersion.value =
        detected?.loaderVersion ?? importPlanDefaultLoaderVersion(snapshot?.loaderVersion)
    internalUpdating.value = false
    scheduleRescan()
}

async function openRootInFileManager() {
    const path = activeSnapshot.value?.minecraftRoot
    if (!path) return
    try {
        await instanceImport.openPath(path)
    } catch (error) {
        console.error('[SymlinkMethodCards] open root failed', error)
    }
}

async function openVersionPath() {
    const path = activeInstance.value?.path
    if (!path) return
    try {
        await instanceImport.openPath(path)
    } catch (error) {
        console.error('[SymlinkMethodCards] open version path failed', error)
    }
}

function formatCounts(counts?: ImportPlanCounts | null) {
    if (!counts) return formatMessage(messages.notAvailable)
    return formatBytes(counts.bytes)
}

function formatFiles(count: number): string {
    return formatMessage(messages.statsFiles, { files: count })
}

const totalCounts = computed(() => {
    let hasAny = false
    const totals = {
        cache: { files: 0, bytes: 0 },
        local: { files: 0, bytes: 0 },
        network: { files: 0, bytes: 0 },
        migrate: { files: 0, bytes: 0 },
    }
    for (const snapshot of Object.values(snapshots.value)) {
        if (!snapshot) continue
        hasAny = true
        totals.cache.files += snapshot.cache.files
        totals.cache.bytes += snapshot.cache.bytes
        totals.local.files += snapshot.local.files
        totals.local.bytes += snapshot.local.bytes
        totals.network.files += snapshot.network.files
        totals.network.bytes += snapshot.network.bytes
        totals.migrate.files += snapshot.migrate.files
        totals.migrate.bytes += snapshot.migrate.bytes
    }
    return hasAny ? totals : null
})

const statRows = computed(() =>
    [
        {
            key: 'cache',
            label: formatMessage(messages.statsCache),
            tooltip: formatMessage(messages.statsCacheTooltip),
            counts: totalCounts.value?.cache,
        },
        {
            key: 'local',
            label: formatMessage(messages.statsLocal),
            tooltip: formatMessage(messages.statsLocalTooltip),
            counts: totalCounts.value?.local,
        },
        {
            key: 'network',
            label: formatMessage(messages.statsNetwork),
            tooltip: formatMessage(messages.statsNetworkTooltip),
            counts: totalCounts.value?.network,
        },
        {
            key: 'migrate',
            label: formatMessage(messages.statsMigrate),
            tooltip: formatMessage(messages.statsMigrateTooltip),
            counts: totalCounts.value?.migrate,
        },
    ].map((row) => ({
        ...row,
        size: formatCounts(row.counts),
        files: row.counts ? formatFiles(row.counts.files) : formatMessage(messages.notAvailable),
    })),
)

const confirmLabel = computed(() =>
    instances.value.length > 1 && activeIndex.value < instances.value.length - 1
        ? formatMessage(messages.next)
        : formatMessage(messages.confirm),
)

function planReady(index: number) {
    const snapshot = snapshots.value[index]
    return (
        !!snapshot &&
        snapshot.requestId === requestIds.value[index] &&
        snapshot.stage === 'done' &&
        !scanning.value[index] &&
        !planErrors.value[index]
    )
}

const canConfirm = computed(() => {
    if (!isOpen.value || pageAnim.value || !instances.value.length) return false
    return activeIndex.value < instances.value.length - 1
        ? planReady(activeIndex.value)
        : instances.value.every((_, index) => planReady(index))
})

function cancelPlan(index: number) {
    const requestId = requestIds.value[index]
    requestIds.value[index] = null
    scanning.value[index] = false
    if (requestId) {
        console.debug('[SymlinkMethodCards] cancel import plan', index, requestId)
        void instanceImport.cancelImportPlan(requestId).catch((error) => {
            console.error('[SymlinkMethodCards] cancel import plan failed', index, requestId, error)
        })
    }
}

function scheduleRescan(index = activeIndex.value) {
    if (!isOpen.value || internalUpdating.value) return
    if (index === activeIndex.value) saveCurrentChoices()
    cancelPlan(index)
    snapshots.value[index] = null
    scanning.value[index] = true
    planErrors.value[index] = null
    const previous = rescanTimers.get(index)
    if (previous !== undefined) window.clearTimeout(previous)
    rescanTimers.set(
        index,
        window.setTimeout(() => {
            rescanTimers.delete(index)
            if (!isOpen.value) return
            if (index === activeIndex.value) saveCurrentChoices()
            void startPlan(index)
        }, 500),
    )
}

async function ensureListening() {
    if (unlisten) return
    if (listeningPromise) return listeningPromise
    const generation = listenerGeneration
    const pending = (async () => {
        try {
            const release = await instanceImport.listenImportPlan((snapshot) => {
                if (generation === listenerGeneration) handleSnapshot(snapshot)
            })
            if (generation !== listenerGeneration) {
                release()
                return
            }
            unlisten = release
            console.debug('[SymlinkMethodCards] import plan listener ready')
        } catch (error) {
            if (generation !== listenerGeneration) return
            console.error('[SymlinkMethodCards] listenImportPlan failed', error)
            planErrors.value[activeIndex.value] = String(error)
            throw error
        } finally {
            if (generation === listenerGeneration) listeningPromise = null
        }
    })()
    listeningPromise = pending
    return pending
}

function stopListening() {
    listenerGeneration++
    unlisten?.()
    unlisten = null
    if (listeningPromise) {
        listeningPromise = null
    }
}

function saveCurrentChoices() {
    instanceChoices.value[activeIndex.value] = {
        gameVersion: gameVersion.value,
        loader: loader.value,
        loaderVersion: loaderVersion.value,
        touched: { ...touched.value },
    }
}

function resetActiveFields() {
    gameVersion.value = ''
    loader.value = 'vanilla'
    loaderVersion.value = 'latest'
    touched.value = {
        gameVersion: false,
        loader: false,
        loaderVersion: false,
    }
}

async function startPlan(index = activeIndex.value) {
    if (!isOpen.value) return
    cancelPlan(index)
    const instance = instances.value[index]
    if (!instance?.path) {
        console.warn('[SymlinkMethodCards] no import path', index, instance)
        snapshots.value[index] = null
        scanning.value[index] = false
        planErrors.value[index] = formatMessage(messages.missingPath)
        return
    }

    const requestId = crypto.randomUUID()
    requestIds.value[index] = requestId
    snapshots.value[index] = null
    scanning.value[index] = true
    planErrors.value[index] = null

    try {
        await ensureListening()
        if (!isOpen.value || requestIds.value[index] !== requestId) return
        const saved = instanceChoices.value[index]
        const request = {
            requestId,
            launcherType: instance.launcherType ?? 'Generic',
            basePath: instance.basePath ?? '',
            instanceFolder: instance.name,
            instancePath: instance.path,
            gameVersion: saved?.touched.gameVersion ? saved.gameVersion : null,
            loader: saved?.touched.loader ? saved.loader : null,
            loaderVersion: saved?.touched.loaderVersion ? saved.loaderVersion : null,
        }
        console.debug('[SymlinkMethodCards] start import plan', index, request)
        await instanceImport.startImportPlan(request)
        console.debug('[SymlinkMethodCards] import plan started', index, requestId)
    } catch (error) {
        console.error('[SymlinkMethodCards] start import plan failed', error)
        if (requestIds.value[index] !== requestId) return
        requestIds.value[index] = null
        scanning.value[index] = false
        planErrors.value[index] = String(error)
    }
}

async function startAllPlans() {
    const generation = viewGeneration
    // TODO(B2): re-estimate cache/local/network incrementally when overrides
    // change, without rescanning the whole .minecraft folder.
    for (let index = 0; index < instances.value.length; index++) {
        if (!isOpen.value || generation !== viewGeneration) return
        await startPlan(index)
    }
}

const pendingSnapshots = new Map<number, ImportPlanSnapshot>()
let snapshotFlushPending = false

function handleSnapshot(snapshot: ImportPlanSnapshot) {
    let index = Object.entries(requestIds.value).findIndex(
        ([, requestId]) => requestId === snapshot.requestId,
    )
    if (index === -1) {
        index = instances.value.findIndex(
            (instance) => instance.path && snapshot.importPath === instance.path,
        )
    }
    if (index === -1 || requestIds.value[index] !== snapshot.requestId) return

    pendingSnapshots.set(index, snapshot)
    if (!snapshotFlushPending) {
        snapshotFlushPending = true
        requestAnimationFrame(() => {
            snapshotFlushPending = false
            const batch = [...pendingSnapshots.entries()]
            pendingSnapshots.clear()
            for (const [batchIndex, batchSnapshot] of batch) {
                applyImportPlanSnapshot(batchIndex, batchSnapshot)
            }
        })
    }
}

function applyImportPlanSnapshot(index: number, snapshot: ImportPlanSnapshot) {
    if (requestIds.value[index] !== snapshot.requestId) return
    if (snapshot.stage === 'done' || snapshot.stage === 'error') {
        console.debug('[SymlinkMethodCards] import plan final snapshot', index, snapshot)
    }
    const detected = detectedByInstance.value[index]
    if (snapshot.gameVersion) {
        if (!detected) {
            detectedByInstance.value[index] = {
                gameVersion: importPlanDefaultGameVersion(snapshot.gameVersion),
                loader: importPlanDefaultLoader(snapshot.loader),
                loaderVersion: importPlanDefaultLoaderVersion(snapshot.loaderVersion),
                modCount: snapshot.modCount,
            }
        } else if (snapshot.modCount > detected.modCount) {
            detected.modCount = snapshot.modCount
        }
    }

    const merged = reduceImportPlanSnapshot(
        snapshots.value[index] ?? null,
        snapshot,
        snapshot.requestId,
    )
    if (merged) snapshots.value[index] = merged
    scanning.value[index] = snapshot.stage === 'resolving' || snapshot.stage === 'scanning'
    planErrors.value[index] =
        snapshot.stage === 'error' ? (snapshot.error ?? formatMessage(messages.planFailed)) : null

    internalUpdating.value = true
    if (index === activeIndex.value) {
        if (!touched.value.gameVersion && snapshot.gameVersion) {
            gameVersion.value = importPlanDefaultGameVersion(snapshot.gameVersion)
        }
        if (!touched.value.loader && snapshot.loader) {
            loader.value = importPlanDefaultLoader(snapshot.loader)
        }
        if (!touched.value.loaderVersion && snapshot.loaderVersion) {
            loaderVersion.value = importPlanDefaultLoaderVersion(snapshot.loaderVersion)
        }
    }
    internalUpdating.value = false

    if (index === activeIndex.value && snapshot.stage === 'done') {
        void loadLoaderVersions()
    }
}

async function goTo(index: number) {
    if (
        pageAnim.value ||
        index === activeIndex.value ||
        index < 0 ||
        index >= instances.value.length
    ) {
        return
    }
    saveCurrentChoices()
    const generation = viewGeneration
    const direction = index > activeIndex.value ? 'next' : 'prev'
    pageAnim.value = `out-${direction}`
    await wait(150)
    if (!isOpen.value || generation !== viewGeneration) return

    internalUpdating.value = true
    activeIndex.value = index
    loaderVersions.value = []
    const saved = instanceChoices.value[index]
    const snapshot = snapshots.value[index] ?? null
    const detected = detectedByInstance.value[index]
    gameVersion.value = saved?.touched.gameVersion
        ? saved.gameVersion
        : (detected?.gameVersion ?? importPlanDefaultGameVersion(snapshot?.gameVersion))
    loader.value = saved?.touched.loader
        ? saved.loader
        : (detected?.loader ?? importPlanDefaultLoader(snapshot?.loader))
    loaderVersion.value = saved?.touched.loaderVersion
        ? saved.loaderVersion
        : (detected?.loaderVersion ?? importPlanDefaultLoaderVersion(snapshot?.loaderVersion))
    touched.value = saved?.touched ?? { gameVersion: false, loader: false, loaderVersion: false }
    internalUpdating.value = false

    await nextTick()
    if (!isOpen.value || generation !== viewGeneration) return
    pageAnim.value = `in-${direction}`
    await wait(160)
    if (generation !== viewGeneration) return
    pageAnim.value = null
    scrollActiveNavIntoView()
}

function wait(ms: number) {
    return new Promise<void>((resolve) => window.setTimeout(resolve, ms))
}

function scrollActiveNavIntoView() {
    const track = instanceNavTrackRef.value
    if (!track) return
    const active = track.querySelector<HTMLElement>('[data-active="true"]')
    if (active) scrollNavElementIntoView(active)
}

function scrollFocusedNavIntoView(event: FocusEvent) {
    if (event.currentTarget instanceof HTMLElement) {
        scrollNavElementIntoView(event.currentTarget)
    }
}

function scrollNavElementIntoView(element: HTMLElement) {
    const track = instanceNavTrackRef.value
    if (!track) return
    const trackRect = track.getBoundingClientRect()
    const elementRect = element.getBoundingClientRect()
    if (elementRect.left < trackRect.left) {
        track.scrollLeft -= trackRect.left - elementRect.left
    } else if (elementRect.right > trackRect.right) {
        track.scrollLeft += elementRect.right - trackRect.right
    }
}

function handleGameVersionSearch(query: string) {
    touched.value.gameVersion = true
    gameVersion.value = query.trim()
}

function handleLoaderVersionSearch(query: string) {
    touched.value.loaderVersion = true
    loaderVersion.value = query.trim()
}

function handleConfirm() {
    if (!canConfirm.value) return
    if (instances.value.length > 1 && activeIndex.value < instances.value.length - 1) {
        void goTo(activeIndex.value + 1)
        return
    }
    saveCurrentChoices()
    const choices: SymlinkMethodChoice[] = instances.value.map((instance, index) => {
        const saved = instanceChoices.value[index]
        const snapshot = snapshots.value[index] ?? null
        return {
            instanceName: instance.name,
            instancePath: instance.path,
            symlink: false,
            gameVersion:
                (saved?.touched.gameVersion
                    ? saved.gameVersion
                    : importPlanDefaultGameVersion(snapshot?.gameVersion)) || null,
            loader:
                (saved?.touched.loader
                    ? saved.loader
                    : importPlanDefaultLoader(snapshot?.loader)) || null,
            loaderVersion:
                (saved?.touched.loaderVersion
                    ? saved.loaderVersion
                    : importPlanDefaultLoaderVersion(snapshot?.loaderVersion)) || null,
            gameDirOverride: null,
        }
    })

    stopAllPlans()
    isOpen.value = false
    modal.value?.hide()
    emit('confirm', choices)
}

function handleCancel() {
    stopAllPlans()
    isOpen.value = false
    modal.value?.hide()
    emit('cancel')
}

function onModalHide() {
    if (!isOpen.value) return
    stopAllPlans()
    isOpen.value = false
    emit('cancel')
}

function show(options: { instances: SymlinkMethodInstance[] }) {
    console.debug('[SymlinkMethodCards] show', options)
    stopAllPlans()
    for (const timer of rescanTimers.values()) window.clearTimeout(timer)
    rescanTimers.clear()
    pendingSnapshots.clear()
    snapshotFlushPending = false
    instances.value = options.instances
    instanceChoices.value = {}
    snapshots.value = {}
    detectedByInstance.value = {}
    requestIds.value = {}
    scanning.value = {}
    planErrors.value = {}
    pageAnim.value = null
    internalUpdating.value = true
    activeIndex.value = 0
    resetActiveFields()
    internalUpdating.value = false
    isOpen.value = true
    modal.value?.show()
    void startAllPlans()
}

function hide() {
    if (!isOpen.value) return
    stopAllPlans()
    isOpen.value = false
    modal.value?.hide()
}

function stopAllPlans() {
    viewGeneration++
    for (const timer of rescanTimers.values()) window.clearTimeout(timer)
    rescanTimers.clear()
    for (let index = 0; index < instances.value.length; index++) {
        cancelPlan(index)
    }
    stopListening()
    scanning.value = {}
}

watch(
    loader,
    () => {
        if (internalUpdating.value || !isOpen.value) return
        touched.value.loader = true
        touched.value.loaderVersion = true
        loaderVersion.value = 'latest'
        scheduleRescan()
    },
    { flush: 'sync' },
)

watch(
    gameVersion,
    () => {
        if (internalUpdating.value || !isOpen.value) return
        touched.value.gameVersion = true
        scheduleRescan()
    },
    { flush: 'sync' },
)

watch(
    loaderVersion,
    () => {
        if (internalUpdating.value || !isOpen.value) return
        touched.value.loaderVersion = true
        scheduleRescan()
    },
    { flush: 'sync' },
)

watch([loader, gameVersion], () => {
    if (!isOpen.value) return
    void loadLoaderVersions()
})

onMounted(async () => {
    await ensureListening().catch(() => {})
})

onUnmounted(() => {
    stopAllPlans()
})

defineExpose({ show, hide })
</script>

<style scoped>
.page-out-next {
    animation: page-out-next 150ms cubic-bezier(0.4, 0, 1, 1) both;
}

.page-out-prev {
    animation: page-out-prev 150ms cubic-bezier(0.4, 0, 1, 1) both;
}

.page-in-next {
    animation: page-in-next 170ms cubic-bezier(0, 0, 0.2, 1) both;
}

.page-in-prev {
    animation: page-in-prev 170ms cubic-bezier(0, 0, 0.2, 1) both;
}

.instance-selector-track {
    scrollbar-width: none;
}

.instance-selector-track::-webkit-scrollbar {
    display: none;
}

@keyframes page-out-next {
    from {
        transform: translateX(0);
        opacity: 1;
    }
    to {
        transform: translateX(-100%);
        opacity: 0;
    }
}

@keyframes page-out-prev {
    from {
        transform: translateX(0);
        opacity: 1;
    }
    to {
        transform: translateX(100%);
        opacity: 0;
    }
}

@keyframes page-in-next {
    from {
        transform: translateX(100%);
        opacity: 0;
    }
    to {
        transform: translateX(0);
        opacity: 1;
    }
}

@keyframes page-in-prev {
    from {
        transform: translateX(-100%);
        opacity: 0;
    }
    to {
        transform: translateX(0);
        opacity: 1;
    }
}
</style>
