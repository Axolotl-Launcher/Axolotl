<script setup lang="ts">
import { Button, commonMessages, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { ref, useTemplateRef } from 'vue'
import { useRouter } from 'vue-router'

const modal = useTemplateRef('modal')
const router = useRouter()
const version = ref<number>()
const { formatMessage } = useVIntl()
const messages = defineMessages({
    title: {
        id: 'app.install.missing-java.title',
        defaultMessage: 'Java {version} is needed to install this instance',
    },
    body: {
        id: 'app.install.missing-java.body',
        defaultMessage:
            'This installation cannot continue because no usable Java {version} installation was found. You can install or configure Java {version} in Java settings, then try installing again.',
    },
    openSettings: {
        id: 'app.install.missing-java.open-settings',
        defaultMessage: 'Open Java settings',
    },
})

async function openJavaSettings() {
    await router.push({ path: '/settings', hash: '#java-performance' })
    modal.value?.hide()
}

function show(requiredVersion: number) {
    version.value = requiredVersion
    modal.value?.show()
}

defineExpose({ show })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title, { version })"
        max-width="min(32rem, 95vw)"
    >
        <p class="m-0 leading-relaxed text-[var(--color-text-primary)]">
            {{ formatMessage(messages.body, { version }) }}
        </p>
        <template #actions>
            <div class="flex flex-wrap justify-end gap-2">
                <Button type="outlined" @click="modal?.hide()">
                    {{ formatMessage(commonMessages.closeButton) }}
                </Button>
                <Button type="colored" color="brand" @click="openJavaSettings">
                    {{ formatMessage(messages.openSettings) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>
