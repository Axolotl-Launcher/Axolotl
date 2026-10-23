<script setup lang="ts">
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import TagItem from '@modrinth/ui/src/components/base/TagItem.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, nextTick, ref } from 'vue'

import { usePreview } from '../../../lib/state'
import type { GameInstance } from '../../../lib/types'
import PreviewModal from '../../../overlays/PreviewModal.vue'
import InstanceIcon from '../../../shell/InstanceIcon.vue'
import { useLibraryGroups } from '../groups'
import { messages } from '../messages'
import { FAVORITES_GROUP_ID, type InstanceGroupDefinition } from '../state'

/**
 * Port of `components/ui/modal/InstanceGroupModal.vue`. `list()` and
 * `list_groups()` read the preview's instance state; like the app's IPC calls
 * they resolve on the next tick, so the loading row only flashes.
 */
const { formatMessage } = useVIntl()
const preview = usePreview()
const { library, libraryGroups } = useLibraryGroups()

const props = defineProps<{
    instanceIds: string[]
    existingGroupName?: string
    existingGroupId?: string
}>()

const emit = defineEmits<{
    (e: 'applied'): void
}>()

const modal = ref<InstanceType<typeof PreviewModal>>()
const groupNameInput = ref<InstanceType<typeof StyledInput>>()
const newGroupName = ref('')
const newGroupSearch = ref('')
const creatingGroup = ref(false)
const submitted = ref(false)
const loadingInstances = ref(false)
const loadError = ref(false)
const submitError = ref<
    typeof messages.groupModalSaveFailed | typeof messages.groupModalMembershipFailed | null
>(null)
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
        return formatMessage(messages.pinned)
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
        await new Promise<void>((resolve) => preview.clock.after(16, resolve))
        if (generation !== loadGeneration) return
        const instances = preview.instances.instances.value.map((instance) => ({
            ...instance,
            groups: [...instance.groups],
        }))
        const groups = libraryGroups.value
        allInstances.value = instances
        const map = new Map<string, string>()
        for (const group of groups) {
            map.set(group.id, group.name)
        }
        groupNameMap.value = map

        if (!props.existingGroupName) {
            const customGroups = groups.filter((group) => group.id !== 'group:favorites')
            let groupNumber = customGroups.length + 1
            const existingNames = new Set(groups.map((group) => group.name.toLowerCase()))
            while (existingNames.has(`group ${groupNumber}`)) {
                groupNumber++
            }
            newGroupName.value = `Group ${groupNumber}`
        }

        if (props.existingGroupName) {
            const groupId = props.existingGroupId
            selectedNewGroupInstanceIds.value = new Set(
                allInstances.value
                    .filter((instance) => (instance.groups || []).includes(groupId || ''))
                    .map((instance) => instance.id),
            )
        } else {
            selectedNewGroupInstanceIds.value = new Set(ids ?? props.instanceIds)
        }
    } catch {
        if (generation !== loadGeneration) return
        loadError.value = true
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
            if (!createdGroup.value) createdGroup.value = library.createGroup(name)
            else if (createdGroup.value.name !== name) {
                createdGroup.value = library.renameGroup(createdGroup.value.id, name)
            }
            groupId = createdGroup.value.id
        }
        if (!groupId) return
        const targetGroupId = groupId
        const updates = allInstances.value
            .filter(
                (instance) =>
                    selectedIds.has(instance.id) !==
                    (instance.groups || []).includes(targetGroupId),
            )
            .map((instance) => ({
                instance_id: instance.id,
                add_group_ids: selectedIds.has(instance.id) ? [targetGroupId] : [],
                remove_group_ids: selectedIds.has(instance.id) ? [] : [targetGroupId],
            }))
        if (updates.length) library.updateMemberships(updates)
        submitted.value = true
        creatingGroup.value = false
        await nextTick()
        modal.value?.hide()
        emit('applied')
    } catch {
        submitError.value = createdGroup.value
            ? messages.groupModalMembershipFailed
            : messages.groupModalSaveFailed
    } finally {
        creatingGroup.value = false
    }
}

defineExpose({ show })
</script>

<template>
    <PreviewModal
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
                        ? formatMessage(messages.groupModalAddTitle, {
                              groupName: existingGroupName,
                          })
                        : formatMessage(messages.groupModalHeader)
                }}
            </span>
        </template>

        <template v-if="!existingGroupName">
            <div class="flex flex-col gap-2.5 p-6">
                <label for="new-group-name" class="font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.groupModalGroupName) }}
                </label>
                <StyledInput
                    id="new-group-name"
                    ref="groupNameInput"
                    v-model="newGroupName"
                    :placeholder="formatMessage(messages.groupModalGroupNamePlaceholder)"
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
                    :placeholder="formatMessage(messages.groupModalSearch)"
                    class="w-full"
                    :disabled="loadingInstances || creatingGroup"
                />
            </div>

            <div
                v-if="loadingInstances"
                class="px-6 text-[var(--color-text-tertiary)]"
                role="status"
            >
                {{ formatMessage(messages.groupModalLoading) }}
            </div>
            <Admonition v-else-if="loadError" type="critical" class="mx-6">
                {{ formatMessage(messages.groupModalLoadFailed) }}
                <Button @click="loadInstances(initialInstanceIds)">{{
                    formatMessage(messages.groupModalRetry)
                }}</Button>
            </Admonition>
            <div
                v-else-if="newGroupInstances.length === 0"
                class="flex items-center justify-center py-12 text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.groupModalNoInstances) }}
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
                                    ? messages.groupModalAdded
                                    : messages.groupModalAdd,
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
                    {{ formatMessage(messages.groupModalCancel) }}
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
                            existingGroupName ? messages.groupModalSave : messages.groupModalCreate,
                        )
                    }}
                </Button>
            </div>
        </template>
    </PreviewModal>
</template>
