<script setup lang="ts">
import { CheckIcon } from '@modrinth/assets'
import { commonMessages, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { computed, nextTick, ref } from 'vue'

import InstancePickerList from '@/components/ui/instance/InstancePickerList.vue'
import type { GameInstance } from '@/helpers/types'

const props = defineProps<{
    instances: GameInstance[]
    selectedInstanceId: string | null
    selectionPending: boolean
    saveSelection: (instance: GameInstance) => Promise<boolean>
}>()

const { formatMessage } = useVIntl()
const modal = ref<InstanceType<typeof NewModal>>()
const instancePicker = ref<InstanceType<typeof InstancePickerList>>()
const submitting = ref(false)
const busy = computed(() => submitting.value || props.selectionPending)

const messages = defineMessages({
    title: {
        id: 'app.home.minimal.picker.title',
        defaultMessage: 'Choose a Home instance',
    },
    search: {
        id: 'app.home.minimal.picker.search',
        defaultMessage: 'Search instances',
    },
    noInstances: {
        id: 'app.home.minimal.picker.no-instances',
        defaultMessage: 'No instances available',
    },
    noResults: {
        id: 'app.home.minimal.picker.no-results',
        defaultMessage: 'No matching instances',
    },
    select: {
        id: 'app.home.minimal.picker.select',
        defaultMessage: 'Choose {name}',
    },
})

function show() {
    instancePicker.value?.reset()
    modal.value?.show()
    void nextTick(() => instancePicker.value?.focus())
}

async function selectInstance(instance: GameInstance) {
    if (busy.value) return

    submitting.value = true
    try {
        if (!(await props.saveSelection(instance))) return
    } finally {
        submitting.value = false
    }
    await nextTick()
    await modal.value?.hide()
}

defineExpose({ show })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        :disable-close="busy"
        max-width="560px"
        width="min(560px, calc(100vw - 2rem))"
        scrollable
        max-content-height="min(36rem, 70vh)"
    >
        <p v-if="busy" role="status" class="m-0 mb-2 text-sm text-[var(--color-text-tertiary)]">
            {{ formatMessage(commonMessages.savingButton) }}
        </p>
        <fieldset :disabled="busy" class="m-0 flex min-w-0 flex-col gap-4 border-0 p-0">
            <InstancePickerList
                ref="instancePicker"
                :instances="instances"
                :search-placeholder="formatMessage(messages.search)"
                :no-instances-message="formatMessage(messages.noInstances)"
                :no-matches-message="formatMessage(messages.noResults)"
                :select-label="
                    (instance) => formatMessage(messages.select, { name: instance.name })
                "
                @select="selectInstance"
            >
                <template #action="{ instance }">
                    <CheckIcon
                        v-if="instance.id === selectedInstanceId"
                        class="size-5 shrink-0 text-brand"
                        aria-hidden="true"
                    />
                </template>
            </InstancePickerList>
        </fieldset>
    </NewModal>
</template>
