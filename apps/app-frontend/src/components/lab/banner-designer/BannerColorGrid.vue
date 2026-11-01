<script setup lang="ts">
import { CheckIcon } from '@modrinth/assets'
import { useVIntl } from '@modrinth/ui'

import BannerItemIcon from '@/components/lab/banner-designer/BannerItemIcon.vue'
import { DYE_COLOR_DEFINITIONS, type DyeColor, hexToRgb } from '@/lab/banner-designer/colors.ts'
import { colorNameMessages } from '@/lab/banner-designer/messages.ts'

withDefaults(
    defineProps<{
        label: string
        /** Shows the dye or banner item icon and the color name next to each swatch. */
        detailed?: boolean
        itemKind?: 'dye' | 'banner'
    }>(),
    {
        detailed: false,
        itemKind: 'dye',
    },
)

const selected = defineModel<DyeColor>({ required: true })
const { formatMessage } = useVIntl()

/**
 * Picks a black or white tick so the selection marker stays readable on every dye.
 * Uses the WCAG relative luminance: the bright dyes (white, yellow, lime, light gray,
 * orange, light blue, pink, cyan) fall above the threshold and get a dark tick.
 */
function tickColor(hex: string): string {
    const [red, green, blue] = hexToRgb(hex)
    const linear = (channel: number) => {
        const value = channel / 255
        return value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
    }
    const luminance = 0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue)
    return luminance > 0.25 ? '#000000' : '#ffffff'
}
</script>

<template>
    <div
        class="grid gap-2"
        :class="detailed ? 'grid-cols-4 max-[720px]:grid-cols-2' : 'grid-cols-8'"
        role="radiogroup"
        :aria-label="label"
    >
        <button
            v-for="color in DYE_COLOR_DEFINITIONS"
            :key="color.id"
            v-tooltip="formatMessage(colorNameMessages[color.id])"
            type="button"
            role="radio"
            :aria-checked="selected === color.id"
            class="flex min-w-0 cursor-pointer items-center rounded-[var(--radius-md)] border border-surface-5 bg-surface-2 transition-[border-color,background-color] hover:bg-surface-4 focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-brand"
            :class="[
                detailed ? 'gap-2 px-2 py-1.5 text-left' : 'justify-center p-1',
                selected === color.id ? 'border-brand bg-brand-highlight' : '',
            ]"
            @click="selected = color.id"
        >
            <span
                class="relative flex size-5 shrink-0 items-center justify-center rounded-[4px] border border-[color-mix(in_srgb,var(--color-text-primary)_20%,transparent)]"
                :style="{ backgroundColor: color.hex }"
                aria-hidden="true"
            >
                <CheckIcon
                    v-if="selected === color.id"
                    :class="detailed ? 'size-3.5' : 'size-3'"
                    :style="{ color: tickColor(color.hex) }"
                />
            </span>
            <template v-if="detailed">
                <span
                    class="min-w-0 flex-1 truncate text-sm text-[var(--color-text-primary)]"
                    :class="selected === color.id ? 'font-semibold' : ''"
                    >{{ formatMessage(colorNameMessages[color.id]) }}</span
                >
                <BannerItemIcon
                    :item="itemKind === 'banner' ? color.bannerItem : color.dyeItem"
                    :size="18"
                />
            </template>
        </button>
    </div>
</template>
