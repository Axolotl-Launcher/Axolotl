<template>
    <PreviewModal ref="modal" :header="formatMessage(messages.title)" :on-hide="resetDeviceLogin">
        <div class="flex min-w-[24rem] flex-col gap-4">
            <p v-if="deviceError" class="m-0 text-sm text-red">{{ deviceError }}</p>
        </div>
    </PreviewModal>
</template>

<script setup lang="ts">
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { ref } from 'vue'

import PreviewModal from '../overlays/PreviewModal.vue'
import { minecraftLoginMessages as messages } from './accounts-messages'

/**
 * Port of the app's MinecraftLoginModal (apps/app-frontend/src/components/ui/
 * MinecraftLoginModal.vue). The app asks Microsoft for a device code
 * (`begin_device_login`) and shows the request's error in the dialog when it
 * fails. The preview contacts no sign-in service, so the dialog says so on that
 * error line and never shows a code.
 */
const { formatMessage } = useVIntl()
const modal = ref<InstanceType<typeof PreviewModal>>()
const deviceError = ref<string | null>(null)

function resetDeviceLogin() {
    deviceError.value = null
}

function hide() {
    resetDeviceLogin()
    modal.value?.hide()
}

function showDeviceLogin() {
    resetDeviceLogin()
    modal.value?.show()
    deviceError.value = formatMessage(messages.desktopOnly)
}

defineExpose({ showDeviceLogin, hide })
</script>
