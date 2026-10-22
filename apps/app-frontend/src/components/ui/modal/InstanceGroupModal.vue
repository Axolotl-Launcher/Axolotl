<template>
    <NewModal
        ref="modal"
        no-padding
        scrollable
        actions-divider
        max-width="560px"
        width="560px"
        :disable-close="creatingGroup"
        :on-hide="onHide"
    >
        <template #title>
            <span class="text-2xl font-semibold text-[var(--color-text-primary)]">
                {{
                    existingGroupName
                        ? formatMessage(messages.addToGroupTitle, { groupName: existingGroupName })
                        : formatMessage(messages.header)
                }}
            </span>
        </template>

        <template v-if="!existingGroupName">
            <div class="flex flex-col gap-2.5 p-6">
                <label for="new-group-name" class="font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.groupName) }}
                </label>
                <StyledInput
                    id="new-group-name"
                    ref="groupNameInput"
                    v-model="newGroupName"
                    :placeholder="formatMessage(messages.groupNamePlaceholder)"
                    :maxlength="32"
                    :disabled="loadingInstances || creatingGroup"
                    @click="groupNameInput?.select()"
                />
            </div>

            <div class="h-px bg-divider" />
        </template>

        <div class="flex h-[400px] flex-col gap-3 overflow-y-auto bg-surface-2 py-4">
            <div class="px-6">
                <StyledInput
                    v-model="newGroupSearch"
                    :icon="SearchIcon"
                    :placeholder="formatMessage(messages.searchInstance)"
                    class="w-full"
                    :disabled="loadingInstances || creatingGroup"
                />
            </div>

            <div
                v-if="loadingInstances"
                class="px-6 text-[var(--color-text-tertiary)]"
                role="status"
            >
                {{ formatMessage(messages.loadingInstances) }}
            </div>
            <Admonition v-else-if="loadError" type="critical" class="mx-6">
                {{ formatMessage(messages.loadFailed) }}
                <Button @click="loadInstances(initialInstanceIds)">{{
                    formatMessage(messages.retry)
                }}</Button>
            </Admonition>
            <div
                v-else-if="newGroupInstances.length === 0"
                class="flex items-center justify-center py-12 text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.noInstancesFound) }}
            </div>
            <div v-else class="flex flex-col gap-1">
                <div
                    v-for="instance in newGroupInstances"
                    :key="instance.id"
                    class="flex items-center justify-between gap-4 px-6 py-1.5 hover:bg-surface-3"
                    :class="{ 'opacity-60': selectedNewGroupInstanceIds.has(instance.id) }"
                >
                    <div class="flex min-w-0 items-center gap-2.5">
                        <InstanceIcon
                            :icon-path="instance.icon_path"
                            :instance-id="instance.id"
                            :loader="instance.loader"
                            class="!size-[2rem] !rounded-md"
                        />
                        <div class="flex min-w-0 items-center gap-2">
                            <span class="truncate font-semibold text-[var(--color-text-primary)]">{{
                                instance.name
                            }}</span>
                            <TagItem
                                v-if="instance.groups && instance.groups.length > 0"
                                class="shrink-0"
                            >
                                {{ getGroupName(instance.groups[0]) }}
                            </TagItem>
                        </div>
                    </div>
                    <Button
                        :type="selectedNewGroupInstanceIds.has(instance.id) ? 'outlined' : 'base'"
                        :disabled="creatingGroup"
                        @click="toggleNewGroupInstance(instance.id)"
                    >
                        <CheckIcon v-if="selectedNewGroupInstanceIds.has(instance.id)" />
                        {{
                            formatMessage(
                                selectedNewGroupInstanceIds.has(instance.id)
                                    ? messages.added
                                    : messages.add,
                            )
                        }}
                    </Button>
                </div>
            </div>
        </div>
        <Admonition v-if="submitError" type="critical" class="mx-6 my-3" role="alert">
            {{ formatMessage(submitError) }}
        </Admonition>

        <template #actions>
            <div class="flex items-center justify-end gap-2">
                <Button type="outlined" :disabled="creatingGroup" @click="modal?.hide()">
                    <XIcon />
                    {{ formatMessage(messages.cancel) }}
                </Button>
                <Button
                    type="colored"
                    color="brand"
                    :disabled="!canSubmit"
                    :loading="creatingGroup"
                    @click="submitGroups"
                >
                    <CheckIcon v-if="existingGroupName" />
                    <PlusIcon v-else />
                    {{
                        formatMessage(
                            existingGroupName ? messages.saveChanges : messages.createGroup,
                        )
                    }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>

<script setup lang="ts">
import { CheckIcon, PlusIcon, SearchIcon, XIcon } from '@modrinth/assets'
import {
    Admonition,
    Button,
    defineMessages,
    injectNotificationManager,
    NewModal,
    StyledInput,
    TagItem,
    useVIntl,
} from '@modrinth/ui'
import { computed, nextTick, ref } from 'vue'

import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import { FAVORITES_GROUP_ID } from '@/composables/useInstanceGroups'
import { list } from '@/helpers/instance'
import {
    create_group,
    type InstanceGroupDefinition,
    list_groups,
    rename_group,
    update_group_memberships,
} from '@/helpers/instance-groups'
import type { GameInstance } from '@/helpers/types'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()

const props = defineProps<{
    instanceIds: string[]
    existingGroupName?: string
    existingGroupId?: string
}>()

const emit = defineEmits<{
    (e: 'applied'): void
}>()

const messages = defineMessages({
    header: {
        id: 'app.instances.batch-edit-groups.header',
        defaultMessage: 'Create group',
    },
    addToGroupTitle: {
        id: 'app.instances.batch-edit-groups.add-to-group-title',
        defaultMessage: 'Add instances to "{groupName}"',
    },
    groupName: {
        id: 'app.instances.batch-edit-groups.group-name',
        defaultMessage: 'Group name',
    },
    groupNamePlaceholder: {
        id: 'app.instances.batch-edit-groups.group-name-placeholder',
        defaultMessage: 'Enter group name',
    },
    searchInstance: {
        id: 'app.instances.batch-edit-groups.search-instance',
        defaultMessage: 'Search instances',
    },
    noInstancesFound: {
        id: 'app.instances.batch-edit-groups.no-instances-found',
        defaultMessage: 'No instances found',
    },
    add: {
        id: 'app.instances.batch-edit-groups.add',
        defaultMessage: 'Add',
    },
    added: {
        id: 'app.instances.batch-edit-groups.added',
        defaultMessage: 'Added',
    },
    cancel: {
        id: 'app.instances.batch-edit-groups.cancel',
        defaultMessage: 'Cancel',
    },
    createGroup: {
        id: 'app.instances.batch-edit-groups.create',
        defaultMessage: 'Create group',
    },
    pinnedGroupName: {
        id: 'app.instances.group.pinned',
        defaultMessage: 'Pinned',
    },
    saveChanges: {
        id: 'app.instances.batch-edit-groups.save-changes',
        defaultMessage: 'Save',
    },
    loadingInstances: {
        id: 'app.instances.batch-edit-groups.loading',
        defaultMessage: 'Loading instances…',
    },
    loadFailed: {
        id: 'app.instances.batch-edit-groups.load-failed',
        defaultMessage: 'Could not load instances. Try again.',
    },
    retry: { id: 'app.instances.batch-edit-groups.retry', defaultMessage: 'Retry' },
    saveFailed: {
        id: 'app.instances.batch-edit-groups.save-failed',
        defaultMessage: 'Could not save the group. Your changes are kept; try again.',
    },
    membershipFailed: {
        id: 'app.instances.batch-edit-groups.membership-failed',
        defaultMessage:
            'The group was created, but its instances could not be saved. Retry to finish saving your changes.',
    },
})

const modal = ref<InstanceType<typeof NewModal>>()
const groupNameInput = ref<InstanceType<typeof StyledInput>>()
const newGroupName = ref('')
const newGroupSearch = ref('')
const creatingGroup = ref(false)
const submitted = ref(false)
const loadingInstances = ref(false)
const loadError = ref(false)
const submitError = ref<typeof messages.saveFailed | typeof messages.membershipFailed | null>(null)
const createdGroup = ref<InstanceGroupDefinition | null>(null)
let loadGeneration = 0
let initialInstanceIds: string[] = []
const allInstances = ref<GameInstance[]>([])
const groupNameMap = ref(new Map<string, string>())
const selectedNewGroupInstanceIds = ref(new Set<string>())

const canCreateGroup = computed(() => newGroupName.value.trim().length > 0)

const canSaveGroups = computed(() => {
    if (!props.existingGroupName) return false
    const groupId = props.existingGroupId
    if (!groupId) return false
    return allInstances.value.some((instance) => {
        const isSelected = selectedNewGroupInstanceIds.value.has(instance.id)
        const hasGroup = (instance.groups || []).includes(groupId)
        return isSelected !== hasGroup
    })
})

const canSubmit = computed(
    () =>
        !loadingInstances.value &&
        !loadError.value &&
        !creatingGroup.value &&
        !submitted.value &&
        (props.existingGroupName ? canSaveGroups.value : canCreateGroup.value),
)

const newGroupInstances = computed(() => {
    const search = newGroupSearch.value.toLowerCase()
    return allInstances.value.filter((instance) => {
        if (!search) return true
        return instance.name.toLowerCase().includes(search)
    })
})

function getGroupName(groupId: string) {
    if (groupId === FAVORITES_GROUP_ID) {
        return formatMessage(messages.pinnedGroupName)
    }
    return groupNameMap.value.get(groupId) || groupId
}

function show(ids?: string[]) {
    if (creatingGroup.value) return
    newGroupSearch.value = ''
    newGroupName.value = ''
    submitted.value = false
    submitError.value = null
    createdGroup.value = null
    allInstances.value = []
    selectedNewGroupInstanceIds.value = new Set()
    initialInstanceIds = [...(ids ?? props.instanceIds)]
    void loadInstances(initialInstanceIds)
    modal.value?.show()
}

function onHide() {
    loadGeneration++
    loadingInstances.value = false
}

async function loadInstances(ids: string[]) {
    const generation = ++loadGeneration
    loadingInstances.value = true
    loadError.value = false
    try {
        const [instances, groups] = await Promise.all([list(), list_groups()])
        if (generation !== loadGeneration) return
        allInstances.value = instances as GameInstance[]
        const map = new Map<string, string>()
        for (const g of groups) {
            map.set(g.id, g.name)
        }
        groupNameMap.value = map

        if (!props.existingGroupName) {
            const customGroups = groups.filter((g: { id: string }) => g.id !== 'group:favorites')
            let groupNumber = customGroups.length + 1
            const existingNames = new Set(groups.map((g: { name: string }) => g.name.toLowerCase()))
            while (existingNames.has(`group ${groupNumber}`)) {
                groupNumber++
            }
            newGroupName.value = `Group ${groupNumber}`
        }

        if (props.existingGroupName) {
            const groupId = props.existingGroupId
            selectedNewGroupInstanceIds.value = new Set(
                allInstances.value
                    .filter((i) => (i.groups || []).includes(groupId || ''))
                    .map((i) => i.id),
            )
        } else {
            selectedNewGroupInstanceIds.value = new Set(ids ?? props.instanceIds)
        }
    } catch (error) {
        if (generation !== loadGeneration) return
        loadError.value = true
        handleError(error)
    } finally {
        if (generation === loadGeneration) loadingInstances.value = false
    }
}

function toggleNewGroupInstance(instanceId: string) {
    if (creatingGroup.value || loadingInstances.value || loadError.value) return
    const next = new Set(selectedNewGroupInstanceIds.value)
    if (next.has(instanceId)) {
        next.delete(instanceId)
    } else {
        next.add(instanceId)
    }
    selectedNewGroupInstanceIds.value = next
}

async function submitGroups() {
    if (!canSubmit.value) return
    const name = newGroupName.value.trim()
    const selectedIds = new Set(selectedNewGroupInstanceIds.value)
    creatingGroup.value = true
    submitError.value = null
    try {
        let groupId = props.existingGroupId
        if (!props.existingGroupName) {
            if (!createdGroup.value) createdGroup.value = await create_group(name)
            else if (createdGroup.value.name !== name) {
                createdGroup.value = await rename_group(createdGroup.value.id, name)
            }
            groupId = createdGroup.value.id
        }
        if (!groupId) return
        const updates = allInstances.value
            .filter(
                (instance) =>
                    selectedIds.has(instance.id) !== (instance.groups || []).includes(groupId),
            )
            .map((instance) => ({
                instance_id: instance.id,
                add_group_ids: selectedIds.has(instance.id) ? [groupId] : [],
                remove_group_ids: selectedIds.has(instance.id) ? [] : [groupId],
            }))
        if (updates.length) await update_group_memberships(updates)
        submitted.value = true
        creatingGroup.value = false
        await nextTick()
        modal.value?.hide()
        emit('applied')
    } catch (error) {
        submitError.value = createdGroup.value ? messages.membershipFailed : messages.saveFailed
        handleError(error)
    } finally {
        creatingGroup.value = false
    }
}

defineExpose({ show })
</script>
