<script setup lang="ts">
import { ChevronDownIcon, EyeIcon, GlobeIcon, PlusIcon } from '@modrinth/assets'
import {
    Button,
    Card,
    EmptyState,
    NavTabs,
    ProjectHeader,
    ProjectPageSkeleton,
    ProjectSidebarCompatibility,
    SidebarSection,
    useVIntl,
} from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import BrowseInstanceSelector from '@/components/browse/BrowseInstanceSelector.vue'
import {
    schematicMessages as messages,
    schematicSourceMessages,
} from '@/components/browse/schematics/messages'
import SchematicInstallButton from '@/components/browse/schematics/SchematicInstallButton.vue'
import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import TranslatedProjectDescription from '@/components/ui/TranslatedProjectDescription.vue'
import { list as listInstances } from '@/helpers/instance'
import { isServerProjectLink } from '@/helpers/instance-project'
import { readProjectPreviewTitle } from '@/helpers/project-preview'
import { getSchematic, isSchematicFileName, isSchematicSource } from '@/helpers/schematics'
import { getBrowseDefaultInstanceId, setBrowseDefaultInstanceId } from '@/helpers/settings'
import { getThirdPartyAccount, thirdPartyAccountKey } from '@/helpers/third-party-accounts'
import type { GameInstance } from '@/helpers/types'
import { useBreadcrumbs } from '@/store/breadcrumbs'

const { formatMessage } = useVIntl()
const route = useRoute()
const router = useRouter()
const queryClient = useQueryClient()
const breadcrumbs = useBreadcrumbs()
const previewTitle = computed(() => readProjectPreviewTitle(route.path))
const projectId = computed(() => String(route.params.id ?? ''))
const source = computed(() =>
    isSchematicSource(route.params.source) ? route.params.source : 'reden',
)
const accountQuery = useQuery({
    queryKey: thirdPartyAccountKey,
    queryFn: getThirdPartyAccount,
    retry: false,
})
const connected = computed(() => accountQuery.data.value?.status === 'connected')
const projectQuery = useQuery({
    queryKey: computed(() => [
        'schematics',
        'detail',
        source.value,
        projectId.value,
        source.value === 'shimu' ? accountQuery.data.value?.user?.userId : null,
        source.value === 'shimu' ? accountQuery.data.value?.flowId : null,
    ]),
    queryFn: () => getSchematic(source.value, projectId.value),
    enabled: computed(() => !!projectId.value && (source.value !== 'shimu' || connected.value)),
    retry: false,
})
const project = projectQuery.data
const displayProject = computed(() => ({
    title: project.value?.title ?? '',
    description: project.value?.summary ?? '',
    icon_url: project.value?.cover ?? undefined,
    project_type: 'schematic',
    status: 'approved' as const,
    categories: [],
    game_versions: project.value?.gameVersions ?? [],
    loaders: [],
    client_side: 'unknown' as const,
    server_side: 'unknown' as const,
}))
const instanceQuery = useQuery({
    queryKey: ['schematic-install-targets'],
    queryFn: listInstances,
})
const instances = computed(() =>
    (instanceQuery.data.value ?? []).filter((instance) => !isServerProjectLink(instance.link)),
)
const selectedInstanceId = ref(
    typeof route.query.i === 'string' ? route.query.i : getBrowseDefaultInstanceId(),
)
const selectedInstance = computed(
    () => instances.value.find((instance) => instance.id === selectedInstanceId.value) ?? null,
)
const picker = ref<InstanceType<typeof BrowseInstanceSelector> | null>(null)
const activeTab = computed(() => (route.params.tab === 'files' ? 'files' : 'description'))
const projectTabs = computed(() => {
    const path = `/project/${source.value}/${encodeURIComponent(projectId.value)}`
    return [
        {
            label: formatMessage(messages.description),
            href: router.resolve({ path, query: route.query }).href,
        },
        {
            label: formatMessage(messages.files),
            href: router.resolve({ path: `${path}/files`, query: route.query }).href,
        },
    ]
})
const browseLink = computed(() => {
    if (typeof route.query.b === 'string') {
        try {
            const target = new URL(route.query.b, 'https://launcher.invalid')
            if (
                target.origin === 'https://launcher.invalid' &&
                target.pathname === '/browse/schematic'
            )
                return `${target.pathname}${target.search}`
        } catch {
            /* Use the category route when the origin is unavailable. */
        }
    }
    return { path: '/browse/schematic', query: { i: selectedInstance.value?.id } }
})
const description = computed(() => {
    if (project.value?.descriptionFormat !== 'html') return project.value?.description ?? ''
    const document = new DOMParser().parseFromString(project.value?.description ?? '', 'text/html')
    for (const attachment of document.querySelectorAll('.re-file')) attachment.remove()
    for (const element of document.querySelectorAll('[src], [href], [poster]')) {
        for (const attribute of ['src', 'href', 'poster']) {
            const value = element.getAttribute(attribute)
            if (value?.startsWith('/'))
                element.setAttribute(attribute, new URL(value, 'https://www.mcimg.com').href)
        }
    }
    return document.body.innerHTML
})
watch(
    project,
    (value) => {
        if (!value) return
        breadcrumbs.setName('Project', value.title)
        breadcrumbs.setNameIcon('Project', value.cover)
    },
    { immediate: true },
)
watch(projectQuery.isError, (failed) => {
    if (failed && source.value === 'shimu')
        void queryClient.invalidateQueries({ queryKey: thirdPartyAccountKey })
})
function chooseInstance(instance: GameInstance) {
    selectedInstanceId.value = instance.id
    setBrowseDefaultInstanceId(instance.id)
    void router.replace({ query: { ...route.query, i: instance.id } })
}
function preview(fileName: string) {
    void router.push({
        name: 'SchematicWorkshop',
        query: { source: source.value, project: projectId.value, file: fileName },
    })
}
</script>

<template>
    <template
        v-if="(source === 'shimu' && accountQuery.isPending.value) || projectQuery.isLoading.value"
    >
        <Teleport to="#sidebar-teleport-target">
            <ProjectPageSkeleton variant="sidebar" />
        </Teleport>
        <ProjectPageSkeleton :title="previewTitle" />
    </template>
    <EmptyState
        v-else-if="source === 'shimu' && !connected"
        class="p-6"
        :heading="formatMessage(messages.loginTitle)"
        :description="formatMessage(messages.loginDescription)"
    >
        <template #actions>
            <Button type="colored" :to="browseLink">{{ formatMessage(messages.login) }}</Button>
        </template>
    </EmptyState>
    <EmptyState
        v-else-if="!project || projectQuery.isError.value"
        class="p-6"
        :heading="formatMessage(messages.loadError)"
    >
        <template #actions>
            <Button @click="projectQuery.refetch()">{{ formatMessage(messages.retry) }}</Button>
        </template>
    </EmptyState>
    <div v-else>
        <Teleport to="#sidebar-teleport-target">
            <ProjectSidebarCompatibility
                :project="displayProject"
                :tags="{ loaders: [], gameVersions: [] }"
                class="gap-2 border-0 border-b border-solid border-[--brand-gradient-border] p-4"
            />
            <SidebarSection
                v-if="project.author"
                :title="formatMessage(messages.creator)"
                title-class="font-semibold"
                class="gap-2 border-0 border-b border-solid border-[--brand-gradient-border] p-4"
            >
                <span class="break-words font-medium">{{ project.author }}</span>
            </SidebarSection>
            <SidebarSection
                :title="formatMessage(messages.sourceHeading)"
                class="gap-2 border-0 border-b border-solid border-[--brand-gradient-border] p-4"
            >
                <span class="flex items-center gap-2">
                    <GlobeIcon class="size-5 text-[var(--color-text-tertiary)]" />
                    {{ formatMessage(schematicSourceMessages[source]) }}
                </span>
            </SidebarSection>
        </Teleport>
        <div class="flex flex-col gap-4 p-6">
            <ProjectHeader :project="displayProject" :show-followers="false">
                <template #actions>
                    <Button
                        size="xl"
                        class="flex min-w-0 items-center gap-2"
                        :disabled="instanceQuery.isPending.value || instanceQuery.isError.value"
                        @click="picker?.show()"
                    >
                        <InstanceIcon
                            v-if="selectedInstance"
                            class="shrink-0"
                            size="1.25rem"
                            :icon-path="selectedInstance.icon_path"
                            :instance-id="selectedInstance.id"
                            :loader="selectedInstance.loader"
                        />
                        <PlusIcon v-else class="size-5 shrink-0" />
                        <span class="max-w-40 truncate font-medium">
                            {{ selectedInstance?.name ?? formatMessage(messages.selectInstance) }}
                        </span>
                        <ChevronDownIcon
                            class="size-4 shrink-0 text-[var(--color-text-tertiary)]"
                        />
                    </Button>
                    <SchematicInstallButton
                        :project="project"
                        :instance-id="selectedInstance?.id ?? null"
                        prominent
                        @choose-instance="picker?.show()"
                        @authorization-error="
                            queryClient.invalidateQueries({ queryKey: thirdPartyAccountKey })
                        "
                    />
                </template>
            </ProjectHeader>
            <div
                v-if="instanceQuery.isError.value"
                role="alert"
                class="flex flex-wrap items-center gap-3"
            >
                <span>{{ formatMessage(messages.instancesError) }}</span>
                <Button @click="instanceQuery.refetch()">{{
                    formatMessage(messages.retry)
                }}</Button>
            </div>
            <NavTabs :links="projectTabs" route-sync />
            <Card v-if="activeTab === 'description'" class="m-0">
                <TranslatedProjectDescription
                    v-if="description.trim() && project.descriptionFormat !== 'plain'"
                    :description="description"
                    :format="project.descriptionFormat === 'html' ? 'html' : 'markdown'"
                    :active="false"
                    :translations="{}"
                    mode="bilingual"
                    :style="'weakened'"
                />
                <p v-else class="m-0 whitespace-pre-wrap">
                    {{ project.description || project.summary }}
                </p>
            </Card>
            <Card v-else class="m-0">
                <div class="divide-y divide-surface-4">
                    <div
                        v-for="file in project.files"
                        :key="`${project.source}:${project.id}:${file.name}`"
                        class="flex flex-wrap items-center gap-3 py-3 first:pt-0 last:pb-0"
                    >
                        <span class="min-w-0 flex-1 break-all font-medium">{{ file.name }}</span>
                        <Button
                            v-if="isSchematicFileName(file.name)"
                            type="quiet"
                            @click="preview(file.name)"
                        >
                            <EyeIcon />{{ formatMessage(messages.preview) }}
                        </Button>
                        <SchematicInstallButton
                            :project="project"
                            :instance-id="selectedInstance?.id ?? null"
                            :file-name="file.name"
                            @choose-instance="picker?.show()"
                            @authorization-error="
                                queryClient.invalidateQueries({ queryKey: thirdPartyAccountKey })
                            "
                        />
                    </div>
                </div>
            </Card>
        </div>
    </div>
    <BrowseInstanceSelector
        ref="picker"
        :instances="instances"
        :selected-instance="selectedInstance"
        :selected-count="0"
        :install-current="async () => true"
        :clear-current="() => {}"
        @select="chooseInstance"
    />
</template>
