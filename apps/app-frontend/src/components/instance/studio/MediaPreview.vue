<script setup lang="ts">
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { computed, ref, watch } from 'vue'

const props = defineProps<{
    kind: 'image' | 'video'
    url: string
    name: string
    loading: boolean
    failed: boolean
}>()

const emit = defineEmits<{ openInSystem: [] }>()

const messages = defineMessages({
    loading: {
        id: 'instance.files.studio.preview-loading',
        defaultMessage: 'Loading preview...',
    },
    previewFailed: {
        id: 'instance.files.studio.preview-failed',
        defaultMessage: 'This file could not be previewed.',
    },
    openInSystem: {
        id: 'instance.files.studio.open-in-system',
        defaultMessage: 'Open in system',
    },
})

const { formatMessage } = useVIntl()

const renderFailed = ref(false)

watch(
    () => props.url,
    () => {
        renderFailed.value = false
    },
)

const unavailable = computed(() => props.failed || renderFailed.value)
</script>

<template>
    <div class="flex size-full items-center justify-center overflow-auto bg-surface-1 p-6">
        <div v-if="unavailable" class="flex flex-col items-center gap-3 text-center" role="status">
            <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.previewFailed) }}
            </p>
            <Button type="outlined" @click="emit('openInSystem')">
                {{ formatMessage(messages.openInSystem) }}
            </Button>
        </div>
        <p v-else-if="loading" class="m-0 text-sm text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.loading) }}
        </p>
        <img
            v-else-if="kind === 'image'"
            :src="url"
            :alt="name"
            class="max-h-full max-w-full object-contain"
            @error="renderFailed = true"
        />
        <video
            v-else
            :src="url"
            controls
            class="max-h-full max-w-full"
            @error="renderFailed = true"
        />
    </div>
</template>
