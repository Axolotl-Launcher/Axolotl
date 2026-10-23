<script setup lang="ts">
import BookIcon from '@modrinth/assets/icons/book.svg?component'
import BracesIcon from '@modrinth/assets/icons/braces.svg?component'
import FolderOpenIcon from '@modrinth/assets/icons/folder-open.svg?component'
import GridIcon from '@modrinth/assets/icons/grid.svg?component'
import HelpCircleIcon from '@modrinth/assets/icons/help-circle.svg?component'
import MapIcon from '@modrinth/assets/icons/map.svg?component'
import PackageOpenIcon from '@modrinth/assets/icons/package-open.svg?component'
import PaletteIcon from '@modrinth/assets/icons/palette.svg?component'
import SparklesIcon from '@modrinth/assets/icons/sparkles.svg?component'
import BigOptionButton from '@modrinth/ui/src/components/base/BigOptionButton.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import type { MessageDescriptor } from '@modrinth/ui/src/composables/i18n.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import type { Component } from 'vue'
import { computed, ref } from 'vue'

import PreviewModal from '../../../overlays/PreviewModal.vue'
import type { ClassificationResult } from '../classify'
import { confirmDropMessages as messages } from '../messages'

/**
 * Port of `packages/ui/src/components/flows/drop/ConfirmDropTypeModal.vue`: "What
 * would you like to import this as?" with the options the classification allows,
 * the help link and the "Import this as something else" escape hatch.
 */
const { formatMessage } = useVIntl()

interface DropOption {
    type: string
    icon: Component
    titleMsg: MessageDescriptor
    descMsg: MessageDescriptor
    innerBase?: string
}

const optionByType: Record<string, DropOption> = {
    mod: {
        type: 'mod',
        icon: PackageOpenIcon,
        titleMsg: messages.modTitle,
        descMsg: messages.modDesc,
    },
    instance: {
        type: 'instance',
        icon: FolderOpenIcon,
        titleMsg: messages.instanceTitle,
        descMsg: messages.instanceDesc,
    },
    dot_minecraft: {
        type: 'dot_minecraft',
        icon: FolderOpenIcon,
        titleMsg: messages.dotMinecraftTitle,
        descMsg: messages.dotMinecraftDesc,
    },
    modpack: {
        type: 'modpack',
        icon: GridIcon,
        titleMsg: messages.modpackTitle,
        descMsg: messages.modpackDesc,
    },
    resource_pack: {
        type: 'resource_pack',
        icon: PaletteIcon,
        titleMsg: messages.resourcePackTitle,
        descMsg: messages.resourcePackDesc,
    },
    data_pack: {
        type: 'data_pack',
        icon: BracesIcon,
        titleMsg: messages.dataPackTitle,
        descMsg: messages.dataPackDesc,
    },
    shader_pack: {
        type: 'shader_pack',
        icon: SparklesIcon,
        titleMsg: messages.shaderPackTitle,
        descMsg: messages.shaderPackDesc,
    },
    world_save: {
        type: 'world_save',
        icon: MapIcon,
        titleMsg: messages.worldTitle,
        descMsg: messages.worldDesc,
    },
    litematic: {
        type: 'litematic',
        icon: BookIcon,
        titleMsg: messages.schematicTitle,
        descMsg: messages.schematicDesc,
    },
}

const props = defineProps<{
    classification: ClassificationResult | null
    fileName: string
}>()

const emit = defineEmits<{
    confirm: [type: string, innerBase?: string]
    cancel: []
    help: []
}>()

const modal = ref<InstanceType<typeof PreviewModal> | null>(null)

function handleCancel() {
    if (props.classification) {
        // First cancel: null the classification to show all options (custom mode).
        emit('cancel')
    } else {
        // Second cancel: close the modal (the hide emits `cancel`).
        void modal.value?.hide()
    }
}

/** Groups of option types shown per classification. */
const OPTION_GROUPS: Record<string, string[]> = {
    mod: ['mod'],
    instance: ['instance'],
    modpack: ['modpack'],
    resource_pack: ['resource_pack'],
    data_pack: ['data_pack'],
    shader_pack: ['shader_pack'],
    world_save: ['world_save'],
    litematic: ['litematic'],
}

const detectedTypeName = computed((): string => {
    const classification = props.classification
    const itemType = classification?.item_type
    if (!itemType) return ''
    if (itemType === 'resource_pack' && classification.candidates?.length === 1) {
        const candidate = optionByType[classification.candidates[0]]
        if (candidate) return formatMessage(candidate.titleMsg)
    }
    if (itemType === 'multiple' && classification.choices?.[0]) {
        const choice = optionByType[classification.choices[0].itemType]
        if (choice) return formatMessage(choice.titleMsg)
    }
    const option = optionByType[itemType]
    if (option) return formatMessage(option.titleMsg)
    switch (itemType) {
        case 'launcher':
            return formatMessage(messages.launcherTitle)
        case 'hmcl_launcher':
            return formatMessage(messages.hmclLauncherTitle)
        default:
            return ''
    }
})

const showNotThisType = computed(() => !!props.classification && !!detectedTypeName.value)

const visibleOptions = computed((): DropOption[] => {
    const classification = props.classification
    if (!classification)
        return Object.values(optionByType).filter((option) => option.type !== 'instance')

    const itemType = classification.item_type
    if (itemType === 'launcher' || itemType === 'hmcl_launcher') {
        return OPTION_GROUPS.instance.map((key) => optionByType[key])
    }
    if (itemType === 'world_save') return OPTION_GROUPS.world_save.map((key) => optionByType[key])
    if (itemType === 'multiple' && classification.choices?.length) {
        const result: DropOption[] = []
        for (const choice of classification.choices) {
            const base = optionByType[choice.itemType]
            if (!base) continue
            if (choice.itemType === 'resource_pack' && choice.candidates?.length) {
                for (const candidate of choice.candidates) {
                    const option = optionByType[candidate]
                    if (option) result.push({ ...option, innerBase: choice.innerBase })
                }
            } else {
                result.push({ ...base, innerBase: choice.innerBase })
            }
        }
        return result
    }
    if (itemType === 'resource_pack') {
        const candidates = classification.candidates?.length
            ? classification.candidates
            : ['resource_pack']
        return candidates.map((candidate) => optionByType[candidate]).filter(Boolean)
    }
    const keys = OPTION_GROUPS[itemType]
    return keys ? keys.map((key) => optionByType[key]) : Object.values(optionByType)
})

function show() {
    modal.value?.show()
}

async function hide() {
    await modal.value?.hide()
}

defineExpose({ show, hide })
</script>

<template>
    <PreviewModal
        ref="modal"
        max-width="560px"
        :closable="true"
        :close-on-click-outside="true"
        :on-after-hide="() => emit('cancel')"
    >
        <template #title>
            <span class="text-[var(--color-text-primary)]">{{
                formatMessage(messages.title)
            }}</span>
        </template>

        <div class="flex flex-col gap-4">
            <span class="text-sm text-[var(--color-text-tertiary)]">{{ fileName }}</span>

            <div class="grid grid-cols-2 gap-3">
                <BigOptionButton
                    v-for="option in visibleOptions"
                    :key="`${option.type}-${option.innerBase ?? ''}`"
                    :icon="option.icon"
                    :title="formatMessage(option.titleMsg)"
                    :description="formatMessage(option.descMsg)"
                    no-icon-border
                    @click="emit('confirm', option.type, option.innerBase)"
                />
            </div>
        </div>

        <template #actions>
            <div class="flex w-full items-center justify-between">
                <button
                    class="group/help flex items-center gap-2 transition-colors hover:cursor-pointer"
                    @click="emit('help')"
                >
                    <HelpCircleIcon
                        class="size-5 text-[var(--color-text-tertiary)] transition-colors group-hover/help:text-[var(--color-text-primary)]"
                    />
                    <span
                        class="text-sm text-[var(--color-text-tertiary)] transition-colors group-hover/help:text-[var(--color-text-primary)]"
                        >{{ formatMessage(messages.help) }}</span
                    >
                </button>
                <Button class="flex items-center gap-2" @click="handleCancel"
                    >{{
                        showNotThisType
                            ? formatMessage(messages.notThisType, { type: detectedTypeName })
                            : formatMessage(messages.cancel)
                    }}
                </Button>
            </div>
        </template>
    </PreviewModal>
</template>
