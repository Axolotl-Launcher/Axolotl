<script setup lang="ts">
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import RotateCounterClockwiseIcon from '@modrinth/assets/icons/rotate-counter-clockwise.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import ArmorTrimPanel from '@modrinth/ui/src/components/skin/ArmorTrimPanel.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import {
    ARMOR_SLOTS,
    type ArmorPreviewConfig,
} from '@modrinth/ui/src/composables/skin-rendering/armor-preview-types.ts'
import { computed, onMounted, onUnmounted } from 'vue'

import { messages } from '../messages'

/**
 * Port of the app's `ArmorTrimTab` (apps/app-frontend/src/components/ui/skin):
 * the draft armour configuration, hosted in the list column while the armour
 * toggle is open, with its own save / reset / restore-defaults actions.
 */
const draft = defineModel<ArmorPreviewConfig>({ required: true })

const props = defineProps<{
    saved: ArmorPreviewConfig
    /** Stable id shared with the toggle so it can point `aria-controls` here. */
    panelId: string
}>()

const emit = defineEmits<{
    save: []
    reset: []
    defaults: []
    close: []
}>()

const { formatMessage } = useVIntl()

/**
 * The tab is mounted only while it is showing, so this listens exactly for that
 * window. Capture phase, like the app: anything closer could otherwise stop the
 * event before it reaches a bubble listener. A modal on top owns Escape while
 * it is up, so the key is left to it.
 */
function handleKeydown(event: KeyboardEvent) {
    if (event.key !== 'Escape') return
    if (
        event.target instanceof Element &&
        event.target.closest('[role="dialog"], [role="alertdialog"], [aria-modal="true"]')
    ) {
        return
    }

    emit('close')
}

onMounted(() => window.addEventListener('keydown', handleKeydown, true))
onUnmounted(() => window.removeEventListener('keydown', handleKeydown, true))

const isDirty = computed(() =>
    ARMOR_SLOTS.some((slot) => {
        const piece = draft.value[slot]
        const applied = props.saved[slot]
        return (
            !applied ||
            applied.material !== piece.material ||
            applied.trimPattern !== piece.trimPattern ||
            applied.trimMaterial !== piece.trimMaterial
        )
    }),
)
</script>

<template>
    <div
        :id="panelId"
        role="region"
        :aria-label="formatMessage(messages.armorTitle)"
        class="flex min-h-[24rem] flex-col gap-4 rounded-2xl border border-solid border-surface-5 bg-surface-2 p-4"
    >
        <header class="flex flex-wrap items-start justify-between gap-3">
            <div class="min-w-0">
                <h2 class="m-0 text-lg font-bold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.armorTitle) }}
                </h2>
                <p class="m-0 mt-1 text-sm leading-6 text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.armorDescription) }}
                </p>
            </div>
            <div class="flex shrink-0 flex-wrap items-center gap-2">
                <span
                    v-if="isDirty"
                    class="text-sm font-semibold text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.armorUnsaved) }}
                </span>
                <Button type="outlined" @click="emit('close')"
                    ><XIcon />{{ formatMessage(messages.armorClose) }}</Button
                >
            </div>
        </header>

        <ArmorTrimPanel v-model="draft" />

        <footer
            class="mt-auto flex flex-wrap items-center justify-between gap-2 border-0 border-t border-solid border-surface-5 pt-3"
        >
            <div class="flex flex-wrap items-center gap-2">
                <Button type="outlined" :disabled="!isDirty" @click="emit('reset')"
                    ><RotateCounterClockwiseIcon />{{
                        formatMessage(messages.armorResetToSaved)
                    }}</Button
                >
                <Button type="outlined" @click="emit('defaults')">{{
                    formatMessage(messages.armorRestoreDefaults)
                }}</Button>
            </div>
            <Button type="colored" color="brand" :disabled="!isDirty" @click="emit('save')"
                ><CheckIcon />{{ formatMessage(messages.armorSave) }}</Button
            >
        </footer>
    </div>
</template>
