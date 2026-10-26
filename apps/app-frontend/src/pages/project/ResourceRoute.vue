<script setup lang="ts">
import { Button, defineMessages, ProjectPageSkeleton, useVIntl } from '@modrinth/ui'
import { computed, onScopeDispose, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { createProjectPreviewState, readProjectPreviewTitle } from '@/helpers/project-preview'
import { resourceGet } from '@/helpers/resources'

const route = useRoute()
const router = useRouter()
const previewTitle = computed(() => readProjectPreviewTitle(route.path))
const { formatMessage } = useVIntl()
const failed = ref(false)
let controller: AbortController | null = null
const messages = defineMessages({
    failed: {
        id: 'app.resources.route.failed',
        defaultMessage: 'No resource source could be opened.',
    },
    retry: { id: 'app.resources.route.retry', defaultMessage: 'Retry' },
})

async function openResource() {
    controller?.abort()
    const request = new AbortController()
    controller = request
    failed.value = false
    try {
        const resource = await resourceGet(
            { kind: 'resource', id: String(route.params.id) },
            undefined,
            undefined,
            request.signal,
        )
        const project = resource.defaultProject
        const metadata = resource.projects.find(
            (metadata) =>
                metadata.project.source === project.source &&
                metadata.project.projectId === project.projectId,
        )!
        const path =
            project.source === 'modrinth'
                ? `/project/${project.projectId}`
                : project.source === 'curseforge'
                  ? `/project/curseforge/${project.projectId}`
                  : project.source === 'mcarchive'
                    ? `/project/mcarchive/${metadata.slug ?? project.projectId}`
                    : `/project/planet-minecraft/${project.projectId}`
        request.signal.throwIfAborted()
        await router.replace({
            path,
            query: route.query,
            hash: route.hash,
            state: createProjectPreviewState(path, metadata.title),
        })
    } catch {
        if (!request.signal.aborted) failed.value = true
    }
}
watch(
    () => route.params.id,
    () => {
        void openResource()
    },
    { immediate: true },
)
onScopeDispose(() => controller?.abort())
</script>

<template>
    <div v-if="failed" class="flex items-center gap-3 p-6" role="alert">
        <span>{{ formatMessage(messages.failed) }}</span>
        <Button @click="openResource">{{ formatMessage(messages.retry) }}</Button>
    </div>
    <ProjectPageSkeleton v-else :title="previewTitle" />
</template>
