<template>
    <MultiStageModal
        ref="modal"
        :stages="ctx.stageConfigs"
        :context="ctx"
        :fade="fade"
        disable-progress
        @hide="handleHide"
    />
</template>

<script setup lang="ts">
import { computed, inject, useTemplateRef } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'

import { injectTags } from '../../../providers'
import MultiStageModal from '../../base/MultiStageModal.vue'
import {
    createCreationFlowContext,
    type CreationFlowContextValue,
    type FlowType,
    type LoaderManifestResolver,
    type ModpackSearchResult,
    type OptiFabricCompatibilityResolver,
    type ProjectVersionCompatibility,
    provideCreationFlowContext,
} from './creation-flow-context'

const props = withDefaults(
    defineProps<{
        type?: FlowType
        availableLoaders?: string[]
        showSnapshotToggle?: boolean
        disableClose?: boolean
        initialLoader?: string
        initialGameVersion?: string
        fetchExistingInstanceNames?: () => Promise<string[]>
        onBack?: (() => void) | null
        fade?: 'standard' | 'warning' | 'danger'
        searchModpacks?: (query: string, limit?: number) => Promise<ModpackSearchResult>
        getProjectVersions?: (projectId: string) => Promise<ProjectVersionCompatibility[]>
        hasCompatibleOptiFabric?: OptiFabricCompatibilityResolver
        getLoaderManifest?: LoaderManifestResolver
        finishDisabled?: boolean
        finishDisabledTooltip?: string
        /** Called when a file is dropped or picked in the import-instance stage */
        onImportFileReceived?: (payload: {
            file: File | null
            filePath: string | null
            source: 'file-picker' | 'drag-drop'
        }) => void
    }>(),
    {
        type: 'world',
        availableLoaders: () => ['fabric', 'neoforge', 'forge', 'quilt'],
        showSnapshotToggle: false,
        disableClose: false,
        initialLoader: undefined,
        initialGameVersion: undefined,
        fetchExistingInstanceNames: undefined,
        onBack: null,
    },
)

const emit = defineEmits<{
    (e: 'hide' | 'browse-modpacks'): void
    (e: 'create', config: CreationFlowContextValue): void
}>()

const modal = useTemplateRef<ComponentExposed<typeof MultiStageModal>>('modal')

const ctx = createCreationFlowContext(
    modal,
    props.type,
    {
        browseModpacks: () => emit('browse-modpacks'),
        create: (config) => emit('create', config),
    },
    {
        availableLoaders: props.availableLoaders,
        showSnapshotToggle: props.showSnapshotToggle,
        disableClose: props.disableClose,
        initialLoader: props.initialLoader,
        initialGameVersion: props.initialGameVersion,
        fetchExistingInstanceNames: props.fetchExistingInstanceNames,
        onBack: props.onBack ?? undefined,
        searchModpacks: props.searchModpacks,
        getProjectVersions: props.getProjectVersions,
        hasCompatibleOptiFabric: props.hasCompatibleOptiFabric,
        getLoaderManifest: props.getLoaderManifest,
        finishDisabled: computed(() => props.finishDisabled ?? false),
        finishDisabledTooltip: computed(() => props.finishDisabledTooltip),
        onImportFileReceived: props.onImportFileReceived,
    },
)
provideCreationFlowContext(ctx)
const setCtx = inject('setCreationFlowCtx', null) as ((c: typeof ctx) => void) | null
setCtx?.(ctx)

const tags = injectTags(null)

async function show(options?: {
    skipSetupType?: boolean
    initialMode?: 'custom' | 'import'
    onBack?: () => void
}) {
    ctx.skipSetupType.value = options?.skipSetupType ?? false
    ctx.onBack = options?.onBack ?? null
    // Newly published snapshots must be selectable without restarting the app.
    void tags?.refreshGameVersions?.()
    await ctx.reset()
    void ctx.prefetchLoaderMetadata()

    if (options?.skipSetupType) {
        if (options.initialMode === 'import') {
            ctx.setImportMode()
        } else {
            ctx.setSetupType('custom')
        }
    } else {
        modal.value?.setStage(0)
    }

    modal.value?.show()
}

function hide() {
    modal.value?.hide()
}

function handleHide() {
    emit('hide')
}

defineExpose({ show, hide, ctx })
</script>
