<script setup lang="ts">
import { DragDropProvider } from '@dnd-kit/vue'
import {
    ArrowUpDownIcon,
    ClipboardCopyIcon,
    CollectionIcon,
    EyeIcon,
    FolderOpenIcon,
    GridIcon,
    LayersIcon,
    MinusIcon,
    MoreVerticalIcon,
    PinIcon,
    PlayIcon,
    PlusIcon,
    SearchIcon,
    SortAscIcon,
    SortDescIcon,
    StarIcon,
    StopCircleIcon,
    TrashIcon,
    XIcon,
} from '@modrinth/assets'
import {
    Button,
    Combobox,
    commonMessages,
    defineMessages,
    FloatingActionBar,
    formatLoader,
    injectNotificationManager,
    PopoutMenu,
    StyledInput,
    useVIntl,
} from '@modrinth/ui'
import { computed, onMounted, onUnmounted, ref, shallowRef, watch } from 'vue'
import { useRouter } from 'vue-router'
import Draggable from 'vuedraggable'

import ContextMenu from '@/components/ui/ContextMenu.vue'
import InstanceGroup from '@/components/ui/library/InstanceGroup.vue'
import ConfirmDeleteInstanceModal from '@/components/ui/modal/ConfirmDeleteInstanceModal.vue'
import InstanceGroupModal from '@/components/ui/modal/InstanceGroupModal.vue'
import { UNGROUPED_GROUP_KEY, useGridGrouping } from '@/composables/useGridGrouping'
import { FAVORITES_GROUP_ID, useInstanceGroups } from '@/composables/useInstanceGroups'
import { trackEvent } from '@/helpers/analytics'
import { process_listener } from '@/helpers/events'
import { install_duplicate_instance } from '@/helpers/install'
import { kill, remove, run, set_pinned } from '@/helpers/instance'
import { create_group as createGroup } from '@/helpers/instance-groups'
import { updateRunningInstanceIds } from '@/helpers/instance-running-state'
import {
    getLastLibraryDisplayMode,
    setLastLibraryDisplayMode,
} from '@/helpers/library-display-mode'
import { get_all, get_by_instance_id } from '@/helpers/process'
import type { GameInstance } from '@/helpers/types'
import { showInstanceInFolder } from '@/helpers/utils.js'

const { handleError } = injectNotificationManager()

const { formatMessage } = useVIntl()
const router = useRouter()

const messages = defineMessages({
    search: { id: 'app.instances.search', defaultMessage: 'Search' },
    select: { id: 'app.instances.select', defaultMessage: 'Select...' },
    groupBy: { id: 'app.instances.group-by', defaultMessage: 'Group by' },
    addContent: { id: 'app.instances.add-content', defaultMessage: 'Add content' },
    createInstance: {
        id: 'app.library.create-instance',
        defaultMessage: 'Create new instance',
    },
    newGroup: {
        id: 'app.library.group.new',
        defaultMessage: 'New group',
    },
    viewInstance: { id: 'app.instances.view-instance', defaultMessage: 'View instance' },
    duplicateInstance: {
        id: 'app.instances.duplicate-instance',
        defaultMessage: 'Duplicate instance',
    },
    copyPath: { id: 'app.instances.copy-path', defaultMessage: 'Copy path' },
    pinToHome: { id: 'app.instances.pin-to-home', defaultMessage: 'Pin to Home' },
    unpinFromHome: { id: 'app.instances.unpin-from-home', defaultMessage: 'Unpin from Home' },
    name: { id: 'app.instances.sort.name', defaultMessage: 'Name' },
    lastPlayed: { id: 'app.instances.sort.last-played', defaultMessage: 'Last played' },
    dateCreated: { id: 'app.instances.sort.date-created', defaultMessage: 'Date created' },
    dateModified: { id: 'app.instances.sort.date-modified', defaultMessage: 'Date modified' },
    gameVersion: { id: 'app.instances.group.game-version', defaultMessage: 'Game version' },
    group: { id: 'app.instances.group.group', defaultMessage: 'Custom group' },
    loader: { id: 'app.instances.group.loader', defaultMessage: 'Loader' },
    none: { id: 'app.instances.group.none', defaultMessage: 'No grouping' },
    ungrouped: { id: 'app.instances.group.ungrouped', defaultMessage: 'No group' },
    editGroups: { id: 'app.instances.edit-groups', defaultMessage: 'Edit groups' },
    selectAll: { id: 'app.instances.select-all', defaultMessage: 'Select all' },
    deselectAll: { id: 'app.instances.deselect-all', defaultMessage: 'Deselect all' },
    selectedCount: {
        id: 'app.instances.selected-count',
        defaultMessage: '{count, plural, one {# selected} other {# selected}}',
    },
    view: { id: 'app.library.view', defaultMessage: 'View' },
    standardView: { id: 'app.library.view.standard', defaultMessage: 'Standard grid' },
    cardsView: { id: 'app.library.view.cards', defaultMessage: 'Library cards' },
    sortBy: { id: 'app.instances.sort-by', defaultMessage: 'Sort by' },
    ascAlphabetical: {
        id: 'app.instances.sort.direction.asc-alphabetical',
        defaultMessage: 'A–Z',
    },
    descAlphabetical: {
        id: 'app.instances.sort.direction.desc-alphabetical',
        defaultMessage: 'Z–A',
    },
    ascVersion: {
        id: 'app.instances.sort.direction.asc-version',
        defaultMessage: 'Oldest version first',
    },
    descVersion: {
        id: 'app.instances.sort.direction.desc-version',
        defaultMessage: 'Newest version first',
    },
    ascRecency: {
        id: 'app.instances.sort.direction.asc-recency',
        defaultMessage: 'Least recent first',
    },
    descRecency: {
        id: 'app.instances.sort.direction.desc-recency',
        defaultMessage: 'Most recent first',
    },
    newGroupFromSelection: {
        id: 'app.library.selection.new-group',
        defaultMessage: 'New group',
    },
    removeFromGroup: {
        id: 'app.library.selection.remove-from-group',
        defaultMessage: 'Remove from group',
    },
    addToPinned: {
        id: 'app.library.context-menu.pin-instance',
        defaultMessage: 'Pin in Library',
    },
    removeFromPinned: {
        id: 'app.library.context-menu.unpin-instance',
        defaultMessage: 'Unpin from Library',
    },
    removeFromContextMenu: {
        id: 'app.library.context-menu.remove-from-group',
        defaultMessage: 'Remove from group',
    },
    ascDate: {
        id: 'app.instances.sort.direction.asc-date',
        defaultMessage: 'Oldest first',
    },
    descDate: {
        id: 'app.instances.sort.direction.desc-date',
        defaultMessage: 'Newest first',
    },
})

const optionMessages = {
    Name: messages.name,
    'Last played': messages.lastPlayed,
    'Date created': messages.dateCreated,
    'Date modified': messages.dateModified,
    'Game version': messages.gameVersion,
    Group: messages.group,
    Loader: messages.loader,
    None: messages.none,
}

const formatOption = (option) =>
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

const instanceOptions = ref(null)
const backgroundContextMenu = ref(null)
const currentDeleteInstance = ref(null)
const currentContextSectionKey = ref('')
const confirmModal = ref(null)
const search = ref('')
const displayMode = ref(getLastLibraryDisplayMode())
const runningInstanceIds = ref(new Set<string>())
const startingInstanceIds = ref(new Set<string>())
let unlistenProcesses: (() => void) | undefined
let disposed = false

async function refreshRunningInstances() {
    const processes = await get_all().catch(handleError)
    if (processes && !disposed)
        runningInstanceIds.value = new Set(processes.map((process) => process.instance_id))
}

function setInstanceRunning(instanceId: string, running: boolean) {
    runningInstanceIds.value = updateRunningInstanceIds(runningInstanceIds.value, {
        instance_id: instanceId,
        event: running ? 'launched' : 'finished',
    })
}

onMounted(async () => {
    await refreshRunningInstances()
    if (disposed) return
    const unlisten = await process_listener((event: { instance_id: string; event: string }) => {
        if (!disposed && (event.event === 'launched' || event.event === 'finished')) {
            runningInstanceIds.value = updateRunningInstanceIds(runningInstanceIds.value, event)
        }
    })
    if (disposed) unlisten()
    else unlistenProcesses = unlisten
})

onUnmounted(() => {
    disposed = true
    unlistenProcesses?.()
})

function removeGroup(groupKey: string) {
    if (groupKey === FAVORITES_GROUP_ID) return
    deleteGroupById(groupKey).catch(handleError)
}

function handleRenameGroup(oldKey: string, newKey: string) {
    if (oldKey === FAVORITES_GROUP_ID) return
    renameGroupById(oldKey, newKey).catch(handleError)
}

function canMoveGroupUp(groupKey: string) {
    if (grouping.value !== 'Group' || groupKey === FAVORITES_GROUP_ID) return false
    const idx = draggableSections.value.findIndex((s) => s.key === groupKey)
    return idx > 0 && orderedLibraryGroupIds.value.length > 1
}

function canMoveGroupDown(groupKey: string) {
    if (grouping.value !== 'Group' || groupKey === FAVORITES_GROUP_ID) return false
    const idx = draggableSections.value.findIndex((s) => s.key === groupKey)
    return idx >= 0 && idx < draggableSections.value.length - 1
}

function handleMoveGroup(groupKey: string, direction: -1 | 1) {
    if (grouping.value !== 'Group') return
    const sections = [...draggableSections.value]
    const idx = sections.findIndex((s) => s.key === groupKey)
    if (idx < 0) return
    const newIdx = idx + direction
    if (newIdx < 0 || newIdx >= sections.length) return
    const [item] = sections.splice(idx, 1)
    sections.splice(newIdx, 0, item)
    draggableSections.value = sections
    onGroupReorder()
    reorderGroups(
        draggableSections.value
            .map((s) => s.key)
            .filter((key) => key !== UNGROUPED_GROUP_KEY && key !== FAVORITES_GROUP_ID),
    ).catch(handleError)
}

const addToExistingGroupModal = ref(null)
const addToExistingGroupName = ref('')
const addToExistingGroupId = ref('')

function handleAddToGroup(groupKey: string) {
    const group = libraryGroups.value.find((g) => g.id === groupKey)
    addToExistingGroupName.value = group?.name || groupKey
    addToExistingGroupId.value = groupKey
    addToExistingGroupModal.value?.show()
}

const handleNewInstance = () => {
    router.push('/create')
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

    let groupNumber = libraryGroups.value.filter((g) => g.id !== FAVORITES_GROUP_ID).length + 1
    const existingNames = new Set(libraryGroups.value.map((g) => g.name.toLowerCase()))
    while (existingNames.has(`group ${groupNumber}`)) {
        groupNumber++
    }
    const groupName = `Group ${groupNumber}`

    createGroup(groupName)
        .then((group) => {
            const updates = instanceIds.map((instanceId) => {
                return {
                    instance_id: instanceId,
                    add_group_ids: [group.id],
                    remove_group_ids: [],
                }
            })
            return updateMemberships(updates).then(() => group)
        })
        .then((group) => {
            state.value.group = 'Group'
            groupPendingNameEdit.value = group.id
        })
        .catch(handleError)
}

const busy = computed(
    () => creatingGroup.value || removingFromGroup.value || deletingInstances.value,
)

const selectedGroupedInstances = computed(() => {
    if (grouping.value !== 'Group') return []
    return props.instances.filter(
        (i) =>
            selectedInstanceIds.value.has(i.id) &&
            (i.groups || []).some((id) => id !== FAVORITES_GROUP_ID && id !== UNGROUPED_GROUP_KEY),
    )
})

async function removeSelectedInstancesFromGroups() {
    if (busy.value || selectedGroupedInstances.value.length === 0) return

    removingFromGroup.value = true
    try {
        const updates = selectedGroupedInstances.value.map((instance) => ({
            instance_id: instance.id,
            add_group_ids: [],
            remove_group_ids: instance.groups.filter((id) => id !== FAVORITES_GROUP_ID),
        }))
        await updateMemberships(updates)
        clearLibraryInstanceSelection()
    } catch (err) {
        handleError(err)
    } finally {
        removingFromGroup.value = false
    }
}

const newGroupModal = ref(null)

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

function setDisplayMode(mode) {
    displayMode.value = mode
    setLastLibraryDisplayMode(mode)
}

const filteredInstances = computed(() =>
    props.instances.filter((instance) =>
        instance.name.toLowerCase().includes(search.value.toLowerCase()),
    ),
)

const {
    state,
    grouping,
    filteredResults,
    isSectionCollapsed,
    setSectionCollapsed,
    isSortAscending,
    toggleSortDirection,
} = useGridGrouping(props.label, filteredInstances, {
    formatLoader: (loader) => formatLoader(formatMessage, loader),
})

const {
    libraryGroups,
    orderedLibraryGroupIds,
    renameGroupById,
    deleteGroupById,
    reorderGroups,
    updateMemberships,
} = useInstanceGroups(filteredInstances)

const groupPendingNameEdit = ref<string | null>(null)

const groupStableIds = ref(new Map<string, string>())
let groupStableIdCounter = 0

const groupNameMap = computed(() => {
    const map = new Map<string, string>()
    for (const group of libraryGroups.value) {
        map.set(group.id, group.name)
    }
    return map
})

function generateGroupStableId(): string {
    return `gid_${++groupStableIdCounter}`
}

function getOrCreateGroupStableId(key: string): string {
    if (groupStableIds.value.has(key)) {
        return groupStableIds.value.get(key)!
    }
    const id = generateGroupStableId()
    const newMap = new Map(groupStableIds.value)
    newMap.set(key, id)
    groupStableIds.value = newMap
    return id
}

type DisplaySection = { id: string; key: string; instances: typeof props.instances }

const displaySections = computed(() => {
    const resultMap = new Map(filteredResults.value)
    const result: DisplaySection[] = []

    if (state.value.group === 'Group') {
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
            } else if (libraryGroups.value.some((g) => g.id === key)) {
                result.push({
                    id: getOrCreateGroupStableId(key),
                    key,
                    instances: [] as typeof props.instances,
                })
            }
        }

        const remaining = Array.from(resultMap.entries()).sort((a, b) => a[0].localeCompare(b[0]))
        for (const [key, instances] of remaining) {
            result.push({ id: getOrCreateGroupStableId(key), key, instances })
        }
    } else {
        const remaining = Array.from(resultMap.entries())
        if (state.value.group === 'Game version') {
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
    if (state.value.group !== 'Group') return null
    const instances = filteredResults.value.get(FAVORITES_GROUP_ID)
    if (!instances || instances.length === 0) return null
    return {
        id: getOrCreateGroupStableId(FAVORITES_GROUP_ID),
        key: FAVORITES_GROUP_ID,
        instances,
    }
})

const draggableSections = shallowRef<DisplaySection[]>([])

const hideUngroupedHeader = computed(() => {
    if (grouping.value !== 'Group') return false
    return !draggableSections.value.some((s) => s.key !== UNGROUPED_GROUP_KEY)
})

const flatVisibleInstanceIds = computed(() => {
    const ids: string[] = []
    if (pinnedSection.value && !isSectionCollapsed(pinnedSection.value.key)) {
        for (const inst of pinnedSection.value.instances) {
            ids.push(inst.id)
        }
    }
    for (const section of draggableSections.value) {
        if (isSectionCollapsed(section.key)) continue
        for (const inst of section.instances) {
            ids.push(inst.id)
        }
    }
    return [...new Set(ids)]
})

watch(
    displaySections,
    (sections) => {
        draggableSections.value = sections
    },
    { immediate: true },
)

const sortDirectionLabels = {
    Name: { asc: messages.ascAlphabetical, desc: messages.descAlphabetical },
    'Game version': { asc: messages.ascVersion, desc: messages.descVersion },
    'Last played': { asc: messages.ascRecency, desc: messages.descRecency },
    'Date created': { asc: messages.ascDate, desc: messages.descDate },
    'Date modified': { asc: messages.ascDate, desc: messages.descDate },
}

const sortDirectionLabel = computed(() => {
    const labels = sortDirectionLabels[state.value.sortBy] ?? sortDirectionLabels['Name']
    return formatMessage(isSortAscending.value ? labels.asc : labels.desc)
})

async function deleteInstance() {
    if (currentDeleteInstance.value) {
        await remove(currentDeleteInstance.value.id)
    }
}

async function duplicateInstance(p) {
    await install_duplicate_instance(p).catch(handleError)
}

function getInstanceProxy(instanceId) {
    const instanceData = props.instances.find((i) => i.id === instanceId)
    if (!instanceData) return null
    return {
        instance: instanceData,
        playing:
            runningInstanceIds.value.has(instanceId) || startingInstanceIds.value.has(instanceId),
        play: async (_e, context) => {
            if (
                runningInstanceIds.value.has(instanceId) ||
                startingInstanceIds.value.has(instanceId)
            )
                return
            startingInstanceIds.value = new Set(startingInstanceIds.value).add(instanceId)
            try {
                await run(instanceId)
                setInstanceRunning(instanceId, true)
            } finally {
                const next = new Set(startingInstanceIds.value)
                next.delete(instanceId)
                startingInstanceIds.value = next
                trackEvent('InstanceStart', {
                    loader: instanceData.loader,
                    game_version: instanceData.game_version,
                    source: context,
                })
            }
        },
        stop: async (_e, context) => {
            if (!runningInstanceIds.value.has(instanceId)) return
            await kill(instanceId).finally(() => {
                setInstanceRunning(instanceId, false)
                trackEvent('InstanceStop', {
                    loader: instanceData.loader,
                    game_version: instanceData.game_version,
                    source: context,
                })
            })
        },
        seeInstance: () => router.push(`/instance/${encodeURIComponent(instanceId)}`),
        openFolder: () => showInstanceInFolder(instanceId),
        addContent: () =>
            router.push({
                path: `/browse/${instanceData.loader === 'vanilla' ? 'datapack' : 'mod'}`,
                query: { i: instanceId },
            }),
    }
}

const handleRightClick = (event, instanceId, sectionKey) => {
    const item = getInstanceProxy(instanceId)
    if (!item) return
    currentContextSectionKey.value = sectionKey
    const isInCustomGroup =
        grouping.value === 'Group' &&
        sectionKey !== UNGROUPED_GROUP_KEY &&
        sectionKey !== FAVORITES_GROUP_ID
    const baseOptions = [
        {
            section: 'pin',
            name: item.instance.groups?.includes(FAVORITES_GROUP_ID)
                ? 'remove_from_pinned'
                : 'add_to_pinned',
        },
        { type: 'divider' },
        { name: 'add_content', section: 'manage', shown: !item.instance.link },
        { name: 'edit', section: 'manage' },
        { name: 'duplicate', section: 'manage' },
        { section: 'pin', name: item.instance.pinned_at ? 'unpin' : 'pin' },
        { name: 'open', section: 'navigate' },
        { name: 'copy', section: 'navigate' },
        ...(isInCustomGroup
            ? [{ name: 'remove_from_group' }, { type: 'divider' }]
            : [{ type: 'divider' }]),
        {
            name: 'delete',
            section: 'danger',
            color: 'danger',
        },
    ]

    instanceOptions.value.showMenu(
        event,
        item,
        item.playing
            ? [
                  {
                      name: 'stop',
                      section: 'primary',
                      color: 'danger',
                  },
                  ...baseOptions,
              ]
            : [
                  {
                      name: 'play',
                      section: 'primary',
                      color: 'primary',
                  },
                  ...baseOptions,
              ],
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

const handleOptionsClick = async (args) => {
    switch (args.option) {
        case 'play':
            if ((await get_by_instance_id(args.item.instance.id).catch(handleError))?.length) break
            args.item.play(null, 'InstanceGridContextMenu')
            break
        case 'stop':
            if (!(await get_by_instance_id(args.item.instance.id).catch(handleError))?.length) break
            args.item.stop(null, 'InstanceGridContextMenu')
            break
        case 'add_content':
            await args.item.addContent()
            break
        case 'edit':
            await args.item.seeInstance()
            break
        case 'duplicate':
            if (args.item.instance.install_stage == 'installed')
                await duplicateInstance(args.item.instance.id)
            break
        case 'pin':
            await set_pinned(args.item.instance.id, true).catch(handleError)
            break
        case 'unpin':
            await set_pinned(args.item.instance.id, false).catch(handleError)
            break
        case 'open':
            await args.item.openFolder()
            break
        case 'copy':
            await navigator.clipboard.writeText(args.item.instance.id)
            break
        case 'add_to_pinned':
            await updateMemberships([
                {
                    instance_id: args.item.instance.id,
                    add_group_ids: [FAVORITES_GROUP_ID],
                    remove_group_ids: [],
                },
            ]).catch(handleError)
            break
        case 'remove_from_pinned':
            await updateMemberships([
                {
                    instance_id: args.item.instance.id,
                    add_group_ids: [],
                    remove_group_ids: [FAVORITES_GROUP_ID],
                },
            ]).catch(handleError)
            break
        case 'remove_from_group':
            await updateMemberships([
                {
                    instance_id: args.item.instance.id,
                    add_group_ids: [],
                    remove_group_ids: [currentContextSectionKey.value],
                },
            ]).catch(handleError)
            break
        case 'delete':
            currentDeleteInstance.value = args.item.instance
            confirmModal.value.show()
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
const batchEditModal = ref(null)

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

let longPressTimer = null
let longPressTriggered = false

function startLongPress(instanceId) {
    longPressTriggered = false
    longPressTimer = setTimeout(() => {
        longPressTriggered = true
        if (!selectMode.value) {
            selectMode.value = true
        }
        toggleInstanceSelection(instanceId)
    }, 500)
}

function cancelLongPress() {
    if (longPressTimer) {
        clearTimeout(longPressTimer)
        longPressTimer = null
    }
}

function handleCardClick(instanceId, event) {
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

function handleToggleInstance(instanceId, shiftKey) {
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
            for (const id of range) {
                newSet.delete(id)
            }
        } else {
            for (const id of range) {
                newSet.add(id)
            }
        }

        selectedInstanceIds.value = newSet
        if (newSet.size === 0) selectMode.value = false
        anchorInstanceId.value = null
        return
    }

    toggleInstanceSelection(instanceId)
    anchorInstanceId.value = selectedInstanceIds.value.has(instanceId) ? instanceId : null
}

function toggleInstanceSelection(instanceId) {
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

function handleCheckboxClick(instanceId, event) {
    if (!selectMode.value) {
        selectMode.value = true
    }
    const shiftKey = event?.shiftKey ?? false
    handleToggleInstance(instanceId, shiftKey)
}

const batchDeleteConfirmModal = ref(null)
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

function onGroupReorder() {
    if (grouping.value !== 'Group') return
    const groupIds = draggableSections.value
        .map((s) => s.key)
        .filter((key) => key !== UNGROUPED_GROUP_KEY)
    reorderGroups(groupIds).catch(handleError)
}

type InstanceDragData = { instanceId: string; fromGroup: string }
type InstanceGroupDropData = { groupId: string }

async function handleInstanceDragEnd(event: {
    operation: { source?: { data?: unknown }; target?: { data?: unknown } }
}) {
    const sourceData = event.operation.source?.data as InstanceDragData | undefined
    const targetData = event.operation.target?.data as InstanceGroupDropData | undefined
    if (!sourceData || !targetData) return
    if (sourceData.fromGroup === targetData.groupId) return

    const instance = props.instances.find((i: GameInstance) => i.id === sourceData.instanceId)
    if (!instance) return

    await updateMemberships([
        {
            instance_id: sourceData.instanceId,
            add_group_ids: targetData.groupId === UNGROUPED_GROUP_KEY ? [] : [targetData.groupId],
            remove_group_ids:
                sourceData.fromGroup === UNGROUPED_GROUP_KEY ? [] : [sourceData.fromGroup],
        },
    ]).catch(handleError)
}
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
                v-model="state.sortBy"
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
                v-model="state.group"
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
            <PopoutMenu :tooltip="formatMessage(messages.view)" placement="bottom-end">
                <Button circular icon-only :aria-label="formatMessage(messages.view)"
                    ><component :is="currentDisplayMode?.icon" />
                </Button>
                <template #menu>
                    <div class="flex w-44 flex-col gap-1 p-1">
                        <Button
                            v-for="option in displayModeOptions"
                            :key="option.id"
                            :type="displayMode === option.id ? 'base' : 'quiet'"
                            class="flex w-full items-center gap-2 !justify-start text-left"
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
        <DragDropProvider @drag-end="handleInstanceDragEnd">
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
            <Draggable
                v-model="draggableSections"
                :group="grouping === 'Group' ? 'groups' : undefined"
                :disabled="grouping !== 'Group'"
                item-key="id"
                :animation="250"
                :swap-threshold="0.75"
                :invert-swap="true"
                :force-fallback="true"
                :fallback-on-body="true"
                :fallback-tolerance="4"
                filter=".instance-group-reorder-ignore, input, textarea"
                handle=".group-drag-handle"
                :prevent-on-filter="false"
                ghost-class="opacity-60"
                chosen-class="shadow-lg"
                drag-class="opacity-80"
                @change="onGroupReorder"
            >
                <template #item="{ element: instanceSection }">
                    <div :key="instanceSection.id" class="min-w-0 w-full">
                        <InstanceGroup
                            :section-key="instanceSection.key"
                            :group-name="
                                groupNameMap.get(instanceSection.key) || instanceSection.key
                            "
                            :instances="instanceSection.instances"
                            :running-instance-ids="runningInstanceIds"
                            :grouping="grouping"
                            :is-collapsed="isSectionCollapsed(instanceSection.key)"
                            :display-mode="displayMode"
                            :select-mode="selectMode"
                            :selected-instance-ids="selectedInstanceIds"
                            :hide-header="
                                (hideUngroupedHeader &&
                                    instanceSection.key === UNGROUPED_GROUP_KEY) ||
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
                </template>
            </Draggable>
        </DragDropProvider>
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
        <InstanceGroupModal
            ref="batchEditModal"
            :instance-ids="[...selectedInstanceIds]"
            @applied="onBatchEditApplied"
        />
        <InstanceGroupModal ref="newGroupModal" :instance-ids="[]" @applied="onBatchEditApplied" />
        <FloatingActionBar
            :shown="selectMode"
            :aria-label="formatMessage(messages.selectedCount, { count: selectedInstanceIds.size })"
            hide-when-modal-open
        >
            <div class="flex items-center gap-0.5">
                <span
                    class="px-3 py-2 text-base font-semibold text-[var(--color-text-primary)] tabular-nums"
                >
                    {{ formatMessage(messages.selectedCount, { count: selectedInstanceIds.size }) }}
                </span>
                <div class="mx-0.5 h-6 w-px bg-surface-5" />
                <Button
                    type="quiet"
                    class="!text-[var(--color-text-default)]"
                    :disabled="busy"
                    @click="clearLibraryInstanceSelection"
                    ><XIcon class="hidden cq-show-icon" />
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
        </FloatingActionBar>
        <InstanceGroupModal
            ref="addToExistingGroupModal"
            :instance-ids="[]"
            :existing-group-name="addToExistingGroupName"
            :existing-group-id="addToExistingGroupId"
            @applied="onBatchEditApplied"
        />
        <ContextMenu ref="instanceOptions" @option-clicked="handleOptionsClick">
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
        </ContextMenu>
        <ContextMenu ref="backgroundContextMenu" @option-clicked="handleBackgroundOption">
            <template #create_instance>
                <PlusIcon /> {{ formatMessage(messages.createInstance) }}
            </template>
        </ContextMenu>
    </div>
</template>
<style lang="scss" scoped></style>
