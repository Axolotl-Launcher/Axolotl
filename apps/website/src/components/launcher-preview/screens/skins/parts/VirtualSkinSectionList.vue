<script setup lang="ts">
import DropdownIcon from '@modrinth/assets/icons/dropdown.svg?component'
import EditIcon from '@modrinth/assets/icons/edit.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import UnknownIcon from '@modrinth/assets/icons/unknown.svg?component'
import Accordion from '@modrinth/ui/src/components/base/Accordion.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import SkinButton from '@modrinth/ui/src/components/skin/SkinButton.vue'
import SkinLikeTextButton from '@modrinth/ui/src/components/skin/SkinLikeTextButton.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { useScrollViewport } from '@modrinth/ui/src/composables/virtual-scroll.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, nextTick, onUnmounted, ref, watch } from 'vue'

import { getBakedSkinTextures, type RenderResult } from '../bake'
import { messages } from '../messages'
import type { Skin } from '../state'

/**
 * Port of the app's `VirtualSkinSectionList`
 * (apps/app-frontend/src/components/ui/skin). Sections are positioned by hand
 * from measured card sizes and only the ones near the viewport are rendered, so
 * a long list of default skins stays cheap. Reordering uses the HTML5 drag
 * events in place of `vuedraggable`, with the same ghost/chosen classes.
 */

type SkinSectionKind = 'saved' | 'default'
type SkinLikeTextButtonExpose = {
    getRootElement: () => HTMLElement | null | undefined
}
type AddSkinButtonRef = SkinLikeTextButtonExpose | SkinLikeTextButtonExpose[] | null

interface DefaultSkinSection {
    title: string
    infoTooltip?: string
    skins: Skin[]
}

interface SkinSection {
    key: string
    title: string
    kind: SkinSectionKind
    infoTooltip?: string
    skins: Skin[]
}

interface VirtualSkinSection {
    section: SkinSection
    top: number
    index: number
}

const SKIN_CARD_ASPECT_WIDTH = 31
const SKIN_CARD_ASPECT_HEIGHT = 40
const SKIN_GRID_GAP = 12
const SKIN_SECTION_FIRST_SPACING = 4
const SKIN_SECTION_SPACING = 24
const SKIN_SECTION_HEADER_HEIGHT = 28
const SKIN_SECTION_CONTENT_SPACING = 8
const SKIN_SECTION_OVERSCAN = 900
const FALLBACK_CARD_WIDTH = 220

const props = defineProps<{
    savedSkins: Skin[]
    defaultSkinSections: DefaultSkinSection[]
    isSkinSelected: (skin: Skin) => boolean
    isSkinActive: (skin: Skin) => boolean
    isAddSkinButtonDragActive: boolean
    readOnly?: boolean
    activeTab?: 'saved' | 'default'
}>()

const emit = defineEmits<{
    select: [skin: Skin]
    edit: [skin: Skin, event: MouseEvent]
    delete: [skin: Skin]
    'reorder-saved-skins': [skins: Skin[]]
    'add-skin': []
    'add-skin-dragenter': [event: DragEvent]
    'add-skin-dragover': [event: DragEvent]
    'add-skin-dragleave': [event: DragEvent]
    'add-skin-drop': [event: DragEvent]
}>()

const addSkinButton = ref<AddSkinButtonRef>(null)
const { formatMessage } = useVIntl()
const { listContainer, relativeScrollTop, scrollContainer, viewportHeight } = useScrollViewport()
const openSectionKeys = ref<Set<string>>(new Set())
const hasSettledInitialLayout = ref(false)
const knownSectionKeys = new Set<string>()
const listWidth = ref(0)
const windowWidth = ref(0)
let listResizeObserver: ResizeObserver | undefined
let enableLayoutTransitionsFrame: number | null = null
let isEnableLayoutTransitionsScheduled = false
let isUnmounted = false

function watchSizes() {
    if (typeof window === 'undefined') return
    windowWidth.value = window.innerWidth
    const element = listContainer.value
    if (element) {
        listResizeObserver?.disconnect()
        listResizeObserver = new ResizeObserver((entries) => {
            const entry = entries[0]
            if (entry) listWidth.value = entry.contentRect.width
        })
        listResizeObserver.observe(element)
    }
}

watch(
    listContainer,
    () => {
        watchSizes()
    },
    { flush: 'post' },
)

if (typeof window !== 'undefined') {
    window.addEventListener('resize', watchSizes, { passive: true })
}

const columnCount = computed(() => {
    // The launcher window is 1280 wide, so the 1300/1750/2050 breakpoints of the
    // real grid all resolve to the 3-column layout.
    if (windowWidth.value >= 2050) return 6
    if (windowWidth.value >= 1750) return 5
    if (windowWidth.value >= 1300) return 4
    return 3
})

const cardWidth = computed(() => {
    if (listWidth.value <= 0) return FALLBACK_CARD_WIDTH
    const gapsWidth = (columnCount.value - 1) * SKIN_GRID_GAP
    return Math.max(0, (listWidth.value - gapsWidth) / columnCount.value)
})

const cardHeight = computed(
    () => (cardWidth.value * SKIN_CARD_ASPECT_HEIGHT) / SKIN_CARD_ASPECT_WIDTH,
)

const sections = computed<SkinSection[]>(() => {
    if (props.activeTab === 'saved') {
        return [
            {
                key: 'saved-skins',
                title: formatMessage(messages.savedSkinsSection),
                kind: 'saved',
                skins: props.savedSkins,
            },
        ]
    }

    return props.defaultSkinSections.map((section) => ({
        key: `default-skins-${section.title}`,
        title: section.title,
        kind: 'default' as const,
        infoTooltip: section.infoTooltip,
        skins: section.skins,
    }))
})

const draggableSavedSkins = ref<Skin[]>([])
const isDraggingSavedSkin = ref(false)
const draggingIndex = ref<number | null>(null)
const dragOverIndex = ref<number | null>(null)
const canReorderSavedSkins = computed(() => draggableSavedSkins.value.length > 1)

const sectionLayouts = computed(() => {
    const layouts: Array<{ section: SkinSection; top: number; height: number; index: number }> = []
    let top = 0

    sections.value.forEach((section, index) => {
        const height = getSectionHeightEstimate(section, index)
        layouts.push({ section, top, height, index })
        top += height
    })

    return layouts
})

const totalHeight = computed(() => {
    const lastSection = sectionLayouts.value[sectionLayouts.value.length - 1]
    return lastSection ? lastSection.top + lastSection.height : 0
})

const visibleSections = computed<VirtualSkinSection[]>(() => {
    if (!listContainer.value || !scrollContainer.value) {
        return sectionLayouts.value.slice(0, 4)
    }

    const viewportStart = Math.max(0, relativeScrollTop.value - SKIN_SECTION_OVERSCAN)
    const viewportEnd = relativeScrollTop.value + viewportHeight.value + SKIN_SECTION_OVERSCAN

    return sectionLayouts.value
        .filter(
            (layout) => layout.top + layout.height >= viewportStart && layout.top <= viewportEnd,
        )
        .map(({ section, top, index }) => ({ section, top, index }))
})

watch(
    sections,
    (nextSections) => {
        const sectionKeys = new Set(nextSections.map((section) => section.key))
        const openKeys = new Set(openSectionKeys.value)

        for (const section of nextSections) {
            if (!knownSectionKeys.has(section.key)) {
                knownSectionKeys.add(section.key)
                openKeys.add(section.key)
            }
        }

        for (const key of knownSectionKeys) {
            if (!sectionKeys.has(key)) {
                knownSectionKeys.delete(key)
                openKeys.delete(key)
            }
        }

        openSectionKeys.value = openKeys
    },
    { immediate: true },
)

watch(
    () => props.savedSkins,
    (nextSkins) => {
        if (isDraggingSavedSkin.value) return
        draggableSavedSkins.value = nextSkins.filter(canDragSavedSkin)
    },
    { immediate: true },
)

watch(
    listWidth,
    (width) => {
        if (
            typeof window === 'undefined' ||
            width <= 0 ||
            hasSettledInitialLayout.value ||
            isEnableLayoutTransitionsScheduled
        ) {
            return
        }

        isEnableLayoutTransitionsScheduled = true
        void nextTick(() => {
            if (isUnmounted) return
            enableLayoutTransitionsFrame = window.requestAnimationFrame(() => {
                if (isUnmounted) return
                enableLayoutTransitionsFrame = window.requestAnimationFrame(() => {
                    if (isUnmounted) return
                    hasSettledInitialLayout.value = true
                    enableLayoutTransitionsFrame = null
                    isEnableLayoutTransitionsScheduled = false
                })
            })
        })
    },
    { immediate: true },
)

onUnmounted(() => {
    isUnmounted = true
    listResizeObserver?.disconnect()
    if (typeof window !== 'undefined') window.removeEventListener('resize', watchSizes)
    if (enableLayoutTransitionsFrame !== null) {
        window.cancelAnimationFrame(enableLayoutTransitionsFrame)
    }
})

function canDragSavedSkin(skin: Skin) {
    return skin.source === 'custom'
}

function savedSkinKey(skin: Skin) {
    return `saved-skin-${skin.source}-${skin.texture_key}-${skin.variant}-${skin.cape_id ?? 'no-cape'}`
}

function isSectionOpen(key: string) {
    return openSectionKeys.value.has(key)
}

function setSectionOpen(key: string, open: boolean) {
    const openKeys = new Set(openSectionKeys.value)
    if (open) openKeys.add(key)
    else openKeys.delete(key)
    openSectionKeys.value = openKeys
}

function getSectionHeightEstimate(section: SkinSection, index: number) {
    const spacing = index === 0 ? SKIN_SECTION_FIRST_SPACING : SKIN_SECTION_SPACING

    if (!isSectionOpen(section.key)) {
        return spacing + SKIN_SECTION_HEADER_HEIGHT
    }

    const cardCount = section.kind === 'saved' ? section.skins.length + 1 : section.skins.length
    const rowCount = Math.ceil(cardCount / columnCount.value)
    const gridHeight = rowCount * cardHeight.value + Math.max(0, rowCount - 1) * SKIN_GRID_GAP

    return spacing + SKIN_SECTION_HEADER_HEIGHT + SKIN_SECTION_CONTENT_SPACING + gridHeight
}

function setAddSkinButton(element: unknown) {
    addSkinButton.value = (element ?? null) as AddSkinButtonRef
}

function getAddSkinButtonElement() {
    const button = Array.isArray(addSkinButton.value)
        ? addSkinButton.value.find((candidate) => candidate?.getRootElement())
        : addSkinButton.value

    return button?.getRootElement()
}

defineExpose({ getAddSkinButtonElement })

/* ------------------------------------------------------------- reordering */

function onDragStart(index: number, event: DragEvent) {
    if (props.readOnly || !canReorderSavedSkins.value) return
    isDraggingSavedSkin.value = true
    draggingIndex.value = index
    dragOverIndex.value = index
    event.dataTransfer?.setData('text/plain', String(index))
    if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move'
}

function onDragOver(index: number, event: DragEvent) {
    if (draggingIndex.value === null) return
    event.preventDefault()
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'move'
    dragOverIndex.value = index
}

function onDrop(index: number) {
    const from = draggingIndex.value
    isDraggingSavedSkin.value = false
    draggingIndex.value = null
    dragOverIndex.value = null
    if (from === null || from === index) return

    const next = [...draggableSavedSkins.value]
    const [moved] = next.splice(from, 1)
    next.splice(index, 0, moved)
    draggableSavedSkins.value = next
    emit('reorder-saved-skins', next)
}

function onDragEnd() {
    isDraggingSavedSkin.value = false
    draggingIndex.value = null
    dragOverIndex.value = null
}

function bakedFor(skin: Skin): RenderResult | undefined {
    return getBakedSkinTextures(skin)
}
</script>

<template>
    <div
        ref="listContainer"
        class="relative w-full"
        :style="{ height: `${totalHeight}px`, overflowAnchor: 'none' }"
    >
        <div
            v-for="{ section, top, index } in visibleSections"
            :key="section.key"
            class="absolute inset-x-0"
            :class="[
                index === 0 ? 'pt-1' : 'pt-6',
                hasSettledInitialLayout
                    ? 'transition-transform duration-300 ease-in-out will-change-transform motion-reduce:transition-none'
                    : '',
            ]"
            :style="{ transform: `translateY(${top}px)` }"
        >
            <Accordion
                button-class="group flex w-full items-center gap-[6px] bg-transparent m-0 p-0 border-none cursor-pointer text-left"
                content-class="pt-2"
                :open-by-default="isSectionOpen(section.key)"
                @on-open="setSectionOpen(section.key, true)"
                @on-close="setSectionOpen(section.key, false)"
            >
                <template #title>
                    {{ section.title }}
                </template>
                <template #button="{ open }">
                    <DropdownIcon
                        class="size-6 shrink-0 text-[var(--color-text-default)] transition-transform duration-300"
                        :class="{ 'rotate-180': open }"
                    />
                    <span
                        class="min-w-0 text-xl font-semibold leading-7 text-[var(--color-text-default)]"
                    >
                        {{ section.title }}
                    </span>
                    <span
                        v-if="section.infoTooltip"
                        v-tooltip="{
                            content: section.infoTooltip,
                            placement: 'top',
                            popperClass: 'dismissable-prompt-tooltip',
                        }"
                        class="inline-flex size-6 shrink-0 items-center justify-center text-[var(--color-text-tertiary)] transition-colors group-hover:text-[var(--color-text-default)]"
                        @click.stop
                    >
                        <UnknownIcon class="size-5" />
                    </span>
                </template>

                <div
                    v-if="section.kind === 'saved'"
                    class="grid w-full grid-cols-3 gap-3 min-[1300px]:grid-cols-4 min-[1750px]:grid-cols-5 min-[2050px]:grid-cols-6"
                >
                    <SkinLikeTextButton
                        :ref="setAddSkinButton"
                        class="box-border aspect-[31/40] w-full min-w-0 rounded-[20px]"
                        dropzone
                        :disabled="readOnly"
                        :drag-active="!readOnly && isAddSkinButtonDragActive"
                        @click="emit('add-skin')"
                        @dragenter="emit('add-skin-dragenter', $event)"
                        @dragover="emit('add-skin-dragover', $event)"
                        @dragleave="emit('add-skin-dragleave', $event)"
                        @drop="emit('add-skin-drop', $event)"
                    >
                        <template #icon>
                            <PlusIcon class="size-8" />
                        </template>
                        {{ formatMessage(messages.addSkinButton) }}
                        <template #subtitle>{{
                            formatMessage(messages.dragAndDropSubtitle)
                        }}</template>
                    </SkinLikeTextButton>

                    <div
                        v-for="(skin, skinIndex) in draggableSavedSkins"
                        :key="savedSkinKey(skin)"
                        class="relative box-border aspect-[31/40] w-full min-w-0 rounded-[20px]"
                        :class="{
                            'skin-reorder-ghost': draggingIndex === skinIndex,
                            'skin-reorder-over':
                                dragOverIndex === skinIndex && draggingIndex !== skinIndex,
                        }"
                        :draggable="!readOnly && canReorderSavedSkins"
                        @dragstart="onDragStart(skinIndex, $event)"
                        @dragover="onDragOver(skinIndex, $event)"
                        @drop.prevent="onDrop(skinIndex)"
                        @dragend="onDragEnd"
                    >
                        <SkinButton
                            class="box-border h-full w-full min-w-0 rounded-[20px]"
                            :forward-image-src="bakedFor(skin)?.forwards"
                            :selected="isSkinSelected(skin)"
                            :active="isSkinActive(skin)"
                            :disabled="readOnly"
                            :is-dragging="isDraggingSavedSkin"
                            @select="emit('select', skin)"
                        >
                            <template v-if="!readOnly" #overlay-buttons>
                                <Button
                                    type="colored"
                                    color="brand"
                                    :aria-label="formatMessage(messages.editSkinButton)"
                                    class="pointer-events-auto"
                                    @click.stop="(event: MouseEvent) => emit('edit', skin, event)"
                                    ><EditIcon /> {{ formatMessage(commonMessages.editButton) }}
                                </Button>
                                <Button
                                    v-show="!skin.is_equipped"
                                    v-tooltip="formatMessage(messages.deleteSkinButton)"
                                    type="colored"
                                    color="red"
                                    circular
                                    icon-only
                                    :aria-label="formatMessage(messages.deleteSkinButton)"
                                    class="pointer-events-auto !rounded-[100%]"
                                    @click.stop="emit('delete', skin)"
                                    ><TrashIcon />
                                </Button>
                            </template>
                        </SkinButton>
                    </div>
                </div>

                <div
                    v-else
                    class="grid w-full grid-cols-3 gap-3 min-[1300px]:grid-cols-4 min-[1750px]:grid-cols-5 min-[2050px]:grid-cols-6"
                >
                    <SkinButton
                        v-for="skin in section.skins"
                        :key="`${section.key}-${skin.source}-${skin.texture_key}-${skin.variant}-${skin.cape_id ?? 'no-cape'}`"
                        class="box-border aspect-[31/40] w-full min-w-0 rounded-[20px]"
                        :forward-image-src="bakedFor(skin)?.forwards"
                        :selected="isSkinSelected(skin)"
                        :active="isSkinActive(skin)"
                        :tooltip="skin.name"
                        :disabled="readOnly"
                        :is-dragging="isDraggingSavedSkin"
                        @select="emit('select', skin)"
                    >
                        <template #overlay-buttons>
                            <Button
                                type="colored"
                                color="brand"
                                :aria-label="formatMessage(messages.editSkinButton)"
                                class="pointer-events-auto"
                                @click.stop="(event: MouseEvent) => emit('edit', skin, event)"
                                ><EditIcon /> {{ formatMessage(commonMessages.editButton) }}
                            </Button>
                        </template>
                    </SkinButton>
                </div>
            </Accordion>
        </div>
    </div>
</template>

<style scoped>
:global(.skin-reorder-ghost) {
    opacity: 0.35;
}

:global(.skin-reorder-over) {
    outline: 2px solid var(--color-brand);
    outline-offset: 2px;
    border-radius: var(--radius-xl);
}
</style>
