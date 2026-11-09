<script setup lang="ts">
import { computed } from 'vue'

import { defineMessages, useVIntl } from '../../../../composables/i18n'
import {
    BUILTIN_GAME_DIRECTORY,
    type ConfiguredMinecraftDirectory,
} from '../../../../utils/minecraft-directories'
import RadioButtons from '../../../base/RadioButtons.vue'

const props = defineProps<{
    modelValue: string
    directories: ConfiguredMinecraftDirectory[]
}>()
const emit = defineEmits<{ 'update:modelValue': [value: string] }>()
const { formatMessage } = useVIntl()
const messages = defineMessages({
    label: {
        id: 'creation-flow.modal.custom-setup.game-dir.label',
        defaultMessage: 'Game directory',
    },
    managed: {
        id: 'creation-flow.modal.custom-setup.game-dir.managed',
        defaultMessage: 'Axolotl directory',
    },
})
const options = computed(() => [
    ...new Set([
        BUILTIN_GAME_DIRECTORY,
        ...props.directories.map((directory) => directory.path),
        props.modelValue,
    ]),
])
</script>

<template>
    <div class="flex min-w-0 flex-col gap-2">
        <span class="font-semibold text-[var(--color-text-primary)]">{{
            formatMessage(messages.label)
        }}</span>
        <RadioButtons
            :model-value="modelValue"
            :items="options"
            force-selection
            @update:model-value="emit('update:modelValue', $event)"
        >
            <template #default="{ item }">
                <span class="min-w-0 break-all">{{
                    item === BUILTIN_GAME_DIRECTORY ? formatMessage(messages.managed) : item
                }}</span>
            </template>
        </RadioButtons>
    </div>
</template>
