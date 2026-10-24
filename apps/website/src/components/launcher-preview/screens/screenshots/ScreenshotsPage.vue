<script setup lang="ts">
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import ClipboardCopyIcon from '@modrinth/assets/icons/clipboard-copy.svg?component'
import EditIcon from '@modrinth/assets/icons/edit.svg?component'
import FileArchiveIcon from '@modrinth/assets/icons/file-archive.svg?component'
import FolderOpenIcon from '@modrinth/assets/icons/folder-open.svg?component'
import MinusIcon from '@modrinth/assets/icons/minus.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import IconButton from '@modrinth/ui/src/components/base/buttons/IconButton.vue'
import type { ComboboxOption } from '@modrinth/ui/src/components/base/Combobox.vue'
import EmptyState from '@modrinth/ui/src/components/base/EmptyState.vue'
import type { ButtonMenuOption } from '@modrinth/ui/src/components/base/menu-options.ts'
import ReadyTransition from '@modrinth/ui/src/components/base/ReadyTransition.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useFormatDateTime } from '@modrinth/ui/src/composables/format-date-time.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import { usePreview } from '../../lib/state'
import type { PreviewScreenshot } from '../../lib/state/screenshots'
import { injectPreviewWindow } from '../../lib/window'
import PreviewConfirmModal from '../../overlays/PreviewConfirmModal.vue'
import PreviewContextMenu from '../../overlays/PreviewContextMenu.vue'
import PreviewFloatingActionBar from '../../overlays/PreviewFloatingActionBar.vue'
import PreviewModal from '../../overlays/PreviewModal.vue'
import ScreenshotDragGather from './drag-gather.vue'
import ScreenshotDragPreview from './drag-preview.vue'
import ScreenshotGroup from './group.vue'
import { messages } from './messages'
import {
    createScreenshotSelection,
    isSelectionModifier,
    screenshotGridTargets,
} from './screenshot-selection'
import {
    fileName,
    getSelectionKey,
    instanceIcon,
    type ScreenshotGroupData,
    useScreenshotsPageState,
} from './state'
import ScreenshotToolbar from './toolbar.vue'
import { useScreenshotDragGather } from './use-screenshot-drag-gather'
import { useScreenshotMarquee } from './use-screenshot-marquee'
import type { ViewerItem, ViewerSavePayload } from './viewer.vue'
import Viewer from './viewer.vue'

const MAX_INSTANCE_GROUP_NAME_LENGTH = 256
const SCREENSHOT_GRID_GAP = 12
const FALLBACK_SCREENSHOT_CARD_WIDTH = 320

const props = withDefaults(
    defineProps<{
        instanceId?: string
        showHeading?: boolean
    }>(),
    {
        instanceId: undefined,
        showHeading: false,
    },
)

const page = useScreenshotsPageState()
const preview = usePreview()
const router = useRouter()
const { layer, toLocal, scale } = injectPreviewWindow()
const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()
const formatDateTime = useFormatDateTime({ dateStyle: 'long', timeStyle: 'short' })
const formatMonth = useFormatDateTime({ month: 'long', year: 'numeric' })

const {
    screenshots: store,
    sort,
    sortModel,
    groupByModel,
    collapsedGroups,
    copiedScreenshotIds,
    highlightedScreenshotId,
    imageSource,
    markCopied,
    setGroupCollapsed,
    highlight,
} = page

const isGlobal = computed(() => !props.instanceId)
const search = ref('')
const selection = createScreenshotSelection()
const { selectedKeys } = selection
const screenshotsPage = ref<HTMLElement>()
const screenshotListContainer = ref<HTMLElement>()
const screenshotListWidth = ref(0)
const windowWidth = ref(1280)
const bulkBusy = ref(false)
/**
 * `syncedScreenshotsQueryOptions` scans every instance folder on first load;
 * the preview repeats that pending phase once per window (the page's state
 * survives navigation, like the app's query cache).
 */
const pending = ref(!page.ready.value)
const error = ref<string | null>(null)
let pendingStop: (() => void) | undefined
const creatingCustomGroup = ref(false)
const updatingCustomGroupMemberships = ref(false)
const movingScreenshots = ref(false)
const screenshotToDelete = ref<PreviewScreenshot | null>(null)
const deleteFromPreview = ref(false)
const deleteModal = ref<InstanceType<typeof PreviewConfirmModal>>()
const bulkDeleteModal = ref<InstanceType<typeof PreviewConfirmModal>>()
const deleteGroupModal = ref<InstanceType<typeof PreviewConfirmModal>>()
const customGroupToDelete = ref<string>()
const groupIdPendingNameEdit = ref<string>()
const screenshotOptionsMenu = ref<InstanceType<typeof PreviewContextMenu>>()
const screenshotOptionsTarget = ref<PreviewScreenshot>()
const imageViewer = ref<InstanceType<typeof Viewer>>()
const exportProgress = ref<{ name: string; value: number } | null>(null)
const saveDialog = ref<{ name: string; count: number } | null>(null)
const exportPath = ref('')
const saveDialogModal = ref<InstanceType<typeof PreviewModal>>()
const activeDrag = ref<{ primarySelectionKey: string; selectionKeys: string[] } | null>(null)
const activeDropGroupId = ref<string | null>(null)
const dragPointer = ref({ x: 0, y: 0 })
const dragOrigin = ref({ x: 0, y: 0 })
let pendingDrag: NonNullable<typeof activeDrag.value> | null = null
let dragPointerId: number | null = null
let suppressClick = false

/**
 * The desktop app's screenshot library for this window: every capture, or the
 * captures of one instance when the page is embedded in an instance.
 */
const allScreenshots = computed(() =>
    props.instanceId
        ? store.screenshots.value.filter((shot) => shot.instance_id === props.instanceId)
        : store.screenshots.value,
)

const visibleScreenshots = computed(() => {
    const needle = search.value.trim().toLocaleLowerCase()
    const filtered = allScreenshots.value.filter((screenshot) => {
        if (!needle) return true
        return (
            fileName(screenshot).toLocaleLowerCase().includes(needle) ||
            screenshot.instance_name.toLocaleLowerCase().includes(needle)
        )
    })

    return [...filtered].sort((a, b) => {
        if (sort.value === 'name') return fileName(a).localeCompare(fileName(b))
        const difference = new Date(a.taken_at).getTime() - new Date(b.taken_at).getTime()
        return sort.value === 'oldest' ? difference : -difference
    })
})

const groupBy = computed(() => groupByModel.value as 'custom' | 'instance' | 'date' | 'none')
const isNarrowingResults = computed(() => search.value.trim().length > 0)

const sortOptions = computed<ComboboxOption<string>[]>(() => [
    { value: 'newest', label: formatMessage(messages.newest) },
    { value: 'oldest', label: formatMessage(messages.oldest) },
    { value: 'name', label: formatMessage(messages.name) },
])
const groupOptions = computed<ComboboxOption<string>[]>(() => [
    { value: 'custom', label: formatMessage(messages.custom) },
    ...(isGlobal.value ? [{ value: 'instance', label: formatMessage(messages.instance) }] : []),
    { value: 'date', label: formatMessage(messages.date) },
    { value: 'none', label: formatMessage(messages.none) },
])

/** The current instant the date groups are measured against. */
const now = new Date(Date.UTC(2026, 6, 29, 10, 0, 0))

function getDateGroup(takenAt: string): { id: string; title: string } {
    const created = new Date(takenAt)
    const startOfDay = (date: Date) =>
        new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime()
    const day = 24 * 3600 * 1000
    if (startOfDay(created) === startOfDay(now)) {
        return { id: 'date:today', title: formatMessage(messages.today) }
    }
    if (startOfDay(created) === startOfDay(now) - day) {
        return { id: 'date:yesterday', title: formatMessage(messages.yesterday) }
    }
    const weekStart = startOfDay(now) - now.getDay() * day
    if (startOfDay(created) >= weekStart) {
        return {
            id: `date:week:${new Date(weekStart).toISOString().slice(0, 10)}`,
            title: formatMessage(messages.thisWeek),
        }
    }
    if (created.getFullYear() === now.getFullYear() && created.getMonth() === now.getMonth()) {
        return {
            id: `date:month:${created.getFullYear()}-${String(created.getMonth() + 1).padStart(2, '0')}`,
            title: formatMessage(messages.thisMonth),
        }
    }
    return {
        id: `date:month:${created.getFullYear()}-${String(created.getMonth() + 1).padStart(2, '0')}`,
        title: formatMonth(created),
    }
}

const groupedScreenshots = computed((): ScreenshotGroupData[] => {
    if (groupBy.value === 'none') {
        return [
            {
                id: 'all',
                title: formatMessage(messages.allScreenshots),
                screenshots: visibleScreenshots.value,
            },
        ]
    }

    if (groupBy.value === 'instance') {
        const byInstance = new Map<string, PreviewScreenshot[]>()
        for (const screenshot of visibleScreenshots.value) {
            const group = byInstance.get(screenshot.instance_id) ?? []
            group.push(screenshot)
            byInstance.set(screenshot.instance_id, group)
        }

        const instances = [...preview.instances.instances.value].sort((a, b) =>
            a.name.localeCompare(b.name),
        )
        const groups: ScreenshotGroupData[] = instances.flatMap((instance) => {
            const found = byInstance.get(instance.id)
            return found
                ? [
                      {
                          id: `instance:${instance.id}`,
                          title: instance.name,
                          screenshots: found,
                          dropInstanceId: instance.id,
                      },
                  ]
                : []
        })

        for (const [instanceId, found] of byInstance) {
            if (groups.some((group) => group.dropInstanceId === instanceId)) continue
            groups.push({
                id: `instance:${instanceId}`,
                title: found[0]?.instance_name ?? instanceId,
                screenshots: found,
                dropInstanceId: instanceId,
            })
        }
        return groups
    }

    if (groupBy.value === 'custom') {
        const byGroup = new Map<string, PreviewScreenshot[]>()
        const ungrouped: PreviewScreenshot[] = []
        const validGroups = new Set(store.groups.value)
        for (const screenshot of visibleScreenshots.value) {
            const groupId = screenshot.group
            if (!groupId || !validGroups.has(groupId)) {
                ungrouped.push(screenshot)
                continue
            }
            const group = byGroup.get(groupId) ?? []
            group.push(screenshot)
            byGroup.set(groupId, group)
        }

        const groups: ScreenshotGroupData[] = store.groups.value.map((group) => ({
            id: `custom:${group}`,
            title: group,
            screenshots: byGroup.get(group) ?? [],
            customGroupId: group,
        }))
        groups.push({
            id: 'custom:ungrouped',
            title: formatMessage(messages.ungrouped),
            screenshots: ungrouped,
            customGroupId: null,
        })
        return isNarrowingResults.value
            ? groups.filter(
                  (group) =>
                      group.screenshots.length > 0 ||
                      group.customGroupId === groupIdPendingNameEdit.value,
              )
            : groups
    }

    const groups = new Map<string, ScreenshotGroupData>()
    for (const screenshot of visibleScreenshots.value) {
        const dateGroup = getDateGroup(screenshot.taken_at)
        const group = groups.get(dateGroup.id) ?? { ...dateGroup, screenshots: [] }
        group.screenshots.push(screenshot)
        groups.set(dateGroup.id, group)
    }
    return [...groups.values()].sort((a, b) => {
        const aTime = Math.max(
            ...a.screenshots.map((screenshot) => new Date(screenshot.taken_at).getTime()),
        )
        const bTime = Math.max(
            ...b.screenshots.map((screenshot) => new Date(screenshot.taken_at).getTime()),
        )
        return sort.value === 'oldest' ? aTime - bTime : bTime - aTime
    })
})

const screenshotColumnCount = computed(() => {
    if (windowWidth.value >= 1536) return 4
    if (windowWidth.value >= 640) return 2
    return 1
})

const screenshotCardWidth = computed(() => {
    if (screenshotListWidth.value <= 0) return FALLBACK_SCREENSHOT_CARD_WIDTH
    const gapsWidth = (screenshotColumnCount.value - 1) * SCREENSHOT_GRID_GAP
    return Math.max(0, (screenshotListWidth.value - gapsWidth) / screenshotColumnCount.value)
})

const screenshotCardHeight = computed(() => (screenshotCardWidth.value * 9) / 16)
const screenshotRowHeight = computed(() => screenshotCardHeight.value + SCREENSHOT_GRID_GAP)

type GroupLayout = {
    id: string
    group: ScreenshotGroupData
    top: number
    height: number
    isOpen: boolean
    gridTop: number
}

const SCREENSHOT_GROUP_HEADER_HEIGHT = 40
const SCREENSHOT_GROUP_CONTENT_SPACING = 10
const SCREENSHOT_GROUP_SPACING = 12
const SCREENSHOT_GRID_MIN_HEIGHT = 45

const screenshotGroupLayouts = computed<GroupLayout[]>(() => {
    const layouts: GroupLayout[] = []
    let top = 0

    for (const group of groupedScreenshots.value) {
        const isHeaderHidden = groupBy.value === 'none'
        const isOpen = isHeaderHidden || search.value.length > 0 || !collapsedGroups.value[group.id]
        const rowCount = Math.ceil(group.screenshots.length / screenshotColumnCount.value)
        const gridHeight =
            rowCount === 0
                ? SCREENSHOT_GRID_MIN_HEIGHT
                : rowCount * screenshotCardHeight.value +
                  Math.max(0, rowCount - 1) * SCREENSHOT_GRID_GAP
        const headerHeight = isHeaderHidden ? 0 : SCREENSHOT_GROUP_HEADER_HEIGHT
        const gridTop = top + headerHeight + SCREENSHOT_GROUP_CONTENT_SPACING
        const height =
            headerHeight +
            (isOpen ? SCREENSHOT_GROUP_CONTENT_SPACING + gridHeight : 0) +
            SCREENSHOT_GROUP_SPACING

        layouts.push({ id: group.id, group, top, height, isOpen, gridTop })
        top += height
    }

    return layouts
})

const selectionOrder = computed(() =>
    screenshotGroupLayouts.value
        .filter((layout) => layout.isOpen)
        .flatMap((layout) => layout.group.screenshots.map(getSelectionKey)),
)
const selectionTargets = computed(() =>
    screenshotGridTargets(
        screenshotGroupLayouts.value.map((layout) => ({
            keys: layout.group.screenshots.map(getSelectionKey),
            gridTop: layout.gridTop,
            isOpen: layout.isOpen,
        })),
        screenshotColumnCount.value,
        screenshotCardWidth.value,
        screenshotCardHeight.value,
        SCREENSHOT_GRID_GAP,
    ),
)

const marquee = useScreenshotMarquee({
    container: screenshotListContainer,
    disabled: () => bulkBusy.value || Boolean(activeDrag.value),
    targets: () => selectionTargets.value,
    snapshot: () => selectedKeys.value,
    apply: selection.box,
})
const selectionRectangle = marquee.rectangle

const selectionActive = computed(() => selectedKeys.value.size > 0)
const selectedScreenshots = computed(() =>
    allScreenshots.value.filter((screenshot) =>
        selectedKeys.value.has(getSelectionKey(screenshot)),
    ),
)
const selectedGroupedScreenshots = computed(() =>
    selectedScreenshots.value.filter((screenshot) => screenshot.group),
)

const {
    items: gatherItems,
    target: gatherTarget,
    isGathering,
    start: startGather,
    updateTarget: updateGatherTarget,
    clear: clearGather,
    finish: finishGather,
} = useScreenshotDragGather(allScreenshots)

const activeDraggedKeys = computed(() => new Set(activeDrag.value?.selectionKeys ?? []))
const activeDraggedScreenshots = computed(() => {
    const drag = activeDrag.value
    if (!drag) return []
    const byKey = new Map(allScreenshots.value.map((shot) => [getSelectionKey(shot), shot]))
    return drag.selectionKeys.flatMap((key) => {
        const shot = byKey.get(key)
        return shot ? [shot] : []
    })
})
const activeDraggedScreenshot = computed(() =>
    activeDrag.value
        ? allScreenshots.value.find(
              (shot) => getSelectionKey(shot) === activeDrag.value?.primarySelectionKey,
          )
        : undefined,
)

const previewItems = computed<ViewerItem[]>(() =>
    visibleScreenshots.value.map((screenshot) => ({
        id: getSelectionKey(screenshot),
        src: imageSource(screenshot),
        alt: fileName(screenshot),
        title: fileName(screenshot),
        description: isGlobal.value
            ? formatMessage(messages.instanceAndDate, {
                  instance: screenshot.instance_name,
                  date: formatDateTime(screenshot.taken_at),
              })
            : formatDateTime(screenshot.taken_at),
        width: screenshot.width,
        height: screenshot.height,
    })),
)

watch(
    selectionOrder,
    (order) => {
        marquee.cancel()
        selection.reconcile(order)
    },
    { flush: 'sync' },
)
watch(bulkBusy, (busy) => {
    if (busy) marquee.cancel()
})

let screenshotListObserver: ResizeObserver | undefined
let copiedTimeouts: ReturnType<typeof window.setTimeout>[] = []
let revealTimeout: ReturnType<typeof window.setTimeout> | undefined
let groupHoverOpenTimeout: ReturnType<typeof window.setTimeout> | undefined

function measureList() {
    if (screenshotListContainer.value) {
        screenshotListWidth.value = screenshotListContainer.value.clientWidth
    }
}

function onWindowResize() {
    windowWidth.value = window.innerWidth
    measureList()
}

onMounted(() => {
    if (pending.value) {
        pendingStop = preview.clock.after(550, () => {
            pending.value = false
            page.ready.value = true
            if (page.skippedFolders.value.length > 0) {
                addNotification({
                    type: 'warning',
                    title: formatMessage(messages.skippedFolders, {
                        count: page.skippedFolders.value.length,
                    }),
                })
            }
        })
    }
    onWindowResize()
    screenshotListObserver = new ResizeObserver(measureList)
    if (screenshotListContainer.value) screenshotListObserver.observe(screenshotListContainer.value)
    window.addEventListener('resize', onWindowResize)
})

onBeforeUnmount(() => {
    pendingStop?.()
    screenshotListObserver?.disconnect()
    window.removeEventListener('resize', onWindowResize)
    for (const timeout of copiedTimeouts) window.clearTimeout(timeout)
    copiedTimeouts = []
    if (revealTimeout) window.clearTimeout(revealTimeout)
    clearDrag()
})

function screenshotByKey(key: string) {
    return allScreenshots.value.find((shot) => getSelectionKey(shot) === key)
}

function toggleGroupSelection(group: ScreenshotGroupData) {
    if (bulkBusy.value) return
    setGroupCollapsed(group.id, false)
    selection.toggleGroup(group.screenshots.map(getSelectionKey))
}

function onSelectionAreaClick(event: MouseEvent) {
    if (suppressClick) {
        suppressClick = false
        event.preventDefault()
        event.stopPropagation()
        return
    }
    if (marquee.click(event)) return
    const target = event.target instanceof Element ? event.target : null
    if (
        !target?.closest(
            '[data-screenshot-card], [data-screenshot-group-header], button, input, a, [role="button"]',
        ) &&
        !isSelectionModifier(event)
    )
        clearSelection()
}

function toggleScreenshotSelection(
    screenshot: PreviewScreenshot,
    event?: MouseEvent | KeyboardEvent,
) {
    if (bulkBusy.value) return
    selection.select(getSelectionKey(screenshot), selectionOrder.value, event)
}

function activateScreenshot(screenshot: PreviewScreenshot, event: MouseEvent | KeyboardEvent) {
    if (selectionActive.value || isSelectionModifier(event)) {
        toggleScreenshotSelection(screenshot, event)
        return
    }
    selection.markAnchor(getSelectionKey(screenshot))
    const index = visibleScreenshots.value.findIndex(
        (candidate) => getSelectionKey(candidate) === getSelectionKey(screenshot),
    )
    if (index >= 0) {
        screenshotOptionsMenu.value?.close()
        imageViewer.value?.show(index)
    }
}

function clearSelection() {
    if (!bulkBusy.value) selection.clear()
}

function showScreenshotOptions(screenshot: PreviewScreenshot, event: MouseEvent) {
    screenshotOptionsTarget.value = screenshot
    const options: ButtonMenuOption[] = [
        ...(event.type === 'contextmenu'
            ? [
                  {
                      id: 'edit',
                      label: formatMessage(messages.edit),
                      icon: EditIcon,
                      action: () => editScreenshot(screenshot),
                  },
                  {
                      id: 'copy',
                      label: formatMessage(messages.copy),
                      icon: ClipboardCopyIcon,
                      action: () => void copyScreenshot(screenshot),
                  },
              ]
            : []),
        {
            id: 'open',
            label: formatMessage(messages.showInFolder),
            icon: FolderOpenIcon,
            action: () => void openScreenshot(screenshot),
        },
        {
            id: 'go-to-instance',
            label: formatMessage(messages.goToInstance),
            action: () => void goToInstance(screenshot),
        },
        { type: 'divider' },
        {
            id: 'delete',
            label: formatMessage(commonMessages.deleteLabel),
            icon: TrashIcon,
            tone: 'red',
            action: () => requestDelete(screenshot),
        },
    ] as ButtonMenuOption[]
    screenshotOptionsMenu.value?.open(event, options)
}

function editScreenshot(screenshot: PreviewScreenshot) {
    const index = visibleScreenshots.value.findIndex(
        (candidate) => getSelectionKey(candidate) === getSelectionKey(screenshot),
    )
    if (index >= 0) {
        screenshotOptionsMenu.value?.close()
        imageViewer.value?.edit(index)
    }
}

function requestDelete(screenshot: PreviewScreenshot, fromPreview = false) {
    deleteFromPreview.value = fromPreview
    screenshotToDelete.value = screenshot
    deleteModal.value?.show()
}

function confirmDelete() {
    const screenshot = screenshotToDelete.value
    if (!screenshot) return
    page.screenshots.remove([screenshot.id])
    selection.reconcile(selectionOrder.value)
    if (deleteFromPreview.value) imageViewer.value?.hide()
    addNotification({ type: 'success', title: formatMessage(messages.deleteSuccess) })
    screenshotToDelete.value = null
    deleteFromPreview.value = false
}

function deleteSelected() {
    const selected = selectedScreenshots.value
    if (bulkBusy.value || selected.length === 0) return
    page.screenshots.remove(selected.map((screenshot) => screenshot.id))
    selection.clear()
    addNotification({
        type: 'success',
        title: formatMessage(messages.bulkDeleteSuccess, { count: selected.length }),
    })
}

async function exportSelected() {
    const selected = selectedScreenshots.value
    if (bulkBusy.value || selected.length === 0) return
    const instanceName =
        selected[0]?.instance_name.replace(/[\\/:*?"<>|]/g, '-') ?? formatMessage(messages.heading)
    // `save()` opens the native save dialog with the ZIP filter; the preview
    // shows the same default name and filter inside the window.
    exportPath.value = isGlobal.value
        ? formatMessage(messages.globalExportFilename)
        : formatMessage(messages.instanceExportFilename, { instance: instanceName })
    saveDialog.value = { count: selected.length }
    saveDialogModal.value?.show()
}

function confirmExport() {
    const dialog = saveDialog.value
    if (!dialog) return
    const name = exportPath.value.trim() || formatMessage(messages.globalExportFilename)
    saveDialog.value = null
    saveDialogModal.value?.hide()
    bulkBusy.value = true
    exportProgress.value = { name, value: 0 }
    let elapsed = 0
    const stop = preview.clock.every((ms) => {
        elapsed += ms
        const value = Math.min(1, elapsed / 1200)
        if (exportProgress.value) exportProgress.value = { name, value }
        if (value >= 1) {
            stop()
            exportProgress.value = null
            bulkBusy.value = false
            selection.clear()
        }
    })
}

async function copyScreenshot(screenshot: PreviewScreenshot) {
    try {
        const response = await fetch(imageSource(screenshot))
        let blob = await response.blob()
        if (blob.type !== 'image/png') {
            blob = await convertToPng(blob)
        }
        await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })])
        markCopied(screenshot.id)
    } catch (error) {
        // Same result the app reports when the clipboard is unavailable: the
        // app's own error notification, with the capture kept in the viewer.
        handleError(error)
    }
}

function convertToPng(blob: Blob): Promise<Blob> {
    return new Promise((resolve, reject) => {
        const image = new Image()
        const url = URL.createObjectURL(blob)
        image.onload = () => {
            const canvas = document.createElement('canvas')
            canvas.width = image.naturalWidth
            canvas.height = image.naturalHeight
            canvas.getContext('2d')?.drawImage(image, 0, 0)
            URL.revokeObjectURL(url)
            canvas.toBlob((png) => (png ? resolve(png) : reject(new Error('no png'))), 'image/png')
        }
        image.onerror = () => {
            URL.revokeObjectURL(url)
            reject(new Error('image decode failed'))
        }
        image.src = url
    })
}

function openScreenshot(screenshot: PreviewScreenshot) {
    // `instance_open_screenshot` hands the file to the operating system's image
    // viewer, which is outside the window; the preview opens the same capture
    // in the page's viewer instead of reporting an artificial result.
    const index = visibleScreenshots.value.findIndex(
        (candidate) => getSelectionKey(candidate) === getSelectionKey(screenshot),
    )
    if (index >= 0) imageViewer.value?.show(index)
}

function goToInstance(screenshot: PreviewScreenshot) {
    return router.push(`/instance/${encodeURIComponent(screenshot.instance_id)}`)
}

function getDefaultCustomGroupName() {
    const existing = new Set(store.groups.value.map((group) => group.trim().toLocaleLowerCase()))
    let number = store.groups.value.length + 1
    while (existing.has(`group ${number}`.toLocaleLowerCase())) number += 1
    return `Group ${number}`
}

async function createCustomGroup() {
    if (creatingCustomGroup.value) return
    creatingCustomGroup.value = true
    try {
        const selected = [...selectedScreenshots.value]
        const name = getDefaultCustomGroupName()
        store.createGroup(name)
        store.moveToGroup(
            selected.map((screenshot) => screenshot.id),
            name,
        )
        groupByModel.value = 'custom'
        selection.clear()
        groupIdPendingNameEdit.value = name
    } finally {
        creatingCustomGroup.value = false
    }
}

function validateCustomGroupName(value: string) {
    const normalized = value.trim()
    return normalized.length > 0 && normalized.length <= MAX_INSTANCE_GROUP_NAME_LENGTH
}

function renameCustomGroup(groupId: string | null | undefined, name: string) {
    if (!groupId) return false
    const trimmed = name.trim()
    if (!trimmed || trimmed === groupId) {
        groupIdPendingNameEdit.value = undefined
        return true
    }
    store.renameGroup(groupId, trimmed)
    if (groupIdPendingNameEdit.value === groupId) groupIdPendingNameEdit.value = undefined
    return true
}

function requestCustomGroupDeletion(groupId: string) {
    const hasScreenshots = allScreenshots.value.some((screenshot) => screenshot.group === groupId)
    customGroupToDelete.value = groupId
    if (hasScreenshots) {
        deleteGroupModal.value?.show()
    } else {
        deleteCustomGroup()
    }
}

function deleteCustomGroup() {
    const group = customGroupToDelete.value
    if (!group) return
    store.deleteGroup(group)
    if (groupIdPendingNameEdit.value === group) groupIdPendingNameEdit.value = undefined
    customGroupToDelete.value = undefined
}

function assignCustomGroup(screenshotsToMove: PreviewScreenshot[], group: string | null) {
    const moved = screenshotsToMove.filter((screenshot) => (screenshot.group ?? null) !== group)
    if (moved.length === 0 || updatingCustomGroupMemberships.value) return
    updatingCustomGroupMemberships.value = true
    try {
        store.moveToGroup(
            moved.map((screenshot) => screenshot.id),
            group,
        )
        selection.clear()
    } finally {
        updatingCustomGroupMemberships.value = false
    }
}

function removeSelectedScreenshotsFromGroups() {
    if (bulkBusy.value || selectedGroupedScreenshots.value.length === 0) return
    assignCustomGroup(selectedGroupedScreenshots.value, null)
}

function moveScreenshotsToInstance(keys: string[], targetInstanceId: string) {
    if (movingScreenshots.value) return
    movingScreenshots.value = true
    try {
        page.moveToInstance(keys, targetInstanceId)
        selection.clear()
    } finally {
        movingScreenshots.value = false
    }
}

function canDropOnGroup(group: ScreenshotGroupData) {
    if (!activeDrag.value) return false
    if (groupBy.value === 'custom') {
        const target = group.customGroupId ?? null
        return activeDraggedScreenshots.value.some(
            (screenshot) => (screenshot.group ?? null) !== target,
        )
    }
    if (groupBy.value === 'instance' && group.dropInstanceId) {
        return activeDraggedScreenshots.value.some(
            (screenshot) => screenshot.instance_id !== group.dropInstanceId,
        )
    }
    return false
}

/** Cards start a drag only for a selected card (or a multi-selection). */
function beginDrag(screenshot: PreviewScreenshot, event: PointerEvent) {
    if (
        bulkBusy.value ||
        (groupBy.value !== 'custom' && !(isGlobal.value && groupBy.value === 'instance'))
    )
        return
    const key = getSelectionKey(screenshot)
    if (!selectedKeys.value.has(key) || isSelectionModifier(event)) return
    const selectionKeys = [key, ...selectedKeys.value].filter(
        (value, index, list) => list.indexOf(value) === index,
    )
    pendingDrag = { primarySelectionKey: key, selectionKeys }
    dragPointerId = event.pointerId
    dragPointer.value = { x: event.clientX, y: event.clientY }
    dragOrigin.value = { x: event.clientX, y: event.clientY }
    document.addEventListener('pointermove', dragMove, true)
    document.addEventListener('pointerup', dragEnd, true)
    document.addEventListener('pointercancel', dragEnd, true)
    window.addEventListener('blur', dragEnd)
}

function groupUnderPointer(): string | null {
    const elements =
        screenshotListContainer.value?.querySelectorAll<HTMLElement>(
            '[data-screenshot-group-id]',
        ) ?? []
    for (const element of elements) {
        const rect = element.getBoundingClientRect()
        if (
            dragPointer.value.x >= rect.left &&
            dragPointer.value.x <= rect.right &&
            dragPointer.value.y >= rect.top &&
            dragPointer.value.y <= rect.bottom
        )
            return element.dataset.screenshotGroupId ?? null
    }
    return null
}

function dragMove(event: PointerEvent) {
    if (event.pointerId !== dragPointerId) return
    if (!activeDrag.value) {
        if (
            !pendingDrag ||
            Math.hypot(event.clientX - dragOrigin.value.x, event.clientY - dragOrigin.value.y) < 4
        )
            return
        activeDrag.value = pendingDrag
        pendingDrag = null
        startGather(activeDrag.value, { x: event.clientX, y: event.clientY })
    }
    dragPointer.value = { x: event.clientX, y: event.clientY }
    updateGatherTarget({ x: event.clientX, y: event.clientY })
    activeDropGroupId.value = groupUnderPointer()
}

function clearDrag() {
    document.removeEventListener('pointermove', dragMove, true)
    document.removeEventListener('pointerup', dragEnd, true)
    document.removeEventListener('pointercancel', dragEnd, true)
    window.removeEventListener('blur', dragEnd)
    clearGroupHoverOpenTimeout()
    clearGather()
    pendingDrag = null
    dragPointerId = null
    activeDrag.value = null
    activeDropGroupId.value = null
}

function dragEnd(event: Event) {
    if (event instanceof PointerEvent && event.pointerId !== dragPointerId) return
    if (!activeDrag.value) {
        clearDrag()
        return
    }

    const targetId = activeDropGroupId.value
    const targetGroup = groupedScreenshots.value.find((group) => group.id === targetId)
    if (event.type === 'pointerup' && targetGroup && canDropOnGroup(targetGroup)) {
        if (groupBy.value === 'custom') {
            assignCustomGroup(activeDraggedScreenshots.value, targetGroup.customGroupId ?? null)
        } else if (targetGroup.dropInstanceId) {
            const keys = activeDraggedScreenshots.value
                .filter((screenshot) => screenshot.instance_id !== targetGroup.dropInstanceId)
                .map((screenshot) => screenshot.id)
            if (keys.length > 0) moveScreenshotsToInstance(keys, targetGroup.dropInstanceId)
        }
    }

    // The release that ends a drag must not also open the viewer; the app's
    // dnd-kit sensors swallow that click the same way.
    if (event.type === 'pointerup') {
        suppressClick = true
        window.setTimeout(() => {
            suppressClick = false
        }, 0)
    }

    clearDrag()
}

const GROUP_HOVER_OPEN_DELAY = 750

function clearGroupHoverOpenTimeout() {
    if (groupHoverOpenTimeout !== undefined) window.clearTimeout(groupHoverOpenTimeout)
    groupHoverOpenTimeout = undefined
}

watch(activeDropGroupId, (groupId) => {
    clearGroupHoverOpenTimeout()
    if (!groupId || !collapsedGroups.value[groupId]) return
    const group = groupedScreenshots.value.find((candidate) => candidate.id === groupId)
    if (!group || !canDropOnGroup(group)) return
    groupHoverOpenTimeout = window.setTimeout(() => {
        groupHoverOpenTimeout = undefined
        if (activeDropGroupId.value === groupId) setGroupCollapsed(groupId, false)
    }, GROUP_HOVER_OPEN_DELAY)
})

async function handleViewerSave(payload: ViewerSavePayload) {
    const screenshot = screenshotByKey(payload.item.id)
    if (!screenshot) return
    const saved = await page.saveEdit(screenshot, payload.pngBytes, payload.mode)
    imageViewer.value?.markSavedAndView(getSelectionKey(saved))
    await revealScreenshot(saved.id)
}

async function revealScreenshot(id: string) {
    if (revealTimeout) window.clearTimeout(revealTimeout)
    const group = groupedScreenshots.value.find((candidate) =>
        candidate.screenshots.some((screenshot) => screenshot.id === id),
    )
    if (group) setGroupCollapsed(group.id, false)
    await nextTick()
    const layout = screenshotGroupLayouts.value.find(
        (candidate) => candidate.group.id === group?.id,
    )
    const index = layout?.group.screenshots.findIndex((screenshot) => screenshot.id === id)
    const scrollTarget =
        screenshotListContainer.value?.closest<HTMLElement>('.app-viewport') ?? undefined
    if (layout && index !== undefined && index >= 0 && scrollTarget) {
        const row = Math.floor(index / screenshotColumnCount.value)
        const top = Math.max(
            0,
            layout.gridTop +
                row * screenshotRowHeight.value +
                screenshotCardHeight.value / 2 -
                scrollTarget.clientHeight / 2,
        )
        scrollTarget.scrollTo({ top, behavior: 'smooth' })
    }
    highlight(id)
}
</script>

<template>
    <PreviewConfirmModal
        ref="deleteModal"
        :title="formatMessage(messages.deleteTitle)"
        :description="
            formatMessage(messages.deleteDescription, {
                name: screenshotToDelete ? fileName(screenshotToDelete) : '',
            })
        "
        :proceed-label="formatMessage(commonMessages.deleteLabel)"
        @proceed="confirmDelete"
    />
    <PreviewConfirmModal
        ref="bulkDeleteModal"
        :title="formatMessage(messages.bulkDeleteTitle)"
        :description="formatMessage(messages.bulkDeleteDescription, { count: selectedKeys.size })"
        :proceed-label="formatMessage(commonMessages.deleteLabel)"
        @proceed="deleteSelected"
    />
    <PreviewConfirmModal
        ref="deleteGroupModal"
        :title="formatMessage(messages.deleteGroup)"
        :description="formatMessage(messages.deleteGroupDescription)"
        :proceed-label="formatMessage(commonMessages.deleteLabel)"
        @proceed="deleteCustomGroup"
    />
    <PreviewContextMenu
        ref="screenshotOptionsMenu"
        :label="formatMessage(commonMessages.actionsLabel)"
    >
        <template #go-to-instance>
            <Avatar
                :src="
                    screenshotOptionsTarget
                        ? instanceIcon(screenshotOptionsTarget.instance_id)
                        : undefined
                "
                :tint-by="screenshotOptionsTarget?.instance_id"
                alt=""
                size="1rem"
                class="shrink-0"
            />
            {{ formatMessage(messages.goToInstance) }}
        </template>
    </PreviewContextMenu>
    <Viewer
        ref="imageViewer"
        :items="previewItems"
        :saving="page.saving.value"
        @save="handleViewerSave"
    >
        <template #actions="{ item }">
            <IconButton
                v-tooltip="
                    formatMessage(
                        copiedScreenshotIds.has(screenshotByKey(item.id)?.id ?? '')
                            ? messages.copied
                            : messages.copy,
                    )
                "
                :label="
                    formatMessage(
                        copiedScreenshotIds.has(screenshotByKey(item.id)?.id ?? '')
                            ? messages.copied
                            : messages.copy,
                    )
                "
                type="quiet"
                @click="screenshotByKey(item.id) && copyScreenshot(screenshotByKey(item.id)!)"
            >
                <CheckIcon
                    v-if="copiedScreenshotIds.has(screenshotByKey(item.id)?.id ?? '')"
                    class="text-green"
                />
                <ClipboardCopyIcon v-else />
            </IconButton>
            <IconButton
                v-tooltip="formatMessage(messages.showInFolder)"
                :label="formatMessage(messages.showInFolder)"
                type="quiet"
                @click="screenshotByKey(item.id) && openScreenshot(screenshotByKey(item.id)!)"
            >
                <FolderOpenIcon />
            </IconButton>
            <IconButton
                v-tooltip="formatMessage(commonMessages.deleteLabel)"
                :label="formatMessage(commonMessages.deleteLabel)"
                type="quiet"
                class="hover:!bg-red hover:!text-[var(--color-accent-contrast)] focus-visible:!bg-red focus-visible:!text-[var(--color-accent-contrast)]"
                @click="screenshotByKey(item.id) && requestDelete(screenshotByKey(item.id)!, true)"
            >
                <TrashIcon />
            </IconButton>
        </template>
    </Viewer>

    <PreviewModal
        ref="saveDialogModal"
        :header="formatMessage(messages.exportZip)"
        max-width="480px"
        :on-hide="() => (saveDialog = null)"
    >
        <div class="flex flex-col gap-4">
            <label
                class="flex flex-col gap-1 text-sm font-semibold text-[var(--color-text-primary)]"
            >
                {{ formatMessage(messages.globalExportFilename) }}
                <StyledInput
                    v-model="exportPath"
                    type="text"
                    :placeholder="formatMessage(messages.globalExportFilename)"
                    wrapper-class="w-full"
                    @keydown.enter="confirmExport"
                />
                <span class="text-xs font-normal text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.zipArchive) }}
                </span>
            </label>
            <div class="flex justify-end gap-2">
                <Button class="!shadow-none" @click="saveDialogModal?.hide()">
                    <XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button
                    type="colored"
                    color="brand"
                    :disabled="!exportPath.trim()"
                    @click="confirmExport"
                >
                    <FileArchiveIcon />
                    {{ formatMessage(commonMessages.saveButton) }}
                </Button>
            </div>
        </div>
    </PreviewModal>

    <div
        ref="screenshotsPage"
        class="flex h-full w-full flex-col gap-3"
        :class="{ 'justify-center': allScreenshots.length === 0 && !groupIdPendingNameEdit }"
    >
        <template v-if="allScreenshots.length > 0 || groupIdPendingNameEdit">
            <h1 v-if="showHeading" class="m-0 text-2xl font-bold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.heading) }}
            </h1>

            <ScreenshotToolbar
                v-model:search="search"
                v-model:sort="sortModel"
                v-model:group="groupByModel"
                :sort-options="sortOptions"
                :group-options="groupOptions"
                @new-group="createCustomGroup"
            />
        </template>

        <ReadyTransition :pending="pending">
            <EmptyState
                v-if="error"
                type="error"
                :heading="formatMessage(messages.errorHeading)"
                :description="error"
            />
            <EmptyState
                v-else-if="allScreenshots.length === 0 && !groupIdPendingNameEdit"
                type="no-images"
                :heading="formatMessage(messages.emptyHeading)"
                :description="formatMessage(messages.emptyDescription)"
            />
            <EmptyState
                v-else-if="visibleScreenshots.length === 0 && !groupIdPendingNameEdit"
                type="no-images"
                :heading="formatMessage(messages.noResultsHeading)"
                :description="formatMessage(messages.noResultsDescription)"
            />

            <div
                v-else
                ref="screenshotListContainer"
                data-screenshot-selection-area
                class="w-full"
                :style="{ overflowAnchor: 'none' }"
                @pointerdown.capture="marquee.start"
                @click.capture="onSelectionAreaClick"
            >
                <div v-for="{ id, group } in screenshotGroupLayouts" :key="id">
                    <ScreenshotGroup
                        :id="group.id"
                        :title="group.title"
                        :screenshots="group.screenshots"
                        :selected-keys="selectedKeys"
                        :selection-active="selectionActive"
                        :active-dragged-keys="activeDraggedKeys"
                        :show-drop-outline="activeDropGroupId === group.id && canDropOnGroup(group)"
                        :can-drag="
                            !bulkBusy &&
                            (groupBy === 'custom' || (isGlobal && groupBy === 'instance'))
                        "
                        :drop-instance-id="
                            groupBy === 'instance' ? group.dropInstanceId : undefined
                        "
                        :drop-custom-group="groupBy === 'custom'"
                        :drop-custom-group-id="group.customGroupId ?? null"
                        :show-instance-name="isGlobal && groupBy !== 'instance'"
                        :highlighted-screenshot-id="highlightedScreenshotId"
                        :copied-screenshot-ids="copiedScreenshotIds"
                        :force-open="search.length > 0"
                        :hide-header="groupBy === 'none'"
                        :editable-title="Boolean(group.customGroupId)"
                        :start-editing-title="groupIdPendingNameEdit === group.customGroupId"
                        :max-title-length="MAX_INSTANCE_GROUP_NAME_LENGTH"
                        :validate-title="validateCustomGroupName"
                        :on-title-change="
                            (name: string) => renameCustomGroup(group.customGroupId, name)
                        "
                        :collapsed="Boolean(collapsedGroups[group.id])"
                        :source-for="imageSource"
                        @update:collapsed="(value) => setGroupCollapsed(group.id, value)"
                        @activate="activateScreenshot"
                        @toggle-selection="toggleScreenshotSelection"
                        @copy="copyScreenshot"
                        @edit="editScreenshot"
                        @more="showScreenshotOptions"
                        @drag-start="beginDrag"
                    >
                        <template #actions="{ startEditing }">
                            <input
                                type="checkbox"
                                class="size-4 shrink-0 cursor-pointer accent-brand"
                                :checked="
                                    group.screenshots.length > 0 &&
                                    group.screenshots.every((screenshot) =>
                                        selectedKeys.has(getSelectionKey(screenshot)),
                                    )
                                "
                                :indeterminate.prop="
                                    group.screenshots.some((screenshot) =>
                                        selectedKeys.has(getSelectionKey(screenshot)),
                                    ) &&
                                    !group.screenshots.every((screenshot) =>
                                        selectedKeys.has(getSelectionKey(screenshot)),
                                    )
                                "
                                :aria-label="
                                    formatMessage(messages.selectGroup, { group: group.title })
                                "
                                :disabled="bulkBusy || group.screenshots.length === 0"
                                @change="toggleGroupSelection(group)"
                            />
                            <div
                                v-if="group.customGroupId"
                                class="duration-250 flex shrink-0 items-center opacity-0 transition-opacity focus-within:opacity-100 group-hover/header:opacity-100"
                            >
                                <IconButton
                                    v-tooltip="formatMessage(messages.editGroup)"
                                    :label="formatMessage(messages.editGroup)"
                                    type="quiet"
                                    size="sm"
                                    @click.stop="startEditing"
                                >
                                    <EditIcon />
                                </IconButton>
                                <IconButton
                                    v-tooltip="formatMessage(messages.deleteGroup)"
                                    :label="formatMessage(messages.deleteGroup)"
                                    type="quiet"
                                    size="sm"
                                    @click.stop="requestCustomGroupDeletion(group.customGroupId)"
                                >
                                    <TrashIcon />
                                </IconButton>
                            </div>
                        </template>
                    </ScreenshotGroup>
                </div>
            </div>
        </ReadyTransition>
    </div>

    <Teleport :to="layer" :disabled="!layer">
        <div class="pointer-events-none fixed inset-0 z-[9999]">
            <div
                v-if="activeDraggedScreenshot"
                class="pointer-events-none absolute w-[320px] transition-all duration-150 ease-out"
                :style="{
                    left: `${toLocal(dragPointer.x, dragPointer.y).x - 160}px`,
                    top: `${toLocal(dragPointer.x, dragPointer.y).y - 45}px`,
                }"
                :class="isGathering ? 'scale-[0.975]' : 'scale-100'"
            >
                <ScreenshotDragPreview
                    :screenshot="activeDraggedScreenshot"
                    :count="activeDraggedScreenshots.length"
                    :thumbnail="imageSource(activeDraggedScreenshot)"
                />
            </div>
        </div>
    </Teleport>
    <ScreenshotDragGather
        v-if="gatherItems.length > 0"
        :items="gatherItems"
        :target="gatherTarget"
        :thumbnail-for="(item) => imageSource(item.screenshot)"
        @complete="finishGather"
    />

    <Teleport :to="layer" :disabled="!layer">
        <div
            v-if="selectionRectangle"
            aria-hidden="true"
            class="pointer-events-none fixed z-[9998] rounded border border-solid border-brand bg-brand-highlight"
            :style="{
                left: `${toLocal(selectionRectangle.left, selectionRectangle.top).x}px`,
                top: `${toLocal(selectionRectangle.left, selectionRectangle.top).y}px`,
                width: `${selectionRectangle.width / scale.value}px`,
                height: `${selectionRectangle.height / scale.value}px`,
            }"
        />
    </Teleport>

    <Teleport :to="layer" :disabled="!layer">
        <div
            v-if="exportProgress"
            class="pointer-events-none fixed inset-0 z-[9999] flex items-end justify-center p-4 pb-24"
        >
            <div
                class="pointer-events-auto w-[min(24rem,100%)] rounded-[var(--radius-xl)] border border-solid border-surface-5 bg-surface-3 p-4 shadow-[0px_1px_3px_0px_rgba(0,0,0,0.3),0px_6px_10px_0px_rgba(0,0,0,0.15)]"
                role="status"
            >
                <p class="m-0 text-sm font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.exportZip) }}
                </p>
                <p class="mb-2 mt-1 truncate text-xs text-[var(--color-text-tertiary)]">
                    {{ exportProgress.name }}
                </p>
                <div class="h-1.5 w-full overflow-hidden rounded-full bg-surface-5">
                    <div
                        class="h-full rounded-full bg-brand transition-[width] duration-200 ease-out"
                        :style="{ width: `${Math.round(exportProgress.value * 100)}%` }"
                    />
                </div>
            </div>
        </div>
    </Teleport>

    <PreviewFloatingActionBar
        :shown="selectionActive"
        :aria-label="formatMessage(messages.selectionAriaLabel)"
        hide-when-modal-open
    >
        <div class="flex items-center gap-0.5">
            <span
                class="px-4 py-2.5 text-base font-semibold tabular-nums text-[var(--color-text-primary)]"
            >
                {{ formatMessage(messages.selectedCount, { count: selectedKeys.size }) }}
            </span>
            <div class="mx-1 h-6 w-px bg-surface-5" />
            <Button
                v-tooltip="formatMessage(commonMessages.clearButton)"
                type="quiet"
                :disabled="bulkBusy"
                @click="clearSelection"
            >
                <XIcon class="cq-show-icon hidden" />
                <span class="bar-label">{{ formatMessage(commonMessages.clearButton) }}</span>
            </Button>
        </div>
        <div class="ml-auto flex items-center gap-0.5">
            <Button
                v-tooltip="formatMessage(messages.newGroup)"
                type="quiet"
                :disabled="bulkBusy"
                @click="createCustomGroup"
            >
                <PlusIcon />
                <span class="bar-label">{{ formatMessage(messages.newGroup) }}</span>
            </Button>
            <Button
                v-if="selectedGroupedScreenshots.length > 0"
                v-tooltip="formatMessage(messages.removeFromGroup)"
                type="quiet"
                :disabled="bulkBusy"
                @click="removeSelectedScreenshotsFromGroups"
            >
                <MinusIcon />
                <span class="bar-label">{{ formatMessage(messages.removeFromGroup) }}</span>
            </Button>
            <Button
                v-tooltip="formatMessage(messages.exportZip)"
                type="quiet"
                :disabled="bulkBusy"
                @click="exportSelected"
            >
                <FileArchiveIcon />
                <span class="bar-label">{{ formatMessage(messages.exportZip) }}</span>
            </Button>
            <div class="mx-1 h-6 w-px bg-surface-5" />
            <Button
                v-tooltip="formatMessage(commonMessages.deleteLabel)"
                type="quiet"
                color="red"
                interaction="filled"
                :disabled="bulkBusy"
                @click="bulkDeleteModal?.show()"
            >
                <TrashIcon />
                <span class="bar-label">{{ formatMessage(commonMessages.deleteLabel) }}</span>
            </Button>
        </div>
    </PreviewFloatingActionBar>
</template>
