<script setup lang="ts">
import { ProjectCard, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'
import { useRoute } from 'vue-router'

import { createProjectDetailsQuery } from '@/helpers/project-links'
import { createProjectPreviewState } from '@/helpers/project-preview'
import type { SchematicProject } from '@/helpers/schematics'

import { schematicMessages as messages, schematicSourceMessages } from './messages'
import SchematicInstallButton from './SchematicInstallButton.vue'

const props = defineProps<{
    project: SchematicProject
    instanceId: string | null
    layout: 'list' | 'compact' | 'grid'
}>()
const emit = defineEmits<{ chooseInstance: []; authorizationError: [] }>()
const { formatMessage } = useVIntl()
const route = useRoute()
const projectLink = computed(() => {
    const path = `/project/${props.project.source}/${encodeURIComponent(props.project.id)}`
    return {
        path,
        query: createProjectDetailsQuery(route.query, route.fullPath),
        state: createProjectPreviewState(path, props.project.title),
    }
})
const summary = computed(() =>
    props.project.author && props.project.summary
        ? formatMessage(messages.authorSummary, {
              author: props.project.author,
              summary: props.project.summary,
          })
        : props.project.summary || props.project.author,
)
</script>

<template>
    <ProjectCard
        :layout="layout"
        :link="projectLink"
        :title="project.title"
        :summary="summary"
        :icon-url="project.cover ?? undefined"
        :banner="project.cover ?? undefined"
        :tags="project.gameVersions"
        :source-label="formatMessage(schematicSourceMessages[project.source])"
    >
        <template #actions>
            <SchematicInstallButton
                :project="project"
                :instance-id="instanceId"
                :compact="layout === 'compact'"
                @choose-instance="emit('chooseInstance')"
                @authorization-error="emit('authorizationError')"
            />
        </template>
    </ProjectCard>
</template>
