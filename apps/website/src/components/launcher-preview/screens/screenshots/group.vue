<script setup lang="ts">
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, onBeforeUnmount, ref, watch } from 'vue'

import type { PreviewScreenshot } from '../../lib/state/screenshots'
import ScreenshotCard from './card.vue'
import { messages } from './messages'
import ScreenshotSection from './section.vue'
import { getSelectionKey } from './state'

const props = defineProps<{
    id: string
    title: string
    screenshots: PreviewScreenshot[]
    selectedKeys: ReadonlySet<string>
    selectionActive: boolean
    activeDraggedKeys: ReadonlySet<string>
    showDropOutline: boolean
    canDrag: boolean
    dropInstanceId?: string
    dropCustomGroup?: boolean
    dropCustomGroupId?: string | null
    showInstanceName: boolean
    highlightedScreenshotId?: string
    copiedScreenshotIds: ReadonlySet<string>
    forceOpen: boolean
    hideHeader?: boolean
    editableTitle?: boolean
    startEditingTitle?: boolean
    maxTitleLength?: number
    validateTitle?: (value: string) => boolean
    onTitleChange?: (value: string) => boolean | void | Promise<boolean | void>
    sourceFor: (screenshot: PreviewScreenshot) => string
}>()

const collapsed = defineModel<boolean>('collapsed', { required: true })
const dropTarget = ref<HTMLElement>()
const { formatMessage } = useVIntl()

const shouldShowGrid = computed(() =>
    Boolean(props.hideHeader || props.forceOpen || !collapsed.value),
)
const renderGrid = ref(shouldShowGrid.value)
let unmountGridTimeout: ReturnType<typeof setTimeout> | undefined

watch(
    shouldShowGrid,
    (showGrid, previouslyShown) => {
        if (unmountGridTimeout) clearTimeout(unmountGridTimeout)
        if (showGrid) {
            renderGrid.value = true
            return
        }
        if (!previouslyShown) {
            renderGrid.value = false
            return
        }
        // The accordion animates its height; keep the grid mounted until it
        // has finished collapsing (the app's own 300ms delay).
        unmountGridTimeout = setTimeout(() => {
            renderGrid.value = false
            unmountGridTimeout = undefined
        }, 300)
    },
    { flush: 'post' },
)

onBeforeUnmount(() => {
    if (unmountGridTimeout) clearTimeout(unmountGridTimeout)
})

const emit = defineEmits<{
    (e: 'activate', screenshot: PreviewScreenshot, event: MouseEvent | KeyboardEvent): void
    (e: 'copy' | 'edit', screenshot: PreviewScreenshot): void
    (e: 'toggle-selection' | 'more', screenshot: PreviewScreenshot, event: MouseEvent): void
    (e: 'drag-start', screenshot: PreviewScreenshot, event: PointerEvent): void
}>()
</script>

<template>
    <div
        ref="dropTarget"
        class="group/instance-container relative select-none pb-3 transition-colors"
        :data-screenshot-group-id="id"
    >
        <Transition
            enter-active-class="transition-opacity duration-150 ease-out"
            enter-from-class="!opacity-0"
            enter-to-class="opacity-100"
            leave-active-class="transition-opacity duration-150 ease-in"
            leave-from-class="opacity-100"
            leave-to-class="!opacity-0"
        >
            <div
                v-if="showDropOutline"
                class="pointer-events-none absolute -inset-2 inset-y-0 z-20 rounded-xl border-2 border-dashed border-brand bg-brand-highlight opacity-40"
            />
        </Transition>
        <ScreenshotSection
            v-model:collapsed="collapsed"
            :title="title"
            :count="screenshots.length"
            :force-open="forceOpen"
            :hide-header="hideHeader"
            :editable="editableTitle"
            :start-editing="startEditingTitle"
            :max-title-length="maxTitleLength"
            :validate-title="validateTitle"
            :on-title-change="onTitleChange"
        >
            <template #actions="{ startEditing }">
                <slot name="actions" :start-editing="startEditing" />
            </template>
            <div v-if="renderGrid" class="relative min-h-[45px] w-full">
                <div
                    class="grid min-h-[45px] w-full grid-cols-1 gap-3 sm:grid-cols-2 2xl:grid-cols-4"
                >
                    <ScreenshotCard
                        v-for="screenshot in screenshots"
                        :key="getSelectionKey(screenshot)"
                        :screenshot="screenshot"
                        :selection-key="getSelectionKey(screenshot)"
                        :selected="selectedKeys.has(getSelectionKey(screenshot))"
                        :selection-active="selectionActive"
                        :active-dragged="activeDraggedKeys.has(getSelectionKey(screenshot))"
                        :can-drag="canDrag"
                        :show-instance-name="showInstanceName"
                        :highlighted="highlightedScreenshotId === screenshot.id"
                        :copied="copiedScreenshotIds.has(screenshot.id)"
                        :source="sourceFor(screenshot)"
                        @activate="(event) => emit('activate', screenshot, event)"
                        @toggle-selection="(event) => emit('toggle-selection', screenshot, event)"
                        @copy="emit('copy', screenshot)"
                        @edit="emit('edit', screenshot)"
                        @more="(event) => emit('more', screenshot, event)"
                        @drag-start="(event) => emit('drag-start', screenshot, event)"
                    />
                    <p
                        v-if="screenshots.length === 0"
                        key="empty-group"
                        class="font-base col-span-full m-0 pl-0.5 pt-1 text-base text-[var(--color-text-tertiary)] opacity-80"
                    >
                        {{ formatMessage(messages.emptyGroup) }}
                    </p>
                </div>
            </div>
        </ScreenshotSection>
    </div>
</template>
