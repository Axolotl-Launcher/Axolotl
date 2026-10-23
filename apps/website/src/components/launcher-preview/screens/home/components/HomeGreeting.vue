<script setup lang="ts">
import PencilIcon from '@modrinth/assets/icons/pencil.svg?component'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, inject, onUnmounted, ref, watch } from 'vue'

import { usePreview } from '../../../lib/state'
import {
    HOME_GREETING_DEFAULT_FONT,
    HOME_GREETING_DEFAULT_FONT_SIZE,
    HOME_GREETING_DEFAULT_MODE,
    type HomeGreetingFont,
    type HomeGreetingMode,
    type HomeGreetingOverride,
    type HomeWidgetSize,
} from '../home-dashboard'
import { HOME_GREETING_EDITOR_KEY, type HomeGreetingEditorContext } from '../home-greeting-editor'
import { getTimeBucket, stableGreetingIndex } from '../home-utils'
import { homeMessages } from '../messages'

const props = withDefaults(
    defineProps<{
        playerName?: string | null
        variant?: 'standard' | 'minimal'
        dashboardSize?: HomeWidgetSize | null
        greetingMode?: HomeGreetingMode
        greetingText?: string
        greetingFont?: HomeGreetingFont
        greetingFontSize?: number
        /** Text this account saved for the greeting. Overrides the provided context. */
        override?: HomeGreetingOverride | null
        /**
         * Whether the heading opens the editor. Defaults to "when the home page
         * provides an editor", so a preview can opt out with `false`.
         */
        editable?: boolean
    }>(),
    {
        playerName: null,
        variant: 'standard',
        dashboardSize: null,
        greetingMode: HOME_GREETING_DEFAULT_MODE,
        greetingText: '',
        greetingFont: HOME_GREETING_DEFAULT_FONT,
        greetingFontSize: HOME_GREETING_DEFAULT_FONT_SIZE,
        override: null,
        editable: undefined,
    },
)

const emit = defineEmits<{
    edit: []
}>()

const greetingEditor = inject<HomeGreetingEditorContext | null>(HOME_GREETING_EDITOR_KEY, null)
const editable = computed(
    () => props.editable ?? (greetingEditor !== null && greetingEditor.canEdit.value),
)
const override = computed(() => props.override ?? greetingEditor?.override.value ?? null)

const greetingGroup = ref<HTMLElement | null>(null)

/** The home page owns the editor, so the heading only has to ask for it. */
function requestEdit() {
    if (!editable.value) return
    if (greetingEditor) greetingEditor.open()
    else emit('edit')
}

/**
 * Every way of closing the editor hands the heading's focus back, and that can
 * land a frame or a transition later. Wait for it briefly and release it,
 * otherwise `group-focus-within` would keep the lift and the icon on with
 * nothing left to explain them.
 */
function releaseEditorFocus(framesLeft: number) {
    const active = document.activeElement
    if (active instanceof HTMLElement && greetingGroup.value?.contains(active)) {
        active.blur()
        return
    }
    if (framesLeft > 0) requestAnimationFrame(() => releaseEditorFocus(framesLeft - 1))
}

watch(
    () => greetingEditor?.editing.value ?? false,
    (editing) => {
        if (editing) return
        requestAnimationFrame(() => releaseEditorFocus(20))
    },
)

const { formatMessage, locale } = useVIntl()
const preview = usePreview()
const now = ref(new Date())
const messages = homeMessages

/** The app re-reads the clock once a minute so the greeting follows the day. */
let cancelTick = preview.clock.after(60_000, function updateClock() {
    now.value = new Date()
    cancelTick = preview.clock.after(60_000, updateClock)
})
onUnmounted(() => cancelTick())

const greetingMessages = {
    'late-night': messages.greetingLateNight,
    dawn: messages.greetingDawn,
    morning: messages.greetingMorning,
    afternoon: messages.greetingAfternoon,
    evening: messages.greetingEvening,
    night: messages.greetingNight,
}

const minimalGreetingMessages = {
    'late-night': messages.greetingMinimalLateNight,
    dawn: messages.greetingMinimalDawn,
    morning: messages.greetingMinimalMorning,
    afternoon: messages.greetingMinimalAfternoon,
    evening: messages.greetingMinimalEvening,
    night: messages.greetingMinimalNight,
}

const greeting = computed(() => {
    const bucket = getTimeBucket(now.value)
    const variants = formatMessage(greetingMessages[bucket]).split('\n').filter(Boolean)
    const seed = `${locale.value}:${now.value.toDateString()}:${bucket}:${props.playerName ?? ''}`
    return variants[stableGreetingIndex(seed, variants.length)] ?? ''
})

const minimalGreeting = computed(() =>
    formatMessage(minimalGreetingMessages[getTimeBucket(now.value)]),
)

/** The name the greeting addresses: the saved nickname wins over the account name. */
const displayName = computed(() => override.value?.nickname || props.playerName || null)

const automaticWelcome = computed(() =>
    displayName.value
        ? formatMessage(messages.greetingWelcomeWithPlayer, { name: displayName.value })
        : formatMessage(messages.greetingWelcome),
)

const dateLabel = computed(() =>
    new Intl.DateTimeFormat(locale.value, {
        weekday: 'long',
        month: 'long',
        day: 'numeric',
    }).format(now.value),
)

const heading = computed(() => {
    // A saved title replaces the whole heading, greeting included.
    const customTitle = override.value?.title
    if (customTitle) return customTitle

    if (props.variant === 'minimal') {
        return displayName.value
            ? formatMessage(messages.greetingMinimalWithPlayer, {
                  name: displayName.value,
                  greeting: minimalGreeting.value,
              })
            : minimalGreeting.value
    }

    const customText = props.greetingText.trim()
    if (props.greetingMode === 'text') return customText || greeting.value
    if (props.greetingMode === 'text-and-greeting') {
        return `${customText || automaticWelcome.value} ${greeting.value}`
    }
    return greeting.value
})

const greetingFontFamilies: Record<HomeGreetingFont, string> = {
    sans: 'var(--font-standard)',
    minecraft: "'bundled-minecraft-font-mrapp', monospace",
    mono: 'var(--mono-font)',
    serif: "Georgia, 'Times New Roman', serif",
}

const headingStyle = computed(() =>
    props.dashboardSize
        ? {
              '--home-greeting-font-family': greetingFontFamilies[props.greetingFont],
              '--home-greeting-font-size': `${props.greetingFontSize}px`,
          }
        : undefined,
)

const editGreetingLabel = computed(() => formatMessage(messages.greetingEdit))
</script>

<template>
    <header
        class="home-greeting flex min-w-0 flex-col"
        :class="
            variant === 'minimal'
                ? 'items-center gap-3 text-center'
                : dashboardSize
                  ? 'h-full justify-center gap-2'
                  : 'gap-1 py-2'
        "
    >
        <span
            v-if="variant !== 'minimal' && dashboardSize"
            class="text-xs font-bold leading-none tracking-normal text-[var(--color-text-tertiary)]"
        >
            {{ dateLabel }}
        </span>
        <div ref="greetingGroup" class="group flex min-w-0 items-center gap-2">
            <h1
                class="m-0 max-w-full break-words font-extrabold text-[var(--color-text-primary)]"
                :class="[
                    dashboardSize ? 'home-greeting-heading' : 'text-2xl',
                    editable
                        ? 'cursor-pointer rounded-lg transition-transform duration-150 focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-shadow group-focus-within:-translate-y-0.5 group-hover:-translate-y-0.5'
                        : '',
                ]"
                :style="headingStyle"
                :role="editable ? 'button' : undefined"
                :tabindex="editable ? 0 : undefined"
                :aria-label="editable ? editGreetingLabel : undefined"
                @click="requestEdit"
                @keydown.enter.prevent="requestEdit"
                @keydown.space.prevent="requestEdit"
            >
                {{ heading }}
            </h1>
            <button
                v-if="editable"
                type="button"
                class="home-greeting-edit m-0 shrink-0 cursor-pointer border-0 bg-transparent p-1 text-[var(--color-text-tertiary)] opacity-0 transition-[opacity,color] duration-150 hover:text-[var(--color-text-primary)] focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-shadow group-focus-within:opacity-100 group-hover:opacity-100"
                :aria-label="editGreetingLabel"
                @click="requestEdit"
            >
                <PencilIcon class="size-4" />
            </button>
        </div>
        <div
            v-if="variant === 'minimal'"
            class="h-0.5 w-8 rounded-full bg-brand"
            aria-hidden="true"
        />
    </header>
</template>

<style scoped>
.home-greeting-heading {
    max-width: 44rem;
    font-family: var(--home-greeting-font-family, var(--font-standard));
    font-size: var(--home-greeting-font-size, 1.375rem);
    line-height: 1.35;
}
</style>
