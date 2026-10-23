<script setup lang="ts">
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, ref, shallowRef, watch } from 'vue'

import { syncSettingsMessages as messages } from '../messages'
import { injectInstanceSettings } from './context'
import {
    SYNCED_OPTIONS,
    type SyncedOption,
    useInstanceSettingsState,
    useSimulatedInvoke,
} from './state'

/**
 * Port of `apps/app-frontend/src/components/ui/instance_settings/SyncSettings.vue`.
 * `get_synced_options_overview` reports an option as unsupported while its
 * global switch (Settings > Instance synchronization) is off, or when the
 * instance's Minecraft version predates the synced file
 * (`synced_options/orchestration.rs`).
 */
const { instance, onInstanceUpdated } = injectInstanceSettings()
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const settingsState = useInstanceSettingsState()
const simulate = useSimulatedInvoke()
const syncedOptions = ref({ ...settingsState.extrasFor(instance.value.id).synced_options })

watch(instance, (updatedInstance) => {
    syncedOptions.value = { ...settingsState.extrasFor(updatedInstance.id).synced_options }
})

type MessageKey = keyof typeof messages

const options: Array<{ key: SyncedOption; label: MessageKey }> = [
    { key: 'game_options', label: 'gameOptions' },
    { key: 'command_history', label: 'commandHistory' },
    { key: 'multiplayer_servers', label: 'multiplayerServers' },
    { key: 'creative_hotbars', label: 'creativeHotbars' },
    { key: 'resource_packs', label: 'resourcePacks' },
]

interface SyncedOptionCapability {
    option: SyncedOption
    supported: boolean
    disabled_reason: string | null
}

/** Release times the capability check compares against, from the Mojang manifest snapshot. */
async function versionSupports(option: SyncedOption, gameVersion: string): Promise<boolean> {
    if (
        option === 'game_options' ||
        option === 'screenshots' ||
        option === 'resource_packs' ||
        option === 'data_packs'
    ) {
        return true
    }
    const data = await import('../../../data')
    const minecraft = await data.loadMinecraft()
    const all = [
        ...minecraft.releases.map((entry) => ({ ...entry, type: 'release' })),
        ...minecraft.snapshots.map((entry) => ({ ...entry, type: 'snapshot' })),
        ...(minecraft.old ?? []).map((entry) => ({ ...entry, type: entry.type as string })),
    ]
    const version = all.find((entry) => entry.id === gameVersion)
    const cutoffId =
        option === 'multiplayer_servers'
            ? 'b1.8'
            : option === 'creative_hotbars'
              ? '1.12'
              : '1.20.2'
    const cutoff = all.find((entry) => entry.id === cutoffId)
    if (!version || !cutoff) return false
    const releaseOnly = option === 'creative_hotbars' || option === 'command_history'
    return (
        Date.parse(version.releaseTime) >= Date.parse(cutoff.releaseTime) &&
        (!releaseOnly || version.type === 'release')
    )
}

async function getSyncedOptionsOverview(): Promise<{ capabilities: SyncedOptionCapability[] }> {
    await simulate(160)
    const capabilities: SyncedOptionCapability[] = []
    for (const option of SYNCED_OPTIONS) {
        if (!settingsState.globalSyncedOptions[option]) {
            capabilities.push({
                option,
                supported: false,
                disabled_reason: "This option is disabled in the app's synced options settings.",
            })
            continue
        }
        const supported = await versionSupports(option, instance.value.game_version)
        capabilities.push({
            option,
            supported,
            disabled_reason: supported ? null : 'Unsupported Minecraft version',
        })
    }
    return { capabilities }
}

const overview = shallowRef<{ capabilities: SyncedOptionCapability[] } | undefined>(undefined)
watch(
    () => [instance.value.id, instance.value.game_version] as const,
    async () => {
        try {
            overview.value = await getSyncedOptionsOverview()
        } catch (error) {
            handleError(error)
        }
    },
    { immediate: true },
)

const optionSections = computed(() => {
    const unsupportedOptionKeys = new Set(
        overview.value?.capabilities
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

const pending = ref(false)

/** `set_instance_synced_option`. */
async function mutate({ option, enabled }: { option: SyncedOption; enabled: boolean }) {
    pending.value = true
    try {
        await simulate(220)
        const extras = settingsState.extrasFor(instance.value.id)
        extras.synced_options[option] = enabled
        syncedOptions.value = { ...extras.synced_options }
        onInstanceUpdated(instance.value)
        overview.value = await getSyncedOptionsOverview()
    } catch (error) {
        handleError(error)
    } finally {
        pending.value = false
    }
}

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
                    :disabled="section.disabled || pending"
                    :aria-label="formatMessage(messages[item.label])"
                    @update:model-value="(value) => mutate({ option: item.key, enabled: !!value })"
                />
            </div>
        </template>
    </div>
</template>
