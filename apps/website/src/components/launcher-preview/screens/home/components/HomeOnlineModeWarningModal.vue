<script setup lang="ts">
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { ref } from 'vue'

import PreviewModal from '../../../overlays/PreviewModal.vue'
import { homeMessages } from '../messages'

/** Port of `components/multiplayer/servers/OnlineModeWarningModal.vue`. */
const { formatMessage } = useVIntl()
const messages = homeMessages

const modal = ref<InstanceType<typeof PreviewModal> | null>(null)
const serverName = ref('')
const accountName = ref('')
let settled = false
let resolveShow: ((value: boolean) => void) | null = null

function answer(value: boolean) {
    if (settled) return
    settled = true
    const resolve = resolveShow
    resolveShow = null
    if (resolve) resolve(value)
    modal.value?.hide()
}

function show(payload: { serverName: string; accountName: string }): Promise<boolean> {
    serverName.value = payload.serverName
    accountName.value = payload.accountName
    settled = false
    modal.value?.show()
    return new Promise<boolean>((resolve) => {
        resolveShow = resolve
    })
}

defineExpose({ show })
</script>

<template>
    <PreviewModal
        ref="modal"
        :header="formatMessage(messages.onlineModeWarningTitle)"
        max-width="560px"
        width="min(560px, calc(100% - 2rem))"
        :on-hide="() => answer(false)"
    >
        <div class="flex flex-col gap-4">
            <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{
                    formatMessage(messages.onlineModeWarningBody, {
                        server: serverName,
                        account: accountName,
                    })
                }}
            </p>
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="answer(false)">{{
                    formatMessage(messages.onlineModeWarningCancel)
                }}</Button>
                <Button type="colored" color="brand" @click="answer(true)">{{
                    formatMessage(messages.onlineModeWarningConfirm)
                }}</Button>
            </div>
        </div>
    </PreviewModal>
</template>
