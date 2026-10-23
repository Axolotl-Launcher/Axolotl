<script setup lang="ts">
import SaveIcon from '@modrinth/assets/icons/save.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, ref } from 'vue'

import SettingsSection from '../components/SettingsSection.vue'
import { messages } from '../messages'
import { useSettingsPage } from '../state'
import { parseTerracottaPublicNodes, validatePublicNodes } from './multiplayer/terracotta'
import { theseusError } from './resources/proxy'

/**
 * Terracotta public nodes (apps/app-frontend/src/components/ui/settings/MultiplayerSettings.vue):
 * one node URI per line, validated as typed and saved into the settings that
 * hosting and joining a room read.
 */
const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()
const { settings, simulate } = useSettingsPage()

/** `settings_patch` for the node list, with the patch's own validation. */
async function updateSettings(patch: { terracotta_public_nodes: string[] }) {
    await simulate(160)
    const invalid = validatePublicNodes(patch.terracotta_public_nodes)
    if (invalid) throw theseusError(invalid)
    settings.terracotta_public_nodes = [...patch.terracotta_public_nodes]
}

// `get()`.
await simulate(40)
const savedPublicNodes = ref<string[]>([...settings.terracotta_public_nodes])
const publicNodesInput = ref(savedPublicNodes.value.join('\n'))
const publicNodesTouched = ref(false)
const isSavingPublicNodes = ref(false)

const parsedPublicNodes = computed(() => parseTerracottaPublicNodes(publicNodesInput.value))
const publicNodesChanged = computed(
    () => publicNodesInput.value !== savedPublicNodes.value.join('\n'),
)
const showPublicNodesError = computed(
    () => publicNodesTouched.value && parsedPublicNodes.value.invalidNode !== null,
)

async function savePublicNodes() {
    publicNodesTouched.value = true
    if (parsedPublicNodes.value.invalidNode || isSavingPublicNodes.value) return

    isSavingPublicNodes.value = true
    try {
        const nodes = [...parsedPublicNodes.value.nodes]
        await updateSettings({ terracotta_public_nodes: nodes })
        savedPublicNodes.value = nodes
        publicNodesInput.value = savedPublicNodes.value.join('\n')
        addNotification({
            type: 'success',
            title: formatMessage(messages.appMultiplayerTerracottaPublicNodesSaved),
        })
    } catch (error) {
        handleError(error)
    } finally {
        isSavingPublicNodes.value = false
    }
}
</script>

<template>
    <SettingsSection>
        <template #header>
            <h2
                id="terracotta-public-nodes-title"
                tabindex="-1"
                class="m-0 text-lg font-semibold text-[var(--color-text-primary)]"
            >
                {{ formatMessage(messages.appMultiplayerTerracottaPublicNodes) }}
            </h2>
            <p
                id="terracotta-public-nodes-description"
                class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.appMultiplayerTerracottaPublicNodesDescription) }}
            </p>
        </template>
        <div class="flex flex-col gap-3 p-4">
            <StyledInput
                id="terracotta-public-nodes"
                v-model="publicNodesInput"
                multiline
                resize="vertical"
                :rows="5"
                :error="showPublicNodesError"
                :placeholder="
                    formatMessage(messages.appMultiplayerTerracottaPublicNodesPlaceholder)
                "
                :input-attrs="{
                    'aria-labelledby': 'terracotta-public-nodes-title',
                    'aria-invalid': showPublicNodesError,
                    'aria-describedby': showPublicNodesError
                        ? 'terracotta-public-nodes-description terracotta-public-nodes-error'
                        : 'terracotta-public-nodes-description',
                }"
                :spellcheck="false"
                wrapper-class="w-full"
                @input="publicNodesTouched = true"
            />
            <p
                v-if="showPublicNodesError"
                id="terracotta-public-nodes-error"
                class="m-0 text-sm text-red"
            >
                {{
                    formatMessage(messages.appMultiplayerTerracottaPublicNodeInvalid, {
                        node: parsedPublicNodes.invalidNode,
                    })
                }}
            </p>
            <div class="settings-actions flex justify-end pt-3">
                <Button
                    type="colored"
                    color="brand"
                    native-type="button"
                    :disabled="
                        !publicNodesChanged ||
                        !!parsedPublicNodes.invalidNode ||
                        isSavingPublicNodes
                    "
                    @click="savePublicNodes"
                >
                    <SpinnerIcon v-if="isSavingPublicNodes" class="animate-spin" />
                    <SaveIcon v-else />
                    {{ formatMessage(messages.appMultiplayerTerracottaSavePublicNodes) }}
                </Button>
            </div>
        </div>
    </SettingsSection>
</template>

<style scoped>
.settings-actions {
    border-top: 1px solid
        var(--settings-divider, color-mix(in srgb, var(--surface-4) 55%, transparent));
}
</style>
