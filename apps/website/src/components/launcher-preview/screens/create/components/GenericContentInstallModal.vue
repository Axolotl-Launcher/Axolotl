<script setup lang="ts">
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import FolderOpenIcon from '@modrinth/assets/icons/folder-open.svg?component'
import PackageOpenIcon from '@modrinth/assets/icons/package-open.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import InstanceRowCard from '@modrinth/ui/src/components/base/InstanceRowCard.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, ref } from 'vue'

import PreviewModal from '../../../overlays/PreviewModal.vue'
import type { GenericInstallTarget } from '../import-flow'
import { genericInstallMessages as messages } from '../messages'

/**
 * Port of `packages/ui/src/components/flows/drop/GenericContentInstallModal.vue`:
 * picks the instance a dropped content file goes into.
 */
const { formatMessage } = useVIntl()

const emit = defineEmits<{
    install: [instanceId: string]
    cancel: []
    navigateCreate: []
}>()

const modal = ref<InstanceType<typeof PreviewModal> | null>(null)
const searchQuery = ref('')
const internalInstances = ref<GenericInstallTarget[]>([])
const internalFileName = ref('')
const contentTypeLabel = ref('')

const contentTypeMap: Record<string, string> = {
    mod: 'Mod',
    resource_pack: 'Resource Pack',
    data_pack: 'Data Pack',
    shader_pack: 'Shader Pack',
    world_save: 'World',
    litematic: 'Schematic',
    schematic: 'Schematic',
}

const filteredInstances = computed(() => {
    const query = searchQuery.value.toLowerCase().trim()
    if (!query) return internalInstances.value
    return internalInstances.value.filter((instance) => instance.name.toLowerCase().includes(query))
})

function show(options: {
    contentType: string
    fileName: string
    instances?: GenericInstallTarget[]
}) {
    contentTypeLabel.value = contentTypeMap[options.contentType] ?? options.contentType
    internalFileName.value = options.fileName
    searchQuery.value = ''
    internalInstances.value = options.instances ?? []
    modal.value?.show()
}

function hide() {
    modal.value?.hide()
}

defineExpose({ show, hide })
</script>

<template>
    <PreviewModal
        ref="modal"
        max-width="560px"
        :closable="true"
        :close-on-click-outside="true"
        :on-hide="() => emit('cancel')"
    >
        <template #title>
            <span class="text-[var(--color-text-primary)]">{{
                formatMessage(messages.title, { type: contentTypeLabel })
            }}</span>
        </template>

        <div class="flex flex-col gap-4">
            <span class="truncate text-sm text-[var(--color-text-tertiary)]">{{
                internalFileName
            }}</span>

            <StyledInput
                v-model="searchQuery"
                :placeholder="formatMessage(messages.searchPlaceholder)"
                :icon="SearchIcon"
                class="w-full"
            />

            <div
                v-if="filteredInstances.length === 0"
                class="flex flex-col items-center gap-2 py-8 text-[var(--color-text-tertiary)]"
            >
                <PackageOpenIcon class="size-8" />
                <span class="text-sm">{{ formatMessage(messages.noInstances) }}</span>
            </div>

            <div v-else class="flex flex-col gap-2">
                <InstanceRowCard
                    v-for="instance in filteredInstances"
                    :key="instance.id"
                    :name="instance.name"
                    :version="instance.gameVersion"
                    :loader="instance.loader"
                    @select="emit('install', instance.id)"
                >
                    <template #prepend>
                        <FolderOpenIcon
                            v-if="!instance.iconUrl"
                            class="size-6 shrink-0 text-[var(--color-text-tertiary)]"
                            stroke-width="1.5"
                        />
                        <img
                            v-else
                            :src="instance.iconUrl"
                            alt=""
                            class="size-6 shrink-0 rounded object-cover"
                            loading="lazy"
                            decoding="async"
                        />
                    </template>
                    <template #append>
                        <Button size="2xs" @click.stop="emit('install', instance.id)">
                            <DownloadIcon class="size-4" />
                            {{ formatMessage(messages.install) }}
                        </Button>
                    </template>
                </InstanceRowCard>
            </div>

            <button
                class="hover:text-brand-hover flex items-center gap-2 self-start text-sm text-brand transition-colors hover:cursor-pointer"
                @click="emit('navigateCreate')"
            >
                <PlusIcon class="size-4" />
                {{ formatMessage(messages.createNew) }}
            </button>
        </div>

        <template #actions>
            <div class="flex w-full items-center justify-end">
                <Button type="quiet" @click="emit('cancel')">
                    {{ formatMessage(messages.cancel) }}
                </Button>
            </div>
        </template>
    </PreviewModal>
</template>
