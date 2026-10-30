<script setup lang="ts">
import { SearchIcon } from '@modrinth/assets'
import { Button, defineMessages, NewModal, StyledInput, useVIntl } from '@modrinth/ui'
import { computed, ref, useTemplateRef } from 'vue'

import BannerColorGrid from '@/components/lab/banner-designer/BannerColorGrid.vue'
import BannerPreview from '@/components/lab/banner-designer/BannerPreview.vue'
import { type DyeColor, dyeColorDefinition } from '@/lab/banner-designer/colors.ts'
import {
    colorNameMessages,
    patternCategoryMessages,
    patternNameMessages,
} from '@/lab/banner-designer/messages.ts'
import {
    BANNER_PATTERN_CATEGORIES,
    BANNER_PATTERNS,
    type BannerPatternId,
} from '@/lab/banner-designer/patterns.ts'
import {
    type BannerDesign,
    type BannerLayer,
    withLayerReplaced,
} from '@/lab/banner-designer/types.ts'

const props = defineProps<{
    design: BannerDesign
    layer?: BannerLayer | null
}>()

const emit = defineEmits<{
    submit: [pattern: BannerPatternId, color: DyeColor]
}>()

const { formatMessage, locale } = useVIntl()
const modal = useTemplateRef<InstanceType<typeof NewModal>>('modal')

const messages = defineMessages({
    addTitle: { id: 'app.lab.banner-designer.layer-editor.add-title', defaultMessage: 'Add layer' },
    editTitle: {
        id: 'app.lab.banner-designer.layer-editor.edit-title',
        defaultMessage: 'Edit layer',
    },
    description: {
        id: 'app.lab.banner-designer.layer-editor.description',
        defaultMessage: 'Choose a dye, then pick the pattern it paints.',
    },
    dyeColor: {
        id: 'app.lab.banner-designer.layer-editor.dye-color',
        defaultMessage: 'Dye color',
    },
    pattern: { id: 'app.lab.banner-designer.layer-editor.pattern', defaultMessage: 'Pattern' },
    search: {
        id: 'app.lab.banner-designer.layer-editor.search',
        defaultMessage: 'Search patterns',
    },
    empty: {
        id: 'app.lab.banner-designer.layer-editor.empty',
        defaultMessage: 'No patterns match your search.',
    },
    preview: {
        id: 'app.lab.banner-designer.layer-editor.preview',
        defaultMessage: 'Banner preview',
    },
    needsItem: {
        id: 'app.lab.banner-designer.layer-editor.needs-item',
        defaultMessage: 'Needs a banner pattern item',
    },
    cancel: { id: 'app.lab.banner-designer.layer-editor.cancel', defaultMessage: 'Cancel' },
    add: { id: 'app.lab.banner-designer.layer-editor.add', defaultMessage: 'Add layer' },
    save: { id: 'app.lab.banner-designer.layer-editor.save', defaultMessage: 'Save layer' },
})

const color = ref<DyeColor>('black')
const pattern = ref<BannerPatternId>('stripe_bottom')
const search = ref('')
/** The layer the dialog is open for, or `null` while adding a new one. */
const editing = ref<BannerLayer | null>(null)

const isEditing = computed(() => Boolean(editing.value))

const previewDesign = computed<BannerDesign>(() =>
    withLayerReplaced(props.design, editing.value?.id ?? null, {
        id: 'banner-layer-pending',
        pattern: pattern.value,
        color: color.value,
    }),
)
const previewColor = computed(() => dyeColorDefinition(color.value))

const groups = computed(() => {
    const query = search.value.trim().toLocaleLowerCase(locale.value)
    return BANNER_PATTERN_CATEGORIES.map((category) => ({
        category,
        patterns: BANNER_PATTERNS.filter((entry) => {
            if (entry.category !== category) return false
            if (!query) return true
            return [
                formatMessage(patternNameMessages[entry.id]),
                entry.id,
                formatMessage(patternCategoryMessages[entry.category]),
            ].some((value) => value.toLocaleLowerCase(locale.value).includes(query))
        }),
    })).filter((group) => group.patterns.length > 0)
})

function tileDesign(id: BannerPatternId): BannerDesign {
    return {
        baseColor: props.design.baseColor,
        layers: [{ id: `banner-layer-tile-${id}`, pattern: id, color: color.value }],
    }
}

/**
 * Opens the dialog for `layer`, or for a new layer when it is null. The layer is
 * handed over explicitly: the `layer` prop only catches up on the next render,
 * so reading it here would open the previously edited layer.
 */
function show(layer: BannerLayer | null = props.layer ?? null) {
    editing.value = layer
    color.value = layer?.color ?? 'black'
    pattern.value = layer?.pattern ?? 'stripe_bottom'
    search.value = ''
    modal.value?.show()
}

function submit() {
    emit('submit', pattern.value, color.value)
    modal.value?.hide()
}

defineExpose({ show })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(isEditing ? messages.editTitle : messages.addTitle)"
        width="min(58rem, calc(95vw - 10rem))"
        max-width="58rem"
    >
        <div class="flex min-h-0 gap-5 max-[820px]:flex-col">
            <div class="flex w-56 shrink-0 flex-col gap-4 max-[820px]:w-full">
                <div
                    class="flex flex-col items-center gap-2 rounded-[var(--radius-md)] bg-surface-2 p-3"
                >
                    <BannerPreview
                        :design="previewDesign"
                        :scale="4"
                        :label="formatMessage(messages.preview)"
                        class="rounded-[2px] border border-surface-5"
                    />
                    <span
                        class="flex items-center gap-1.5 text-xs text-[var(--color-text-tertiary)]"
                    >
                        <span
                            class="size-3 rounded-[3px] border border-[color-mix(in_srgb,var(--color-text-primary)_20%,transparent)]"
                            :style="{ backgroundColor: previewColor.hex }"
                            aria-hidden="true"
                        ></span>
                        {{ formatMessage(colorNameMessages[color]) }}
                    </span>
                </div>

                <div class="flex flex-col gap-2">
                    <h3
                        class="m-0 text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.dyeColor) }}
                    </h3>
                    <BannerColorGrid v-model="color" :label="formatMessage(messages.dyeColor)" />
                </div>
            </div>

            <div class="flex min-h-0 min-w-0 flex-1 flex-col gap-3">
                <StyledInput
                    v-model="search"
                    :icon="SearchIcon"
                    clearable
                    :placeholder="formatMessage(messages.search)"
                />

                <div class="h-[24rem] overflow-y-auto pr-1">
                    <p
                        v-if="groups.length === 0"
                        class="m-0 flex h-full items-center justify-center text-sm text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.empty) }}
                    </p>
                    <section v-for="group in groups" :key="group.category" class="mb-3 last:mb-0">
                        <h3
                            class="m-0 mb-2 text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]"
                        >
                            {{ formatMessage(patternCategoryMessages[group.category]) }}
                        </h3>
                        <div class="grid grid-cols-[repeat(auto-fill,minmax(5.5rem,1fr))] gap-2">
                            <button
                                v-for="entry in group.patterns"
                                :key="entry.id"
                                v-tooltip="
                                    entry.patternItem
                                        ? `${formatMessage(patternNameMessages[entry.id])} · ${formatMessage(messages.needsItem)}`
                                        : formatMessage(patternNameMessages[entry.id])
                                "
                                type="button"
                                class="flex min-w-0 cursor-pointer flex-col items-center gap-1.5 rounded-[var(--radius-md)] border border-surface-5 bg-surface-2 p-1.5 transition-[border-color,background-color] hover:bg-surface-4 focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-brand"
                                :class="
                                    pattern === entry.id ? 'border-brand bg-brand-highlight' : ''
                                "
                                :aria-pressed="pattern === entry.id"
                                @click="pattern = entry.id"
                            >
                                <BannerPreview
                                    :design="tileDesign(entry.id)"
                                    :scale="2"
                                    :label="formatMessage(patternNameMessages[entry.id])"
                                    class="rounded-[2px]"
                                />
                                <span
                                    class="line-clamp-2 text-center text-[11px] leading-tight text-[var(--color-text-tertiary)]"
                                >
                                    {{ formatMessage(patternNameMessages[entry.id]) }}
                                </span>
                            </button>
                        </div>
                    </section>
                </div>
            </div>
        </div>

        <template #actions>
            <div class="flex items-center justify-end gap-2">
                <Button type="quiet" @click="modal?.hide()">{{
                    formatMessage(messages.cancel)
                }}</Button>
                <Button type="colored" color="brand" @click="submit">{{
                    formatMessage(isEditing ? messages.save : messages.add)
                }}</Button>
            </div>
        </template>
    </NewModal>
</template>
