<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import { commonMessages, defineMessages, MultiStageModal } from '@modrinth/ui'
import { computed, ref, useTemplateRef, watch } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'

import {
    type CreateServerFlowContextValue,
    provideCreateServerFlow,
} from '@/components/multiplayer/servers/create-server-flow'
import EulaModal from '@/components/multiplayer/servers/EulaModal.vue'
import {
    createModpackServerFlowContext,
    provideModpackServerFlow,
} from '@/components/multiplayer/servers/modpack/create-modpack-server-flow'

const emit = defineEmits<{
    created: [serverId: string]
}>()

const modal = useTemplateRef<ComponentExposed<typeof MultiStageModal>>('modal')
const eulaModal = useTemplateRef<ComponentExposed<typeof EulaModal>>('eulaModal')

const ctx = createModpackServerFlowContext(modal)
// The modpack context is a superset of the base flow context; `stageConfigs` is
// invariant on the context type, so narrow to the base view for consumers.
provideCreateServerFlow(ctx as unknown as CreateServerFlowContextValue)
provideModpackServerFlow(ctx)

const messages = defineMessages({
    downloadInBackground: {
        id: 'app.servers.modpack.download-in-background',
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

// The download keeps running in the background even if the wizard is closed.
// Report the finished server once the flow reaches a terminal success state.
watch(
    () => ctx.installPhase.value,
    (phase) => {
        if (!wasHiddenDuringInstall.value || creationReported.value) return
        if ((phase === 'done' || phase === 'eula') && ctx.createdServer.value) {
            creationReported.value = true
            emit('created', ctx.createdServer.value.id)
        }
    },
)

function show(project: Labrinth.Projects.v2.Project, version: Labrinth.Versions.v2.Version) {
    wizardShown.value = true
    wasHiddenDuringInstall.value = false
    creationReported.value = false
    ctx.reset()
    ctx.setPack(project, version)
    modal.value?.setStage(0)
    modal.value?.show()
}

function handleHide() {
    wizardShown.value = false
    if (
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
        breadcrumbs
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
