<script setup lang="ts">
import { CheckIcon, ListIcon, XIcon } from '@modrinth/assets'
import { defineMessages, type MessageDescriptor, Toggle, useVIntl } from '@modrinth/ui'
import { computed, nextTick, ref } from 'vue'

import {
    HOME_MINIMAL_ELEMENTS,
    type HomeMinimalArrangement,
    type HomeMinimalElement,
    type HomeMinimalLayout,
    type HomeMinimalOptions,
    type HomeMinimalPreset,
    resolveHomeMinimalLayout,
    setHomeMinimalPreset,
    updateHomeMinimalLayout,
} from './home-minimal'

/**
 * The minimal Home's settings, in a panel above the floating controls. Every
 * change applies at once, so the page behind it is the preview.
 */
const props = defineProps<{
    options: HomeMinimalOptions | null | undefined
    /** False when there is no instance to switch to, e.g. on a fresh install. */
    canChangeInstance: boolean
}>()

const emit = defineEmits<{
    update: [options: HomeMinimalOptions]
    close: []
    changeInstance: []
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    title: { id: 'app.home.minimal.customize', defaultMessage: 'Customize Minimal Home' },
    close: { id: 'app.home.minimal.customize.close', defaultMessage: 'Close' },
    preset: { id: 'app.home.minimal.preset', defaultMessage: 'Style' },
    presetPure: { id: 'app.home.minimal.preset.pure', defaultMessage: 'Pure' },
    presetClassic: { id: 'app.home.minimal.preset.classic', defaultMessage: 'Classic' },
    presetStandard: { id: 'app.home.minimal.preset.standard', defaultMessage: 'Standard' },
    presetCustom: { id: 'app.home.minimal.preset.custom', defaultMessage: 'Custom' },
    show: { id: 'app.home.minimal.show', defaultMessage: 'Show' },
    date: { id: 'app.home.minimal.show.date', defaultMessage: 'Date' },
    greeting: { id: 'app.home.minimal.show.greeting', defaultMessage: 'Greeting' },
    status: { id: 'app.home.minimal.show.status', defaultMessage: 'Status line' },
    meta: { id: 'app.home.minimal.show.meta', defaultMessage: 'Info tags' },
    switch: { id: 'app.home.minimal.show.switch', defaultMessage: 'Switch instance button' },
    recent: { id: 'app.home.launch.recent', defaultMessage: 'Recently played' },
    layout: { id: 'app.home.minimal.layout', defaultMessage: 'Layout' },
    surface: { id: 'app.home.minimal.surface', defaultMessage: 'Background' },
    surfaceCard: { id: 'app.home.minimal.surface.card', defaultMessage: 'Card' },
    surfacePlain: { id: 'app.home.minimal.surface.plain', defaultMessage: 'None' },
    arrangement: { id: 'app.home.minimal.arrangement', defaultMessage: 'Arrangement' },
    arrangementRow: { id: 'app.home.minimal.arrangement.row', defaultMessage: 'Side by side' },
    arrangementStacked: {
        id: 'app.home.minimal.arrangement.stacked',
        defaultMessage: 'Stacked',
    },
    arrangementBar: { id: 'app.home.minimal.arrangement.bar', defaultMessage: 'Strip' },
    position: { id: 'app.home.minimal.position', defaultMessage: 'Vertical position' },
    positionUpper: { id: 'app.home.minimal.position.upper', defaultMessage: 'Slightly up' },
    positionCenter: { id: 'app.home.minimal.position.center', defaultMessage: 'Centered' },
    density: { id: 'app.home.minimal.density', defaultMessage: 'Spacing' },
    densityComfortable: {
        id: 'app.home.minimal.density.comfortable',
        defaultMessage: 'Roomy',
    },
    densityCompact: { id: 'app.home.minimal.density.compact', defaultMessage: 'Compact' },
    changeInstance: {
        id: 'app.home.minimal.change-instance',
        defaultMessage: 'Change Home instance',
    },
})

const elementLabels: Record<HomeMinimalElement, MessageDescriptor> = {
    date: messages.date,
    greeting: messages.greeting,
    status: messages.status,
    meta: messages.meta,
    switch: messages.switch,
    recent: messages.recent,
}

/** Each preset has a small drawing of its layout; the label stays one word. */
const presets: { id: HomeMinimalPreset; label: MessageDescriptor }[] = [
    { id: 'pure', label: messages.presetPure },
    { id: 'classic', label: messages.presetClassic },
    { id: 'standard', label: messages.presetStandard },
    { id: 'custom', label: messages.presetCustom },
]

const arrangements: { value: HomeMinimalArrangement; label: MessageDescriptor }[] = [
    { value: 'row', label: messages.arrangementRow },
    { value: 'stacked', label: messages.arrangementStacked },
    { value: 'bar', label: messages.arrangementBar },
]

type Choice<K extends keyof Omit<HomeMinimalLayout, 'show'>> = {
    key: K
    label: MessageDescriptor
    values: { value: HomeMinimalLayout[K]; label: MessageDescriptor }[]
}

const choices = [
    {
        key: 'surface',
        label: messages.surface,
        values: [
            { value: 'card', label: messages.surfaceCard },
            { value: 'plain', label: messages.surfacePlain },
        ],
    } satisfies Choice<'surface'>,
    {
        key: 'position',
        label: messages.position,
        values: [
            { value: 'upper', label: messages.positionUpper },
            { value: 'center', label: messages.positionCenter },
        ],
    } satisfies Choice<'position'>,
    {
        key: 'density',
        label: messages.density,
        values: [
            { value: 'comfortable', label: messages.densityComfortable },
            { value: 'compact', label: messages.densityCompact },
        ],
    } satisfies Choice<'density'>,
]

const preset = computed(() => props.options?.preset ?? 'standard')
const layout = computed(() => resolveHomeMinimalLayout(props.options))

function choosePreset(next: HomeMinimalPreset) {
    if (next === preset.value) return
    emit('update', setHomeMinimalPreset(props.options, next))
}

function setShown(element: HomeMinimalElement, shown: boolean) {
    emit('update', updateHomeMinimalLayout(props.options, { show: { [element]: shown } }))
}

type ChoiceKey = keyof Omit<HomeMinimalLayout, 'show'>

function setChoice<K extends ChoiceKey>(key: K, value: HomeMinimalLayout[K]) {
    if (layout.value[key] === value) return
    const patch: Partial<Omit<HomeMinimalLayout, 'show'>> = {}
    patch[key] = value
    emit('update', updateHomeMinimalLayout(props.options, patch))
}

/*
 * Radio groups move with the arrow keys and keep one tab stop, as the ARIA
 * radio pattern does: the checked option is the one Tab reaches.
 */
const panel = ref<HTMLElement>()
function moveInGroup<T extends string>(
    event: KeyboardEvent,
    values: readonly T[],
    current: T,
    choose: (value: T) => void,
    group: string,
) {
    const step =
        event.key === 'ArrowRight' || event.key === 'ArrowDown'
            ? 1
            : event.key === 'ArrowLeft' || event.key === 'ArrowUp'
              ? -1
              : 0
    if (!step) return
    event.preventDefault()
    const index = values.indexOf(current)
    const next = values[(index + step + values.length) % values.length]
    choose(next)
    void nextTick(() =>
        panel.value
            ?.querySelector<HTMLElement>(`[data-group="${group}"] [data-option="${next}"]`)
            ?.focus(),
    )
}
</script>

<template>
    <section
        ref="panel"
        class="home-minimal-customizer"
        role="dialog"
        :aria-label="formatMessage(messages.title)"
        @keydown.esc.stop="emit('close')"
    >
        <header class="flex min-w-0 items-center gap-2">
            <h2
                class="m-0 min-w-0 flex-1 truncate text-sm font-bold text-[var(--color-text-primary)]"
            >
                {{ formatMessage(messages.title) }}
            </h2>
            <button
                v-tooltip="formatMessage(messages.close)"
                type="button"
                class="home-minimal-customizer-icon-button"
                :aria-label="formatMessage(messages.close)"
                @click="emit('close')"
            >
                <XIcon aria-hidden="true" />
            </button>
        </header>

        <div
            class="home-minimal-customizer-presets"
            role="radiogroup"
            data-group="preset"
            :aria-label="formatMessage(messages.preset)"
        >
            <button
                v-for="item in presets"
                :key="item.id"
                type="button"
                role="radio"
                class="home-minimal-customizer-preset"
                :data-preset="item.id"
                :data-option="item.id"
                :aria-checked="preset === item.id"
                :tabindex="preset === item.id ? 0 : -1"
                @click="choosePreset(item.id)"
                @keydown="
                    moveInGroup(
                        $event,
                        presets.map((entry) => entry.id),
                        preset,
                        choosePreset,
                        'preset',
                    )
                "
            >
                <span class="home-minimal-thumb" :data-thumb="item.id" aria-hidden="true">
                    <template v-if="item.id === 'pure'">
                        <i class="thumb-dot" /><i class="thumb-line" /><i class="thumb-button" />
                    </template>
                    <template v-else-if="item.id === 'classic'">
                        <i class="thumb-line" /><i class="thumb-rule" />
                        <i class="thumb-strip"
                            ><i class="thumb-dot" /><i class="thumb-line" /><i class="thumb-button"
                        /></i>
                    </template>
                    <template v-else-if="item.id === 'standard'">
                        <i class="thumb-line" />
                        <i class="thumb-card"
                            ><i class="thumb-dot" /><i class="thumb-line" /><i class="thumb-button"
                        /></i>
                        <i class="thumb-recent"
                            ><i class="thumb-tile" /><i class="thumb-tile" /><i class="thumb-tile"
                        /></i>
                    </template>
                    <template v-else>
                        <i class="thumb-slider" style="--knob: 30%" />
                        <i class="thumb-slider" style="--knob: 70%" />
                        <i class="thumb-slider" style="--knob: 48%" />
                    </template>
                </span>
                <span class="home-minimal-customizer-preset-label">{{
                    formatMessage(item.label)
                }}</span>
                <span
                    v-if="preset === item.id"
                    class="home-minimal-customizer-check"
                    aria-hidden="true"
                    ><CheckIcon
                /></span>
            </button>
        </div>

        <fieldset class="home-minimal-customizer-group">
            <legend>{{ formatMessage(messages.show) }}</legend>
            <label
                v-for="element in HOME_MINIMAL_ELEMENTS"
                :key="element"
                class="home-minimal-customizer-row"
                :data-element="element"
            >
                <span>{{ formatMessage(elementLabels[element]) }}</span>
                <Toggle
                    small
                    :model-value="layout.show[element]"
                    @update:model-value="(value) => setShown(element, !!value)"
                />
            </label>
        </fieldset>

        <fieldset class="home-minimal-customizer-group">
            <legend>{{ formatMessage(messages.layout) }}</legend>
            <div
                class="home-minimal-customizer-arrangements"
                role="radiogroup"
                data-group="arrangement"
                :aria-label="formatMessage(messages.arrangement)"
            >
                <button
                    v-for="option in arrangements"
                    :key="option.value"
                    type="button"
                    role="radio"
                    class="home-minimal-customizer-arrangement"
                    :data-value="option.value"
                    :data-option="option.value"
                    :aria-checked="layout.arrangement === option.value"
                    :tabindex="layout.arrangement === option.value ? 0 : -1"
                    @click="setChoice('arrangement', option.value)"
                    @keydown="
                        moveInGroup(
                            $event,
                            arrangements.map((entry) => entry.value),
                            layout.arrangement,
                            (value) => setChoice('arrangement', value),
                            'arrangement',
                        )
                    "
                >
                    <span class="home-minimal-glyph" :data-glyph="option.value" aria-hidden="true">
                        <i class="thumb-dot" /><i class="thumb-line" /><i class="thumb-button" />
                    </span>
                    {{ formatMessage(option.label) }}
                </button>
            </div>
            <div
                v-for="choice in choices"
                :key="choice.key"
                class="home-minimal-customizer-row"
                :data-choice="choice.key"
            >
                <span>{{ formatMessage(choice.label) }}</span>
                <span
                    class="home-minimal-customizer-segments"
                    role="radiogroup"
                    :data-group="choice.key"
                    :aria-label="formatMessage(choice.label)"
                >
                    <button
                        v-for="option in choice.values"
                        :key="option.value"
                        type="button"
                        role="radio"
                        class="home-minimal-customizer-segment"
                        :aria-checked="layout[choice.key] === option.value"
                        :data-value="option.value"
                        :data-option="option.value"
                        :tabindex="layout[choice.key] === option.value ? 0 : -1"
                        @click="setChoice(choice.key, option.value)"
                        @keydown="
                            moveInGroup(
                                $event,
                                choice.values.map((entry) => entry.value),
                                layout[choice.key],
                                (value) => setChoice(choice.key, value),
                                choice.key,
                            )
                        "
                    >
                        {{ formatMessage(option.label) }}
                    </button>
                </span>
            </div>
        </fieldset>

        <button
            type="button"
            class="home-minimal-customizer-instance"
            :disabled="!canChangeInstance"
            @click="emit('changeInstance')"
        >
            <ListIcon aria-hidden="true" />
            {{ formatMessage(messages.changeInstance) }}
        </button>
    </section>
</template>

<style scoped>
.home-minimal-customizer {
    @apply fixed z-[41] flex w-[20rem] flex-col gap-3 overflow-y-auto p-3;
    max-width: calc(100vw - 2rem);
    max-height: calc(100vh - var(--top-bar-height, 3rem) - 5rem);
    box-sizing: border-box;
    border: 1px solid var(--color-divider);
    border-radius: var(--radius-lg);
    background: var(--surface-3);
    box-shadow:
        var(--shadow-card),
        0 0.5rem 1.5rem rgb(0 0 0 / 22%);
}

.home-minimal-customizer-icon-button {
    @apply flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-full border-0 bg-transparent p-0 text-[var(--color-text-tertiary)];
}

.home-minimal-customizer-icon-button:hover,
.home-minimal-customizer-icon-button:focus-visible {
    @apply text-[var(--color-text-primary)];
    background: var(--surface-4);
    outline: none;
}

.home-minimal-customizer-icon-button svg {
    @apply size-4;
}

.home-minimal-customizer-presets {
    @apply grid grid-cols-4 gap-1.5;
}

.home-minimal-customizer-preset {
    @apply relative flex min-w-0 cursor-pointer flex-col items-stretch gap-1 rounded-[var(--radius-md)] border-0 bg-transparent p-1 text-center;
    transition: background-color 120ms ease;
}

.home-minimal-customizer-preset-label {
    @apply truncate text-xs font-semibold text-[var(--color-text-default)];
}

.home-minimal-customizer-preset[aria-checked='true'] .home-minimal-customizer-preset-label {
    @apply text-[var(--color-text-primary)];
}

.home-minimal-customizer-preset:hover {
    background: var(--surface-4);
}

.home-minimal-customizer-check {
    @apply absolute right-0.5 top-0.5 flex size-4 items-center justify-center rounded-full bg-brand text-[var(--color-accent-contrast)];
}

.home-minimal-customizer-check svg {
    @apply size-2.5;
    stroke-width: 3;
}

/*
 * Layout drawings. They are made of the theme's own tokens only: a page
 * tile, ink in the text colour, and the accent for the launch button.
 */
.home-minimal-thumb {
    --thumb-ink: color-mix(in srgb, var(--color-text-primary) 38%, transparent);
    --thumb-faint: color-mix(in srgb, var(--color-text-primary) 12%, transparent);
    @apply flex aspect-[4/3] w-full flex-col items-center justify-center gap-[3px] rounded-[var(--radius-sm)];
    background: var(--surface-2);
    box-shadow: inset 0 0 0 1px var(--color-divider);
    transition: box-shadow 120ms ease;
}

.home-minimal-customizer-preset:hover .home-minimal-thumb {
    box-shadow: inset 0 0 0 1px var(--color-text-tertiary);
}

.home-minimal-customizer-preset[aria-checked='true'] .home-minimal-thumb {
    box-shadow: inset 0 0 0 2px var(--color-brand);
}

.home-minimal-thumb [class^='thumb-'],
.home-minimal-glyph [class^='thumb-'] {
    display: block;
    flex: none;
}

.home-minimal-thumb .thumb-dot,
.home-minimal-glyph .thumb-dot {
    @apply size-2 rounded-full;
    background: var(--thumb-ink);
}

.home-minimal-thumb .thumb-line,
.home-minimal-glyph .thumb-line {
    @apply h-[3px] w-5 rounded-full;
    background: var(--thumb-ink);
}

.home-minimal-thumb .thumb-button,
.home-minimal-glyph .thumb-button {
    @apply h-1 w-3 rounded-full bg-brand;
}

.home-minimal-thumb .thumb-rule {
    @apply -mt-0.5 h-0.5 w-1.5 rounded-full bg-brand;
}

.home-minimal-thumb .thumb-strip,
.home-minimal-thumb .thumb-card {
    @apply mt-0.5 flex items-center gap-1 rounded-[3px] px-1;
    background: var(--thumb-faint);
}

.home-minimal-thumb .thumb-strip {
    @apply h-2.5 w-[78%];
}

.home-minimal-thumb .thumb-strip .thumb-line,
.home-minimal-thumb .thumb-card .thumb-line {
    @apply w-auto flex-1;
}

.home-minimal-thumb .thumb-strip .thumb-dot {
    @apply size-1.5;
}

.home-minimal-thumb .thumb-card {
    @apply h-4 w-[78%] gap-1.5;
}

.home-minimal-thumb .thumb-recent {
    @apply flex w-[78%] gap-[2px];
}

.home-minimal-thumb .thumb-recent .thumb-tile {
    @apply h-1.5 flex-1 rounded-[2px];
    background: var(--thumb-faint);
}

.home-minimal-thumb[data-thumb='custom'] {
    @apply gap-1.5;
}

.home-minimal-thumb .thumb-slider {
    @apply relative h-[3px] w-[62%] rounded-full;
    background: var(--thumb-faint);
}

.home-minimal-thumb .thumb-slider::after {
    @apply absolute top-1/2 size-1.5 -translate-x-1/2 -translate-y-1/2 rounded-full;
    left: var(--knob);
    background: var(--thumb-ink);
    content: '';
}

.home-minimal-customizer-preset[aria-checked='true'] .thumb-slider::after {
    @apply bg-brand;
}

/* Arrangement: three drawn options in a row, each with a short label. */
.home-minimal-customizer-arrangements {
    @apply mb-1 grid grid-cols-3 gap-1.5;
}

.home-minimal-customizer-arrangement {
    @apply flex min-w-0 cursor-pointer flex-col items-center gap-1 rounded-[var(--radius-md)] border border-solid border-[var(--color-divider)] bg-transparent px-1 pb-1 pt-1.5 text-xs font-semibold text-[var(--color-text-default)];
    transition:
        border-color 120ms ease,
        background-color 120ms ease;
}

.home-minimal-customizer-arrangement:hover {
    background: var(--surface-4);
}

.home-minimal-customizer-arrangement[aria-checked='true'] {
    @apply border-brand text-[var(--color-text-primary)];
    background: var(--color-brand-highlight);
}

.home-minimal-glyph {
    --thumb-ink: color-mix(in srgb, var(--color-text-primary) 45%, transparent);
    @apply flex h-5 items-center justify-center gap-1;
}

.home-minimal-glyph[data-glyph='row'] .thumb-dot {
    @apply size-3 rounded-[3px];
}

.home-minimal-glyph[data-glyph='stacked'] {
    @apply flex-col gap-[2px];
}

.home-minimal-glyph[data-glyph='stacked'] .thumb-line {
    @apply w-3;
}

.home-minimal-glyph[data-glyph='bar'] {
    @apply w-9 rounded-[3px] px-1;
    background: color-mix(in srgb, var(--color-text-primary) 12%, transparent);
}

.home-minimal-glyph[data-glyph='bar'] .thumb-dot {
    @apply size-1.5;
}

.home-minimal-glyph[data-glyph='bar'] .thumb-line {
    @apply w-auto flex-1;
}

.home-minimal-glyph[data-glyph='bar'] .thumb-button {
    @apply w-2;
}

.home-minimal-customizer-group {
    @apply m-0 flex min-w-0 flex-col gap-0.5 border-0 p-0;
}

.home-minimal-customizer-group legend {
    @apply mb-1 p-0 text-xs font-bold text-[var(--color-text-tertiary)];
}

.home-minimal-customizer-row {
    @apply flex min-h-8 min-w-0 cursor-pointer items-center justify-between gap-3 rounded-[var(--radius-sm)] px-1 text-[0.8125rem] font-semibold text-[var(--color-text-default)];
}

div.home-minimal-customizer-row {
    @apply cursor-default;
}

.home-minimal-customizer-segments {
    @apply flex shrink-0 rounded-full p-0.5;
    background: var(--surface-4);
}

.home-minimal-customizer-segment {
    @apply cursor-pointer rounded-full border-0 bg-transparent px-2.5 py-1 text-xs font-semibold text-[var(--color-text-tertiary)];
    transition:
        background-color 120ms ease,
        color 120ms ease;
}

.home-minimal-customizer-segment[aria-checked='true'] {
    @apply bg-brand text-[var(--color-accent-contrast)];
}

.home-minimal-customizer-preset:focus-visible,
.home-minimal-customizer-arrangement:focus-visible,
.home-minimal-customizer-segment:focus-visible,
.home-minimal-customizer-instance:focus-visible {
    outline: none;
    box-shadow: 0 0 0 3px var(--color-brand-shadow);
}

.home-minimal-customizer-instance {
    @apply flex h-9 w-full cursor-pointer items-center justify-center gap-2 rounded-full border-0 text-sm font-semibold text-[var(--color-text-primary)];
    background: var(--surface-4);
}

.home-minimal-customizer-instance:hover:not(:disabled) {
    filter: brightness(var(--hover-brightness));
}

.home-minimal-customizer-instance:disabled {
    @apply cursor-not-allowed opacity-50;
}

.home-minimal-customizer-instance svg {
    @apply size-4;
}
</style>
