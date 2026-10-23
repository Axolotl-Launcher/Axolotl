<script setup lang="ts">
import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, ref, useTemplateRef, watch } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'

import MultiStageModal from '../../../overlays/PreviewMultiStageModal.vue'
import { createCreateServerFlowContext, provideCreateServerFlow } from './create-server-flow.ts'
import EulaModal from './EulaModal.vue'

const emit = defineEmits<{
    created: [serverId: string]
}>()

const modal = useTemplateRef<ComponentExposed<typeof MultiStageModal>>('modal')
const eulaModal = useTemplateRef<ComponentExposed<typeof EulaModal>>('eulaModal')

const ctx = createCreateServerFlowContext(modal)
provideCreateServerFlow(ctx)

const messages = defineMessages({
    downloadInBackground: {
        id: 'app.servers.wizard.download-in-background',
        defaultMessage: 'Download in background',
    },
})

const wizardShown = ref(false)
const wasHiddenDuringInstall = ref(false)
const creationReported = ref(false)

const cancelButton = computed(() => {
    if (ctx.installPhase.value === 'done') {
        return null
    }
    // The download continues in the background once the wizard closes; only the
    // first-run boot locks closing until the server reaches its EULA gate.
    return {
        label: ctx.formatMessage(
            ctx.installPhase.value === 'downloading'
                ? messages.downloadInBackground
                : commonMessages.cancelButton,
        ),
        disabled: ctx.installPhase.value === 'first-run',
        onClick: () => modal.value?.hide(),
    }
})

watch(ctx.showEulaModal, (visible) => {
    if (visible) {
        // When the setup finished in the background, don't pop a EULA dialog over
        // whatever page the user is on; starting the server gates on it instead.
        if (wizardShown.value) eulaModal.value?.show()
    } else {
        eulaModal.value?.hide()
    }
})

function show() {
    wizardShown.value = true
    wasHiddenDuringInstall.value = false
    creationReported.value = false
    ctx.reset()
    modal.value?.setStage(0)
    modal.value?.show()
}

function handleHide() {
    const wasShown = wizardShown.value
    wizardShown.value = false
    // An explicit "Finish" (wizard still open at a terminal state) navigates to
    // the new server. A background close (wizard dismissed mid-install) leaves
    // the server in the list instead of yanking the user to another page.
    if (
        wasShown &&
        !wasHiddenDuringInstall.value &&
        ctx.createdServer.value &&
        (ctx.installPhase.value === 'done' || ctx.installPhase.value === 'eula')
    ) {
        if (!creationReported.value) {
            creationReported.value = true
            emit('created', ctx.createdServer.value.id)
        }
    } else {
        wasHiddenDuringInstall.value = true
    }
}

defineExpose({ show, hide: () => modal.value?.hide() })
</script>

<template>
    <MultiStageModal
        ref="modal"
        :stages="ctx.stageConfigs"
        :context="ctx"
        :back-button-enabled="
            (flowCtx) =>
                flowCtx.installPhase.value !== 'downloading' &&
                flowCtx.installPhase.value !== 'first-run'
        "
        :cancel-button="cancelButton"
        @hide="handleHide"
    />
    <EulaModal
        ref="eulaModal"
        :text="ctx.eulaText.value"
        @continue="ctx.acceptEula"
        @decline="ctx.declineEula"
    />
</template>
