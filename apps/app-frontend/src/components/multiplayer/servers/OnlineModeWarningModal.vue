<script setup lang="ts">
import { Button, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { ref } from 'vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
    title: {
        id: 'app.servers.online-mode-warning.title',
        defaultMessage: 'This server requires a premium account',
    },
    body: {
        id: 'app.servers.online-mode-warning.body',
        defaultMessage:
            '“{server}” has online-mode enabled, but the account you are using (“{account}”) is an offline account. The server will refuse the connection. Start anyway?',
    },
    confirm: {
        id: 'app.servers.online-mode-warning.confirm',
        defaultMessage: 'Start anyway',
    },
    cancel: {
        id: 'app.servers.online-mode-warning.cancel',
        defaultMessage: 'Cancel',
    },
})

const modal = ref<InstanceType<typeof NewModal> | null>(null)
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
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        max-width="560px"
        width="min(560px, calc(100vw - 2rem))"
        :on-hide="() => answer(false)"
    >
        <div class="flex flex-col gap-4">
            <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{
                    formatMessage(messages.body, {
                        server: serverName,
                        account: accountName,
                    })
                }}
            </p>
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="answer(false)">{{
                    formatMessage(messages.cancel)
                }}</Button>
                <Button type="colored" color="brand" @click="answer(true)">{{
                    formatMessage(messages.confirm)
                }}</Button>
            </div>
        </div>
    </NewModal>
</template>
