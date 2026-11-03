<template>
    <NewModal
        ref="modal"
        max-width="560px"
        :closable="true"
        :close-on-click-outside="true"
        :on-after-hide="() => emit('cancel')"
    >
        <template #title>
            <span class="text-[var(--color-text-primary)]">{{
                formatMessage(isMinecraftRoot ? messages.addMinecraftTitle : messages.title)
            }}</span>
        </template>

        <div class="flex flex-col gap-4">
            <p v-if="isMinecraftRoot" class="m-0">
                {{ formatMessage(messages.addMinecraftPrompt) }}
            </p>
            <span class="text-[var(--color-text-tertiary)] text-sm break-all">{{
                isMinecraftRoot ? minecraftRootPath : props.fileName
            }}</span>

            <div v-if="!isMinecraftRoot" class="grid grid-cols-2 gap-3">
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
            <div v-if="isMinecraftRoot" class="flex w-full justify-end gap-2">
                <Button type="outlined" @click="emit('confirm', 'instance')">
                    {{ formatMessage(messages.importMinecraftInstances) }}
                </Button>
                <Button color="brand" type="colored" @click="emit('confirm', 'dot_minecraft')">
                    {{ formatMessage(messages.addMinecraftConfirm) }}
                </Button>
            </div>
            <div v-else class="flex w-full items-center justify-between">
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
    </NewModal>
</template>

<script setup lang="ts">
import {
    BookIcon,
    BracesIcon,
    FolderOpenIcon,
    GridIcon,
    HelpCircleIcon,
    MapIcon,
    PackageOpenIcon,
    PaletteIcon,
    SparklesIcon,
} from '@modrinth/assets'
import type { Component } from 'vue'
import { computed, ref } from 'vue'

import BigOptionButton from '#ui/components/base/BigOptionButton.vue'
import Button from '#ui/components/base/buttons/Button.vue'
import NewModal from '#ui/components/modal/NewModal.vue'
import { useDebugLogger } from '#ui/composables/debug-logger'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import type { ClassificationResult } from '#ui/composables/use-global-drop'

const { formatMessage } = useVIntl()
const debug = useDebugLogger('ConfirmDropTypeModal')

const messages = defineMessages({
    title: {
        id: 'drop.confirm.title',
        defaultMessage: 'What would you like to import this as?',
    },
    addMinecraftTitle: {
        id: 'drop.confirm.minecraft-root.title',
        defaultMessage: 'Add .minecraft folder',
    },
    addMinecraftPrompt: {
        id: 'drop.confirm.minecraft-root.prompt',
        defaultMessage:
            'This looks like a .minecraft folder. Would you like to add it directly to the launcher?',
    },
    addMinecraftConfirm: {
        id: 'drop.confirm.minecraft-root.add',
        defaultMessage: 'Yes, add folder',
    },
    importMinecraftInstances: {
        id: 'drop.confirm.minecraft-root.import',
        defaultMessage: 'No, import instances',
    },
    cancel: {
        id: 'drop.confirm.cancel',
        defaultMessage: 'Cancel',
    },
    help: {
        id: 'drop.confirm.help',
        defaultMessage: 'What can I drop?',
    },
    notThisType: {
        id: 'drop.confirm.not-this',
        defaultMessage: 'Import this as something else',
    },
    dotMinecraftTitle: {
        id: 'drop.confirm.as-dot-minecraft',
        defaultMessage: '.minecraft',
    },
    dotMinecraftDesc: {
        id: 'drop.confirm.as-dot-minecraft-desc',
        defaultMessage: 'Scan a .minecraft folder to create instances',
    },
    launcherTitle: {
        id: 'drop.confirm.as-launcher',
        defaultMessage: 'Launcher',
    },
    hmclLauncherTitle: {
        id: 'drop.confirm.as-hmcl-launcher',
        defaultMessage: 'HMCL Launcher',
    },
    modTitle: {
        id: 'drop.confirm.as-mod',
        defaultMessage: 'Mod',
    },
    modDesc: {
        id: 'drop.confirm.as-mod-desc',
        defaultMessage: 'Install a mod JAR to a Minecraft instance',
    },
    instanceTitle: {
        id: 'drop.confirm.as-instance',
        defaultMessage: 'Instance',
    },
    instanceDesc: {
        id: 'drop.confirm.as-instance-desc',
        defaultMessage: 'Import Minecraft instances from another launcher',
    },
    modpackTitle: {
        id: 'drop.confirm.as-modpack',
        defaultMessage: 'Modpack',
    },
    modpackDesc: {
        id: 'drop.confirm.as-modpack-desc',
        defaultMessage: 'Create a new instance from a modpack',
    },
    resourcePackTitle: {
        id: 'drop.confirm.as-resource-pack',
        defaultMessage: 'Resource Pack',
    },
    resourcePackDesc: {
        id: 'drop.confirm.as-resource-pack-desc',
        defaultMessage: 'Install a resource pack to an instance',
    },
    dataPackTitle: {
        id: 'drop.confirm.as-data-pack',
        defaultMessage: 'Data Pack',
    },
    dataPackDesc: {
        id: 'drop.confirm.as-data-pack-desc',
        defaultMessage: 'Install a data pack to an save',
    },
    shaderPackTitle: {
        id: 'drop.confirm.as-shader-pack',
        defaultMessage: 'Shader Pack',
    },
    shaderPackDesc: {
        id: 'drop.confirm.as-shader-pack-desc',
        defaultMessage: 'Install a shader pack to an instance',
    },
    worldTitle: {
        id: 'drop.confirm.as-world',
        defaultMessage: 'World Save',
    },
    worldDesc: {
        id: 'drop.confirm.as-world-desc',
        defaultMessage: 'Import a world save into an instance',
    },
    schematicTitle: {
        id: 'drop.confirm.as-schematic',
        defaultMessage: 'Schematic',
    },
    schematicDesc: {
        id: 'drop.confirm.as-schematic-desc',
        defaultMessage: 'Import a Litematic schematic',
    },
})

interface DropOption {
    type: string
    icon: Component
    titleMsg: { id: string; defaultMessage: string }
    descMsg: { id: string; defaultMessage: string }
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
    (e: 'confirm', type: string, innerBase?: string): void
    (e: 'cancel' | 'help'): void
}>()

const modal = ref<InstanceType<typeof NewModal> | null>(null)

const minecraftRootPath = computed(
    () => props.classification?.base_path ?? props.classification?.file_path ?? '',
)
const isMinecraftRoot = computed(() => {
    const classification = props.classification
    return (
        classification?.item_type === 'launcher' &&
        classification.launcher_type === 'Generic' &&
        minecraftRootPath.value.split(/[\\/]/).filter(Boolean).at(-1)?.toLowerCase() ===
            '.minecraft'
    )
})

function handleCancel() {
    if (props.classification) {
        // First cancel: null the classification to show all options (custom mode)
        emit('cancel')
    } else {
        // Second cancel: close the modal (hide triggers @hide → emit('cancel'))
        modal.value?.hide()
    }
}

/** Groups of option types shown per classification. Keys are DroppedItemType variant names. */
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

/**
 * Lookup the user-facing type name for a classification's item_type.
 *
 * Uses `optionByType` for content types (mod, modpack, etc.) and falls
 * back to dedicated launcher messages for launcher types so the cancel
 * button correctly shows "This is not a Launcher" etc.
 */
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
    // Check display options first (content types)
    const option = optionByType[itemType]
    if (option) return formatMessage(option.titleMsg)
    // Fall back to launcher-type labels
    switch (itemType) {
        case 'launcher':
            return formatMessage(messages.launcherTitle)
        case 'hmcl_launcher':
            return formatMessage(messages.hmclLauncherTitle)
        default:
            return ''
    }
})

/**
 * Whether to show "这不是{type}" instead of "Cancel".
 * Only shown when there's a classification with a known type name.
 * Launcher types (no matching optionByType) fall back to "Cancel".
 */
const showNotThisType = computed((): boolean => {
    return !!props.classification && !!detectedTypeName.value
})

const visibleOptions = computed((): DropOption[] => {
    const classification = props.classification
    if (!classification) {
        const all = Object.values(optionByType).filter((o) => o.type !== 'instance')
        debug('visibleOptions: no classification, showing all options except instance', {
            count: all.length,
            types: all.map((o) => o.type),
        })
        return all
    }

    const itemType = classification.item_type
    debug('visibleOptions: input', {
        item_type: itemType,
        file_path: classification.file_path,
        launcher_type: classification.launcher_type,
    })

    if (itemType === 'shortcut_resolved' && classification.resolved_to) {
        const resolvedType = classification.resolved_to.item_type
        debug('visibleOptions: shortcut_resolved, delegating to resolved type', { resolvedType })
        if (resolvedType !== 'unknown' && resolvedType !== 'shortcut_resolved') {
            if (resolvedType === 'resource_pack') {
                const candidates = classification.resolved_to.candidates?.length
                    ? classification.resolved_to.candidates
                    : ['resource_pack']
                const result = candidates.map((c) => optionByType[c]).filter(Boolean)
                debug('visibleOptions: result from shortcut resource pack candidates', {
                    types: result.map((o) => o.type),
                })
                return result
            }
            const keys = OPTION_GROUPS[resolvedType]
            const result = keys ? keys.map((k) => optionByType[k]) : Object.values(optionByType)
            debug('visibleOptions: result from shortcut delegation', {
                types: result.map((o) => o.type),
            })
            return result
        }
    }

    if (itemType === 'launcher' || itemType === 'hmcl_launcher') {
        const result = OPTION_GROUPS.instance.map((k) => optionByType[k])
        debug('visibleOptions: launcher type, showing instance only', {
            types: result.map((o) => o.type),
        })
        return result
    }

    if (itemType === 'world_save') {
        const result = OPTION_GROUPS.world_save.map((k) => optionByType[k])
        debug('visibleOptions: world_save type, showing world only', {
            types: result.map((o) => o.type),
        })
        return result
    }

    if (itemType === 'multiple' && classification.choices?.length) {
        const result: DropOption[] = []
        for (const choice of classification.choices) {
            const baseOption = optionByType[choice.itemType]
            if (!baseOption) continue
            if (choice.itemType === 'resource_pack' && choice.candidates?.length) {
                for (const candidate of choice.candidates) {
                    const candidateOption = optionByType[candidate]
                    if (candidateOption) {
                        result.push({ ...candidateOption, innerBase: choice.innerBase })
                    }
                }
            } else {
                result.push({ ...baseOption, innerBase: choice.innerBase })
            }
        }
        debug('visibleOptions: multiple choices', {
            types: result.map((o) => o.type),
        })
        return result
    }

    if (itemType === 'resource_pack') {
        const candidates = classification.candidates?.length
            ? classification.candidates
            : ['resource_pack']
        const result = candidates.map((c) => optionByType[c]).filter(Boolean)
        debug('visibleOptions: resource pack candidates', {
            candidates,
            resultTypes: result.map((o) => o.type),
        })
        return result
    }

    const keys = OPTION_GROUPS[itemType]
    const result = keys ? keys.map((k) => optionByType[k]) : Object.values(optionByType)
    debug('visibleOptions: matched OPTION_GROUPS entry', {
        itemType,
        matchedGroup: keys,
        resultTypes: result.map((o) => o.type),
    })
    return result
})

function show() {
    modal.value?.show()
}

async function hide() {
    await modal.value?.hide()
}

defineExpose({ show, hide })
</script>
