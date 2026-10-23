<script setup lang="ts">
import ArrowUpDownIcon from '@modrinth/assets/icons/arrow-up-down.svg?component'
import ClipboardCopyIcon from '@modrinth/assets/icons/clipboard-copy.svg?component'
import CollectionIcon from '@modrinth/assets/icons/collection.svg?component'
import EyeIcon from '@modrinth/assets/icons/eye.svg?component'
import FolderOpenIcon from '@modrinth/assets/icons/folder-open.svg?component'
import GridIcon from '@modrinth/assets/icons/grid.svg?component'
import LayersIcon from '@modrinth/assets/icons/layers.svg?component'
import MinusIcon from '@modrinth/assets/icons/minus.svg?component'
import MoreVerticalIcon from '@modrinth/assets/icons/more-vertical.svg?component'
import PinIcon from '@modrinth/assets/icons/pin.svg?component'
import PlayIcon from '@modrinth/assets/icons/play.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import SortAscIcon from '@modrinth/assets/icons/sort-asc.svg?component'
import SortDescIcon from '@modrinth/assets/icons/sort-desc.svg?component'
import StarIcon from '@modrinth/assets/icons/star.svg?component'
import StopCircleIcon from '@modrinth/assets/icons/stop-circle.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import PopoutMenu from '@modrinth/ui/src/components/base/PopoutMenu.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { formatLoader } from '@modrinth/ui/src/utils/tag-messages.ts'
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import { useInstanceApi } from '../../../lib/platform/instance'
import { revealInFolder } from '../../../lib/platform/opener'
import { usePreview } from '../../../lib/state'
import type { GameInstance } from '../../../lib/types'
import { injectPreviewWindow } from '../../../lib/window'
import PreviewContextMenu from '../../../overlays/PreviewContextMenu.vue'
import PreviewFloatingActionBar from '../../../overlays/PreviewFloatingActionBar.vue'
import { useLocalJobs } from '../../downloads/local-jobs'
import {
    type InstanceDragData,
    type InstanceGroupDropData,
    provideInstanceDragDrop,
} from '../drag-drop'
import { useGridGrouping } from '../grid-grouping'
import { useLibraryGroups } from '../groups'
import { messages } from '../messages'
import {
    FAVORITES_GROUP_ID,
    type GridGroupingOption,
    type GridSortOption,
    type LibraryDisplayMode,
    UNGROUPED_GROUP_KEY,
} from '../state'
import ConfirmDeleteInstanceModal from './ConfirmDeleteInstanceModal.vue'
import InstanceGroup from './InstanceGroup.vue'
import InstanceGroupModal from './InstanceGroupModal.vue'

/**
 * Port of `components/GridDisplay.vue`. Instance data, groups and processes
 * come from the preview's launcher state; `open` reports the folder the app
 * would reveal in the system file manager.
 */
const { formatMessage } = useVIntl()
const router = useRouter()
const preview = usePreview()
const { remove } = useInstanceApi()
const previewWindow = injectPreviewWindow()
const { layer } = previewWindow
const localJobs = useLocalJobs()
const { library, libraryGroups, orderedLibraryGroupIds } = useLibraryGroups()

const optionMessages: Record<string, { id: string; defaultMessage: string }> = {
    Name: messages.name,
    'Last played': messages.lastPlayed,
    'Date created': messages.dateCreated,
    'Date modified': messages.dateModified,
    'Game version': messages.gameVersion,
    Group: messages.group,
    Loader: messages.loader,
    None: messages.none,
}

const formatOption = (option: string) =>
    optionMessages[option] ? formatMessage(optionMessages[option]) : option

const props = withDefaults(
    defineProps<{
        instances?: GameInstance[]
        label?: string
    }>(),
    {
        instances: () => [],
        label: '',
    },
)

const instanceOptions = ref<InstanceType<typeof PreviewContextMenu>>()
const backgroundContextMenu = ref<InstanceType<typeof PreviewContextMenu>>()
const currentDeleteInstance = ref<GameInstance | null>(null)
const currentContextSectionKey = ref('')
const confirmModal = ref<InstanceType<typeof ConfirmDeleteInstanceModal>>()
const search = ref('')
const displayMode = library.displayMode

const runningInstanceIds = computed(() => library.runningIds())

function removeGroup(groupKey: string) {
    if (groupKey === FAVORITES_GROUP_ID) return
    library.deleteGroup(groupKey)
}

function handleRenameGroup(oldKey: string, newKey: string) {
    if (oldKey === FAVORITES_GROUP_ID) return
    library.renameGroup(oldKey, newKey)
}

function canMoveGroupUp(groupKey: string) {
    if (grouping.value !== 'Group' || groupKey === FAVORITES_GROUP_ID) return false
    const idx = draggableSections.value.findIndex((section) => section.key === groupKey)
    return idx > 0 && orderedLibraryGroupIds.value.length > 1
}

function canMoveGroupDown(groupKey: string) {
    if (grouping.value !== 'Group' || groupKey === FAVORITES_GROUP_ID) return false
    const idx = draggableSections.value.findIndex((section) => section.key === groupKey)
    return idx >= 0 && idx < draggableSections.value.length - 1
}

function handleMoveGroup(groupKey: string, direction: -1 | 1) {
    if (grouping.value !== 'Group') return
    const sections = [...draggableSections.value]
    const idx = sections.findIndex((section) => section.key === groupKey)
    if (idx < 0) return
    const newIdx = idx + direction
    if (newIdx < 0 || newIdx >= sections.length) return
    const [item] = sections.splice(idx, 1)
    sections.splice(newIdx, 0, item)
    library.setGroupOrder(
        sections
            .map((section) => section.key)
            .filter((key) => key !== UNGROUPED_GROUP_KEY && key !== FAVORITES_GROUP_ID),
    )
}

const addToExistingGroupModal = ref<InstanceType<typeof InstanceGroupModal>>()
const addToExistingGroupName = ref('')
const addToExistingGroupId = ref('')

function handleAddToGroup(groupKey: string) {
    const group = libraryGroups.value.find((entry) => entry.id === groupKey)
    addToExistingGroupName.value = group?.name || groupKey
    addToExistingGroupId.value = groupKey
    addToExistingGroupModal.value?.show()
}

const handleNewInstance = () => {
    void router.push('/create')
}

function clearLibraryInstanceSelection() {
    selectedInstanceIds.value.clear()
    selectMode.value = false
    anchorInstanceId.value = null
}

function createGroupFromSelection() {
    if (busy.value) return

    const instanceIds = [...selectedInstanceIds.value]
    clearLibraryInstanceSelection()

    let groupNumber =
        libraryGroups.value.filter((group) => group.id !== FAVORITES_GROUP_ID).length + 1
    const existingNames = new Set(libraryGroups.value.map((group) => group.name.toLowerCase()))
    while (existingNames.has(`group ${groupNumber}`)) {
        groupNumber++
    }
    const group = library.createGroup(`Group ${groupNumber}`)
    library.updateMemberships(
        instanceIds.map((instanceId) => ({
            instance_id: instanceId,
            add_group_ids: [group.id],
            remove_group_ids: [],
        })),
    )
    gridState.group = 'Group'
    groupPendingNameEdit.value = group.id
}

const selectedGroupedInstances = computed(() => {
    if (grouping.value !== 'Group') return []
    return props.instances.filter(
        (instance) =>
            selectedInstanceIds.value.has(instance.id) &&
            (instance.groups || []).some(
                (id) => id !== FAVORITES_GROUP_ID && id !== UNGROUPED_GROUP_KEY,
            ),
    )
})

function removeSelectedInstancesFromGroups() {
    if (busy.value || selectedGroupedInstances.value.length === 0) return

    removingFromGroup.value = true
    try {
        library.updateMemberships(
            selectedGroupedInstances.value.map((instance) => ({
                instance_id: instance.id,
                add_group_ids: [],
                remove_group_ids: instance.groups.filter((id) => id !== FAVORITES_GROUP_ID),
            })),
        )
        clearLibraryInstanceSelection()
    } finally {
        removingFromGroup.value = false
    }
}

const newGroupModal = ref<InstanceType<typeof InstanceGroupModal>>()

function openNewGroupModal() {
    newGroupModal.value?.show([...selectedInstanceIds.value])
}

const displayModeOptions = computed(() => [
    { id: 'standard', label: formatMessage(messages.standardView), icon: GridIcon },
    { id: 'cards', label: formatMessage(messages.cardsView), icon: CollectionIcon },
])

const currentDisplayMode = computed(() =>
    displayModeOptions.value.find((option) => option.id === displayMode.value),
)

function setDisplayMode(mode: string) {
    displayMode.value = mode as LibraryDisplayMode
}

const filteredInstances = computed(() =>
    props.instances.filter((instance) =>
        instance.name.toLowerCase().includes(search.value.toLowerCase()),
    ),
)

const gridState = library.gridState(props.label)

const {
    grouping,
    filteredResults,
    isSectionCollapsed,
    setSectionCollapsed,
    isSortAscending,
    toggleSortDirection,
} = useGridGrouping(gridState, filteredInstances, {
    formatLoader: (loader) => formatLoader(formatMessage, loader),
})

const sortByModel = computed({
    get: () => gridState.sortBy,
    set: (value: GridSortOption) => {
        gridState.sortBy = value
    },
})

const groupModel = computed({
    get: () => gridState.group,
    set: (value: GridGroupingOption) => {
        gridState.group = value
    },
})

const groupPendingNameEdit = ref<string | null>(null)

const groupStableIds = new Map<string, string>()
let groupStableIdCounter = 0

const groupNameMap = computed(() => {
    const map = new Map<string, string>()
    for (const group of libraryGroups.value) {
        map.set(group.id, group.name)
    }
    return map
})

function getOrCreateGroupStableId(key: string): string {
    let id = groupStableIds.get(key)
    if (!id) {
        id = `gid_${++groupStableIdCounter}`
        groupStableIds.set(key, id)
    }
    return id
}

type DisplaySection = { id: string; key: string; instances: GameInstance[] }

const displaySections = computed(() => {
    const resultMap = new Map(filteredResults.value)
    const result: DisplaySection[] = []

    if (gridState.group === 'Group') {
        if (resultMap.has(UNGROUPED_GROUP_KEY)) {
            result.push({
                id: getOrCreateGroupStableId(UNGROUPED_GROUP_KEY),
                key: UNGROUPED_GROUP_KEY,
                instances: resultMap.get(UNGROUPED_GROUP_KEY)!,
            })
            resultMap.delete(UNGROUPED_GROUP_KEY)
        }

        for (const key of orderedLibraryGroupIds.value) {
            if (key === FAVORITES_GROUP_ID) {
                resultMap.delete(key)
                continue
            }
            if (resultMap.has(key)) {
                result.push({
                    id: getOrCreateGroupStableId(key),
                    key,
                    instances: resultMap.get(key)!,
                })
                resultMap.delete(key)
            } else if (libraryGroups.value.some((group) => group.id === key)) {
                result.push({ id: getOrCreateGroupStableId(key), key, instances: [] })
            }
        }

        const remaining = [...resultMap.entries()].sort((a, b) => a[0].localeCompare(b[0]))
        for (const [key, instances] of remaining) {
            result.push({ id: getOrCreateGroupStableId(key), key, instances })
        }
    } else {
        const remaining = [...resultMap.entries()]
        if (gridState.group === 'Game version') {
            remaining.sort((a, b) => a[0].localeCompare(b[0], undefined, { numeric: true }))
        } else {
            remaining.sort((a, b) => a[0].localeCompare(b[0]))
        }

        for (const [key, instances] of remaining) {
            result.push({ id: getOrCreateGroupStableId(key), key, instances })
        }
    }

    return result
})

const pinnedSection = computed(() => {
    if (gridState.group !== 'Group') return null
    const instances = filteredResults.value.get(FAVORITES_GROUP_ID)
    if (!instances || instances.length === 0) return null
    return {
        id: getOrCreateGroupStableId(FAVORITES_GROUP_ID),
        key: FAVORITES_GROUP_ID,
        instances,
    }
})

/** The app keeps a copy for vuedraggable; its group handle is not rendered, so it mirrors. */
const draggableSections = displaySections

const hideUngroupedHeader = computed(() => {
    if (grouping.value !== 'Group') return false
    return !draggableSections.value.some((section) => section.key !== UNGROUPED_GROUP_KEY)
})

const flatVisibleInstanceIds = computed(() => {
    const ids: string[] = []
    if (pinnedSection.value && !isSectionCollapsed(pinnedSection.value.key)) {
        for (const instance of pinnedSection.value.instances) ids.push(instance.id)
    }
    for (const section of draggableSections.value) {
        if (isSectionCollapsed(section.key)) continue
        for (const instance of section.instances) ids.push(instance.id)
    }
    return [...new Set(ids)]
})

const sortDirectionLabels: Record<
    string,
    { asc: { id: string; defaultMessage: string }; desc: { id: string; defaultMessage: string } }
> = {
    Name: { asc: messages.ascAlphabetical, desc: messages.descAlphabetical },
    'Game version': { asc: messages.ascVersion, desc: messages.descVersion },
    'Last played': { asc: messages.ascRecency, desc: messages.descRecency },
    'Date created': { asc: messages.ascDate, desc: messages.descDate },
    'Date modified': { asc: messages.ascDate, desc: messages.descDate },
}

const sortDirectionLabel = computed(() => {
    const labels = sortDirectionLabels[gridState.sortBy] ?? sortDirectionLabels.Name
    return formatMessage(isSortAscending.value ? labels.asc : labels.desc)
})

async function deleteInstance() {
    if (currentDeleteInstance.value) {
        await remove(currentDeleteInstance.value.id)
    }
}

function getInstanceProxy(instanceId: string) {
    const instanceData = props.instances.find((instance) => instance.id === instanceId)
    if (!instanceData) return null
    return {
        instance: instanceData,
        playing: runningInstanceIds.value.has(instanceId) || library.isLaunching(instanceId),
        play: () => library.launch(instanceData),
        stop: () => library.stop(instanceId),
        seeInstance: () => router.push(`/instance/${encodeURIComponent(instanceId)}`),
        openFolder: () => revealInFolder(instanceData.path),
        addContent: () =>
            router.push({
                path: `/browse/${instanceData.loader === 'vanilla' ? 'datapack' : 'mod'}`,
                query: { i: instanceId },
            }),
    }
}

type InstanceProxy = NonNullable<ReturnType<typeof getInstanceProxy>>

const handleRightClick = (event: MouseEvent, instanceId: string, sectionKey: string) => {
    const item = getInstanceProxy(instanceId)
    if (!item) return
    currentContextSectionKey.value = sectionKey
    const isInCustomGroup =
        grouping.value === 'Group' &&
        sectionKey !== UNGROUPED_GROUP_KEY &&
        sectionKey !== FAVORITES_GROUP_ID
    const pinnedToHome = preview.instances.pinnedIds.value.includes(instanceId)
    const baseOptions = [
        {
            section: 'pin' as const,
            name: item.instance.groups?.includes(FAVORITES_GROUP_ID)
                ? 'remove_from_pinned'
                : 'add_to_pinned',
        },
        { type: 'divider' },
        { name: 'add_content', section: 'manage' as const, shown: !item.instance.link },
        { name: 'edit', section: 'manage' as const },
        { name: 'duplicate', section: 'manage' as const },
        { section: 'pin' as const, name: pinnedToHome ? 'unpin' : 'pin' },
        { name: 'open', section: 'navigate' as const },
        { name: 'copy', section: 'navigate' as const },
        ...(isInCustomGroup
            ? [{ name: 'remove_from_group' }, { type: 'divider' }]
            : [{ type: 'divider' }]),
        {
            name: 'delete',
            section: 'danger' as const,
            color: 'danger',
        },
    ]

    instanceOptions.value?.showMenu(
        event,
        item,
        item.playing
            ? [{ name: 'stop', section: 'primary', color: 'danger' }, ...baseOptions]
            : [{ name: 'play', section: 'primary', color: 'primary' }, ...baseOptions],
    )
}

const handleBackgroundContextMenu = (event: MouseEvent) => {
    if (
        event.target instanceof HTMLElement &&
        (event.target.isContentEditable || event.target.matches('input, textarea, select'))
    )
        return
    event.preventDefault()
    backgroundContextMenu.value?.showMenu(event, null, [{ name: 'create_instance' }])
}

const handleBackgroundOption = ({ option }: { option: string }) => {
    if (option === 'create_instance') {
        handleNewInstance()
    }
}

async function handleOptionsClick(args: { item: unknown; option: string }) {
    const item = args.item as InstanceProxy
    switch (args.option) {
        case 'play':
            if (library.isRunning(item.instance.id) || library.isLaunching(item.instance.id)) break
            item.play()
            break
        case 'stop':
            if (!library.isRunning(item.instance.id) && !library.isLaunching(item.instance.id))
                break
            item.stop()
            break
        case 'add_content':
            await item.addContent()
            break
        case 'edit':
            await item.seeInstance()
            break
        case 'duplicate':
            if (item.instance.install_stage == 'installed') localJobs.duplicate(item.instance)
            break
        case 'pin':
            preview.instances.pin(item.instance.id)
            break
        case 'unpin':
            preview.instances.unpin(item.instance.id)
            break
        case 'open':
            item.openFolder()
            break
        case 'copy':
            await navigator.clipboard?.writeText(item.instance.id).catch(() => undefined)
            break
        case 'add_to_pinned':
            library.updateMemberships([
                {
                    instance_id: item.instance.id,
                    add_group_ids: [FAVORITES_GROUP_ID],
                    remove_group_ids: [],
                },
            ])
            break
        case 'remove_from_pinned':
            library.updateMemberships([
                {
                    instance_id: item.instance.id,
                    add_group_ids: [],
                    remove_group_ids: [FAVORITES_GROUP_ID],
                },
            ])
            break
        case 'remove_from_group':
            library.updateMemberships([
                {
                    instance_id: item.instance.id,
                    add_group_ids: [],
                    remove_group_ids: [currentContextSectionKey.value],
                },
            ])
            break
        case 'delete':
            currentDeleteInstance.value = item.instance
            confirmModal.value?.show()
            break
    }
}

// Selection mode
const selectMode = ref(false)
const selectedInstanceIds = ref(new Set<string>())
const anchorInstanceId = ref<string | null>(null)
const creatingGroup = ref(false)
const removingFromGroup = ref(false)
const deletingInstances = ref(false)
const busy = computed(
    () => creatingGroup.value || removingFromGroup.value || deletingInstances.value,
)

watch(flatVisibleInstanceIds, (ids) => {
    const visibleIds = new Set(ids)
    selectedInstanceIds.value = new Set(
        [...selectedInstanceIds.value].filter((id) => visibleIds.has(id)),
    )
    if (anchorInstanceId.value && !visibleIds.has(anchorInstanceId.value)) {
        anchorInstanceId.value = null
    }
    if (selectedInstanceIds.value.size === 0) selectMode.value = false
})

let cancelLongPressTimer: (() => void) | null = null
let longPressTriggered = false

function startLongPress(instanceId: string) {
    longPressTriggered = false
    cancelLongPressTimer?.()
    cancelLongPressTimer = preview.clock.after(500, () => {
        cancelLongPressTimer = null
        longPressTriggered = true
        if (!selectMode.value) {
            selectMode.value = true
        }
        toggleInstanceSelection(instanceId)
    })
}

function cancelLongPress() {
    cancelLongPressTimer?.()
    cancelLongPressTimer = null
}

function handleCardClick(instanceId: string, event?: MouseEvent) {
    if (longPressTriggered) {
        longPressTriggered = false
        return
    }
    const shiftKey = event?.shiftKey ?? false
    if (selectMode.value || shiftKey) {
        if (!selectMode.value) {
            selectMode.value = true
        }
        handleToggleInstance(instanceId, shiftKey)
    }
}

function handleToggleInstance(instanceId: string, shiftKey: boolean) {
    const flatIds = flatVisibleInstanceIds.value
    const anchor = anchorInstanceId.value

    if (shiftKey && anchor && flatIds.length) {
        const anchorIndex = flatIds.indexOf(anchor)
        const targetIndex = flatIds.indexOf(instanceId)

        if (anchorIndex === -1 || targetIndex === -1) {
            toggleInstanceSelection(instanceId)
            return
        }

        const start = Math.min(anchorIndex, targetIndex)
        const end = Math.max(anchorIndex, targetIndex)
        const range = flatIds.slice(start, end + 1)
        const newSet = new Set(selectedInstanceIds.value)

        if (newSet.has(instanceId)) {
            for (const id of range) newSet.delete(id)
        } else {
            for (const id of range) newSet.add(id)
        }

        selectedInstanceIds.value = newSet
        if (newSet.size === 0) selectMode.value = false
        anchorInstanceId.value = null
        return
    }

    toggleInstanceSelection(instanceId)
    anchorInstanceId.value = selectedInstanceIds.value.has(instanceId) ? instanceId : null
}

function toggleInstanceSelection(instanceId: string) {
    const newSet = new Set(selectedInstanceIds.value)
    if (newSet.has(instanceId)) {
        newSet.delete(instanceId)
    } else {
        newSet.add(instanceId)
    }
    selectedInstanceIds.value = newSet
    if (newSet.size === 0) {
        selectMode.value = false
    }
}

function handleCheckboxClick(instanceId: string, event?: MouseEvent) {
    if (!selectMode.value) {
        selectMode.value = true
    }
    handleToggleInstance(instanceId, event?.shiftKey ?? false)
}

const batchDeleteConfirmModal = ref<InstanceType<typeof ConfirmDeleteInstanceModal>>()
const batchDeleteTargets = ref<
    Pick<GameInstance, 'id' | 'name' | 'icon_path' | 'loader' | 'game_version' | 'path'>[]
>([])

function showBatchDeleteConfirmation() {
    if (busy.value) return
    const visibleIds = new Set(flatVisibleInstanceIds.value)
    batchDeleteTargets.value = props.instances
        .filter(
            (instance) => selectedInstanceIds.value.has(instance.id) && visibleIds.has(instance.id),
        )
        .map(({ id, name, icon_path, loader, game_version, path }) => ({
            id,
            name,
            icon_path,
            loader,
            game_version,
            path,
        }))
    if (batchDeleteTargets.value.length > 0) batchDeleteConfirmModal.value?.show()
}

async function batchDeleteInstances() {
    if (busy.value) return
    const targets = [...batchDeleteTargets.value]
    deletingInstances.value = true
    try {
        const failed: typeof targets = []
        const errors: unknown[] = []
        for (const target of targets) {
            try {
                await remove(target.id)
                selectedInstanceIds.value.delete(target.id)
            } catch (error) {
                failed.push(target)
                errors.push(error)
            }
        }
        if (selectedInstanceIds.value.size === 0) clearLibraryInstanceSelection()
        if (errors.length) {
            batchDeleteTargets.value = failed
            throw new AggregateError(errors, 'Some instances could not be deleted')
        }
    } finally {
        deletingInstances.value = false
    }
}

function onBatchEditApplied() {
    selectedInstanceIds.value.clear()
    selectMode.value = false
}

function handleInstanceDragEnd(
    sourceData: InstanceDragData,
    targetData: InstanceGroupDropData | null,
): boolean {
    if (!sourceData || !targetData) return false
    if (sourceData.fromGroup === targetData.groupId) return false

    const instance = props.instances.find((entry) => entry.id === sourceData.instanceId)
    if (!instance) return false

    library.updateMemberships([
        {
            instance_id: sourceData.instanceId,
            add_group_ids: targetData.groupId === UNGROUPED_GROUP_KEY ? [] : [targetData.groupId],
            remove_group_ids:
                sourceData.fromGroup === UNGROUPED_GROUP_KEY ? [] : [sourceData.fromGroup],
        },
    ])
    return true
}

provideInstanceDragDrop({
    window: previewWindow,
    clock: preview.clock,
    onDragEnd: handleInstanceDragEnd,
})
</script>
<template>
    <div class="flex flex-col gap-4 pb-16" @contextmenu="handleBackgroundContextMenu">
        <div class="flex gap-2">
            <StyledInput
                v-model="search"
                :icon="SearchIcon"
                type="text"
                :placeholder="formatMessage(messages.search)"
                clearable
                wrapper-class="flex-1"
            />
            <Button @click="openNewGroupModal"
                ><PlusIcon />
                {{ formatMessage(messages.newGroup) }}
            </Button>
            <Button type="colored" color="brand" @click="router.push('/create')"
                ><PlusIcon />
                {{ formatMessage(messages.createInstance) }}
            </Button>
        </div>
        <div data-tooltip-group="library-toolbar" class="flex flex-wrap items-center gap-2">
            <Combobox
                v-model="sortByModel"
                v-tooltip="{ content: formatMessage(messages.sortBy), triggers: ['hover'] }"
                width="content"
                :options="
                    ['Name', 'Last played', 'Date created', 'Date modified', 'Game version'].map(
                        (value) => ({
                            value,
                            label: formatOption(value),
                        }),
                    )
                "
                :placeholder="formatMessage(messages.select)"
            >
                <template #selected="{ label: selectedLabel }">
                    <div class="flex min-w-0 items-center gap-1">
                        <ArrowUpDownIcon class="size-5 shrink-0 text-[var(--color-text-default)]" />
                        <span class="truncate font-semibold text-[var(--color-text-tertiary)]">{{
                            selectedLabel
                        }}</span>
                    </div>
                </template>
            </Combobox>
            <button
                v-tooltip="{ content: sortDirectionLabel, triggers: ['hover'] }"
                type="button"
                class="flex h-[40px] w-[40px] shrink-0 cursor-pointer items-center justify-center rounded-xl border-none bg-surface-4 p-0 text-button-text transition-all hover:bg-surface-4 hover:text-[var(--color-text-primary)] active:scale-[0.97]"
                :aria-label="sortDirectionLabel"
                :aria-pressed="isSortAscending"
                @click="toggleSortDirection()"
            >
                <SortAscIcon v-if="isSortAscending" class="size-5" />
                <SortDescIcon v-else class="size-5" />
            </button>
            <div class="mx-2 h-6 w-px bg-surface-5" />
            <Combobox
                v-model="groupModel"
                v-tooltip="{ content: formatMessage(messages.groupBy), triggers: ['hover'] }"
                width="content"
                :options="
                    ['Group', 'Loader', 'Game version', 'None'].map((value) => ({
                        value,
                        label: formatOption(value),
                    }))
                "
                :placeholder="formatMessage(messages.select)"
            >
                <template #selected="{ label: selectedLabel }">
                    <div class="flex min-w-0 items-center gap-1">
                        <LayersIcon class="size-5 shrink-0 text-[var(--color-text-default)]" />
                        <span class="truncate font-semibold text-[var(--color-text-tertiary)]">{{
                            selectedLabel
                        }}</span>
                    </div>
                </template>
            </Combobox>
            <PopoutMenu
                :tooltip="formatMessage(messages.view)"
                placement="bottom-end"
                :container="layer ?? undefined"
            >
                <Button circular icon-only :label="formatMessage(messages.view)"
                    ><component :is="currentDisplayMode?.icon" />
                </Button>
                <template #menu>
                    <div class="flex w-44 flex-col gap-1 p-1">
                        <Button
                            v-for="option in displayModeOptions"
                            :key="option.id"
                            :type="displayMode === option.id ? 'base' : 'quiet'"
                            class="flex w-full items-center !justify-start gap-2 text-left"
                            :aria-pressed="displayMode === option.id"
                            @click="setDisplayMode(option.id)"
                        >
                            <component :is="option.icon" class="size-4" />
                            {{ option.label }}
                        </Button>
                    </div>
                </template>
            </PopoutMenu>
        </div>
        <InstanceGroup
            v-if="pinnedSection"
            :key="pinnedSection.id"
            :section-key="pinnedSection.key"
            :group-name="groupNameMap.get(pinnedSection.key) || pinnedSection.key"
            :instances="pinnedSection.instances"
            :running-instance-ids="runningInstanceIds"
            :grouping="grouping"
            :is-collapsed="isSectionCollapsed(pinnedSection.key)"
            :display-mode="displayMode"
            :select-mode="selectMode"
            :selected-instance-ids="selectedInstanceIds"
            :is-sort-ascending="isSortAscending"
            @toggle-collapse="(key: string) => setSectionCollapsed(key, !isSectionCollapsed(key))"
            @handle-context-menu="
                (event: MouseEvent, instanceId: string, sectionKey: string) =>
                    handleRightClick(event, instanceId, sectionKey)
            "
            @handle-checkbox-click="
                (instanceId: string, event: MouseEvent) => handleCheckboxClick(instanceId, event)
            "
            @handle-card-click="
                (instanceId: string, event: MouseEvent) => handleCardClick(instanceId, event)
            "
            @start-long-press="(instanceId: string) => startLongPress(instanceId)"
            @cancel-long-press="cancelLongPress"
            @new-instance="handleNewInstance"
        >
            <template #moreIcon>
                <MoreVerticalIcon />
            </template>
        </InstanceGroup>
        <div>
            <div
                v-for="instanceSection in draggableSections"
                :key="instanceSection.id"
                class="w-full min-w-0"
            >
                <InstanceGroup
                    :section-key="instanceSection.key"
                    :group-name="groupNameMap.get(instanceSection.key) || instanceSection.key"
                    :instances="instanceSection.instances"
                    :running-instance-ids="runningInstanceIds"
                    :grouping="grouping"
                    :is-collapsed="isSectionCollapsed(instanceSection.key)"
                    :display-mode="displayMode"
                    :select-mode="selectMode"
                    :selected-instance-ids="selectedInstanceIds"
                    :hide-header="
                        (hideUngroupedHeader && instanceSection.key === UNGROUPED_GROUP_KEY) ||
                        grouping === 'None'
                    "
                    :is-sort-ascending="isSortAscending"
                    :can-move-up="canMoveGroupUp(instanceSection.key)"
                    :can-move-down="canMoveGroupDown(instanceSection.key)"
                    :group-pending-name-edit="groupPendingNameEdit"
                    @toggle-collapse="
                        (key: string) => setSectionCollapsed(key, !isSectionCollapsed(key))
                    "
                    @handle-context-menu="
                        (event: MouseEvent, instanceId: string, sectionKey: string) =>
                            handleRightClick(event, instanceId, sectionKey)
                    "
                    @handle-checkbox-click="
                        (instanceId: string, event: MouseEvent) =>
                            handleCheckboxClick(instanceId, event)
                    "
                    @handle-card-click="
                        (instanceId: string, event: MouseEvent) =>
                            handleCardClick(instanceId, event)
                    "
                    @start-long-press="(instanceId: string) => startLongPress(instanceId)"
                    @cancel-long-press="cancelLongPress"
                    @delete-group="removeGroup"
                    @rename-group="handleRenameGroup"
                    @move-group="handleMoveGroup"
                    @add-to-group="handleAddToGroup"
                    @new-instance="handleNewInstance"
                    @rename-complete="groupPendingNameEdit = null"
                >
                    <template #moreIcon>
                        <MoreVerticalIcon />
                    </template>
                </InstanceGroup>
            </div>
        </div>
        <ConfirmDeleteInstanceModal
            ref="confirmModal"
            :instance-id="currentDeleteInstance?.id"
            :symlink-target="currentDeleteInstance?.symlink_target"
            :instances="currentDeleteInstance ? [currentDeleteInstance] : []"
            :delete-action="deleteInstance"
        />
        <ConfirmDeleteInstanceModal
            ref="batchDeleteConfirmModal"
            :count="batchDeleteTargets.length"
            :instances="batchDeleteTargets"
            :delete-action="batchDeleteInstances"
        />
        <InstanceGroupModal ref="newGroupModal" :instance-ids="[]" @applied="onBatchEditApplied" />
        <PreviewFloatingActionBar
            :shown="selectMode"
            :aria-label="formatMessage(messages.selectedCount, { count: selectedInstanceIds.size })"
            hide-when-modal-open
        >
            <div class="flex items-center gap-0.5">
                <span
                    class="px-3 py-2 text-base font-semibold tabular-nums text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.selectedCount, { count: selectedInstanceIds.size }) }}
                </span>
                <div class="mx-0.5 h-6 w-px bg-surface-5" />
                <Button
                    type="quiet"
                    class="!text-[var(--color-text-default)]"
                    :disabled="busy"
                    @click="clearLibraryInstanceSelection"
                    ><XIcon class="cq-show-icon hidden" />
                    <span class="bar-label">{{ formatMessage(commonMessages.clearButton) }}</span>
                </Button>
            </div>
            <div class="ml-auto flex items-center gap-0.5">
                <Button
                    v-if="grouping === 'Group'"
                    type="quiet"
                    :disabled="busy"
                    @click="createGroupFromSelection"
                    ><PlusIcon />
                    <span class="bar-label">{{
                        formatMessage(messages.newGroupFromSelection)
                    }}</span>
                </Button>
                <Button
                    v-if="selectedGroupedInstances.length > 0"
                    type="quiet"
                    :disabled="busy"
                    @click="removeSelectedInstancesFromGroups"
                    ><MinusIcon />
                    <span class="bar-label">{{ formatMessage(messages.removeFromGroup) }}</span>
                </Button>
                <div
                    v-if="grouping === 'Group' || selectedGroupedInstances.length > 0"
                    class="mx-1 h-6 w-px bg-surface-5"
                />
                <Button
                    type="quiet"
                    color="red"
                    :disabled="busy"
                    @click="showBatchDeleteConfirmation"
                    ><TrashIcon />
                    <span class="bar-label">{{ formatMessage(commonMessages.deleteLabel) }}</span>
                </Button>
            </div>
        </PreviewFloatingActionBar>
        <InstanceGroupModal
            ref="addToExistingGroupModal"
            :instance-ids="[]"
            :existing-group-name="addToExistingGroupName"
            :existing-group-id="addToExistingGroupId"
            @applied="onBatchEditApplied"
        />
        <PreviewContextMenu ref="instanceOptions" @option-clicked="handleOptionsClick">
            <template #play> <PlayIcon /> {{ formatMessage(commonMessages.playButton) }} </template>
            <template #stop>
                <StopCircleIcon /> {{ formatMessage(commonMessages.stopButton) }}
            </template>
            <template #add_content>
                <PlusIcon /> {{ formatMessage(messages.addContent) }}
            </template>
            <template #edit> <EyeIcon /> {{ formatMessage(messages.viewInstance) }} </template>
            <template #duplicate>
                <ClipboardCopyIcon /> {{ formatMessage(messages.duplicateInstance) }}
            </template>
            <template #pin> <PinIcon /> {{ formatMessage(messages.pinToHome) }} </template>
            <template #unpin>
                <PinIcon class="rotate-45" /> {{ formatMessage(messages.unpinFromHome) }}
            </template>
            <template #delete>
                <TrashIcon /> {{ formatMessage(commonMessages.deleteLabel) }}
            </template>
            <template #open>
                <FolderOpenIcon /> {{ formatMessage(commonMessages.openFolderButton) }}
            </template>
            <template #copy>
                <ClipboardCopyIcon /> {{ formatMessage(messages.copyPath) }}
            </template>
            <template #add_to_pinned>
                <StarIcon /> {{ formatMessage(messages.addToPinned) }}
            </template>
            <template #remove_from_pinned>
                <StarIcon
                    style="color: var(--color-text-default); fill: var(--color-text-default)"
                />
                {{ formatMessage(messages.removeFromPinned) }}
            </template>
            <template #remove_from_group>
                <MinusIcon />
                {{ formatMessage(messages.removeFromContextMenu) }}
            </template>
        </PreviewContextMenu>
        <PreviewContextMenu ref="backgroundContextMenu" @option-clicked="handleBackgroundOption">
            <template #create_instance>
                <PlusIcon /> {{ formatMessage(messages.createInstance) }}
            </template>
        </PreviewContextMenu>
    </div>
</template>
