<script setup lang="ts">
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, nextTick, ref } from 'vue'

import { usePreview } from '../../../../lib/state/index'
import PreviewModal from '../../../../overlays/PreviewModal.vue'
import { messages } from '../../messages'

/**
 * Forced-reading confirmation before privileged link actions can be enabled
 * (apps/app-frontend/src/components/ui/modal/PrivilegedConsentModal.vue): a
 * ten-second countdown and an acknowledgement before "Enable" unlocks.
 */
const { formatMessage } = useVIntl()
const { clock } = usePreview()

const modal = ref<InstanceType<typeof PreviewModal>>()
const cancelWrap = ref<HTMLElement>()
const acknowledged = ref(false)
const secondsLeft = ref(0)
const settled = ref(true)
let resolver: ((ok: boolean) => void) | undefined
let cancelCountdown: (() => void) | undefined

const title = computed(() => formatMessage(messages.appPrivilegedConsentTitle))
const confirmDisabled = computed(() => secondsLeft.value > 0 || !acknowledged.value)
const confirmLabel = computed(() => {
    if (secondsLeft.value > 0) {
        return `${formatMessage(messages.appPrivilegedConsentConfirm)} (${secondsLeft.value})`
    }
    return formatMessage(messages.appPrivilegedConsentConfirm)
})

function stopCountdown() {
    cancelCountdown?.()
    cancelCountdown = undefined
}

function countdownStep() {
    cancelCountdown = clock.after(1000, () => {
        secondsLeft.value -= 1
        if (secondsLeft.value <= 0) stopCountdown()
        else countdownStep()
    })
}

function finish(ok: boolean) {
    if (settled.value) return
    settled.value = true
    stopCountdown()
    modal.value?.hide()
    const resolve = resolver
    resolver = undefined
    resolve?.(ok)
}

function request(): Promise<boolean> {
    return new Promise<boolean>((resolve) => {
        acknowledged.value = false
        secondsLeft.value = 10
        settled.value = false
        resolver = resolve
        stopCountdown()
        countdownStep()
        modal.value?.show()
        void nextTick(() => cancelWrap.value?.querySelector('button')?.focus())
    })
}

defineExpose({ request })
</script>

<template>
    <PreviewModal
        ref="modal"
        :header="title"
        fade="danger"
        max-width="42rem"
        width="42rem"
        :on-hide="() => finish(false)"
    >
        <div class="flex w-full flex-col gap-4">
            <Admonition
                type="critical"
                :header="formatMessage(messages.appPrivilegedConsentWarning)"
            >
                {{ formatMessage(messages.appPrivilegedConsentDanger) }}
            </Admonition>
            <Checkbox
                v-model="acknowledged"
                :label="formatMessage(messages.appPrivilegedConsentAcknowledge)"
            />
        </div>
        <template #actions>
            <div ref="cancelWrap" class="flex w-full flex-row justify-end gap-2">
                <Button @click="finish(false)">{{
                    formatMessage(commonMessages.cancelButton)
                }}</Button>
                <Button
                    type="colored"
                    color="red"
                    :disabled="confirmDisabled"
                    @click="finish(true)"
                >
                    {{ confirmLabel }}
                </Button>
            </div>
        </template>
    </PreviewModal>
</template>
