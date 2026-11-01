<script setup lang="ts">
import { CheckIcon } from '@modrinth/assets'
import { Button, defineMessages, NewModal, Toggle, useVIntl } from '@modrinth/ui'
import { computed, ref, useTemplateRef } from 'vue'

import BannerColorGrid from '@/components/lab/banner-designer/BannerColorGrid.vue'
import BannerPreview from '@/components/lab/banner-designer/BannerPreview.vue'
import { DYE_COLORS, type DyeColor, dyeColorDefinition } from '@/lab/banner-designer/colors.ts'
import { colorNameMessages } from '@/lab/banner-designer/messages.ts'
import {
    type BannerColorReplaceOptions,
    describeColorUsage,
    replaceBannerColors,
} from '@/lab/banner-designer/recolor.ts'
import type { BannerDesign } from '@/lab/banner-designer/types.ts'

const props = defineProps<{ design: BannerDesign }>()

const emit = defineEmits<{
    replace: [options: BannerColorReplaceOptions]
}>()

const { formatMessage } = useVIntl()
const modal = useTemplateRef<InstanceType<typeof NewModal>>('modal')

const messages = defineMessages({
    title: { id: 'app.lab.banner-designer.replace-color.title', defaultMessage: 'Replace colors' },
    description: {
        id: 'app.lab.banner-designer.replace-color.description',
        defaultMessage: 'Pick the colors to change, then choose the color they become.',
    },
    colorsInUse: {
        id: 'app.lab.banner-designer.replace-color.colors-in-use',
        defaultMessage: 'Colors in use',
    },
    selectAll: {
        id: 'app.lab.banner-designer.replace-color.select-all',
        defaultMessage: 'Select all',
    },
    clear: { id: 'app.lab.banner-designer.replace-color.clear', defaultMessage: 'Clear' },
    empty: {
        id: 'app.lab.banner-designer.replace-color.empty',
        defaultMessage:
            'This banner has no patterns yet. Use Base color to recolor a plain banner.',
    },
    layerCount: {
        id: 'app.lab.banner-designer.replace-color.layer-count',
        defaultMessage: '{count, plural, one {# layer} other {# layers}}',
    },
    baseTag: { id: 'app.lab.banner-designer.replace-color.base-tag', defaultMessage: 'base' },
    includeBase: {
        id: 'app.lab.banner-designer.replace-color.include-base',
        defaultMessage: 'Also recolor the base',
    },
    replaceWith: {
        id: 'app.lab.banner-designer.replace-color.replace-with',
        defaultMessage: 'Replace with',
    },
    preview: { id: 'app.lab.banner-designer.replace-color.preview', defaultMessage: 'Preview' },
    cancel: { id: 'app.lab.banner-designer.replace-color.cancel', defaultMessage: 'Cancel' },
    confirm: { id: 'app.lab.banner-designer.replace-color.confirm', defaultMessage: 'Replace' },
})

/** Every color the patterns paint, with how many layers use it. */
const rows = computed(() =>
    describeColorUsage(props.design)
        .filter((usage) => usage.layers > 0)
        .map((usage) => ({ ...usage, hex: dyeColorDefinition(usage.color).hex })),
)

const selected = ref<DyeColor[]>([])
const target = ref<DyeColor>('white')
const includeBase = ref(true)

const allSelected = computed(
    () => rows.value.length > 0 && rows.value.every((row) => selected.value.includes(row.color)),
)
/** The base only changes when its color is one of the replaced colors. */
const baseSelected = computed(() => selected.value.includes(props.design.baseColor))
const canApply = computed(
    () => selected.value.length > 0 && selected.value.some((color) => color !== target.value),
)

const previewDesign = computed(() =>
    replaceBannerColors(props.design, {
        colors: selected.value,
        target: target.value,
        includeBase: includeBase.value,
    }),
)

/**
 * A replacement needs a color the design does not paint yet: picking one that is
 * already there would leave the button disabled, so the first free dye is the
 * default.
 */
function defaultTarget(): DyeColor {
    const painted = new Set(rows.value.map((row) => row.color))
    return DYE_COLORS.find((color) => !painted.has(color)) ?? 'white'
}

function toggle(color: DyeColor) {
    if (selected.value.includes(color)) {
        selected.value = selected.value.filter((entry) => entry !== color)
        return
    }
    selected.value = [...selected.value, color]
    // The switch is hidden while the base is not selected, so it must not come
    // back in a stale "off" state that contradicts the checked row.
    if (color === props.design.baseColor) includeBase.value = true
}

function toggleAll() {
    const selecting = !allSelected.value
    selected.value = selecting ? rows.value.map((row) => row.color) : []
    if (selecting) includeBase.value = true
}

function apply() {
    if (!canApply.value) return
    emit('replace', {
        colors: [...selected.value],
        target: target.value,
        includeBase: includeBase.value,
    })
    modal.value?.hide()
}

function show() {
    selected.value = rows.value.map((row) => row.color)
    target.value = defaultTarget()
    includeBase.value = true
    modal.value?.show()
}

defineExpose({ show })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        width="min(36rem, calc(95vw - 10rem))"
        max-width="36rem"
    >
        <div class="flex flex-col gap-4">
            <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.description) }}
            </p>

            <section class="flex flex-col gap-2">
                <div class="flex items-center justify-between gap-3">
                    <h3
                        class="m-0 text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.colorsInUse) }}
                    </h3>
                    <Button v-if="rows.length > 1" type="quiet" size="2xs" @click="toggleAll">{{
                        formatMessage(allSelected ? messages.clear : messages.selectAll)
                    }}</Button>
                </div>

                <div v-if="rows.length" class="flex flex-col gap-1.5">
                    <button
                        v-for="row in rows"
                        :key="row.color"
                        type="button"
                        role="checkbox"
                        :aria-checked="selected.includes(row.color)"
                        class="flex cursor-pointer items-center gap-2.5 rounded-[var(--radius-md)] border bg-surface-1 px-2.5 py-1.5 text-left transition-[border-color,background-color] hover:bg-surface-3 focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-brand"
                        :class="selected.includes(row.color) ? 'border-brand' : 'border-surface-5'"
                        @click="toggle(row.color)"
                    >
                        <span
                            class="flex size-4 shrink-0 items-center justify-center rounded-[4px] border"
                            :class="
                                selected.includes(row.color)
                                    ? 'border-brand bg-brand text-brand-inverted'
                                    : 'border-[color-mix(in_srgb,var(--color-text-primary)_25%,transparent)] bg-surface-2'
                            "
                            aria-hidden="true"
                        >
                            <CheckIcon
                                v-if="selected.includes(row.color)"
                                class="size-3"
                                stroke-width="3"
                            />
                        </span>
                        <span
                            class="size-5 shrink-0 rounded-[4px] border border-[color-mix(in_srgb,var(--color-text-primary)_20%,transparent)]"
                            :style="{ backgroundColor: row.hex }"
                            aria-hidden="true"
                        ></span>
                        <span
                            class="min-w-0 flex-1 truncate text-sm text-[var(--color-text-primary)]"
                        >
                            {{ formatMessage(colorNameMessages[row.color]) }}
                        </span>
                        <span
                            class="shrink-0 text-xs tabular-nums text-[var(--color-text-tertiary)]"
                        >
                            {{ formatMessage(messages.layerCount, { count: row.layers }) }}
                            <template v-if="row.base">
                                · {{ formatMessage(messages.baseTag) }}
                            </template>
                        </span>
                    </button>
                </div>
                <p
                    v-else
                    class="m-0 rounded-[var(--radius-md)] border border-dashed border-surface-5 p-4 text-center text-xs text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.empty) }}
                </p>
            </section>

            <div
                v-if="baseSelected"
                class="flex items-center justify-between gap-3 rounded-[var(--radius-md)] border border-surface-5 bg-surface-1 px-3 py-2"
            >
                <span class="min-w-0 text-sm text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.includeBase) }}
                </span>
                <Toggle
                    v-model="includeBase"
                    small
                    :aria-label="formatMessage(messages.includeBase)"
                />
            </div>

            <section v-if="rows.length" class="flex flex-col gap-2">
                <h3
                    class="m-0 text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.replaceWith) }}
                </h3>
                <div class="flex items-start gap-3">
                    <div class="min-w-0 flex-1">
                        <BannerColorGrid
                            v-model="target"
                            :label="formatMessage(messages.replaceWith)"
                        />
                    </div>
                    <BannerPreview
                        :design="previewDesign"
                        :scale="4"
                        :label="formatMessage(messages.preview)"
                        class="rounded-[2px] border border-surface-5"
                    />
                </div>
            </section>
        </div>

        <template #actions>
            <div class="flex items-center justify-end gap-2">
                <Button type="quiet" @click="modal?.hide()">{{
                    formatMessage(messages.cancel)
                }}</Button>
                <Button type="colored" color="brand" :disabled="!canApply" @click="apply">{{
                    formatMessage(messages.confirm)
                }}</Button>
            </div>
        </template>
    </NewModal>
</template>
