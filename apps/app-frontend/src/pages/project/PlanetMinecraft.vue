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
import { getPlanetMinecraftProject, type PlanetMinecraftVersion } from '@/helpers/planet-minecraft'
import { readProjectPreviewTitle } from '@/helpers/project-preview'
import { injectContentSelection } from '@/providers/content-selection'
import { injectDownloadManager } from '@/providers/download-manager'
import { useBreadcrumbs } from '@/store/breadcrumbs'

import SourceProjectSkeleton from './SourceProjectSkeleton.vue'

type PlanetMinecraftContentInstallRequest = {
    projectId: string
    versionId: string
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
const project = ref<Awaited<ReturnType<typeof getPlanetMinecraftProject>> | null>(null)
const loading = ref(false)
const busyVersionId = ref<string | null>(null)
let projectRequest: AbortController | null = null
let operationGeneration = 0
const manualDownload = ref<{
    instanceId: string
    verifiable: boolean
    request: PlanetMinecraftContentInstallRequest
    pageUrl: string
    fileName: string | null
} | null>(null)

const messages = defineMessages({
    unavailable: {
        id: 'app.project.planet-minecraft.unavailable',
        defaultMessage:
            'This Planet Minecraft project is unavailable. Reopen the project and try again.',
    },
    chooseInstance: { id: 'app.browse.choose-instance', defaultMessage: 'Choose instance' },
    versions: { id: 'app.project.planet-minecraft.versions', defaultMessage: 'Downloads' },
    install: { id: 'app.project.planet-minecraft.install', defaultMessage: 'Install' },
    manualTitle: {
        id: 'app.project.planet-minecraft.manual.title',
        defaultMessage: 'Manual download required',
    },
    manualDescription: {
        id: 'app.project.planet-minecraft.manual.description',
        defaultMessage: '{fileName} must be downloaded from Planet Minecraft, then imported here.',
    },
    unverifiable: {
        id: 'app.resources.manual.unverifiable',
        defaultMessage:
            'This source provides no file hash. Its artifact cannot be verified; download it from the source page and use the local import workflow.',
    },
    unknownFile: {
        id: 'app.project.planet-minecraft.unknown-file',
        defaultMessage: 'Downloaded file',
    },
    openSource: {
        id: 'app.project.planet-minecraft.manual.open-source',
        defaultMessage: 'Open source page',
    },
    importFile: {
        id: 'app.project.planet-minecraft.manual.import-file',
        defaultMessage: 'Import downloaded file',
    },
    pickFile: {
        id: 'app.project.planet-minecraft.manual.pick-file',
        defaultMessage: 'Choose downloaded archive',
    },
    archiveFilter: {
        id: 'app.project.planet-minecraft.manual.archive-filter',
        defaultMessage: 'Minecraft archives',
    },
    installed: {
        id: 'app.project.planet-minecraft.installed',
        defaultMessage: 'Installed {fileName}',
    },
})

const selectedInstance = computed(() => contentSelection.targetInstance.value)

watch(
    () => selectedInstance.value?.id,
    () => {
        operationGeneration++
        void preview.value?.cancel()
        manualDownload.value = null
        busyVersionId.value = null
    },
)
onScopeDispose(() => {
    projectRequest?.abort()
    operationGeneration++
})

function createRequest(version: PlanetMinecraftVersion): PlanetMinecraftContentInstallRequest {
    if (!project.value) throw new Error(formatMessage(messages.unavailable))
    return { projectId: project.value.id, versionId: version.id, projectType: 'mod' }
}

async function reviewInstall(
    instanceId: string,
    request: PlanetMinecraftContentInstallRequest,
    filePath?: string,
) {
    const key = 'selected-file'
    const source = 'planet_minecraft' as const
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
                release: { kind: 'release', reference: release },
                resourceType: 'mod',
                scope: { kind: 'instance' },
            },
        ],
        installDependencies: false,
        importFiles: filePath ? { [key]: filePath } : undefined,
    })
    if (job) downloads.trackJob(job)
    return !!job
}

async function install(version: PlanetMinecraftVersion) {
    if (!selectedInstance.value) {
        await contentSelection.refreshInstances(
            typeof route.query.i === 'string' ? route.query.i : undefined,
        )
        instanceSelector.value?.show()
        return
    }
    if (busyVersionId.value) return
    const instanceId = selectedInstance.value.id
    const generation = ++operationGeneration
    busyVersionId.value = version.id
    try {
        const request = createRequest(version)
        if (!version.download.sha256 || !version.download.directUrl) {
            manualDownload.value = {
                instanceId,
                verifiable: !!version.download.sha256,
                request,
                pageUrl: version.download.pageUrl,
                fileName: version.download.fileName ?? null,
            }
            return
        }
        await reviewInstall(instanceId, request)
    } catch (error) {
        if (generation === operationGeneration) handleError(error)
    } finally {
        if (generation === operationGeneration) busyVersionId.value = null
    }
}

async function importDownloadedFile() {
    const pending = manualDownload.value
    if (!pending || pending.instanceId !== selectedInstance.value?.id || busyVersionId.value) return
    const generation = ++operationGeneration
    busyVersionId.value = pending.request.versionId
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
        if (generation === operationGeneration) busyVersionId.value = null
    }
}

async function loadProject(id = route.params.id) {
    if (typeof id !== 'string' || !id) return
    projectRequest?.abort()
    const request = new AbortController()
    projectRequest = request
    operationGeneration++
    void preview.value?.cancel()
    busyVersionId.value = null
    project.value = null
    manualDownload.value = null
    loading.value = true
    try {
        const result = await getPlanetMinecraftProject(id, request.signal)
        request.signal.throwIfAborted()
        project.value = result
        breadcrumbs.setName('Project', project.value.title)
        breadcrumbs.setNameIcon('Project', project.value.icon_url ?? project.value.iconUrl ?? null)
    } catch (error) {
        if (!request.signal.aborted) handleError(error)
    } finally {
        if (projectRequest === request) loading.value = false
    }
}
watch(
    () => route.params.id,
    (id) => void loadProject(id),
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
        <section class="flex flex-wrap items-start justify-between gap-3">
            <div class="min-w-0">
                <h1 class="m-0 text-2xl font-bold text-[var(--color-text-primary)]">
                    {{ project.title }}
                </h1>
                <p v-if="project.summary" class="mb-0 mt-2 text-[var(--color-text-tertiary)]">
                    {{ project.summary }}
                </p>
            </div>
            <Button class="flex items-center gap-2" @click="instanceSelector?.show()"
                ><InstanceIcon
                    v-if="selectedInstance"
                    size="1.25rem"
                    :icon-path="selectedInstance.icon_path"
                    :instance-id="selectedInstance.id"
                    :loader="selectedInstance.loader"
                />
                {{ selectedInstance?.name ?? formatMessage(messages.chooseInstance) }}
            </Button>
        </section>
        <Card><ResourceDescription source="planet_minecraft" :project-id="project.id" /></Card>
        <Card v-if="manualDownload" class="flex flex-wrap items-center justify-between gap-3">
            <div class="min-w-0">
                <h2 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.manualTitle) }}
                </h2>
                <p class="mb-0 mt-1 text-sm text-[var(--color-text-tertiary)]">
                    {{
                        formatMessage(messages.manualDescription, {
                            fileName:
                                manualDownload.fileName ?? formatMessage(messages.unknownFile),
                        })
                    }}
                </p>
            </div>
            <div class="flex gap-2">
                <Button type="outlined" @click="openUrl(manualDownload!.pageUrl)"
                    ><ExternalIcon /> {{ formatMessage(messages.openSource) }}
                </Button>
                <Button
                    v-if="manualDownload.verifiable"
                    :disabled="busyVersionId !== null"
                    @click="importDownloadedFile"
                    ><FileArchiveIcon /> {{ formatMessage(messages.importFile) }}
                </Button>
            </div>
            <p v-if="!manualDownload.verifiable" class="m-0">
                {{ formatMessage(messages.unverifiable) }}
            </p>
        </Card>
        <section class="flex flex-col gap-3">
            <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.versions) }}
            </h2>
            <Card
                v-for="version in project.versions"
                :key="version.id"
                class="flex flex-wrap items-center justify-between gap-3"
            >
                <div class="min-w-0">
                    <div class="font-semibold text-[var(--color-text-primary)]">
                        {{ version.name }}
                    </div>
                    <div class="text-sm text-[var(--color-text-tertiary)]">
                        {{ version.gameVersions.join(', ') }}
                    </div>
                </div>
                <Button :disabled="busyVersionId !== null" @click="install(version)"
                    ><SpinnerIcon
                        v-if="busyVersionId === version.id"
                        class="animate-spin"
                    /><DownloadIcon v-else />
                    {{ formatMessage(messages.install) }}
                </Button>
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
        @select="contentSelection.setTarget"
    />
</template>
