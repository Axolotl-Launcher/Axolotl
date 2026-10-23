<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, onMounted, useTemplateRef } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'

import { injectCreateServerFlow } from '../create-server-flow.ts'
import ServerPropertiesEditor from '../ServerPropertiesEditor.vue'

const { formatMessage } = useVIntl()
const ctx = injectCreateServerFlow()

const messages = defineMessages({
    heading: {
        id: 'app.servers.wizard.configure-heading',
        defaultMessage: 'Adjust the server settings, or finish to edit them later.',
    },
})

const editor = useTemplateRef<ComponentExposed<typeof ServerPropertiesEditor>>('editor')

onMounted(() => {
    ctx.saveServerProperties.value = () => editor.value?.save() ?? Promise.resolve(true)
})

const serverId = computed(() => ctx.createdServer.value?.id ?? '')
</script>

<template>
    <div class="flex flex-col gap-4">
        <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.heading) }}
        </p>

        <div class="max-h-[32rem] overflow-y-auto pr-2">
            <ServerPropertiesEditor v-if="serverId !== ''" ref="editor" :server-id="serverId" />
        </div>
    </div>
</template>
