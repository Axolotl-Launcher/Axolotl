<script setup lang="ts">
import { LogInIcon, SpinnerIcon } from '@modrinth/assets'
import { commonMessages, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { ref } from 'vue'

withDefaults(
    defineProps<{
        onFlowCancel?: () => void
    }>(),
    {
        onFlowCancel: () => async () => {},
    },
)

const modal = ref()
const { formatMessage } = useVIntl()
const messages = defineMessages({
    signInBrowser: {
        id: 'app.auth.sign-in-browser',
        defaultMessage: 'Please sign in in the browser window that just opened to continue.',
    },
})

function show() {
    modal.value.show()
}

function hide() {
    modal.value.hide()
}

defineExpose({ show, hide })
</script>
<template>
    <NewModal ref="modal" :on-hide="onFlowCancel">
        <template #title>
            <span
                class="items-center gap-2 text-lg font-extrabold text-[var(--color-text-primary)]"
            >
                <LogInIcon /> {{ formatMessage(commonMessages.signInButton) }}
            </span>
        </template>

        <div class="flex justify-center gap-2">
            <SpinnerIcon class="w-12 h-12 animate-spin" />
        </div>
        <p class="text-sm text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.signInBrowser) }}
        </p>
    </NewModal>
</template>
