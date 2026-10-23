<script setup lang="ts">
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import {
    defineMessages,
    type MessageDescriptor,
    useVIntl,
} from '@modrinth/ui/src/composables/i18n.ts'

import type { ServerPackStatus } from './worlds-api.ts'

const { formatMessage } = useVIntl()

const name = defineModel<string>('name')
const address = defineModel<string>('address')
const resourcePack = defineModel<ServerPackStatus>('resourcePack')

const resourcePackOptions: ServerPackStatus[] = ['enabled', 'prompt', 'disabled']

const resourcePackOptionMessages: Record<ServerPackStatus, MessageDescriptor> = defineMessages({
    enabled: {
        id: 'instance.add-server.resource-pack.enabled',
        defaultMessage: 'Enabled',
    },
    prompt: {
        id: 'instance.add-server.resource-pack.prompt',
        defaultMessage: 'Prompt',
    },
    disabled: {
        id: 'instance.add-server.resource-pack.disabled',
        defaultMessage: 'Disabled',
    },
})

const messages = defineMessages({
    name: {
        id: 'instance.server-modal.name',
        defaultMessage: 'Name',
    },
    address: {
        id: 'instance.server-modal.address',
        defaultMessage: 'Address',
    },
    resourcePack: {
        id: 'instance.server-modal.resource-pack',
        defaultMessage: 'Resource pack',
    },
    placeholderName: {
        id: 'instance.server-modal.placeholder-name',
        defaultMessage: 'Minecraft Server',
    },
    placeholderAddress: {
        id: 'app.world.server-modal.placeholder-address',
        defaultMessage: 'example.modrinth.gg',
    },
    selectAnOption: {
        id: 'app.world.server-modal.select-an-option',
        defaultMessage: 'Select an option',
    },
})

defineExpose({ resourcePackOptions })
</script>
<template>
    <div class="w-full space-y-4">
        <label class="flex flex-col gap-2">
            <span class="font-semibold text-[var(--color-text-primary)]">{{
                formatMessage(messages.name)
            }}</span>
            <StyledInput
                v-model="name"
                :placeholder="formatMessage(messages.placeholderName)"
                autocomplete="off"
                wrapper-class="w-full"
            />
        </label>
        <label class="flex flex-col gap-2">
            <span class="font-semibold text-[var(--color-text-primary)]">{{
                formatMessage(messages.address)
            }}</span>
            <StyledInput
                v-model="address"
                :placeholder="formatMessage(messages.placeholderAddress)"
                autocomplete="off"
                wrapper-class="w-full"
            />
        </label>
        <label class="flex flex-col gap-2">
            <span class="font-semibold text-[var(--color-text-primary)]">{{
                formatMessage(messages.resourcePack)
            }}</span>
            <Combobox
                v-model="resourcePack"
                :options="
                    resourcePackOptions.map((o) => ({
                        value: o,
                        label: formatMessage(resourcePackOptionMessages[o]),
                    }))
                "
                name="Server resource pack"
                :display-value="
                    resourcePack
                        ? formatMessage(resourcePackOptionMessages[resourcePack])
                        : formatMessage(messages.selectAnOption)
                "
            />
        </label>
    </div>
</template>
