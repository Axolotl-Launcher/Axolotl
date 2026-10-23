<script setup lang="ts">
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import InstanceRowCard from '@modrinth/ui/src/components/base/InstanceRowCard.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, ref } from 'vue'

import PreviewModal from '../../../overlays/PreviewModal.vue'
import type { LauncherSelection, ScanInstance, ScanResult } from '../import-flow'
import { launcherImportMessages as messages } from '../messages'

/**
 * Port of `packages/ui/src/components/flows/drop/LauncherImportModal.vue`: the
 * instance checklist of a scanned launcher, grouped by launcher. A single instance
 * is confirmed right away, as in the app.
 */
const { formatMessage } = useVIntl()

const emit = defineEmits<{ confirm: [selections: LauncherSelection[]]; cancel: [] }>()

const modal = ref<InstanceType<typeof PreviewModal> | null>(null)
const internalResults = ref<ScanResult[]>([])
const selectedMap = ref(new Map<string, Set<string>>())
const isConfirming = ref(false)

const allInstances = computed(() => {
    const items: (ScanInstance & { launcherType: string; launcherName: string })[] = []
    for (const group of internalResults.value) {
        for (const instance of group.instances) {
            items.push({
                ...instance,
                launcherType: group.launcherType,
                launcherName: group.launcherName,
            })
        }
    }
    return items
})

const primaryLauncherName = computed(() => internalResults.value[0]?.launcherName ?? '')

// The app cannot know the base path from the scan result and shows the launcher name.
const primaryBasePath = computed(() => internalResults.value[0]?.launcherName ?? '')

const selectedCount = computed(() => {
    let count = 0
    for (const [, names] of selectedMap.value) count += names.size
    return count
})

function isSelected(launcherType: string, name: string): boolean {
    return selectedMap.value.get(launcherType)?.has(name) ?? false
}

function onHide() {
    if (isConfirming.value) {
        isConfirming.value = false
        return
    }
    emit('cancel')
}

function toggleInstance(launcherType: string, instance: ScanInstance) {
    const set = selectedMap.value.get(launcherType)
    if (!set) {
        selectedMap.value.set(launcherType, new Set([instance.name]))
    } else if (set.has(instance.name)) {
        set.delete(instance.name)
        if (set.size === 0) selectedMap.value.delete(launcherType)
    } else {
        set.add(instance.name)
    }
    selectedMap.value = new Map(selectedMap.value)
}

function selectAll() {
    const map = new Map<string, Set<string>>()
    for (const group of internalResults.value) {
        map.set(group.launcherType, new Set(group.instances.map((instance) => instance.name)))
    }
    selectedMap.value = map
}

function deselectAll() {
    selectedMap.value = new Map()
}

function handleConfirm() {
    if (selectedCount.value === 0) return
    isConfirming.value = true
    const selections: LauncherSelection[] = []
    for (const [launcherType, names] of selectedMap.value) {
        const group = internalResults.value.find((entry) => entry.launcherType === launcherType)
        if (!group) continue
        const instances = group.instances
            .filter((instance) => names.has(instance.name))
            .map((instance) => ({
                name: instance.name,
                path: instance.path,
                compatibleMode: instance.compatibleMode,
                versionPath: instance.versionPath,
            }))
        if (instances.length > 0) {
            selections.push({ launcherType, launcherName: group.launcherName, instances })
        }
    }
    modal.value?.hide()
    setTimeout(() => emit('confirm', selections), 0)
}

function show(results: ScanResult[]) {
    internalResults.value = results
    deselectAll()
    const items = allInstances.value
    if (items.length === 1) {
        const item = items[0]
        emit('confirm', [
            {
                launcherType: item.launcherType,
                launcherName: item.launcherName,
                instances: [
                    {
                        name: item.name,
                        path: item.path,
                        compatibleMode: item.compatibleMode,
                        versionPath: item.versionPath,
                    },
                ],
            },
        ])
        return
    }
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
        :on-hide="onHide"
    >
        <template #title>
            <span class="text-[var(--color-text-primary)]">{{
                formatMessage(messages.title, { launcherName: primaryLauncherName })
            }}</span>
        </template>

        <div class="flex flex-col gap-4">
            <span class="text-sm text-[var(--color-text-tertiary)]">
                {{
                    formatMessage(messages.subtitle, {
                        basePath: primaryBasePath,
                        count: allInstances.length,
                    })
                }}
            </span>

            <div class="flex items-center gap-4 text-sm">
                <button
                    class="hover:text-brand-hover cursor-pointer text-brand transition-colors"
                    @click="selectAll"
                >
                    {{ formatMessage(messages.selectAll) }}
                </button>
                <button
                    class="hover:text-brand-hover cursor-pointer text-brand transition-colors"
                    @click="deselectAll"
                >
                    {{ formatMessage(messages.deselectAll) }}
                </button>
                <span class="ml-auto text-[var(--color-text-tertiary)]">{{
                    formatMessage(messages.selected, { n: selectedCount })
                }}</span>
            </div>

            <div class="flex max-h-[360px] flex-col gap-3 overflow-y-auto">
                <div
                    v-for="group in internalResults"
                    :key="group.launcherType"
                    class="flex flex-col gap-1"
                >
                    <span
                        class="px-1 text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]"
                    >
                        {{ group.launcherName }}
                    </span>
                    <div class="flex flex-col gap-1">
                        <InstanceRowCard
                            v-for="instance in group.instances"
                            :key="`${group.launcherType}:${instance.name}`"
                            :name="instance.name"
                            :version="instance.version"
                            :loader="instance.loader"
                            @select="toggleInstance(group.launcherType, instance)"
                        >
                            <template #prepend>
                                <Checkbox
                                    :model-value="isSelected(group.launcherType, instance.name)"
                                    @update:model-value="
                                        toggleInstance(group.launcherType, instance)
                                    "
                                    @click.stop
                                />
                            </template>
                        </InstanceRowCard>
                    </div>
                </div>
            </div>

            <div
                v-if="allInstances.length === 0"
                class="flex items-center justify-center py-8 text-[var(--color-text-tertiary)]"
            >
                <span class="text-sm">{{ formatMessage(messages.noInstances) }}</span>
            </div>
        </div>

        <template #actions>
            <div class="flex w-full items-center justify-between">
                <Button type="quiet" @click="emit('cancel')"
                    >{{ formatMessage(messages.cancel) }}
                </Button>
                <Button
                    :disabled="selectedCount === 0"
                    class="flex items-center gap-2"
                    @click="handleConfirm"
                    ><DownloadIcon class="size-4" />
                    {{ formatMessage(messages.importAction, { n: selectedCount }) }}
                </Button>
            </div>
        </template>
    </PreviewModal>
</template>
