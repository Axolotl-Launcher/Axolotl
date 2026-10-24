<script setup lang="ts">
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import ClipboardCopyIcon from '@modrinth/assets/icons/clipboard-copy.svg?component'
import EditIcon from '@modrinth/assets/icons/edit.svg?component'
import MoreHorizontalIcon from '@modrinth/assets/icons/more-horizontal.svg?component'
import IconButton from '@modrinth/ui/src/components/base/buttons/IconButton.vue'
import { useFormatDateTime } from '@modrinth/ui/src/composables/format-date-time.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, onMounted, ref, watch } from 'vue'

import type { PreviewScreenshot } from '../../lib/state/screenshots'
import { messages } from './messages'
import { isSelectionModifier } from './screenshot-selection'
import { fileName, screenshotSources } from './state'

const props = defineProps<{
    screenshot: PreviewScreenshot
    selectionKey: string
    selected: boolean
    selectionActive: boolean
    activeDragged: boolean
    canDrag: boolean
    showInstanceName: boolean
    highlighted: boolean
    copied: boolean
    source: string
}>()

const emit = defineEmits<{
    (e: 'activate', event: MouseEvent | KeyboardEvent): void
    (e: 'copy' | 'edit'): void
    (e: 'toggle-selection' | 'more', event: MouseEvent): void
    (e: 'drag-start', event: PointerEvent): void
}>()

const card = ref<HTMLElement>()
const image = ref<HTMLImageElement>()
const imageReady = ref(false)
const failedSource = ref<string | null>(null)
const { formatMessage } = useVIntl()
const formatTime = useFormatDateTime({ dateStyle: 'medium', timeStyle: 'short' })
const name = computed(() => fileName(props.screenshot))

const imageUrl = computed(() => {
    if (failedSource.value === props.source) return screenshotSources(props.screenshot).gallery
    return props.source
})

function activate(event: MouseEvent | KeyboardEvent) {
    if (event instanceof KeyboardEvent) {
        if (event.target !== event.currentTarget || (event.key !== 'Enter' && event.key !== ' ')) {
            return
        }
        event.preventDefault()
    }
    emit('activate', event)
}

function startDrag(event: PointerEvent) {
    if (event.button !== 0 || event.pointerType !== 'mouse') return
    const target = event.target instanceof Element ? event.target : null
    if (target?.closest('button, input, a')) return
    if (!props.canDrag || !props.selected || isSelectionModifier(event)) return
    emit('drag-start', event)
}

async function markImageLoaded() {
    const loadedImage = image.value
    if (!loadedImage) return
    try {
        await loadedImage.decode()
    } catch {
        if (!loadedImage.complete || loadedImage.naturalWidth === 0) return
    }
    await new Promise<void>((resolve) => {
        window.requestAnimationFrame(() => window.requestAnimationFrame(() => resolve()))
    })
    if (image.value === loadedImage) imageReady.value = true
}

onMounted(() => {
    if (image.value?.complete && image.value.naturalWidth > 0) void markImageLoaded()
})

watch(
    () => props.source,
    () => {
        imageReady.value = false
        failedSource.value = null
    },
)
</script>

<template>
    <article
        ref="card"
        role="button"
        tabindex="0"
        class="group relative isolate aspect-video min-w-0 cursor-pointer overflow-hidden rounded-xl border border-solid border-surface-5 bg-surface-2 p-0 text-left shadow-sm transition-[filter] hover:brightness-110 focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand"
        :class="{
            '!border-brand brightness-110': selected,
            'animate-pulse !border-brand ring-2 ring-brand': highlighted,
            'opacity-50': activeDragged,
            'cursor-grab active:cursor-grabbing': canDrag && selected,
        }"
        data-screenshot-card
        :data-screenshot-id="screenshot.id"
        :data-selection-key="selectionKey"
        :aria-label="
            selectionActive
                ? formatMessage(selected ? messages.deselect : messages.select, { name })
                : name
        "
        :aria-pressed="selectionActive ? selected : undefined"
        @click="activate"
        @dragstart.prevent
        @pointerdown="startDrag"
        @contextmenu.prevent.stop="emit('more', $event)"
        @keydown="activate"
    >
        <button
            type="button"
            class="selection-button group/selection absolute right-0.5 top-0 z-[3] flex size-[50px] cursor-pointer items-start justify-center border-0 bg-transparent p-0 pt-4"
            :aria-label="formatMessage(selected ? messages.deselect : messages.select, { name })"
            :aria-pressed="selected"
            @click.stop="emit('toggle-selection', $event)"
        >
            <span
                class="relative flex size-6 items-center justify-center rounded-full opacity-0 transition-opacity duration-200 ease-out group-focus-within:opacity-100 group-hover:opacity-100 group-hover/selection:brightness-125"
                :class="
                    selected
                        ? 'border-0 !opacity-100'
                        : 'border-2 border-solid border-[var(--color-text-default)] bg-transparent'
                "
            >
                <span v-if="selected" class="absolute inset-0 rounded-full bg-brand" />
                <CheckIcon v-if="selected" class="relative size-4 invert [stroke-width:3]" />
            </span>
        </button>
        <img
            ref="image"
            :src="imageUrl"
            :alt="name"
            loading="lazy"
            decoding="async"
            draggable="false"
            class="screenshot-card-fade absolute inset-0 z-[1] h-full w-full object-cover"
            :class="imageReady ? 'opacity-100' : 'opacity-0'"
            @load="markImageLoaded"
            @error="failedSource = source"
        />
        <div
            aria-hidden="true"
            class="pointer-events-none absolute inset-0 bg-surface-4"
            :class="{ 'animate-pulse': !imageReady }"
        />
        <div
            class="absolute inset-x-0 bottom-0 z-[2] flex items-end justify-between gap-2 bg-gradient-to-t from-surface-1 to-transparent p-3 pt-[120px] text-[var(--color-text-primary)] opacity-0 transition-opacity duration-200 group-focus-within:opacity-100 group-hover:opacity-100"
        >
            <div class="min-w-0">
                <div v-tooltip="name" class="truncate text-sm font-semibold">
                    {{ name }}
                </div>
                <div class="truncate text-xs text-[var(--color-text-tertiary)]">
                    {{
                        showInstanceName
                            ? screenshot.instance_name
                            : formatTime(screenshot.taken_at)
                    }}
                </div>
            </div>
            <div
                v-if="!selectionActive"
                class="flex shrink-0 translate-y-1 gap-1 opacity-0 transition group-focus-within:translate-y-0 group-focus-within:opacity-100 group-hover:translate-y-0 group-hover:opacity-100"
                @click.stop
            >
                <IconButton
                    v-tooltip="formatMessage(messages.edit)"
                    :label="formatMessage(messages.edit)"
                    type="quiet"
                    class="bg-surface-2 text-[var(--color-text-primary)] hover:bg-surface-3"
                    @click="emit('edit')"
                >
                    <EditIcon />
                </IconButton>
                <IconButton
                    v-tooltip="formatMessage(copied ? messages.copied : messages.copy)"
                    :label="formatMessage(copied ? messages.copied : messages.copy)"
                    type="quiet"
                    class="bg-surface-2 text-[var(--color-text-primary)] hover:bg-surface-3"
                    @click="emit('copy')"
                >
                    <CheckIcon v-if="copied" class="text-green" />
                    <ClipboardCopyIcon v-else />
                </IconButton>
                <IconButton
                    v-tooltip="formatMessage(messages.moreActions)"
                    :label="formatMessage(messages.moreActions)"
                    type="quiet"
                    class="bg-surface-2 text-[var(--color-text-primary)] hover:bg-surface-3"
                    @click="emit('more', $event)"
                >
                    <MoreHorizontalIcon />
                </IconButton>
            </div>
        </div>
    </article>
</template>

<style scoped>
.screenshot-card-fade {
    transition: opacity 350ms ease-in-out;
}
</style>
