<script setup lang="ts">
import { useDroppable } from '@dnd-kit/vue'
import {
    ArrowDownIcon,
    ArrowUpIcon,
    DropdownIcon,
    EditIcon,
    PlusIcon,
    TrashIcon,
    XIcon,
} from '@modrinth/assets'
import {
    Button,
    Checkbox,
    commonMessages,
    defineMessages,
    IconButton,
    InlineEditableText,
    NewModal,
    TagItem,
    useVIntl,
} from '@modrinth/ui'
import { computed, nextTick, ref, watch } from 'vue'

import ContextMenu from '@/components/ui/ContextMenu.vue'
import Instance from '@/components/ui/Instance.vue'
import DraggableInstanceCard from '@/components/ui/library/DraggableInstanceCard.vue'
import { UNGROUPED_GROUP_KEY } from '@/composables/useGridGrouping'
import { FAVORITES_GROUP_ID } from '@/composables/useInstanceGroups'
import type { GameInstance } from '@/helpers/types'

const MAX_GROUP_NAME_LENGTH = 256

const { formatMessage } = useVIntl()

const messages = defineMessages({
    ungrouped: { id: 'app.instances.group.ungrouped', defaultMessage: 'No group' },
    pinned: { id: 'app.instances.group.pinned', defaultMessage: 'Pinned' },
    emptyGroup: {
        id: 'app.instances.group.empty',
        defaultMessage: 'Drag and drop to add instances.',
    },
    editGroupName: {
        id: 'app.instances.group.edit-name',
        defaultMessage: 'Edit group name',
    },
    deleteGroup: {
        id: 'app.instances.group.delete-group',
        defaultMessage: 'Delete group',
    },
    deleteGroupDescription: {
        id: 'app.instances.group.delete-description',
        defaultMessage: 'Instances in this group will be ungrouped.',
    },
    moveGroupUp: {
        id: 'app.instances.group.move-up',
        defaultMessage: 'Move group up',
    },
    moveGroupDown: {
        id: 'app.instances.group.move-down',
        defaultMessage: 'Move group down',
    },
    addToGroup: {
        id: 'app.instances.group.add-to-group',
        defaultMessage: 'Add to group',
    },
    newInstance: {
        id: 'app.instances.group.new-instance',
        defaultMessage: 'New instance',
    },
    collapseGroup: {
        id: 'app.instances.group.collapse',
        defaultMessage: 'Collapse group',
    },
    expandGroup: {
        id: 'app.instances.group.expand',
        defaultMessage: 'Expand group',
    },
    renameGroup: {
        id: 'app.instances.group.rename-group',
        defaultMessage: 'Rename group',
    },
    groupNameEmpty: {
        id: 'app.instances.group.name-empty',
        defaultMessage: 'Group names cannot be empty.',
    },
    groupNameTooLong: {
        id: 'app.instances.group.name-too-long',
        defaultMessage: 'Group names cannot be longer than {maxLength} characters.',
    },
})

const props = defineProps<{
    sectionKey: string
    groupName?: string
    instances: GameInstance[]
    runningInstanceIds: Set<string>
    grouping: string
    isCollapsed: boolean
    displayMode: string
    selectMode: boolean
    selectedInstanceIds: Set<string>
    isSortAscending?: boolean
    hideHeader?: boolean
    canMoveUp?: boolean
    canMoveDown?: boolean
    groupPendingNameEdit?: string | null
}>()

// Events are grouped only where the payload shapes genuinely match; the
// per-event names are documentation, not part of the type.
const emit = defineEmits<{
    (e: 'toggleCollapse' | 'deleteGroup' | 'addToGroup' | 'startLongPress', id: string): void
    (e: 'handleCheckboxClick' | 'handleCardClick', instanceId: string, event: MouseEvent): void
    (e: 'handleContextMenu', event: MouseEvent, instanceId: string, sectionKey: string): void
    (e: 'renameGroup', oldKey: string, newKey: string): void
    (e: 'cancelLongPress' | 'newInstance' | 'renameComplete'): void
    (e: 'moveGroup', groupKey: string, direction: -1 | 1): void
}>()

const groupOptions = ref<InstanceType<typeof ContextMenu>>()
const groupContainerRef = ref<HTMLElement>()
const groupNameInput = ref<InstanceType<typeof InlineEditableText>>()
const deletingGroup = ref(false)
const confirmDeleteGroupModal = ref<InstanceType<typeof NewModal>>()
const groupName = ref(props.groupName ?? props.sectionKey)

const isUngrouped = computed(() => props.sectionKey === UNGROUPED_GROUP_KEY)
const isFavorites = computed(() => props.sectionKey === FAVORITES_GROUP_ID)
const isCustomGroup = computed(
    () => props.grouping === 'Group' && !isUngrouped.value && !isFavorites.value,
)
const isGroupDragActive = computed(() => props.grouping === 'Group')
const effectiveCollapsed = computed(() => (props.hideHeader ? false : props.isCollapsed))

const { isDropTarget } = useDroppable({
    id: computed(() => `instance-group:${props.sectionKey}`),
    element: groupContainerRef,
    disabled: computed(() => props.hideHeader || !isGroupDragActive.value),
    data: computed(() => ({
        groupId: props.sectionKey,
    })),
})

watch(
    () => [props.sectionKey, props.groupName],
    () => {
        groupName.value = props.groupName ?? props.sectionKey
    },
)

watch(
    () => props.groupPendingNameEdit,
    async (pendingId) => {
        if (pendingId !== props.sectionKey) return

        await nextTick()
        groupContainerRef.value?.scrollIntoView({ block: 'nearest' })
        await groupNameInput.value?.startEditing()
        emit('renameComplete')
    },
    { immediate: true, flush: 'post' },
)

function validateGroupName(value: string) {
    const normalized = value.trim()
    if (normalized.length === 0) return false
    if (normalized.length > MAX_GROUP_NAME_LENGTH) return false
    return true
}

async function updateGroupName(value: string) {
    const newName = value.trim()
    if (!newName || newName === groupName.value) return false
    emit('renameGroup', props.sectionKey, newName)
    return true
}

function openGroupContextMenu(event: MouseEvent) {
    groupOptions.value?.showMenu(
        event,
        props.sectionKey,
        isCustomGroup.value
            ? [
                  { name: 'new_instance' },
                  { name: 'add_instances' },
                  { name: 'edit_name' },
                  { type: 'divider' },
                  { name: 'delete_group', color: 'danger' },
              ]
            : [{ name: 'new_instance' }],
    )
}

async function handleGroupOption({ option }: { option: string }) {
    if (option === 'new_instance') {
        emit('newInstance')
    } else if (option === 'add_instances') {
        emit('addToGroup', props.sectionKey)
    } else if (option === 'edit_name') {
        groupNameInput.value?.startEditing()
    } else if (option === 'delete_group') {
        requestGroupDeletion()
    }
}

function requestGroupDeletion() {
    if (isUngrouped.value || isFavorites.value) return

    if (props.instances.length > 0) {
        confirmDeleteGroupModal.value?.show()
    } else {
        void removeGroup()
    }
}

async function removeGroup() {
    if (deletingGroup.value) return

    deletingGroup.value = true
    emit('deleteGroup', props.sectionKey)
    setTimeout(() => {
        deletingGroup.value = false
        confirmDeleteGroupModal.value?.hide()
    }, 300)
}

function onCardClick(instanceId: string, event: MouseEvent) {
    emit('handleCardClick', instanceId, event)
}

function onStartLongPress(instanceId: string) {
    emit('startLongPress', instanceId)
}
</script>

<template>
    <div
        ref="groupContainerRef"
        class="instance-group group/instance-container relative select-none pb-3"
        @contextmenu.prevent.stop="openGroupContextMenu"
    >
        <div
            v-if="!hideHeader && isDropTarget && isGroupDragActive"
            class="pointer-events-none absolute -inset-2 inset-y-0 z-20 rounded-xl border-2 border-dashed border-brand opacity-40 bg-transparent transition-opacity duration-150"
        />
        <div
            v-if="!hideHeader && (grouping === 'Group' || sectionKey !== UNGROUPED_GROUP_KEY)"
            class="group/header flex h-10 w-full items-center gap-2 border-0 border-b border-solid border-b-surface-5"
        >
            <div
                class="flex min-w-0 cursor-pointer items-center gap-2"
                @click="emit('toggleCollapse', sectionKey)"
            >
                <button
                    class="flex shrink-0 items-center border-0 bg-transparent p-0 cursor-pointer"
                    type="button"
                    :aria-expanded="!effectiveCollapsed"
                    :aria-label="
                        formatMessage(
                            effectiveCollapsed ? messages.expandGroup : messages.collapseGroup,
                        )
                    "
                    @click.stop="emit('toggleCollapse', sectionKey)"
                >
                    <DropdownIcon
                        class="size-5 shrink-0 text-[var(--color-text-tertiary)] transition-transform duration-300"
                        :class="{ 'rotate-180': !effectiveCollapsed }"
                    />
                </button>
                <InlineEditableText
                    v-if="!isUngrouped && !isFavorites"
                    ref="groupNameInput"
                    v-model="groupName"
                    activation-mode="manual"
                    class="text-base font-semibold !h-10 text-[var(--color-text-default)] select-none"
                    max-width="24rem"
                    :max-length="MAX_GROUP_NAME_LENGTH"
                    :on-change="updateGroupName"
                    :validate="validateGroupName"
                />
                <span
                    v-else
                    class="text-base font-semibold text-[var(--color-text-default)] select-none"
                >
                    {{
                        isFavorites
                            ? formatMessage(messages.pinned)
                            : formatMessage(messages.ungrouped)
                    }}
                </span>
                <TagItem v-if="instances.length" class="shrink-0 border-surface-3 bg-surface-2">
                    {{ instances.length }}
                </TagItem>
            </div>
            <div class="min-w-0 flex-1" />
            <div
                v-if="isCustomGroup"
                class="flex shrink-0 items-center opacity-0 transition-opacity duration-250 group-hover/header:opacity-100 focus-within:opacity-100"
            >
                <IconButton
                    v-tooltip="formatMessage(messages.moveGroupUp)"
                    :label="formatMessage(messages.moveGroupUp)"
                    type="quiet"
                    size="sm"
                    :disabled="!canMoveUp"
                    @click.stop="emit('moveGroup', sectionKey, -1)"
                >
                    <ArrowUpIcon />
                </IconButton>
                <IconButton
                    v-tooltip="formatMessage(messages.moveGroupDown)"
                    :label="formatMessage(messages.moveGroupDown)"
                    type="quiet"
                    size="sm"
                    :disabled="!canMoveDown"
                    @click.stop="emit('moveGroup', sectionKey, 1)"
                >
                    <ArrowDownIcon />
                </IconButton>
                <IconButton
                    v-tooltip="formatMessage(messages.editGroupName)"
                    :label="formatMessage(messages.editGroupName)"
                    type="quiet"
                    size="sm"
                    @click.stop="groupNameInput?.startEditing()"
                >
                    <EditIcon />
                </IconButton>
                <IconButton
                    v-tooltip="formatMessage(messages.addToGroup)"
                    :label="formatMessage(messages.addToGroup)"
                    type="quiet"
                    size="sm"
                    @click.stop="emit('addToGroup', sectionKey)"
                >
                    <PlusIcon />
                </IconButton>
                <IconButton
                    v-tooltip="formatMessage(messages.deleteGroup)"
                    :label="formatMessage(messages.deleteGroup)"
                    type="quiet"
                    size="sm"
                    :disabled="deletingGroup"
                    @click.stop="handleGroupOption({ option: 'delete_group' })"
                >
                    <TrashIcon />
                </IconButton>
            </div>
        </div>
        <div v-if="!effectiveCollapsed" class="mt-2.5">
            <section
                class="grid grid-cols-[repeat(auto-fill,minmax(16rem,1fr))] w-full gap-3 mr-auto scroll-smooth overflow-y-auto"
                :class="{
                    'grid-cols-[repeat(auto-fill,minmax(13rem,1fr))] gap-4':
                        displayMode === 'cards',
                }"
            >
                <template v-for="instance in instances" :key="instance.id">
                    <DraggableInstanceCard
                        v-if="isGroupDragActive"
                        :instance="instance"
                        :instance-group-id="sectionKey"
                        :disabled="selectMode"
                        @card-click="(event: MouseEvent) => onCardClick(instance.id, event)"
                        @contextmenu="
                            (event: MouseEvent) =>
                                emit('handleContextMenu', event, instance.id, props.sectionKey)
                        "
                        @start-long-press="onStartLongPress(instance.id)"
                        @cancel-long-press="emit('cancelLongPress')"
                    >
                        <template #default="{ isDragging }">
                            <div
                                :class="{
                                    'pointer-events-none': selectMode,
                                    'relative cursor-pointer select-none rounded-lg transition-all hover:brightness-90 active:scale-[0.98]': true,
                                }"
                                @click.capture="
                                    (event: MouseEvent) => {
                                        if (event.shiftKey && !selectMode) {
                                            event.stopPropagation()
                                            emit('handleCardClick', instance.id, event)
                                        }
                                    }
                                "
                            >
                                <Instance
                                    :instance="instance"
                                    :playing="runningInstanceIds.has(instance.id)"
                                    :disabled="selectMode || isDragging"
                                    :variant="displayMode === 'cards' ? 'library' : 'standard'"
                                    :class="{
                                        'opacity-50':
                                            selectMode && !selectedInstanceIds.has(instance.id),
                                    }"
                                />
                            </div>
                        </template>
                        <template #overlay>
                            <div
                                class="absolute right-2 bottom-2 z-10 transition-opacity"
                                :class="
                                    selectMode && selectedInstanceIds.has(instance.id)
                                        ? ''
                                        : 'opacity-0 group-hover:opacity-100'
                                "
                                @click.stop="emit('handleCheckboxClick', instance.id, $event)"
                            >
                                <Checkbox :model-value="selectedInstanceIds.has(instance.id)" />
                            </div>
                            <div
                                v-if="!selectMode"
                                class="absolute right-2 top-2 opacity-0 group-hover:opacity-100 transition-opacity"
                                @click.stop="
                                    emit(
                                        'handleContextMenu',
                                        $event as MouseEvent,
                                        instance.id,
                                        props.sectionKey,
                                    )
                                "
                            >
                                <Button type="quiet" size="2xs" circular icon-only
                                    ><slot name="moreIcon" />
                                </Button>
                            </div>
                        </template>
                    </DraggableInstanceCard>

                    <div v-else class="group relative">
                        <div
                            class="relative cursor-pointer select-none rounded-lg transition-all hover:brightness-90 active:scale-[0.98]"
                            @click.capture="
                                (event: MouseEvent) => {
                                    if (event.shiftKey && !selectMode) {
                                        event.stopPropagation()
                                        emit('handleCardClick', instance.id, event)
                                    }
                                }
                            "
                            @click="emit('handleCardClick', instance.id, $event as MouseEvent)"
                            @mousedown="!selectMode && emit('startLongPress', instance.id)"
                            @mouseup="emit('cancelLongPress')"
                            @mouseleave="emit('cancelLongPress')"
                            @touchstart="!selectMode && emit('startLongPress', instance.id)"
                            @touchend="emit('cancelLongPress')"
                            @touchcancel="emit('cancelLongPress')"
                        >
                            <div :class="{ 'pointer-events-none': selectMode }">
                                <Instance
                                    :instance="instance"
                                    :playing="runningInstanceIds.has(instance.id)"
                                    :disabled="selectMode"
                                    :variant="displayMode === 'cards' ? 'library' : 'standard'"
                                    :class="{
                                        'opacity-50':
                                            selectMode && !selectedInstanceIds.has(instance.id),
                                    }"
                                    @contextmenu.prevent.stop="
                                        (event: MouseEvent) =>
                                            emit(
                                                'handleContextMenu',
                                                event,
                                                instance.id,
                                                props.sectionKey,
                                            )
                                    "
                                />
                            </div>
                        </div>
                        <div
                            class="absolute right-2 bottom-2 z-10 transition-opacity"
                            :class="
                                selectMode && selectedInstanceIds.has(instance.id)
                                    ? ''
                                    : 'opacity-0 group-hover:opacity-100'
                            "
                            @click.stop="emit('handleCheckboxClick', instance.id, $event)"
                        >
                            <Checkbox :model-value="selectedInstanceIds.has(instance.id)" />
                        </div>
                        <div
                            v-if="!selectMode"
                            class="absolute right-2 top-2 opacity-0 group-hover:opacity-100 transition-opacity"
                            @click.stop="
                                emit(
                                    'handleContextMenu',
                                    $event as MouseEvent,
                                    instance.id,
                                    props.sectionKey,
                                )
                            "
                        >
                            <Button type="quiet" size="2xs" circular icon-only
                                ><slot name="moreIcon" />
                            </Button>
                        </div>
                    </div>
                </template>
                <p
                    v-if="instances.length === 0"
                    class="col-span-full m-0 pt-1 pl-0.5 text-base font-base text-[var(--color-text-tertiary)] opacity-80"
                >
                    {{ formatMessage(messages.emptyGroup) }}
                </p>
            </section>
        </div>
    </div>

    <ContextMenu ref="groupOptions" @option-clicked="handleGroupOption">
        <template #new_instance>
            <PlusIcon />
            {{ formatMessage(messages.newInstance) }}
        </template>
        <template #add_instances>
            <PlusIcon />
            {{ formatMessage(messages.addToGroup) }}
        </template>
        <template #edit_name>
            <EditIcon />
            {{ formatMessage(messages.editGroupName) }}
        </template>
        <template #delete_group>
            <TrashIcon />
            {{ formatMessage(messages.deleteGroup) }}
        </template>
    </ContextMenu>

    <NewModal
        ref="confirmDeleteGroupModal"
        :header="formatMessage(messages.deleteGroup)"
        fade="danger"
        width="500px"
    >
        <p class="m-0 text-base text-[var(--color-text-default)]">
            {{ formatMessage(messages.deleteGroupDescription) }}
        </p>

        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="confirmDeleteGroupModal?.hide()">
                    <XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button type="colored" color="red" :disabled="deletingGroup" @click="removeGroup">
                    <TrashIcon />
                    {{ formatMessage(messages.deleteGroup) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>
