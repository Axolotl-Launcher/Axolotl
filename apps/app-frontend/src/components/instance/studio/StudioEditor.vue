<script setup lang="ts">
import { CodeEditor } from '@modrinth/ui'
import { ref } from 'vue'

const props = defineProps<{
    content: string
    filePath: string
    language: string
    readOnly?: boolean
}>()
const emit = defineEmits<{
    'update:content': [content: string]
    save: []
    blur: []
}>()
const editor = ref<InstanceType<typeof CodeEditor> | null>(null)
async function formatDocument() {
    await editor.value?.formatDocument()
}
defineExpose({ formatDocument })
</script>

<template>
    <CodeEditor
        ref="editor"
        v-bind="props"
        @update:content="emit('update:content', $event)"
        @save="emit('save')"
        @blur="emit('blur')"
    />
</template>
