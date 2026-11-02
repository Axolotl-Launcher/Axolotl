<script setup lang="ts">
import { autoUpdate, computePosition, flip, offset, shift } from '@floating-ui/vue'
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'

import { defineMessages, type MessageDescriptor, useVIntl } from '../../composables/i18n'
import {
    hexToHsv,
    type HsvColor,
    hsvToHex,
    mergeRecentColors,
    MINECRAFT_DYE_COLORS,
    normalizeHex,
    readRecentColors,
    RECENT_COLORS_LIMIT,
    RECENT_COLORS_STORAGE_KEY,
    rememberRecentColor,
} from './color-picker'

const props = withDefaults(
    defineProps<{
        /** `#rrggbb`. The picker never emits a value in another format. */
        modelValue: string
        /** Accessible name for the swatch trigger and the popup. */
        label?: string
        size?: 'sm' | 'md'
        disabled?: boolean
        showDyePresets?: boolean
        showRecentColors?: boolean
        recentColorsLimit?: number
        /** Lets a caller keep its own recent list instead of sharing the global one. */
        recentColorsStorageKey?: string
    }>(),
    {
        label: undefined,
        size: 'md',
        disabled: false,
        showDyePresets: true,
        showRecentColors: true,
        recentColorsLimit: RECENT_COLORS_LIMIT,
        recentColorsStorageKey: RECENT_COLORS_STORAGE_KEY,
    },
)

const emit = defineEmits<{
    'update:modelValue': [value: string]
    change: [value: string]
    open: []
    close: []
}>()

// The component renders a fragment (trigger plus a teleported panel), so Vue has
// no single root to put a caller's `class` on. Forward it to the trigger instead.
defineOptions({ inheritAttrs: false })

const { formatMessage } = useVIntl()

const messages = defineMessages({
    openPicker: {
        id: 'color-picker.open',
        defaultMessage: 'Choose colour',
    },
    dialogLabel: {
        id: 'color-picker.dialog-label',
        defaultMessage: 'Colour picker',
    },
    saturationBrightness: {
        id: 'color-picker.saturation-brightness',
        defaultMessage: 'Saturation and brightness',
    },
    saturationBrightnessValue: {
        id: 'color-picker.saturation-brightness.value',
        defaultMessage: 'Saturation {saturation}%, brightness {brightness}%',
    },
    hue: {
        id: 'color-picker.hue',
        defaultMessage: 'Hue',
    },
    hex: {
        id: 'color-picker.hex',
        defaultMessage: 'Hex colour',
    },
    dyePresets: {
        id: 'color-picker.dye-presets',
        defaultMessage: 'Dye colours',
    },
    recentColors: {
        id: 'color-picker.recent-colors',
        defaultMessage: 'Recently used',
    },
})

const dyeMessages = defineMessages({
    dyeWhite: { id: 'color-picker.dye.white', defaultMessage: 'White' },
    dyeOrange: { id: 'color-picker.dye.orange', defaultMessage: 'Orange' },
    dyeMagenta: { id: 'color-picker.dye.magenta', defaultMessage: 'Magenta' },
    dyeLightBlue: { id: 'color-picker.dye.light-blue', defaultMessage: 'Light blue' },
    dyeYellow: { id: 'color-picker.dye.yellow', defaultMessage: 'Yellow' },
    dyeLime: { id: 'color-picker.dye.lime', defaultMessage: 'Lime' },
    dyePink: { id: 'color-picker.dye.pink', defaultMessage: 'Pink' },
    dyeGray: { id: 'color-picker.dye.gray', defaultMessage: 'Gray' },
    dyeLightGray: { id: 'color-picker.dye.light-gray', defaultMessage: 'Light gray' },
    dyeCyan: { id: 'color-picker.dye.cyan', defaultMessage: 'Cyan' },
    dyePurple: { id: 'color-picker.dye.purple', defaultMessage: 'Purple' },
    dyeBlue: { id: 'color-picker.dye.blue', defaultMessage: 'Blue' },
    dyeBrown: { id: 'color-picker.dye.brown', defaultMessage: 'Brown' },
    dyeGreen: { id: 'color-picker.dye.green', defaultMessage: 'Green' },
    dyeRed: { id: 'color-picker.dye.red', defaultMessage: 'Red' },
    dyeBlack: { id: 'color-picker.dye.black', defaultMessage: 'Black' },
})

const DYE_NAME_MESSAGES: Record<string, MessageDescriptor> = {
    white: dyeMessages.dyeWhite,
    orange: dyeMessages.dyeOrange,
    magenta: dyeMessages.dyeMagenta,
    light_blue: dyeMessages.dyeLightBlue,
    yellow: dyeMessages.dyeYellow,
    lime: dyeMessages.dyeLime,
    pink: dyeMessages.dyePink,
    gray: dyeMessages.dyeGray,
    light_gray: dyeMessages.dyeLightGray,
    cyan: dyeMessages.dyeCyan,
    purple: dyeMessages.dyePurple,
    blue: dyeMessages.dyeBlue,
    brown: dyeMessages.dyeBrown,
    green: dyeMessages.dyeGreen,
    red: dyeMessages.dyeRed,
    black: dyeMessages.dyeBlack,
}

const open = ref(false)
const positioned = ref(false)
const dragging = ref(false)
const dragPointerId = ref<number | null>(null)
const editingHex = ref(false)
const triggerRef = ref<HTMLElement>()
const panelRef = ref<HTMLElement>()
const areaRef = ref<HTMLElement>()
const panelStyle = ref<Record<string, string>>({})
const recentColors = ref<string[]>([])

/**
 * Local HSV is the source of truth while the panel is open. Deriving it from
 * the emitted hex on every move would lose the hue as soon as the value reaches
 * black, because black carries no hue to derive.
 */
const hsv = ref<HsvColor>(hexToHsv(props.modelValue) ?? { h: 0, s: 0, v: 0 })

const displayHex = computed(() => hsvToHex(hsv.value.h, hsv.value.s, hsv.value.v))
const hexDraft = ref(displayHex.value)

/**
 * The colour this picker last reported through `change`. Closing the panel
 * commits, so without this a panel that was merely opened and dismissed would
 * report the colour it opened with — and a caller that treats `change` as "add
 * this colour" would add a stop the user never asked for.
 */
const lastCommitted = ref(displayHex.value)

const triggerClasses = computed(() => (props.size === 'sm' ? 'size-8 p-[3px]' : 'size-9 p-1'))

const swatchRadius = computed(() => (props.size === 'sm' ? 'rounded-[5px]' : 'rounded-[8px]'))

const areaValueText = computed(() =>
    formatMessage(messages.saturationBrightnessValue, {
        saturation: Math.round(hsv.value.s * 100),
        brightness: Math.round(hsv.value.v * 100),
    }),
)

watch(
    () => props.modelValue,
    (next) => {
        const hex = normalizeHex(next)
        if (!hex || hex === displayHex.value) return
        const parsed = hexToHsv(hex)
        if (parsed) hsv.value = parsed
    },
)

watch(displayHex, (hex) => {
    if (!editingHex.value) hexDraft.value = hex
})

watch(
    () => props.disabled,
    (disabled) => {
        if (disabled) closePanel({ commit: false })
    },
)

function clampUnit(value: number) {
    return Math.min(1, Math.max(0, value))
}

function applyHsv(next: HsvColor) {
    hsv.value = { h: next.h, s: clampUnit(next.s), v: clampUnit(next.v) }
    emitValue()
}

function applyHex(hex: string) {
    const parsed = hexToHsv(hex)
    if (parsed) hsv.value = parsed
    emitValue()
}

function emitValue() {
    const hex = displayHex.value
    if (hex === normalizeHex(props.modelValue)) return
    emit('update:modelValue', hex)
}

function commit() {
    const hex = displayHex.value
    if (hex === lastCommitted.value) return
    lastCommitted.value = hex
    if (props.showRecentColors) {
        recentColors.value = rememberRecentColor(
            hex,
            props.recentColorsStorageKey,
            props.recentColorsLimit,
        )
    }
    emit('change', hex)
}

function togglePanel() {
    if (open.value) closePanel()
    else openPanel()
}

function openPanel() {
    if (props.disabled || open.value) return
    if (props.showRecentColors) {
        // Merge instead of replacing: a webview can reject the write and then
        // report the previous list, which would drop the colours committed in
        // this session as soon as the panel is reopened.
        recentColors.value = mergeRecentColors(
            [
                recentColors.value,
                readRecentColors(props.recentColorsStorageKey, props.recentColorsLimit),
            ],
            props.recentColorsLimit,
        )
    }
    positioned.value = false
    // Each session starts from the colour the panel opens with, so a caller
    // that disabled the picker mid-session cannot leave a stale commit behind.
    lastCommitted.value = displayHex.value
    open.value = true
    emit('open')
}

function closePanel({ commit: shouldCommit = true, restoreFocus = false } = {}) {
    if (!open.value) return
    // Before the panel unmounts: the element goes away, the drag flag does not.
    endDrag()
    open.value = false
    emit('close')
    if (shouldCommit) commit()
    // Only pull focus back when the picker dismissed itself, such as on
    // Escape. Restoring it after a click elsewhere would steal the focus the
    // user just moved.
    if (restoreFocus) triggerRef.value?.querySelector<HTMLElement>('button')?.focus()
}

function updateFromPointer(event: PointerEvent) {
    const area = areaRef.value
    if (!area) return
    const rect = area.getBoundingClientRect()
    if (rect.width === 0 || rect.height === 0) return
    applyHsv({
        h: hsv.value.h,
        s: (event.clientX - rect.left) / rect.width,
        v: 1 - (event.clientY - rect.top) / rect.height,
    })
}

/**
 * Ends a drag, whether the pointer was released or the panel closed under it.
 * The flag outlives the element it belongs to, so leaving it set would let the
 * next pointer move over a reopened panel paint a colour nobody is dragging.
 */
function endDrag() {
    const area = areaRef.value
    const pointerId = dragPointerId.value
    if (area && pointerId !== null && area.hasPointerCapture(pointerId)) {
        area.releasePointerCapture(pointerId)
    }
    dragPointerId.value = null
    dragging.value = false
}

function onAreaPointerDown(event: PointerEvent) {
    if (props.disabled) return
    event.preventDefault()
    dragging.value = true
    dragPointerId.value = event.pointerId
    areaRef.value?.focus()
    try {
        ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
    } catch {
        // A pointer that is already gone has nothing to capture. The drag still
        // tracks while the pointer stays over the area, so this is not fatal.
    }
    updateFromPointer(event)
}

function onAreaPointerMove(event: PointerEvent) {
    if (!dragging.value) return
    updateFromPointer(event)
}

function onAreaPointerUp() {
    if (!dragging.value) return
    endDrag()
    commit()
}

function onAreaKeydown(event: KeyboardEvent) {
    const step = event.shiftKey ? 0.1 : 0.01
    const { h, s, v } = hsv.value
    switch (event.key) {
        case 'ArrowLeft':
            applyHsv({ h, s: s - step, v })
            break
        case 'ArrowRight':
            applyHsv({ h, s: s + step, v })
            break
        case 'ArrowUp':
            applyHsv({ h, s, v: v + step })
            break
        case 'ArrowDown':
            applyHsv({ h, s, v: v - step })
            break
        case 'Home':
            applyHsv({ h, s: 0, v })
            break
        case 'End':
            applyHsv({ h, s: 1, v })
            break
        default:
            return
    }
    event.preventDefault()
    commit()
}

function onHueInput(event: Event) {
    const hue = Number((event.target as HTMLInputElement).value)
    if (!Number.isFinite(hue)) return
    applyHsv({ ...hsv.value, h: hue })
}

function onHexInput(event: Event) {
    hexDraft.value = (event.target as HTMLInputElement).value
    const hex = normalizeHex(hexDraft.value)
    if (hex) applyHex(hex)
}

function onHexBlur() {
    editingHex.value = false
    const hex = normalizeHex(hexDraft.value)
    if (hex) {
        hexDraft.value = hex
        applyHex(hex)
    } else {
        hexDraft.value = displayHex.value
    }
    commit()
}

function pickPreset(hex: string) {
    applyHex(hex)
    commit()
}

function onDocumentPointerDown(event: PointerEvent) {
    const target = event.target as Node | null
    if (!target) return
    if (panelRef.value?.contains(target) || triggerRef.value?.contains(target)) return
    closePanel()
}

let stopAutoUpdate: (() => void) | undefined
let positionRequest = 0

async function updatePosition() {
    const trigger = triggerRef.value
    const panel = panelRef.value
    if (!trigger || !panel) return
    // Scrolling or resizing can start several updates before any of them
    // resolves, and they do not resolve in order. Only the newest may write:
    // an older computation landing last would put the panel back where the
    // trigger used to be.
    const request = ++positionRequest
    const { x, y } = await computePosition(trigger, panel, {
        strategy: 'fixed',
        placement: 'bottom-start',
        middleware: [offset(6), flip({ padding: 8 }), shift({ padding: 8 })],
    })
    if (request !== positionRequest) return
    panelStyle.value = { left: `${x}px`, top: `${y}px` }
    positioned.value = true
}

watch(open, async (isOpen) => {
    if (isOpen) {
        document.addEventListener('pointerdown', onDocumentPointerDown, true)
        await nextTick()
        if (triggerRef.value && panelRef.value) {
            stopAutoUpdate = autoUpdate(triggerRef.value, panelRef.value, () => {
                void updatePosition()
            })
            await updatePosition()
            areaRef.value?.focus()
        }
    } else {
        document.removeEventListener('pointerdown', onDocumentPointerDown, true)
        stopAutoUpdate?.()
        stopAutoUpdate = undefined
        positioned.value = false
        panelStyle.value = {}
    }
})

onBeforeUnmount(() => {
    document.removeEventListener('pointerdown', onDocumentPointerDown, true)
    stopAutoUpdate?.()
})
</script>

<template>
    <span ref="triggerRef" class="inline-flex min-w-0" v-bind="$attrs">
        <slot name="trigger" :toggle="togglePanel" :open="open" :expanded="open">
            <button
                type="button"
                :disabled="disabled"
                :aria-label="label ?? formatMessage(messages.openPicker)"
                :aria-expanded="open"
                aria-haspopup="dialog"
                class="inline-flex shrink-0 cursor-pointer items-center justify-center rounded-xl border border-solid border-surface-5 bg-surface-4 transition-[border-color,box-shadow] hover:border-[color:var(--color-brand)] focus-visible:border-[color:var(--color-brand)] focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-shadow disabled:cursor-not-allowed disabled:opacity-50"
                :class="triggerClasses"
                @click="togglePanel"
            >
                <span
                    class="block size-full ring-1 ring-inset ring-black/20"
                    :class="swatchRadius"
                    :style="{ backgroundColor: displayHex }"
                    aria-hidden="true"
                />
            </button>
        </slot>
    </span>

    <Teleport to="body">
        <div
            v-if="open"
            ref="panelRef"
            role="dialog"
            :aria-label="label ?? formatMessage(messages.dialogLabel)"
            class="fixed top-0 left-0 z-[9999] flex w-60 flex-col gap-3 rounded-[14px] border border-solid border-surface-5 bg-surface-4 p-3 shadow-[var(--shadow-raised)]"
            :class="{ 'color-picker-panel': positioned }"
            :style="positioned ? panelStyle : { visibility: 'hidden' }"
            @keydown.escape.stop.prevent="closePanel({ restoreFocus: true })"
        >
            <div
                ref="areaRef"
                role="slider"
                tabindex="0"
                aria-valuemin="0"
                aria-valuemax="100"
                :aria-valuenow="Math.round(hsv.s * 100)"
                :aria-valuetext="areaValueText"
                :aria-label="formatMessage(messages.saturationBrightness)"
                class="relative h-32 w-full cursor-crosshair touch-none overflow-hidden rounded-lg border border-solid border-surface-5 outline-none focus-visible:ring-4 focus-visible:ring-brand-shadow"
                :style="{ backgroundColor: `hsl(${hsv.h} 100% 50%)` }"
                @pointerdown="onAreaPointerDown"
                @pointermove="onAreaPointerMove"
                @pointerup="onAreaPointerUp"
                @pointercancel="onAreaPointerUp"
                @keydown="onAreaKeydown"
            >
                <div
                    class="pointer-events-none absolute inset-0 bg-gradient-to-r from-white to-transparent"
                    aria-hidden="true"
                />
                <div
                    class="pointer-events-none absolute inset-0 bg-gradient-to-t from-black to-transparent"
                    aria-hidden="true"
                />
                <span
                    class="pointer-events-none absolute size-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-[0_0_0_1px_rgb(0_0_0_/_45%)]"
                    :style="{ left: `${hsv.s * 100}%`, top: `${(1 - hsv.v) * 100}%` }"
                    aria-hidden="true"
                />
            </div>

            <input
                type="range"
                min="0"
                max="360"
                step="1"
                :value="Math.round(hsv.h)"
                :aria-label="formatMessage(messages.hue)"
                class="color-picker-hue h-3 w-full min-h-0 cursor-pointer appearance-none rounded-full p-0"
                @input="onHueInput"
                @change="commit()"
            />

            <div class="flex items-center gap-2">
                <span
                    class="size-9 shrink-0 rounded-lg border border-solid border-surface-5"
                    :style="{ backgroundColor: displayHex }"
                    aria-hidden="true"
                />
                <input
                    type="text"
                    maxlength="7"
                    spellcheck="false"
                    autocomplete="off"
                    :value="hexDraft"
                    :aria-label="formatMessage(messages.hex)"
                    class="h-9 w-full min-w-0 rounded-xl border border-solid border-surface-5 bg-surface-3 px-2 text-center font-mono text-sm font-semibold text-[var(--color-text-primary)] outline-none focus:ring-4 focus:ring-brand-shadow"
                    @focus="editingHex = true"
                    @input="onHexInput"
                    @blur="onHexBlur"
                    @keydown.enter.prevent="onHexBlur"
                />
            </div>

            <div v-if="showDyePresets" class="flex flex-col gap-1.5">
                <span class="text-xs font-semibold text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.dyePresets) }}
                </span>
                <div
                    class="grid grid-cols-8 gap-1.5"
                    role="group"
                    :aria-label="formatMessage(messages.dyePresets)"
                >
                    <button
                        v-for="preset in MINECRAFT_DYE_COLORS"
                        :key="preset.id"
                        v-tooltip="formatMessage(DYE_NAME_MESSAGES[preset.id])"
                        type="button"
                        :aria-label="formatMessage(DYE_NAME_MESSAGES[preset.id])"
                        :aria-pressed="displayHex === preset.hex"
                        class="size-5 cursor-pointer rounded-[4px] border border-solid border-[color-mix(in_srgb,var(--color-text-primary)_25%,transparent)] transition-[border-color] hover:border-[color:var(--color-text-primary)] focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-brand"
                        :class="
                            displayHex === preset.hex
                                ? 'outline-2 outline-offset-1 outline-brand'
                                : ''
                        "
                        :style="{ backgroundColor: preset.hex }"
                        @click="pickPreset(preset.hex)"
                    />
                </div>
            </div>

            <div v-if="showRecentColors && recentColors.length > 0" class="flex flex-col gap-1.5">
                <span class="text-xs font-semibold text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.recentColors) }}
                </span>
                <div
                    class="grid grid-cols-8 gap-1.5"
                    role="group"
                    :aria-label="formatMessage(messages.recentColors)"
                >
                    <button
                        v-for="hex in recentColors"
                        :key="hex"
                        v-tooltip="hex"
                        type="button"
                        :aria-label="hex"
                        :aria-pressed="displayHex === hex"
                        class="size-5 cursor-pointer rounded-[4px] border border-solid border-[color-mix(in_srgb,var(--color-text-primary)_25%,transparent)] transition-[border-color] hover:border-[color:var(--color-text-primary)] focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-brand"
                        :class="
                            displayHex === hex ? 'outline-2 outline-offset-1 outline-brand' : ''
                        "
                        :style="{ backgroundColor: hex }"
                        @click="pickPreset(hex)"
                    />
                </div>
            </div>
        </div>
    </Teleport>
</template>

<style scoped lang="scss">
/**
 * The rainbow track and the slider thumb cannot be expressed as utilities: the
 * track needs a six-stop gradient and the thumb needs a pseudo-element.
 */
.color-picker-hue {
    background: linear-gradient(
        to right,
        hsl(0, 100%, 50%),
        hsl(60, 100%, 50%),
        hsl(120, 100%, 50%),
        hsl(180, 100%, 50%),
        hsl(240, 100%, 50%),
        hsl(300, 100%, 50%),
        hsl(360, 100%, 50%)
    );

    &:focus-visible {
        outline: 2px solid var(--color-brand);
        outline-offset: 2px;
    }

    &::-webkit-slider-thumb {
        appearance: none;
        width: 1rem;
        height: 1rem;
        border: 2px solid #ffffff;
        border-radius: 50%;
        background: transparent;
        box-shadow: 0 0 0 1px rgb(0 0 0 / 45%);
    }

    &::-moz-range-thumb {
        width: 1rem;
        height: 1rem;
        border: 2px solid #ffffff;
        border-radius: 50%;
        background: transparent;
        box-shadow: 0 0 0 1px rgb(0 0 0 / 45%);
    }
}

/**
 * The panel is inserted hidden until `computePosition` resolves, so the class
 * is applied with the position rather than with the element. Animating on
 * insert would finish while the panel is still invisible.
 */
.color-picker-panel {
    animation: color-picker-panel-in 160ms cubic-bezier(0, 0, 0.2, 1);

    @media (prefers-reduced-motion: reduce) {
        animation: none;
    }
}

@keyframes color-picker-panel-in {
    from {
        opacity: 0;
        transform: scale(0.96);
    }

    to {
        opacity: 1;
        transform: scale(1);
    }
}
</style>
