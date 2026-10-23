<script setup lang="ts">
import BoxImportIcon from '@modrinth/assets/icons/box-import.svg?component'
import BoxesIcon from '@modrinth/assets/icons/boxes.svg?component'
import PackageIcon from '@modrinth/assets/icons/package.svg?component'
import BigOptionButton from '@modrinth/ui/src/components/base/BigOptionButton.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'

import { setupTypeMessages as messages } from '../messages'
import { useCreateArea } from '../state'

/**
 * Port of `creation-flow-modal/components/SetupTypeStage.vue`, instance flow. The
 * `/create` page opens the flow with `skipSetupType`, so this stage is skipped there.
 */
const { formatMessage } = useVIntl()
const ctx = useCreateArea().flow

function setSetupType(type: 'modpack' | 'custom') {
    ctx.setSetupType(type)
}
</script>

<template>
    <div class="flex flex-col gap-4">
        <span class="font-semibold text-[var(--color-text-primary)]">
            {{ formatMessage(messages.instanceTypeTitle) }}
        </span>

        <div data-onboarding-id="creation-methods" class="flex flex-col gap-3">
            <BigOptionButton
                data-onboarding-id="creation-method-custom"
                :icon="BoxesIcon"
                :title="formatMessage(messages.customSetupTitle)"
                :description="formatMessage(messages.customSetupDescription)"
                @click="setSetupType('custom')"
            />
            <BigOptionButton
                data-onboarding-id="creation-method-modpack"
                :icon="PackageIcon"
                :title="formatMessage(messages.modpackBaseTitle)"
                :description="formatMessage(messages.modpackBaseDescription)"
                @click="setSetupType('modpack')"
            />
            <BigOptionButton
                data-onboarding-id="creation-method-import"
                :icon="BoxImportIcon"
                :title="formatMessage(messages.importInstanceTitle)"
                :description="formatMessage(messages.importInstanceDescription)"
                @click="ctx.setImportMode()"
            />
        </div>
        <span class="text-sm text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.instanceDescription) }}
        </span>
    </div>
</template>
