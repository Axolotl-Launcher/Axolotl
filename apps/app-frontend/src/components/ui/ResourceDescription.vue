<script setup lang="ts">
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { computed, onScopeDispose, ref, useSlots, watch } from 'vue'

import TranslatedProjectDescription from '@/components/ui/TranslatedProjectDescription.vue'
import {
    type Resource,
    resourceDescription,
    resourceGet,
    type ResourceText,
    type SourceId,
    sourceResource,
} from '@/helpers/resources'

const props = defineProps<{ source: SourceId; projectId: string; locator?: string }>()
const slots = useSlots()
const { formatMessage } = useVIntl()
const resource = ref<Resource | null>(null)
const text = ref<ResourceText | null>(null)
const selected = ref<SourceId>(props.source)
const attemptedSource = ref<SourceId>(props.source)
const busy = ref(false)
const pendingSource = ref<SourceId | null>(null)
const error = ref(false)
let controller: AbortController | null = null
const messages = defineMessages({
    retry: { id: 'app.resources.description.retry', defaultMessage: 'Retry description' },
    failed: {
        id: 'app.resources.description.failed',
        defaultMessage: 'The description could not be loaded.',
    },
    reading: {
        id: 'app.resources.description.reading',
        defaultMessage: 'Description from {source}',
    },
})
const sources = computed(() => [
    ...new Set(
        resource.value?.confirmedProjects?.map((project) => project.source) ??
            resource.value?.projects.map((project) => project.project.source) ?? [props.source],
    ),
])
const names: Record<SourceId, string> = {
    modrinth: 'Modrinth',
    curseforge: 'CurseForge',
    mcarchive: 'MCArchive',
    planet_minecraft: 'Planet Minecraft',
}

async function load(source?: SourceId) {
    controller?.abort()
    const request = new AbortController()
    controller = request
    busy.value = true
    pendingSource.value = source ?? props.source
    attemptedSource.value = source ?? props.source
    error.value = false
    try {
        const reference = sourceResource(props.source, props.projectId)
        const current =
            resource.value ??
            (await resourceGet(reference, undefined, props.locator, request.signal))
        const readingSource = source ?? current.defaultProject.source
        pendingSource.value = readingSource
        attemptedSource.value = readingSource
        const description =
            readingSource === props.source && slots.default
                ? null
                : await resourceDescription(reference, readingSource, props.locator, request.signal)
        request.signal.throwIfAborted()
        resource.value = current
        selected.value = readingSource
        text.value = description
    } catch {
        if (!request.signal.aborted) error.value = true
    } finally {
        if (controller === request) {
            busy.value = false
            pendingSource.value = null
        }
    }
}
watch(
    () => [props.source, props.projectId, props.locator],
    () => {
        resource.value = null
        text.value = null
        selected.value = props.source
        void load()
    },
    { immediate: true },
)
onScopeDispose(() => controller?.abort())
</script>

<template>
    <div class="flex flex-col gap-3" :aria-busy="busy">
        <div v-if="sources.length > 1" class="flex flex-wrap items-center gap-2">
            <Button
                v-for="readingSource in sources"
                :key="readingSource"
                size="sm"
                :type="selected === readingSource ? 'outlined' : 'quiet'"
                :disabled="pendingSource === readingSource"
                @click="load(readingSource)"
                >{{ names[readingSource] }}</Button
            >
            <span class="text-sm text-[var(--color-text-tertiary)]">{{
                formatMessage(messages.reading, { source: names[selected] })
            }}</span>
        </div>
        <div v-if="error" class="flex items-center gap-2">
            <span>{{ formatMessage(messages.failed) }}</span>
            <Button @click="load(attemptedSource)">{{ formatMessage(messages.retry) }}</Button>
        </div>
        <slot v-if="selected === source && $slots.default" />
        <template v-else-if="!text" />
        <p v-else-if="text.format === 'plain'" class="m-0 whitespace-pre-wrap">{{ text.body }}</p>
        <TranslatedProjectDescription
            v-else
            :description="text.body"
            :format="text.format"
            :active="false"
            :translations="{}"
            mode="bilingual"
            style="weakened"
        />
    </div>
</template>
