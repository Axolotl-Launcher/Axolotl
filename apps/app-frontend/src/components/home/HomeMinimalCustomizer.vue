<script setup lang="ts">
import { ListIcon, XIcon } from '@modrinth/assets'
import { defineMessages, type MessageDescriptor, Toggle, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import {
    HOME_MINIMAL_ELEMENTS,
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
    presetPureHint: {
        id: 'app.home.minimal.preset.pure-hint',
        defaultMessage: 'Icon, name and launch button',
    },
    presetStandard: { id: 'app.home.minimal.preset.standard', defaultMessage: 'Standard' },
    presetStandardHint: {
        id: 'app.home.minimal.preset.standard-hint',
        defaultMessage: 'Greeting, details and recent',
    },
    presetCustom: { id: 'app.home.minimal.preset.custom', defaultMessage: 'Custom' },
    presetCustomHint: {
        id: 'app.home.minimal.preset.custom-hint',
        defaultMessage: 'Your own mix',
    },
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

const presets: { id: HomeMinimalPreset; label: MessageDescriptor; hint: MessageDescriptor }[] = [
    { id: 'pure', label: messages.presetPure, hint: messages.presetPureHint },
    { id: 'standard', label: messages.presetStandard, hint: messages.presetStandardHint },
    { id: 'custom', label: messages.presetCustom, hint: messages.presetCustomHint },
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
        key: 'arrangement',
        label: messages.arrangement,
        values: [
            { value: 'row', label: messages.arrangementRow },
            { value: 'stacked', label: messages.arrangementStacked },
        ],
    } satisfies Choice<'arrangement'>,
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

function setChoice(key: (typeof choices)[number]['key'], value: string) {
    if (layout.value[key] === value) return
    emit('update', updateHomeMinimalLayout(props.options, { [key]: value }))
}
</script>

<template>
    <section
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
            :aria-label="formatMessage(messages.preset)"
        >
            <button
                v-for="item in presets"
                :key="item.id"
                type="button"
                role="radio"
                class="home-minimal-customizer-preset"
                :data-preset="item.id"
                :aria-checked="preset === item.id"
                @click="choosePreset(item.id)"
            >
                <strong>{{ formatMessage(item.label) }}</strong>
                <span>{{ formatMessage(item.hint) }}</span>
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
                v-for="choice in choices"
                :key="choice.key"
                class="home-minimal-customizer-row"
                :data-choice="choice.key"
            >
                <span>{{ formatMessage(choice.label) }}</span>
                <span
                    class="home-minimal-customizer-segments"
                    role="radiogroup"
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
                        @click="setChoice(choice.key, option.value)"
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
    @apply grid grid-cols-3 gap-1.5;
}

.home-minimal-customizer-preset {
    @apply flex min-w-0 cursor-pointer flex-col items-start gap-0.5 rounded-[var(--radius-md)] border border-solid border-[var(--color-divider)] bg-transparent px-2 py-1.5 text-left;
    transition:
        border-color 120ms ease,
        background-color 120ms ease;
}

.home-minimal-customizer-preset strong {
    @apply text-[0.8125rem] font-bold text-[var(--color-text-primary)];
}

.home-minimal-customizer-preset span {
    @apply text-[0.6875rem] leading-tight text-[var(--color-text-tertiary)];
}

.home-minimal-customizer-preset:hover {
    background: var(--surface-4);
}

.home-minimal-customizer-preset[aria-checked='true'] {
    border-color: var(--color-brand);
    background: var(--color-brand-highlight);
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
