<script setup lang="ts">
import { DownloadIcon, ExternalIcon, FileArchiveIcon, SpinnerIcon } from '@modrinth/assets'
import {
    Button,
    Card,
    commonMessages,
    defineMessages,
    injectNotificationManager,
    useVIntl,
} from '@modrinth/ui'
import { open } from '@tauri-apps/plugin-dialog'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed, onScopeDispose, ref, watch } from 'vue'
import { useRoute } from 'vue-router'

import BrowseInstanceSelector from '@/components/browse/BrowseInstanceSelector.vue'
import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import ResourceDescription from '@/components/ui/ResourceDescription.vue'
import ResourceOperationPreviewModal from '@/components/ui/ResourceOperationPreviewModal.vue'
import {
    getMcArchiveModBySlug,
    type McArchiveFile,
    type McArchiveModVersion,
} from '@/helpers/mcarchive'
import { readProjectPreviewTitle } from '@/helpers/project-preview'
import { injectContentSelection } from '@/providers/content-selection'
import { injectDownloadManager } from '@/providers/download-manager'
import { useBreadcrumbs } from '@/store/breadcrumbs'

import SourceProjectSkeleton from './SourceProjectSkeleton.vue'

type McArchiveContentInstallRequest = {
    projectId: string
    projectSlug: string
    versionId: string
    fileId: string
    projectType: string
}

const route = useRoute()
const previewTitle = computed(() => readProjectPreviewTitle(route.path))
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const contentSelection = injectContentSelection()
const breadcrumbs = useBreadcrumbs()
const preview = ref<InstanceType<typeof ResourceOperationPreviewModal>>()
const downloads = injectDownloadManager()
const instanceSelector = ref<InstanceType<typeof BrowseInstanceSelector>>()
const project = ref<Awaited<ReturnType<typeof getMcArchiveModBySlug>> | null>(null)
const loading = ref(false)
const busyFileId = ref<string | null>(null)
let projectRequest: AbortController | null = null
let operationGeneration = 0
const manualDownload = ref<{
    instanceId: string
    request: McArchiveContentInstallRequest
    fileName: string
    pageUrl: string | null
    expectedSha256: string | null
} | null>(null)

const messages = defineMessages({
    unavailable: {
        id: 'app.project.mcarchive.unavailable',
        defaultMessage: 'This MCArchive project is unavailable. Reopen the project and try again.',
    },
    versions: {
        id: 'app.project.mcarchive.versions',
        defaultMessage: 'Versions',
    },
    chooseInstance: {
        id: 'app.browse.choose-instance',
        defaultMessage: 'Choose instance',
    },
    install: {
        id: 'app.project.mcarchive.install',
        defaultMessage: 'Install',
    },
    installed: {
        id: 'app.project.mcarchive.installed',
        defaultMessage: 'Installed {fileName}',
    },
    manualTitle: {
        id: 'app.project.mcarchive.manual.title',
        defaultMessage: 'Manual download required',
    },
    manualDescription: {
        id: 'app.project.mcarchive.manual.description',
        defaultMessage:
            '{fileName} has no verifiable direct download. Download it from the source page, then import the downloaded file.',
    },
    openSource: {
        id: 'app.project.mcarchive.manual.open-source',
        defaultMessage: 'Open source page',
    },
    importFile: {
        id: 'app.project.mcarchive.manual.import-file',
        defaultMessage: 'Import downloaded file',
    },
    pickFile: {
        id: 'app.project.mcarchive.manual.pick-file',
        defaultMessage: 'Choose downloaded archive',
    },
    archiveFilter: {
        id: 'app.project.mcarchive.manual.archive-filter',
        defaultMessage: 'Minecraft archives',
    },
    source: {
        id: 'app.project.mcarchive.source',
        defaultMessage: 'MCArchive source',
    },
    noFiles: {
        id: 'app.project.mcarchive.no-files',
        defaultMessage: 'No files are available for this release.',
    },
    unverifiable: {
        id: 'app.resources.manual.unverifiable',
        defaultMessage:
            'This source provides no file hash. Its artifact cannot be verified; download it from the source page and use the local import workflow.',
    },
    sha256: {
        id: 'app.project.mcarchive.sha256',
        defaultMessage: 'SHA-256',
    },
})

const selectedInstance = computed(() => contentSelection.targetInstance.value)

watch(
    () => selectedInstance.value?.id,
    () => {
        operationGeneration++
        void preview.value?.cancel()
        manualDownload.value = null
        busyFileId.value = null
    },
)
onScopeDispose(() => {
    projectRequest?.abort()
    operationGeneration++
})

function createRequest(version: McArchiveModVersion, file: McArchiveFile) {
    if (!project.value) throw new Error(formatMessage(messages.unavailable))
    return {
        projectId: project.value.uuid,
        projectSlug: project.value.slug,
        versionId: version.uuid,
        fileId: file.uuid,
        projectType: 'mod',
    } satisfies McArchiveContentInstallRequest
}

async function reviewInstall(
    instanceId: string,
    request: McArchiveContentInstallRequest,
    filePath?: string,
) {
    const key = 'selected-file'
    const source = 'mcarchive' as const
    const release = {
        project: { source, projectId: request.projectId },
        releaseId: request.versionId,
    }
    const job = await preview.value?.showInstall({
        instanceId,
        instanceName:
            contentSelection.instances.value.find((instance) => instance.id === instanceId)?.name ??
            selectedInstance.value?.name ??
            '',
        selections: [
            {
                id: key,
                resource: { kind: 'source', project: release.project },
                release: { kind: 'artifact', reference: { release, artifactId: request.fileId } },
                resourceType: 'mod',
                scope: { kind: 'instance' },
            },
        ],
        installDependencies: false,
        importFiles: filePath ? { [key]: filePath } : undefined,
        locators: { [key]: request.projectSlug },
    })
    if (job) downloads.trackJob(job)
    return !!job
}

async function install(version: McArchiveModVersion, file: McArchiveFile) {
    if (!selectedInstance.value) {
        await contentSelection.refreshInstances(
            typeof route.query.i === 'string' ? route.query.i : undefined,
        )
        instanceSelector.value?.show()
        return
    }
    if (busyFileId.value) return
    const instanceId = selectedInstance.value.id
    const generation = ++operationGeneration
    busyFileId.value = file.uuid
    try {
        const request = createRequest(version, file)
        if (!file.sha256 || !(file.archiveUrl || file.directUrl)) {
            manualDownload.value = {
                instanceId,
                request,
                fileName: file.name,
                pageUrl: file.pageUrl ?? file.redirectUrl ?? project.value?.pageUrl ?? null,
                expectedSha256: file.sha256 ?? null,
            }
            return
        }
        await reviewInstall(instanceId, request)
    } catch (error) {
        if (generation === operationGeneration) handleError(error)
    } finally {
        if (generation === operationGeneration) busyFileId.value = null
    }
}

async function importDownloadedFile() {
    const pending = manualDownload.value
    if (!pending || pending.instanceId !== selectedInstance.value?.id || busyFileId.value) return
    const generation = ++operationGeneration
    busyFileId.value = pending.request.fileId
    try {
        const path = await open({
            multiple: false,
            title: formatMessage(messages.pickFile),
            filters: [{ name: formatMessage(messages.archiveFilter), extensions: ['jar', 'zip'] }],
        })
        if (
            !path ||
            Array.isArray(path) ||
            generation !== operationGeneration ||
            manualDownload.value !== pending
        )
            return

        const accepted = await reviewInstall(pending.instanceId, pending.request, path)
        if (generation === operationGeneration && accepted) manualDownload.value = null
    } catch (error) {
        if (generation === operationGeneration) handleError(error)
    } finally {
        if (generation === operationGeneration) busyFileId.value = null
    }
}

async function selectInstance(instance: (typeof contentSelection.instances.value)[number]) {
    contentSelection.setTarget(instance)
}

async function loadProject(slug = route.params.slug) {
    if (typeof slug !== 'string' || !slug) return
    projectRequest?.abort()
    const request = new AbortController()
    projectRequest = request
    operationGeneration++
    void preview.value?.cancel()
    busyFileId.value = null
    project.value = null
    manualDownload.value = null
    loading.value = true
    try {
        const result = await getMcArchiveModBySlug(slug, request.signal)
        request.signal.throwIfAborted()
        project.value = result
        breadcrumbs.setName('Project', project.value.name)
        breadcrumbs.setNameIcon('Project', project.value.icon_url ?? project.value.iconUrl ?? null)
    } catch (error) {
        if (!request.signal.aborted) handleError(error)
    } finally {
        if (projectRequest === request) loading.value = false
    }
}
watch(
    () => route.params.slug,
    (slug) => void loadProject(slug),
    { immediate: true },
)

void contentSelection
    .refreshInstances(typeof route.query.i === 'string' ? route.query.i : undefined)
    .then(() => {
        if (typeof route.query.i !== 'string' || contentSelection.targetInstance.value) return
        const requested = contentSelection.instances.value.find(
            (instance) => instance.id === route.query.i,
        )
        if (requested) contentSelection.setTarget(requested)
    })
    .catch(handleError)
</script>

<template>
    <SourceProjectSkeleton v-if="loading" :title="previewTitle" />
    <div v-else-if="project" class="mx-auto flex w-full max-w-5xl flex-col gap-5 p-6">
        <section class="flex flex-col gap-3">
            <div class="flex flex-wrap items-start justify-between gap-3">
                <div class="min-w-0">
                    <h1 class="m-0 text-2xl font-bold text-[var(--color-text-primary)]">
                        {{ project.name }}
                    </h1>
                    <p v-if="project.summary" class="mb-0 mt-2 text-[var(--color-text-tertiary)]">
                        {{ project.summary }}
                    </p>
                </div>
                <Button
                    size="md"
                    class="flex min-w-0 items-center gap-2"
                    @click="instanceSelector?.show()"
                    ><InstanceIcon
                        v-if="selectedInstance"
                        class="shrink-0"
                        size="1.25rem"
                        :icon-path="selectedInstance.icon_path"
                        :instance-id="selectedInstance.id"
                        :loader="selectedInstance.loader"
                    />
                    <span class="max-w-48 truncate font-medium">
                        {{ selectedInstance?.name ?? formatMessage(messages.chooseInstance) }}
                    </span>
                </Button>
            </div>
            <ResourceDescription
                source="mcarchive"
                :project-id="project.uuid"
                :locator="project.slug"
            >
                <p
                    v-if="project.description"
                    class="m-0 whitespace-pre-wrap text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ project.description }}
                </p>
            </ResourceDescription>
        </section>

        <Card v-if="manualDownload" class="flex flex-col gap-3">
            <p v-if="!manualDownload.expectedSha256" class="m-0">
                {{ formatMessage(messages.unverifiable) }}
            </p>
            <div>
                <h2 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.manualTitle) }}
                </h2>
                <p class="mb-0 mt-1 text-sm text-[var(--color-text-tertiary)]">
                    {{
                        formatMessage(messages.manualDescription, {
                            fileName: manualDownload.fileName,
                        })
                    }}
                </p>
            </div>
            <div class="flex flex-wrap gap-2">
                <Button
                    v-if="manualDownload.pageUrl"
                    type="outlined"
                    @click="openUrl(manualDownload!.pageUrl!)"
                    ><ExternalIcon />
                    {{ formatMessage(messages.openSource) }}
                </Button>
                <Button
                    v-if="manualDownload.expectedSha256"
                    :disabled="busyFileId !== null"
                    @click="importDownloadedFile"
                    ><FileArchiveIcon />
                    {{ formatMessage(messages.importFile) }}
                </Button>
            </div>
        </Card>

        <section class="flex flex-col gap-3">
            <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.versions) }}
            </h2>
            <Card
                v-for="version in project.modVersions"
                :key="version.uuid"
                class="flex flex-col gap-3"
            >
                <div class="flex flex-wrap items-center justify-between gap-2">
                    <span class="font-semibold text-[var(--color-text-primary)]">{{
                        version.name
                    }}</span>
                    <span class="text-sm text-[var(--color-text-tertiary)]">
                        {{ version.gameVersions.map((gameVersion) => gameVersion.name).join(', ') }}
                    </span>
                </div>
                <div
                    v-if="version.files.length"
                    class="divide-y divide-surface-4 border-y border-surface-4"
                >
                    <div
                        v-for="file in version.files"
                        :key="file.uuid"
                        class="flex min-w-0 items-center gap-3 py-3"
                    >
                        <div class="min-w-0 flex-1">
                            <div
                                class="truncate text-sm font-medium text-[var(--color-text-primary)]"
                            >
                                {{ file.name }}
                            </div>
                            <div
                                v-if="file.sha256"
                                class="truncate text-xs text-[var(--color-text-tertiary)]"
                            >
                                {{ formatMessage(messages.sha256) }}: {{ file.sha256 }}
                            </div>
                        </div>
                        <Button
                            size="md"
                            :disabled="busyFileId !== null"
                            @click="install(version, file)"
                            ><SpinnerIcon v-if="busyFileId === file.uuid" class="animate-spin" />
                            <DownloadIcon v-else />
                            {{ formatMessage(messages.install) }}
                        </Button>
                    </div>
                </div>
                <p v-else class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.noFiles) }}
                </p>
            </Card>
        </section>
    </div>
    <div v-else class="mx-auto flex w-full max-w-5xl flex-col gap-5 p-6">
        <h1 v-if="previewTitle" class="m-0 text-2xl font-bold text-[var(--color-text-primary)]">
            {{ previewTitle }}
        </h1>
        <Card class="flex flex-wrap items-center gap-3" role="alert">
            <span>{{ formatMessage(messages.unavailable) }}</span>
            <Button @click="loadProject()">{{ formatMessage(commonMessages.retryButton) }}</Button>
        </Card>
    </div>
    <ResourceOperationPreviewModal ref="preview" />
    <BrowseInstanceSelector
        ref="instanceSelector"
        :instances="contentSelection.instances.value"
        :selected-instance="selectedInstance"
        :selected-count="0"
        :install-current="async () => true"
        :clear-current="() => {}"
        @select="selectInstance"
    />
</template>
