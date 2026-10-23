<script setup lang="ts">
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { useVirtualScroll } from '@modrinth/ui/src/composables/virtual-scroll.ts'
import { computed, ref } from 'vue'

import Fuse from '../../../../lib/fuzzy-search.ts'
import { getSlotDisplay, type SlotDisplay } from '../display.ts'
import type { TextureAtlas } from '../resources.ts'
import type { CustomTag, RecipeSlotContext, SlotValue } from '../types.ts'
import RecipeItemIcon from './RecipeItemIcon.vue'
import RecipeSlotDragLayer from './RecipeSlotDragLayer.vue'

const props = defineProps<{
    vanillaTags: Record<string, string[]>
    customTags: CustomTag[]
    ctx: RecipeSlotContext
    atlas: TextureAtlas
}>()

const emit = defineEmits<{
    pick: [value: SlotValue]
    addCustomTag: [tag: CustomTag]
    updateCustomTag: [tag: CustomTag]
    deleteCustomTag: [uid: string]
}>()

const { formatMessage } = useVIntl()
const tab = ref<'vanilla' | 'custom'>('vanilla')
const search = ref('')
const newTagId = ref('')
const valueDrafts = ref<Record<string, string>>({})
let lastPickKey = ''
let lastPickAt = 0
const draggingTagUid = ref('')
let suppressClicksUntil = 0

const RECIPE_SLOT_MIME_TYPE = 'application/x-axolotl-recipe-slot'
const isTauriRuntime = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
const TAG_ROW_HEIGHT = 44.8

type StartDrag = (
    event: PointerEvent,
    value: SlotValue,
    display: SlotDisplay,
    atlas: TextureAtlas,
    onFinish?: (moved: boolean) => void,
) => void

const messages = defineMessages({
    vanillaTab: { id: 'app.lab.recipe-generator.tags.vanilla', defaultMessage: 'Vanilla tags' },
    customTab: { id: 'app.lab.recipe-generator.tags.custom', defaultMessage: 'Custom tags' },
    searchPlaceholder: {
        id: 'app.lab.recipe-generator.tags.search-placeholder',
        defaultMessage: 'Search tags',
    },
    empty: {
        id: 'app.lab.recipe-generator.tags.empty',
        defaultMessage: 'No tags match your search.',
    },
    addTag: { id: 'app.lab.recipe-generator.tags.add', defaultMessage: 'Add tag' },
    tagIdPlaceholder: {
        id: 'app.lab.recipe-generator.tags.id-placeholder',
        defaultMessage: 'namespace:tag_id',
    },
    tagValuesPlaceholder: {
        id: 'app.lab.recipe-generator.tags.values-placeholder',
        defaultMessage: 'One item or #tag per line',
    },
    deleteTag: { id: 'app.lab.recipe-generator.tags.delete', defaultMessage: 'Delete tag' },
    useTag: { id: 'app.lab.recipe-generator.tags.use', defaultMessage: 'Use in recipe' },
    noCustomTags: {
        id: 'app.lab.recipe-generator.tags.no-custom',
        defaultMessage: 'No custom tags yet.',
    },
})

const vanillaList = computed(() => Object.keys(props.vanillaTags).sort())
const fuse = computed(() => new Fuse(vanillaList.value, { threshold: 0.4, ignoreLocation: true }))
const visibleVanillaTags = computed(() => {
    const query = search.value.trim()
    if (!query) return vanillaList.value
    return fuse.value.search(query).map((result) => result.item)
})

const {
    listContainer,
    totalHeight,
    visibleTop,
    visibleItems: visibleVanillaRows,
} = useVirtualScroll(visibleVanillaTags, {
    itemHeight: TAG_ROW_HEIGHT,
    bufferSize: 8,
})

function vanillaDisplay(tagId: string) {
    return getSlotDisplay({ kind: 'vanilla_tag', id: tagId }, props.ctx)
}

function pickTag(value: SlotValue) {
    const key = JSON.stringify(value)
    const now = Date.now()
    if (key === lastPickKey && now - lastPickAt < 300) return
    lastPickKey = key
    lastPickAt = now
    emit('pick', value)
}

function pickFromClick(event: MouseEvent, value: SlotValue) {
    if (event.detail > 1 || Date.now() < suppressClicksUntil) return
    pickTag(value)
}

function onTagDragStart(event: DragEvent, value: SlotValue) {
    if (isTauriRuntime) {
        event.preventDefault()
        return
    }
    const dataTransfer = event.dataTransfer
    if (!dataTransfer) return
    const payload = JSON.stringify(value)
    dataTransfer.effectAllowed = 'copy'
    dataTransfer.setData(RECIPE_SLOT_MIME_TYPE, payload)
    dataTransfer.setData('text/plain', payload)
}

function onCustomTagDragStart(event: DragEvent, tag: CustomTag) {
    draggingTagUid.value = tag.uid
    onTagDragStart(event, { kind: 'custom_tag', uid: tag.uid })
}

function onDragEnd() {
    draggingTagUid.value = ''
    suppressClicksUntil = Date.now() + 350
}

function startPointerDrag(
    event: PointerEvent,
    value: SlotValue,
    display: SlotDisplay,
    startDrag: StartDrag,
    dragKey?: string,
) {
    if (!isTauriRuntime || event.button !== 0) return
    if (dragKey) draggingTagUid.value = dragKey
    startDrag(event, value, display, props.atlas, (moved) => {
        draggingTagUid.value = ''
        if (moved) suppressClicksUntil = Date.now() + 350
    })
}

function customTagDisplay(tag: CustomTag): SlotDisplay {
    return getSlotDisplay({ kind: 'custom_tag', uid: tag.uid }, props.ctx)
}

function addCustomTag() {
    const id = newTagId.value.trim()
    if (!id) return
    const tag: CustomTag = {
        uid: crypto.randomUUID(),
        id,
        values: [],
    }
    emit('addCustomTag', tag)
    valueDrafts.value[tag.uid] = ''
    newTagId.value = ''
}

function commitValues(tag: CustomTag) {
    const draft = valueDrafts.value[tag.uid] ?? ''
    const values = draft
        .split('\n')
        .map((line) => line.trim())
        .filter(Boolean)
        .map((line) =>
            line.startsWith('#')
                ? { type: 'tag' as const, id: line.slice(1) }
                : { type: 'item' as const, id: line },
        )
    emit('updateCustomTag', { ...tag, values })
}

function draftText(tag: CustomTag) {
    return tag.values.map((entry) => (entry.type === 'tag' ? `#${entry.id}` : entry.id)).join('\n')
}
</script>

<template>
    <RecipeSlotDragLayer v-slot="{ startDrag }">
        <div class="flex min-h-0 min-w-0 flex-1 flex-col gap-2 p-3">
            <div
                class="recipe-tag-tabs flex gap-1 rounded-[var(--radius-sm)] border border-surface-5 bg-surface-3 p-[0.2rem]"
                role="tablist"
            >
                <button
                    type="button"
                    class="recipe-tag-tab"
                    role="tab"
                    :aria-selected="tab === 'vanilla'"
                    :class="{ active: tab === 'vanilla' }"
                    @click="tab = 'vanilla'"
                >
                    {{ formatMessage(messages.vanillaTab) }}
                </button>
                <button
                    type="button"
                    class="recipe-tag-tab"
                    role="tab"
                    :aria-selected="tab === 'custom'"
                    :class="{ active: tab === 'custom' }"
                    @click="tab = 'custom'"
                >
                    {{ formatMessage(messages.customTab) }}
                </button>
            </div>

            <template v-if="tab === 'vanilla'">
                <StyledInput
                    v-model="search"
                    :placeholder="formatMessage(messages.searchPlaceholder)"
                    clearable
                    class="w-full shrink-0"
                />
                <div
                    v-if="!visibleVanillaTags.length"
                    class="flex min-h-24 items-center justify-center px-4 text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.empty) }}
                </div>
                <div v-else class="recipe-tag-scroll">
                    <div
                        ref="listContainer"
                        class="recipe-tag-virtual"
                        :style="{ height: `${totalHeight}px`, overflowAnchor: 'none' }"
                    >
                        <div class="recipe-tag-window" :style="{ top: `${visibleTop}px` }">
                            <button
                                v-for="item in visibleVanillaRows"
                                :key="item"
                                type="button"
                                :draggable="!isTauriRuntime"
                                class="recipe-tag-row"
                                :title="formatMessage(messages.useTag)"
                                :aria-label="`${formatMessage(messages.useTag)}: ${item}`"
                                :style="{ touchAction: isTauriRuntime ? 'none' : undefined }"
                                @click="pickFromClick($event, { kind: 'vanilla_tag', id: item })"
                                @pointerdown="
                                    startPointerDrag(
                                        $event,
                                        { kind: 'vanilla_tag', id: item },
                                        vanillaDisplay(item),
                                        startDrag,
                                    )
                                "
                                @dragstart="
                                    onTagDragStart($event, { kind: 'vanilla_tag', id: item })
                                "
                                @dragend="onDragEnd"
                            >
                                <RecipeItemIcon
                                    :display="vanillaDisplay(item)"
                                    :atlas="atlas"
                                    :size="26"
                                    :show-count="false"
                                />
                                <span>{{ item }}</span>
                            </button>
                        </div>
                    </div>
                </div>
            </template>

            <template v-else>
                <div class="flex gap-2">
                    <StyledInput
                        v-model="newTagId"
                        :placeholder="formatMessage(messages.tagIdPlaceholder)"
                        class="min-w-0 flex-1"
                        @keydown.enter.prevent="addCustomTag"
                    />
                    <button type="button" class="recipe-add-button" @click="addCustomTag">
                        {{ formatMessage(messages.addTag) }}
                    </button>
                </div>
                <div
                    v-if="!customTags.length"
                    class="flex min-h-24 items-center justify-center px-4 text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.noCustomTags) }}
                </div>
                <div v-else class="recipe-custom-tag-list">
                    <div
                        v-for="tag in customTags"
                        :key="tag.uid"
                        class="recipe-custom-tag"
                        :class="{ 'is-dragging': draggingTagUid === tag.uid }"
                    >
                        <div class="flex items-center gap-2">
                            <StyledInput
                                :model-value="tag.id"
                                size="small"
                                class="min-w-0 flex-1"
                                @update:model-value="
                                    emit('updateCustomTag', { ...tag, id: String($event) })
                                "
                            />
                            <button
                                type="button"
                                class="recipe-delete-button"
                                :title="formatMessage(messages.deleteTag)"
                                :aria-label="formatMessage(messages.deleteTag)"
                                @click="emit('deleteCustomTag', tag.uid)"
                            >
                                <TrashIcon />
                            </button>
                            <button
                                type="button"
                                class="recipe-add-button"
                                :draggable="!isTauriRuntime"
                                :title="formatMessage(messages.useTag)"
                                :aria-label="formatMessage(messages.useTag)"
                                :style="{ touchAction: isTauriRuntime ? 'none' : undefined }"
                                @click="pickFromClick($event, { kind: 'custom_tag', uid: tag.uid })"
                                @pointerdown="
                                    startPointerDrag(
                                        $event,
                                        { kind: 'custom_tag', uid: tag.uid },
                                        customTagDisplay(tag),
                                        startDrag,
                                        tag.uid,
                                    )
                                "
                                @dragstart="onCustomTagDragStart($event, tag)"
                                @dragend="onDragEnd"
                            >
                                <PlusIcon />
                            </button>
                        </div>
                        <textarea
                            :value="valueDrafts[tag.uid] ?? draftText(tag)"
                            :placeholder="formatMessage(messages.tagValuesPlaceholder)"
                            rows="2"
                            class="recipe-tag-values w-full resize-y rounded-[var(--radius-sm)] border border-surface-5 bg-surface-2 p-[0.4rem] font-mono text-xs leading-[1.4] text-[var(--color-text-primary)] outline-none"
                            @input="
                                valueDrafts[tag.uid] = ($event.target as HTMLTextAreaElement).value
                            "
                            @blur="commitValues(tag)"
                        ></textarea>
                    </div>
                </div>
            </template>
        </div>
    </RecipeSlotDragLayer>
</template>

<style scoped>
.recipe-tag-tab {
    flex: 1;
    border: 0;
    border-radius: calc(var(--radius-sm) - 1px);
    background: transparent;
    padding: 0.4rem 0.5rem;
    color: var(--color-text-tertiary);
    cursor: pointer;
    font-size: 0.75rem;
    font-weight: 700;
    white-space: nowrap;
}

.recipe-tag-tab.active {
    background: var(--color-brand);
    color: var(--color-accent-contrast);
}

.recipe-tag-scroll {
    display: flex;
    min-height: 0;
    flex: 1;
    flex-direction: column;
    overflow-y: auto;
    overscroll-behavior: contain;
}

.recipe-tag-virtual {
    position: relative;
    width: 100%;
    min-height: 0;
}

.recipe-tag-window {
    position: absolute;
    inset-inline: 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    padding: 0.1rem 0.25rem 0.25rem 0.1rem;
}

.recipe-tag-row {
    display: flex;
    width: 100%;
    height: 2.5rem;
    flex: 0 0 auto;
    align-items: center;
    gap: 0.5rem;
    border: 1px solid var(--color-surface-5);
    border-radius: var(--radius-sm);
    background: var(--color-surface-4);
    padding: 0.25rem 0.5rem;
    color: var(--color-text-primary);
    cursor: pointer;
    text-align: left;
    transition: border-color 0.15s ease;
}

.recipe-tag-row[draggable='true'],
.recipe-add-button[draggable='true'] {
    cursor: grab;
}

.recipe-tag-row:hover,
.recipe-tag-row:focus-visible {
    border-color: var(--color-brand);
    outline: none;
}

.recipe-tag-row span {
    min-width: 0;
    flex: 1;
    overflow: hidden;
    font-family: monospace;
    font-size: 0.7rem;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.recipe-custom-tag-list {
    display: flex;
    min-height: 0;
    flex: 1;
    flex-direction: column;
    gap: 0.5rem;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: 0.1rem 0.25rem 0.25rem 0.1rem;
}

.recipe-custom-tag {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    border: 1px solid var(--color-surface-5);
    border-radius: var(--radius-sm);
    background: var(--color-surface-3);
    padding: 0.5rem;
}

.recipe-tag-row:active,
.recipe-add-button:active {
    cursor: grabbing;
}

.recipe-custom-tag.is-dragging {
    opacity: 0.6;
    border-color: var(--color-brand);
}

.recipe-tag-values:focus {
    border-color: var(--color-brand);
}

.recipe-add-button,
.recipe-delete-button {
    display: inline-flex;
    height: 2rem;
    flex: 0 0 auto;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--color-surface-5);
    border-radius: var(--radius-sm);
    background: var(--color-surface-4);
    padding: 0 0.6rem;
    color: var(--color-text-primary);
    cursor: pointer;
    font-size: 0.75rem;
    font-weight: 700;
}

.recipe-add-button:hover {
    border-color: var(--color-brand);
}

.recipe-delete-button:hover {
    border-color: var(--color-red);
    color: var(--color-red);
}

.recipe-add-button svg,
.recipe-delete-button svg {
    width: 0.95rem;
    height: 0.95rem;
}
</style>
