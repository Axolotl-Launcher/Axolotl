<script setup lang="ts">
import { defineMessages, injectNotificationManager, Toggle, useVIntl } from '@modrinth/ui'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref, watch } from 'vue'

import {
    get_synced_options_overview,
    set_instance_synced_option,
    type SyncedOption,
} from '@/helpers/instance'
import { instanceKeys } from '@/pages/instance/query-options'
import { injectInstanceSettings } from '@/providers/instance-settings'

const { instance, onInstanceUpdated } = injectInstanceSettings()
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const queryClient = useQueryClient()
const syncedOptions = ref(instance.value?.synced_options ?? {})

watch(instance, (updatedInstance) => {
    syncedOptions.value = updatedInstance.synced_options
})

const messages = defineMessages({
    title: { id: 'instance.settings.sync.title', defaultMessage: 'Settings synchronization' },
    description: {
        id: 'instance.settings.sync.description',
        defaultMessage: 'Choose which settings this instance shares with other instances.',
    },
    unsupported: {
        id: 'instance.settings.sync.unsupported',
        defaultMessage: 'Unavailable for this instance',
    },
    gameOptions: { id: 'instance.settings.sync.game-options', defaultMessage: 'Game settings' },
    commandHistory: {
        id: 'instance.settings.sync.command-history',
        defaultMessage: 'Command history',
    },
    multiplayerServers: {
        id: 'instance.settings.sync.multiplayer-servers',
        defaultMessage: 'Multiplayer servers',
    },
    creativeHotbars: {
        id: 'instance.settings.sync.creative-hotbars',
        defaultMessage: 'Creative hotbars',
    },
    resourcePacks: {
        id: 'instance.settings.sync.resource-packs',
        defaultMessage: 'Resource packs',
    },
})

const options: Array<{ key: SyncedOption; label: keyof typeof messages }> = [
    { key: 'game_options', label: 'gameOptions' },
    { key: 'command_history', label: 'commandHistory' },
    { key: 'multiplayer_servers', label: 'multiplayerServers' },
    { key: 'creative_hotbars', label: 'creativeHotbars' },
    { key: 'resource_packs', label: 'resourcePacks' },
]

const overviewQuery = useQuery({
    queryKey: computed(() => ['instance-synced-options', instance.value.id, 'overview']),
    queryFn: () => get_synced_options_overview(instance.value.id),
})

const optionSections = computed(() => {
    const unsupportedOptionKeys = new Set(
        overviewQuery.data.value?.capabilities
            .filter((capability) => capability.supported === false)
            .map((capability) => capability.option) ?? [],
    )
    const supported = options.filter((option) => !unsupportedOptionKeys.has(option.key))
    const unsupported = options.filter((option) => unsupportedOptionKeys.has(option.key))

    return [
        { key: 'supported', items: supported, disabled: false },
        ...(unsupported.length > 0
            ? [{ key: 'unsupported', items: unsupported, disabled: true }]
            : []),
    ]
})

const mutation = useMutation({
    mutationFn: ({ option, enabled }: { option: SyncedOption; enabled: boolean }) =>
        set_instance_synced_option(instance.value.id, option, enabled),
    onSuccess: async (updatedInstance) => {
        syncedOptions.value = updatedInstance.synced_options
        onInstanceUpdated(updatedInstance)
        await Promise.all([
            queryClient.invalidateQueries({ queryKey: instanceKeys.all }),
            queryClient.invalidateQueries({ queryKey: ['instance-synced-options'] }),
        ])
    },
    onError: handleError,
})

function enabled(option: SyncedOption) {
    return syncedOptions.value?.[option] ?? false
}
</script>

<template>
    <div class="flex flex-col gap-4">
        <div>
            <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.title) }}
            </h2>
            <p class="m-0 text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.description) }}
            </p>
        </div>
        <template v-for="section in optionSections" :key="section.key">
            <h3
                v-if="section.key === 'unsupported'"
                class="m-0 text-sm font-semibold text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.unsupported) }}
            </h3>
            <div
                v-for="item in section.items"
                :key="item.key"
                class="flex items-center justify-between gap-4"
            >
                <span class="text-[var(--color-text-primary)]">{{
                    formatMessage(messages[item.label])
                }}</span>
                <Toggle
                    :model-value="enabled(item.key)"
                    :disabled="section.disabled || mutation.isPending.value"
                    :aria-label="formatMessage(messages[item.label])"
                    @update:model-value="
                        (value) => mutation.mutate({ option: item.key, enabled: value ?? false })
                    "
                />
            </div>
        </template>
    </div>
</template>
