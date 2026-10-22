<script setup lang="ts">
import '../../styles/overlays.css'

import { RightArrowIcon, ShieldIcon } from '@modrinth/assets'
import { PopoverArrow, PopoverContent, PopoverPortal, PopoverRoot, PopoverTrigger } from 'reka-ui'
import { computed, ref } from 'vue'

import { defineMessages, useVIntl } from '#ui/composables/i18n'
import type { ArmorPreviewConfig } from '#ui/composables/skin-rendering'

import ArmorTrimPanel from './ArmorTrimPanel.vue'

const messages = defineMessages({
    armorPreview: { id: 'skin.preview.armor.open', defaultMessage: 'Armor and trims' },
})

const {
    panel = 'popover',
    open = false,
    panelId,
} = defineProps<{
    panel?: 'popover' | 'external'
    /** Whether the panel this button opens is currently open. */
    open?: boolean
    /** id of the panel this external toggle controls, wired to `aria-controls`. */
    panelId?: string
}>()

const emit = defineEmits<{
    (e: 'open' | 'close'): void
}>()

const model = defineModel<ArmorPreviewConfig>({ required: true })
const { formatMessage } = useVIntl()
const isOpen = ref(false)
/**
 * The shield flips about its centre axis into a right arrow while the panel is
 * open, so the same button reads as "back to the list" and closes it.
 */
const isFlipped = computed(() => (panel === 'popover' ? isOpen.value : open))
const portalTarget = computed(() =>
    typeof document !== 'undefined' && document.getElementById('teleports') ? '#teleports' : 'body',
)
</script>

<template>
    <div
        v-if="panel === 'popover'"
        class="pointer-events-auto"
        @pointerdown.stop
        @pointermove.stop
        @pointerup.stop
        @click.stop
    >
        <PopoverRoot v-model:open="isOpen" :modal="false">
            <PopoverTrigger as-child>
                <button
                    class="flex h-10 min-w-0 cursor-pointer items-center justify-center gap-2 rounded-[14px] border-0 bg-surface-4 px-4 py-2.5 text-base font-semibold leading-5 shadow-md transition-[filter,transform] duration-200 hover:brightness-[--hover-brightness] focus-visible:brightness-[--hover-brightness] active:scale-95 [&>svg]:size-5 [&>svg]:shrink-0"
                    :aria-label="formatMessage(messages.armorPreview)"
                    :aria-expanded="isOpen"
                >
                    <span class="relative size-5 shrink-0">
                        <ShieldIcon
                            aria-hidden="true"
                            class="absolute inset-0 size-5 transition-[transform,opacity] duration-200 ease-out"
                            :class="isFlipped ? 'scale-x-0 opacity-0' : 'scale-x-100'"
                        />
                        <RightArrowIcon
                            aria-hidden="true"
                            class="absolute inset-0 size-5 transition-[transform,opacity] duration-200 ease-out"
                            :class="isFlipped ? 'scale-x-100' : 'scale-x-0 opacity-0'"
                        />
                    </span>
                    <span>{{ formatMessage(messages.armorPreview) }}</span>
                </button>
            </PopoverTrigger>
            <PopoverPortal :to="portalTarget">
                <PopoverContent
                    side="right"
                    align="start"
                    :side-offset="4"
                    class="menu-surface armor-preview-popover"
                    @pointerdown.stop
                    @pointermove.stop
                    @pointerup.stop
                    @click.stop
                >
                    <div
                        class="w-[min(19rem,calc(100vw-3rem))] max-h-[min(39rem,calc(100vh-8rem))] overflow-y-auto max-[520px]:max-h-[calc(100vh-6rem)]"
                    >
                        <ArmorTrimPanel v-model="model" />
                    </div>
                    <PopoverArrow class="menu-arrow" :width="14" :height="7" />
                </PopoverContent>
            </PopoverPortal>
        </PopoverRoot>
    </div>
    <div v-else class="pointer-events-auto">
        <button
            class="flex h-10 min-w-0 cursor-pointer items-center justify-center gap-2 rounded-[14px] border-0 bg-surface-4 px-4 py-2.5 text-base font-semibold leading-5 shadow-md transition-[filter,transform] duration-200 hover:brightness-[--hover-brightness] focus-visible:brightness-[--hover-brightness] active:scale-95 [&>svg]:size-5 [&>svg]:shrink-0"
            :aria-label="formatMessage(messages.armorPreview)"
            :aria-expanded="open"
            :aria-controls="panelId"
            @click="open ? emit('close') : emit('open')"
        >
            <span class="relative size-5 shrink-0">
                <ShieldIcon
                    aria-hidden="true"
                    class="absolute inset-0 size-5 transition-[transform,opacity] duration-200 ease-out"
                    :class="isFlipped ? 'scale-x-0 opacity-0' : 'scale-x-100'"
                />
                <RightArrowIcon
                    aria-hidden="true"
                    class="absolute inset-0 size-5 transition-[transform,opacity] duration-200 ease-out"
                    :class="isFlipped ? 'scale-x-100' : 'scale-x-0 opacity-0'"
                />
            </span>
            <span>{{ formatMessage(messages.armorPreview) }}</span>
        </button>
    </div>
</template>
