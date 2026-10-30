<script setup lang="ts">
import { GripVerticalIcon, PencilIcon, TrashIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import BannerItemIcon from '@/components/lab/banner-designer/BannerItemIcon.vue'
import BannerPreview from '@/components/lab/banner-designer/BannerPreview.vue'
import { dyeColorDefinition } from '@/lab/banner-designer/colors.ts'
import { colorNameMessages, patternNameMessages } from '@/lab/banner-designer/messages.ts'
import type { BannerDesign, BannerLayer } from '@/lab/banner-designer/types.ts'

const props = withDefaults(
    defineProps<{
        design: BannerDesign
        layer: BannerLayer
        /** Application order, 1 being the first pattern painted onto the banner. */
        order: number
        /** Dims the card while its copy follows the pointer. */
        dragging?: boolean
        /** Root element, so the drag preview can reuse the card outside a list. */
        as?: 'li' | 'div'
    }>(),
    {
        dragging: false,
        as: 'li',
    },
)

const emit = defineEmits<{
    edit: []
    remove: []
    move: [offset: number]
    dragstart: [event: PointerEvent]
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    drag: {
        id: 'app.lab.banner-designer.layer.drag',
        defaultMessage: 'Drag to reorder, or press the arrow keys',
    },
    moveUp: { id: 'app.lab.banner-designer.layer.move-up', defaultMessage: 'Move up' },
    moveDown: { id: 'app.lab.banner-designer.layer.move-down', defaultMessage: 'Move down' },
    edit: { id: 'app.lab.banner-designer.layer.edit', defaultMessage: 'Edit layer' },
    remove: { id: 'app.lab.banner-designer.layer.remove', defaultMessage: 'Delete layer' },
})

const color = computed(() => dyeColorDefinition(props.layer.color))
const layerDesign = computed<BannerDesign>(() => ({
    baseColor: props.design.baseColor,
    layers: [props.layer],
}))
</script>

<template>
    <component
        :is="as"
        class="group relative flex min-w-0 items-center gap-2 rounded-[var(--radius-md)] border border-surface-5 bg-surface-2 p-2 transition-[border-color,opacity,background-color]"
        :class="[dragging ? 'opacity-40' : '', 'hover:border-surface-5 hover:bg-surface-3']"
    >
        <span
            v-tooltip="formatMessage(messages.drag)"
            class="flex cursor-grab touch-none select-none items-center justify-center rounded-[var(--radius-sm)] p-0.5 text-[var(--color-text-tertiary)] active:cursor-grabbing"
            role="button"
            tabindex="0"
            aria-keyshortcuts="ArrowUp ArrowDown"
            :aria-label="`${formatMessage(messages.drag)} (${order})`"
            @pointerdown.prevent="emit('dragstart', $event)"
            @keydown.up.prevent="emit('move', -1)"
            @keydown.down.prevent="emit('move', 1)"
        >
            <GripVerticalIcon class="size-4" />
        </span>

        <BannerPreview
            :design="layerDesign"
            :scale="2"
            :label="formatMessage(patternNameMessages[layer.pattern])"
            class="rounded-[2px] border border-surface-5"
        />

        <div class="flex min-w-0 flex-1 flex-col gap-1">
            <div class="flex min-w-0 items-center gap-1.5">
                <span
                    class="flex size-4 shrink-0 items-center justify-center rounded-full bg-surface-4 text-[10px] font-semibold tabular-nums text-[var(--color-text-tertiary)]"
                    aria-hidden="true"
                    >{{ order }}</span
                >
                <strong class="truncate text-sm font-semibold text-[var(--color-text-primary)]">{{
                    formatMessage(patternNameMessages[layer.pattern])
                }}</strong>
            </div>
            <span
                class="flex min-w-0 items-center gap-1.5 text-xs text-[var(--color-text-tertiary)]"
            >
                <span
                    class="size-3 shrink-0 rounded-[3px] border border-[color-mix(in_srgb,var(--color-text-primary)_20%,transparent)]"
                    :style="{ backgroundColor: color.hex }"
                    aria-hidden="true"
                ></span>
                <BannerItemIcon :item="color.dyeItem" :size="14" />
                <span class="truncate">{{ formatMessage(colorNameMessages[layer.color]) }}</span>
            </span>
        </div>

        <div class="flex shrink-0 items-center gap-0.5">
            <Button
                v-tooltip="formatMessage(messages.edit)"
                type="quiet"
                size="2xs"
                circular
                icon-only
                :aria-label="formatMessage(messages.edit)"
                @click="emit('edit')"
                ><PencilIcon />
            </Button>
            <Button
                v-tooltip="formatMessage(messages.remove)"
                type="quiet"
                size="2xs"
                circular
                icon-only
                :aria-label="formatMessage(messages.remove)"
                @click="emit('remove')"
                ><TrashIcon />
            </Button>
        </div>
    </component>
</template>
