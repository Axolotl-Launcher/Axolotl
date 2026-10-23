<script setup lang="ts">
import LeftArrowIcon from '@modrinth/assets/icons/left-arrow.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { type Component, computed, markRaw, onBeforeUnmount, onMounted, ref } from 'vue'

import PreviewModal from '../../../overlays/PreviewModal.vue'
import { flowMessages } from '../messages'
import { type CreationModalApi, type CreationStageId, useCreateArea } from '../state'
import CustomSetupStage from './CustomSetupStage.vue'
import ImportInstanceStage from './ImportInstanceStage.vue'
import SetupTypeStage from './SetupTypeStage.vue'

/**
 * The creation flow modal: `creation-flow-modal/index.vue` rendering
 * `MultiStageModal` with the instance flow's stage configs (`stages/*.ts`). The
 * flow mounts it without breadcrumbs, back button or progress bar, so the title is
 * the stage title and the stage's own buttons sit in the actions row.
 */
interface StageButtonConfig {
    label?: string
    icon?: Component | null
    iconPosition?: 'before' | 'after'
    color?: 'brand' | 'standard'
    disabled?: boolean
    loading?: boolean
    tooltip?: string
    iconClass?: string | null
    buttonClass?: string | null
    onboardingId?: string
    onClick?: () => void
}

interface StageConfig {
    id: CreationStageId
    title: string
    stageContent: Component
    skip: boolean
    cannotNavigateForward: boolean
    leftButtonConfig: StageButtonConfig | null
    rightButtonConfig: StageButtonConfig | null
    maxWidth: string
}

const { formatMessage } = useVIntl()
const area = useCreateArea()
const ctx = area.flow

const modal = ref<InstanceType<typeof PreviewModal> | null>(null)
const currentStageIndex = ref(0)

/** `custom-setup-stage.ts` `isForwardBlocked`. */
function isForwardBlocked(): boolean {
    if (!ctx.selectedGameVersion.value) return true
    if (!ctx.hideLoaderChips.value && !ctx.selectedLoader.value) return true
    if (
        !ctx.hideLoaderVersion.value &&
        !ctx.selectedLoaderVersion.value &&
        !loaderBuildsUnresolved()
    ) {
        return true
    }
    return false
}

/**
 * The snapshot lists the game version for the loader without recording its builds;
 * the install then resolves the selected channel (`selectedLoaderVersion ??
 * loaderVersionType` in `handleCreate`). A specific build ("Custom") still needs one.
 */
function loaderBuildsUnresolved(): boolean {
    const loader = ctx.selectedLoader.value
    const gameVersion = ctx.selectedGameVersion.value
    if (!loader || !gameVersion || ctx.loaderVersionType.value === 'other') return false
    const apiLoader = loader === 'neoforge' ? 'neo' : loader
    const entry = ctx.loaderVersionsCache.value[`${apiLoader}:${gameVersion}`]?.gameVersions.find(
        (item) => item.id === gameVersion,
    )
    return entry?.unresolvedBuilds ?? false
}

const setupTypeStage = markRaw(SetupTypeStage)
const importInstanceStage = markRaw(ImportInstanceStage)
const customSetupStage = markRaw(CustomSetupStage)

const stages = computed<StageConfig[]>(() => [
    {
        id: 'setup-type',
        title: formatMessage(flowMessages.createInstanceTitle),
        stageContent: setupTypeStage,
        skip: ctx.skipSetupType.value,
        cannotNavigateForward: false,
        leftButtonConfig: null,
        rightButtonConfig: null,
        maxWidth: '520px',
    },
    {
        id: 'import-instance',
        title: ctx.isImportMode.value
            ? formatMessage(flowMessages.importInstanceTitle)
            : formatMessage(flowMessages.chooseModpackTitle),
        stageContent: importInstanceStage,
        skip: !ctx.isImportMode.value && ctx.setupType.value !== 'modpack',
        cannotNavigateForward: false,
        leftButtonConfig: {
            label: formatMessage(commonMessages.backButton),
            icon: LeftArrowIcon,
            onClick: () => {
                if (ctx.skipSetupType.value) {
                    ctx.back()
                } else {
                    ctx.isImportMode.value = false
                    ctx.setupType.value = null
                    setStage('setup-type')
                }
            },
        },
        rightButtonConfig: null,
        maxWidth: '520px',
    },
    {
        id: 'custom-setup',
        title: formatMessage(flowMessages.createInstanceTitle),
        stageContent: customSetupStage,
        skip:
            ctx.setupType.value === 'modpack' ||
            ctx.setupType.value === 'vanilla' ||
            ctx.isImportMode.value,
        cannotNavigateForward: isForwardBlocked(),
        leftButtonConfig: {
            label: formatMessage(commonMessages.backButton),
            icon: LeftArrowIcon,
            onClick: () => {
                if (ctx.skipSetupType.value) {
                    ctx.back()
                } else {
                    setStage('setup-type')
                }
            },
        },
        rightButtonConfig: {
            label: formatMessage(flowMessages.createInstanceButton),
            icon: PlusIcon,
            iconPosition: 'before',
            color: 'brand',
            onboardingId: 'creation-confirm',
            disabled: isForwardBlocked(),
            loading: ctx.loading.value,
            onClick: () => ctx.finish(),
        },
        maxWidth: '550px',
    },
])

const currentStage = computed(() => stages.value[currentStageIndex.value] ?? stages.value[0])
const resolvedTitle = computed(() => currentStage.value?.title ?? '')
const leftButtonConfig = computed(() => currentStage.value?.leftButtonConfig ?? null)
const rightButtonConfig = computed(() => currentStage.value?.rightButtonConfig ?? null)
const resolvedMaxWidth = computed(() => currentStage.value?.maxWidth ?? '560px')

function setStage(indexOrId: number | CreationStageId) {
    let index = 0
    if (typeof indexOrId === 'number') {
        index = indexOrId
        if (index < 0 || index >= stages.value.length) return
    } else {
        index = stages.value.findIndex((stage) => stage.id === indexOrId)
        if (index === -1) return
    }
    while (index < stages.value.length) {
        if (!stages.value[index]?.skip) break
        index++
    }
    if (index < stages.value.length) currentStageIndex.value = index
}

function show() {
    modal.value?.show()
}

function hide() {
    modal.value?.hide()
}

const api: CreationModalApi = { show, hide, setStage }

onMounted(() => {
    ctx.modal.value = api
})

onBeforeUnmount(() => {
    if (ctx.modal.value === api) ctx.modal.value = null
})

defineExpose({ show, hide, setStage, currentStageIndex })
</script>

<template>
    <PreviewModal
        ref="modal"
        :scrollable="true"
        max-content-height="576px"
        :closable="true"
        :close-on-click-outside="false"
        :width="resolvedMaxWidth"
    >
        <template #title>
            <div class="flex w-full min-w-0 flex-col gap-2">
                <div class="flex w-full min-w-0 items-center gap-3">
                    <span
                        class="min-w-0 flex-1 text-lg font-bold text-[var(--color-text-primary)] sm:text-xl"
                        >{{ resolvedTitle }}</span
                    >
                </div>
            </div>
        </template>

        <component :is="currentStage?.stageContent" />

        <template #actions>
            <div
                class="flex flex-col justify-end gap-2 sm:flex-row"
                :class="leftButtonConfig || rightButtonConfig ? 'mt-4' : ''"
            >
                <Button
                    v-if="leftButtonConfig"
                    v-tooltip="leftButtonConfig.tooltip"
                    type="outlined"
                    :class="leftButtonConfig.buttonClass"
                    :disabled="leftButtonConfig.disabled"
                    @click="leftButtonConfig.onClick"
                    ><component :is="leftButtonConfig.icon" />
                    {{ leftButtonConfig.label }}
                </Button>
                <Button
                    v-if="rightButtonConfig"
                    v-tooltip="rightButtonConfig.tooltip"
                    :type="rightButtonConfig.color === 'standard' ? 'base' : 'colored'"
                    :color="
                        rightButtonConfig.color === 'standard' ? undefined : rightButtonConfig.color
                    "
                    :data-onboarding-id="rightButtonConfig.onboardingId"
                    :class="rightButtonConfig.buttonClass"
                    :disabled="rightButtonConfig.disabled"
                    :loading="rightButtonConfig.loading"
                    @click="rightButtonConfig.onClick"
                >
                    <SpinnerIcon
                        v-if="
                            rightButtonConfig.loading && rightButtonConfig.iconPosition === 'before'
                        "
                        class="animate-spin"
                    />
                    <component
                        :is="rightButtonConfig.icon"
                        v-else-if="rightButtonConfig.iconPosition === 'before'"
                        :class="rightButtonConfig.iconClass"
                    />
                    {{ rightButtonConfig.label }}
                    <SpinnerIcon
                        v-if="
                            rightButtonConfig.loading && rightButtonConfig.iconPosition === 'after'
                        "
                        class="animate-spin"
                    />
                    <component
                        :is="rightButtonConfig.icon"
                        v-else-if="rightButtonConfig.iconPosition === 'after'"
                        :class="rightButtonConfig.iconClass"
                    />
                </Button>
            </div>
        </template>
    </PreviewModal>
</template>
