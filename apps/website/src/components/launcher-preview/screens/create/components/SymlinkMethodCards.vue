<script setup lang="ts">
import CircleAlertIcon from '@modrinth/assets/icons/circle-alert.svg?component'
import CopyIcon from '@modrinth/assets/icons/copy.svg?component'
import FolderOpenIcon from '@modrinth/assets/icons/folder-open.svg?component'
import LinkIcon from '@modrinth/assets/icons/link.svg?component'
import BigOptionButton from '@modrinth/ui/src/components/base/BigOptionButton.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Chips from '@modrinth/ui/src/components/base/Chips.vue'
import Combobox, { type ComboboxOption } from '@modrinth/ui/src/components/base/Combobox.vue'
import HorizontalRule from '@modrinth/ui/src/components/base/HorizontalRule.vue'
import ProgressBar from '@modrinth/ui/src/components/base/ProgressBar.vue'
import RadioButtons from '@modrinth/ui/src/components/base/RadioButtons.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import TagItem from '@modrinth/ui/src/components/base/TagItem.vue'
import { useFormatBytes } from '@modrinth/ui/src/composables/format-bytes.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { formatLoaderLabel } from '@modrinth/ui/src/utils/loaders.ts'
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'

import { usePreview } from '../../../lib/state'
import PreviewModal from '../../../overlays/PreviewModal.vue'
import type {
    ImportPlanCounts,
    ImportPlanSnapshot,
    SymlinkCapability,
    SymlinkMethodChoice,
    SymlinkMethodInstance,
} from '../import-flow'
import { symlinkMessages as messages } from '../messages'
import { useCreateArea } from '../state'

/**
 * Port of `packages/ui/src/components/flows/drop/SymlinkMethodCards.vue`: copy or
 * symbolic link, the import estimate of each instance (`start_import_plan`), the
 * detected game version / loader / loader version with their custom-override tags,
 * the version path and runtime root, the game directory mode for links, and the
 * dots that page through several instances.
 */
const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()
const { clock } = usePreview()
const area = useCreateArea()
const instanceImport = area.importFlow

const emit = defineEmits<{
    confirm: [choices: SymlinkMethodChoice[]]
    cancel: []
}>()

interface InstanceChoice {
    gameVersion: string
    loader: string
    loaderVersion: string
    touched: { gameVersion: boolean; loader: boolean; loaderVersion: boolean }
}

const KNOWN_IMPORT_PLAN_LOADERS = ['fabric', 'forge', 'neoforge', 'quilt', 'cleanroom']

function importPlanDefaultGameVersion(detected: string | null | undefined) {
    return detected ?? ''
}

function importPlanDefaultLoader(detected: string | null | undefined) {
    if (detected && KNOWN_IMPORT_PLAN_LOADERS.includes(detected)) return detected
    return 'vanilla'
}

function importPlanDefaultLoaderVersion(detected: string | null | undefined) {
    return detected?.trim() ? detected : 'latest'
}

function isImportPlanLoaderRecognized(loader: string | null | undefined) {
    return !!loader && KNOWN_IMPORT_PLAN_LOADERS.includes(loader)
}

function importPlanWarnings(
    snapshot: ImportPlanSnapshot | null,
    selected: {
        gameVersion?: string | null
        loader?: string | null
        loaderVersion?: string | null
        detectedGameVersion?: string | null
        detectedLoader?: string | null
        detectedLoaderVersion?: string | null
        detectedModCount?: number
    },
) {
    const loader = selected.loader || 'vanilla'
    const loaderVersion = selected.loaderVersion || ''
    const gameVersion = selected.gameVersion || ''
    const detectedGameVersion = importPlanDefaultGameVersion(
        selected.detectedGameVersion ?? snapshot?.gameVersion,
    )
    const detectedLoader = importPlanDefaultLoader(selected.detectedLoader ?? snapshot?.loader)
    const detectedLoaderVersion = importPlanDefaultLoaderVersion(
        selected.detectedLoaderVersion ?? snapshot?.loaderVersion,
    )
    const gameVersionCustom = gameVersion !== detectedGameVersion
    const loaderTypeCustom = loader !== detectedLoader
    const loaderVersionCustom = loader !== 'vanilla' && loaderVersion !== detectedLoaderVersion
    const loaderVersionMissing =
        loader !== 'vanilla' && (!loaderVersion || loaderVersion === 'latest')
    const loaderTypeUnrecognized =
        (loader === 'vanilla' || !isImportPlanLoaderRecognized(loader)) &&
        ((selected.detectedModCount ?? 0) > 0 || (snapshot?.modCount ?? 0) > 0)
    return {
        gameVersionCustom,
        loaderTypeCustom,
        loaderVersionCustom,
        loaderVersionMissing,
        loaderTypeUnrecognized,
        hasWarnings:
            gameVersionCustom ||
            loaderTypeCustom ||
            loaderVersionCustom ||
            loaderVersionMissing ||
            loaderTypeUnrecognized,
    }
}

const modal = ref<InstanceType<typeof PreviewModal> | null>(null)
const isOpen = ref(false)
const instances = ref<SymlinkMethodInstance[]>([])
const symlinkCapable = ref<SymlinkCapability>('supported')
const activeIndex = ref(0)
const method = ref<'copy' | 'symlink' | null>(null)
const gameDirMode = ref<'isolated' | 'not-isolated'>('isolated')
const methodShake = ref(false)
const gameVersion = ref('')
const loader = ref('vanilla')
const loaderVersion = ref('latest')
const loaderVersions = ref<string[]>([])
let loaderVersionRequest = 0
const touched = ref({ gameVersion: false, loader: false, loaderVersion: false })
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
let cancelRescan: (() => void) | null = null

const activeInstance = computed(() => instances.value[activeIndex.value])
const activeSnapshot = computed(() => snapshots.value[activeIndex.value] ?? null)
const activeGameRoot = computed(
    () =>
        activeSnapshot.value?.minecraftRoot ||
        activeInstance.value?.basePath ||
        activeInstance.value?.path ||
        null,
)
const gameDirOverride = computed(() => {
    const root = activeGameRoot.value
    if (method.value !== 'symlink' || !root) return null
    if (gameDirMode.value === 'isolated') {
        return activeInstance.value?.versionPath ?? activeInstance.value?.path ?? null
    }
    return root
})
const statsLoading = computed(() => Object.values(scanning.value).some(Boolean))
const planError = computed(() => planErrors.value[activeIndex.value] ?? null)
const pageAnimationClass = computed(() => (pageAnim.value ? `page-${pageAnim.value}` : ''))
const symlinkAllowed = computed(() => symlinkCapable.value !== 'unsupported')
const requiresAdmin = computed(() => symlinkCapable.value === 'requires_admin')
const unsupported = computed(() => symlinkCapable.value === 'unsupported')
const symlinkNote = computed(() =>
    requiresAdmin.value ? formatMessage(messages.requiresAdmin) : undefined,
)
const loaderItems = ['vanilla', 'fabric', 'forge', 'neoforge', 'quilt', 'cleanroom']
const formatLoader = (item: string) => formatLoaderLabel(item, formatMessage)
const loaderApiName = computed(() => (loader.value === 'neoforge' ? 'neo' : loader.value))

const gameVersionOptions = computed<ComboboxOption<string>[]>(() => {
    const versions = new Set(area.gameVersions.value.map((version) => version.version))
    if (gameVersion.value) versions.add(gameVersion.value)
    return [...versions].map((version) => ({ value: version, label: version }))
})

const loaderVersionOptions = computed<ComboboxOption<string>[]>(() => {
    const versions = new Set(['latest'])
    if (loaderVersion.value && loaderVersion.value !== 'latest') versions.add(loaderVersion.value)
    for (const version of loaderVersions.value) versions.add(version)
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
        if (request === loaderVersionRequest) loaderVersions.value = versions
    } catch {
        if (request === loaderVersionRequest) loaderVersions.value = []
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

function selectMethod(value: 'copy' | 'symlink') {
    method.value = value
}

const gameDirModeItems = ['isolated', 'not-isolated'] as const

function gameDirModeLabel(mode: (typeof gameDirModeItems)[number]) {
    return mode === 'isolated' ? messages.gameDirIsolated : messages.gameDirNotIsolated
}

function setGameDirMode(mode: (typeof gameDirModeItems)[number]) {
    gameDirMode.value = mode
}

function resetChanges() {
    if (!canReset.value) return
    const snapshot = activeSnapshot.value
    const detected = detectedByInstance.value[activeIndex.value]
    internalUpdating.value = true
    touched.value = { gameVersion: false, loader: false, loaderVersion: false }
    gameVersion.value = detected?.gameVersion ?? importPlanDefaultGameVersion(snapshot?.gameVersion)
    loader.value = detected?.loader ?? importPlanDefaultLoader(snapshot?.loader)
    loaderVersion.value =
        detected?.loaderVersion ?? importPlanDefaultLoaderVersion(snapshot?.loaderVersion)
    internalUpdating.value = false
    scheduleRescan()
}

function openRootInFileManager() {
    const path = activeSnapshot.value?.minecraftRoot
    if (path) instanceImport.openPath(path)
}

function openVersionPath() {
    const path = activeInstance.value?.path
    if (path) instanceImport.openPath(path)
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
        for (const key of ['cache', 'local', 'network', 'migrate'] as const) {
            totals[key].files += snapshot[key].files
            totals[key].bytes += snapshot[key].bytes
        }
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

function cancelPlan(index: number) {
    const requestId = requestIds.value[index]
    requestIds.value[index] = null
    scanning.value[index] = false
    if (requestId) instanceImport.cancelImportPlan(requestId)
}

function scheduleRescan(index = activeIndex.value) {
    if (!isOpen.value || internalUpdating.value) return
    cancelRescan?.()
    cancelRescan = clock.after(500, () => {
        cancelRescan = null
        if (!isOpen.value) return
        if (index === activeIndex.value) saveCurrentChoices()
        startPlan(index)
    })
}

function ensureListening() {
    if (unlisten) return
    unlisten = instanceImport.listenImportPlan(handleSnapshot)
}

function stopListening() {
    unlisten?.()
    unlisten = null
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
    method.value = null
    gameVersion.value = ''
    loader.value = 'vanilla'
    loaderVersion.value = 'latest'
    touched.value = { gameVersion: false, loader: false, loaderVersion: false }
}

function randomRequestId() {
    return globalThis.crypto?.randomUUID?.() ?? `plan-${Math.random().toString(36).slice(2)}`
}

function startPlan(index = activeIndex.value) {
    if (!isOpen.value) return
    cancelPlan(index)
    const instance = instances.value[index]
    if (!instance?.path) {
        snapshots.value[index] = null
        scanning.value[index] = false
        planErrors.value[index] = formatMessage(messages.missingPath)
        return
    }
    const requestId = randomRequestId()
    requestIds.value[index] = requestId
    snapshots.value[index] = null
    scanning.value[index] = true
    planErrors.value[index] = null
    ensureListening()
    const saved = instanceChoices.value[index]
    instanceImport.startImportPlan({
        requestId,
        launcherType: instance.launcherType ?? 'Generic',
        basePath: instance.basePath ?? '',
        instanceFolder: instance.name,
        instancePath: instance.path,
        gameVersion: saved?.touched.gameVersion ? saved.gameVersion : null,
        loader: saved?.touched.loader ? saved.loader : null,
        loaderVersion: saved?.touched.loaderVersion ? saved.loaderVersion : null,
    })
}

function startAllPlans() {
    for (let index = 0; index < instances.value.length; index++) {
        if (!isOpen.value) return
        startPlan(index)
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
            for (const [batchIndex, batchSnapshot] of batch)
                applyImportPlanSnapshot(batchIndex, batchSnapshot)
        })
    }
}

function applyImportPlanSnapshot(index: number, snapshot: ImportPlanSnapshot) {
    if (requestIds.value[index] !== snapshot.requestId) return
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
    snapshots.value[index] = snapshot
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

    if (index === activeIndex.value && snapshot.stage === 'done') void loadLoaderVersions()
}

function wait(ms: number) {
    return new Promise<void>((resolve) => clock.after(ms, resolve))
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
    const direction = index > activeIndex.value ? 'next' : 'prev'
    pageAnim.value = `out-${direction}`
    await wait(150)
    if (!isOpen.value) return

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
    if (!isOpen.value) return
    pageAnim.value = `in-${direction}`
    await wait(160)
    pageAnim.value = null
    scrollActiveNavIntoView()
}

function scrollActiveNavIntoView() {
    const track = instanceNavTrackRef.value
    const active = track?.querySelector<HTMLElement>('[data-active="true"]')
    if (active) scrollNavElementIntoView(active)
}

function scrollFocusedNavIntoView(event: FocusEvent) {
    if (event.currentTarget instanceof HTMLElement) scrollNavElementIntoView(event.currentTarget)
}

function scrollNavElementIntoView(element: HTMLElement) {
    const track = instanceNavTrackRef.value
    if (!track) return
    const trackRect = track.getBoundingClientRect()
    const elementRect = element.getBoundingClientRect()
    if (elementRect.left < trackRect.left) track.scrollLeft -= trackRect.left - elementRect.left
    else if (elementRect.right > trackRect.right)
        track.scrollLeft += elementRect.right - trackRect.right
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
    if (instances.value.length === 0) return
    if (instances.value.length > 1 && activeIndex.value < instances.value.length - 1) {
        void goTo(activeIndex.value + 1)
        return
    }
    if (!method.value) {
        triggerMethodShake()
        return
    }
    saveCurrentChoices()
    const choices: SymlinkMethodChoice[] = instances.value.map((instance, index) => {
        const saved = instanceChoices.value[index]
        const snapshot = snapshots.value[index] ?? null
        const root = snapshot?.minecraftRoot || instance.basePath || instance.path || null
        return {
            instanceName: instance.name,
            instancePath: instance.path,
            symlink: method.value === 'symlink',
            gameVersion:
                saved?.gameVersion || importPlanDefaultGameVersion(snapshot?.gameVersion) || null,
            loader: saved?.loader || importPlanDefaultLoader(snapshot?.loader) || null,
            loaderVersion:
                saved?.loaderVersion ||
                importPlanDefaultLoaderVersion(snapshot?.loaderVersion) ||
                null,
            gameDirOverride:
                method.value === 'symlink' && root
                    ? gameDirMode.value === 'isolated'
                        ? (instance.versionPath ?? instance.path ?? null)
                        : root
                    : null,
        }
    })
    stopAllPlans()
    isOpen.value = false
    modal.value?.hide()
    emit('confirm', choices)
}

function triggerMethodShake() {
    methodShake.value = false
    requestAnimationFrame(() => {
        methodShake.value = true
        clock.after(450, () => {
            methodShake.value = false
        })
    })
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

function show(options: { instances: SymlinkMethodInstance[]; symlinkCapable: SymlinkCapability }) {
    cancelRescan?.()
    cancelRescan = null
    pendingSnapshots.clear()
    snapshotFlushPending = false
    instances.value = options.instances
    symlinkCapable.value = options.symlinkCapable
    instanceChoices.value = {}
    snapshots.value = {}
    detectedByInstance.value = {}
    requestIds.value = {}
    scanning.value = {}
    planErrors.value = {}
    pageAnim.value = null
    internalUpdating.value = true
    activeIndex.value = 0
    gameDirMode.value = 'isolated'
    resetActiveFields()
    internalUpdating.value = false
    isOpen.value = true
    modal.value?.show()
    startAllPlans()
}

function hide() {
    if (!isOpen.value) return
    stopAllPlans()
    isOpen.value = false
    modal.value?.hide()
}

function stopAllPlans() {
    cancelRescan?.()
    cancelRescan = null
    for (let index = 0; index < instances.value.length; index++) cancelPlan(index)
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

onMounted(() => {
    ensureListening()
})

onUnmounted(() => {
    stopAllPlans()
})

defineExpose({ show, hide })
</script>

<template>
    <PreviewModal
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
                    <div class="flex flex-col gap-2" :class="{ 'method-shake': methodShake }">
                        <span class="text-sm font-semibold text-[var(--color-text-primary)]">{{
                            formatMessage(messages.method)
                        }}</span>
                        <BigOptionButton
                            :icon="CopyIcon"
                            :title="formatMessage(messages.copyTitle)"
                            :description="formatMessage(messages.copyDesc)"
                            :selected="method === 'copy'"
                            :no-icon-border="true"
                            @click="selectMethod('copy')"
                        />
                        <BigOptionButton
                            v-if="symlinkAllowed"
                            :icon="LinkIcon"
                            :title="formatMessage(messages.symlinkTitle)"
                            :description="formatMessage(messages.symlinkDesc)"
                            :note="symlinkNote"
                            :selected="method === 'symlink'"
                            :no-icon-border="true"
                            @click="selectMethod('symlink')"
                        />
                        <span v-if="unsupported" class="text-danger text-xs">
                            {{ formatMessage(messages.unsupportedWarning) }}
                        </span>
                    </div>

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

                        <div v-if="method === 'symlink'" class="flex flex-col gap-2">
                            <span class="text-sm font-semibold text-[var(--color-text-primary)]">
                                {{ formatMessage(messages.gameDirLabel) }}
                            </span>
                            <RadioButtons
                                v-model="gameDirMode"
                                :items="[...gameDirModeItems]"
                                @update:model-value="setGameDirMode"
                            >
                                <template #default="{ item }">
                                    {{ formatMessage(gameDirModeLabel(item)) }}
                                </template>
                            </RadioButtons>
                            <span
                                v-if="gameDirOverride"
                                class="break-all text-sm text-[var(--color-text-tertiary)]"
                            >
                                {{ gameDirOverride }}
                            </span>
                        </div>

                        <div v-if="planError" class="text-danger text-xs">{{ planError }}</div>
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
                    <Button class="flex items-center gap-2" @click="handleConfirm"
                        >{{ confirmLabel }}
                    </Button>
                </div>
            </div>
        </template>
    </PreviewModal>
</template>

<style scoped>
.method-shake {
    animation: method-shake 0.45s ease-in-out;
}

@keyframes method-shake {
    0%,
    100% {
        transform: translateX(0);
    }
    20% {
        transform: translateX(-6px);
    }
    40% {
        transform: translateX(6px);
    }
    60% {
        transform: translateX(-4px);
    }
    80% {
        transform: translateX(4px);
    }
}

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
