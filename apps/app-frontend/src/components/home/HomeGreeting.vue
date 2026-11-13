<script setup lang="ts">
import './home-widget.css'

import { PencilIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { computed, inject, onUnmounted, ref, watch } from 'vue'

import {
    HOME_GREETING_DEFAULT_FONT,
    HOME_GREETING_DEFAULT_FONT_SIZE,
    HOME_GREETING_DEFAULT_MODE,
    type HomeGreetingFont,
    type HomeGreetingMode,
    type HomeGreetingOverride,
    type HomeWidgetSize,
} from './home-dashboard'
import { HOME_GREETING_EDITOR_KEY, type HomeGreetingEditorContext } from './home-greeting-editor'
import { getTimeBucket, stableGreetingIndex } from './home-utils'

const props = withDefaults(
    defineProps<{
        playerName?: string | null
        /** `hero` sits in the launch card header: date and heading, no own height. */
        variant?: 'standard' | 'minimal' | 'hero'
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
        /** The minimal Home can hide the date or the heading on their own. */
        showDate?: boolean
        showHeading?: boolean
        /**
         * The minimal Home's slim-strip layout: a smaller heading with a short
         * accent rule under it, as the minimal Home had before the redesign.
         */
        accentRule?: boolean
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
        showDate: true,
        showHeading: true,
        accentRule: false,
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
const now = ref(new Date())
const messages = defineMessages({
    editGreeting: {
        id: 'app.home.greeting.edit',
        defaultMessage: 'Edit greeting',
    },
    withPlayer: {
        id: 'app.home.greeting.with-player',
        defaultMessage: 'Welcome back, {name}. {greeting}',
    },
    welcomeWithPlayer: {
        id: 'app.home.greeting.welcome-with-player',
        defaultMessage: 'Welcome back, {name}.',
    },
    welcome: {
        id: 'app.home.greeting.welcome',
        defaultMessage: 'Welcome back.',
    },
    minimalWithPlayer: {
        id: 'app.home.greeting.minimal.with-player',
        defaultMessage: '{greeting}, {name}',
    },
    minimalLateNight: {
        id: 'app.home.greeting.minimal.late-night',
        defaultMessage: 'Good evening',
    },
    minimalDawn: {
        id: 'app.home.greeting.minimal.dawn',
        defaultMessage: 'Good morning',
    },
    minimalMorning: {
        id: 'app.home.greeting.minimal.morning',
        defaultMessage: 'Good morning',
    },
    minimalAfternoon: {
        id: 'app.home.greeting.minimal.afternoon',
        defaultMessage: 'Good afternoon',
    },
    minimalEvening: {
        id: 'app.home.greeting.minimal.evening',
        defaultMessage: 'Good evening',
    },
    minimalNight: {
        id: 'app.home.greeting.minimal.night',
        defaultMessage: 'Good evening',
    },
    'late-night': {
        id: 'app.home.greeting.late-night',
        defaultMessage:
            'The moon is still working overtime.\nA quiet world is waiting.\nNight shifts build great stories.\nOne more block before dawn?\nThe stars have your server covered.\nLate hours, legendary saves.\nThe torchlight looks especially good now.\nYour next adventure is still awake.\nThe caves are calmer after midnight.\nA peaceful spawn point awaits.\nThe night belongs to patient builders.\nKeep the soundtrack low and the ideas loud.\nEvery great base starts with one block.\nThe End can wait, unless it cannot.\nA small session still counts.\nThe world has been saved for you.',
    },
    dawn: {
        id: 'app.home.greeting.dawn',
        defaultMessage:
            'First light, fresh chunks.\nA new day is loading in.\nThe sunrise buff is active.\nMorning worlds feel brand new.\nCoffee first, diamonds second.\nYour base missed you overnight.\nA calm start makes a fine adventure.\nThe overworld is waking up.\nFresh air, fresh resource packs.\nToday is a good day to explore.\nThe village is already open for trade.\nA quiet morning suits a big build.\nNew day, new coordinates.\nThe creepers are not morning people either.\nStart small and see where it goes.\nYour next session is ready when you are.',
    },
    morning: {
        id: 'app.home.greeting.morning',
        defaultMessage:
            'Good morning, adventurer.\nThe day is full of unexplored chunks.\nA fine time for a fresh start.\nYour tools are ready for the day.\nThe overworld has excellent plans.\nBuild something your future self will love.\nA new session is a clean canvas.\nThe sun is up and so are the villagers.\nLet today be a little more blocky.\nThe mines have been suspiciously quiet.\nYour next project is only one launch away.\nA good morning for a good world.\nThere is always room for one more idea.\nThe crafting table is on standby.\nThe map is waiting for new markers.\nSettle in and make some progress.',
    },
    afternoon: {
        id: 'app.home.greeting.afternoon',
        defaultMessage:
            'Afternoon break, excellent timing.\nA short session can become a great one.\nThe world is ready for your next move.\nTime to check on that half-finished build.\nA little exploration goes a long way.\nThe next biome is calling.\nYour inventory has been waiting patiently.\nThe village market is still open.\nA good hour for a focused project.\nThe redstone probably behaves today.\nYour pickaxe is ready to work.\nA new route is waiting beyond spawn.\nThe afternoon is made for side quests.\nOne quick visit to your world?\nThe next chapter starts here.\nTake a moment and make something.',
    },
    evening: {
        id: 'app.home.greeting.evening',
        defaultMessage:
            "Evening is prime building time.\nThe day is winding down; the world is opening up.\nA familiar world makes a good landing spot.\nTime to return to your favorite project.\nThe sunset looks better from a new tower.\nYour base lights are waiting.\nA relaxed session sounds about right.\nThe villagers are closing shop soon.\nYour next build deserves an evening glow.\nA good time to wander without a plan.\nThe campfire is already lit.\nOne more room for the base?\nThe horizon is looking especially inviting.\nA quiet night starts with a good world.\nThe next block is yours to place.\nMake tonight's progress count.",
    },
    night: {
        id: 'app.home.greeting.night',
        defaultMessage:
            'The night shift is ready.\nA good evening for familiar worlds.\nYour favorite instance is waiting nearby.\nThe stars are out; the plans are in.\nA calm session can end the day well.\nThe world is quieter after dark.\nTime to put a few more blocks in place.\nYour base is glowing in the distance.\nThe next adventure starts at sunset.\nA night well spent has a good save file.\nThe campfire crackles, somewhere.\nThe moon makes every build look dramatic.\nA little Minecraft before tomorrow.\nYour worlds are ready for a visit.\nThe night is still young, in chunks.\nSettle in for a well-earned session.',
    },
})

const minimalGreetingMessages = {
    'late-night': messages.minimalLateNight,
    dawn: messages.minimalDawn,
    morning: messages.minimalMorning,
    afternoon: messages.minimalAfternoon,
    evening: messages.minimalEvening,
    night: messages.minimalNight,
}

const greeting = computed(() => {
    const bucket = getTimeBucket(now.value)
    const variants = formatMessage(messages[bucket]).split('\n').filter(Boolean)
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
        ? formatMessage(messages.welcomeWithPlayer, { name: displayName.value })
        : formatMessage(messages.welcome),
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
            ? formatMessage(messages.minimalWithPlayer, {
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
    props.dashboardSize || props.variant === 'hero'
        ? {
              '--home-greeting-font-family': greetingFontFamilies[props.greetingFont],
              '--home-greeting-font-size': `${props.greetingFontSize}px`,
          }
        : undefined,
)

const updateClock = () => {
    now.value = new Date()
}
const timer = window.setInterval(updateClock, 60_000)

onUnmounted(() => window.clearInterval(timer))
</script>

<template>
    <header
        class="home-greeting home-legible flex min-w-0 flex-col"
        :class="
            variant === 'minimal'
                ? 'items-center gap-3 text-center'
                : variant === 'hero'
                  ? 'gap-1.5'
                  : dashboardSize
                    ? 'h-full justify-center gap-2'
                    : 'gap-1 py-2'
        "
    >
        <span
            v-if="showDate && (variant !== 'standard' || dashboardSize)"
            class="text-xs font-bold leading-none tracking-normal text-[var(--color-text-tertiary)]"
        >
            {{ dateLabel }}
        </span>
        <div v-if="showHeading" ref="greetingGroup" class="group flex min-w-0 items-center gap-2">
            <h1
                class="m-0 max-w-full break-words font-extrabold text-[var(--color-text-primary)]"
                :class="[
                    dashboardSize || variant === 'hero'
                        ? 'home-greeting-heading'
                        : variant === 'minimal' && !accentRule
                          ? 'text-[2rem] leading-tight tracking-tight'
                          : 'text-2xl',
                    editable
                        ? 'cursor-pointer rounded-lg transition-transform duration-150 group-hover:-translate-y-0.5 group-focus-within:-translate-y-0.5 focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-shadow'
                        : '',
                ]"
                :style="headingStyle"
                :role="editable ? 'button' : undefined"
                :tabindex="editable ? 0 : undefined"
                :aria-label="editable ? formatMessage(messages.editGreeting) : undefined"
                @click="requestEdit"
                @keydown.enter.prevent="requestEdit"
                @keydown.space.prevent="requestEdit"
            >
                {{ heading }}
            </h1>
            <button
                v-if="editable"
                type="button"
                class="m-0 shrink-0 cursor-pointer border-0 bg-transparent p-1 text-[var(--color-text-tertiary)] opacity-0 transition-[opacity,color] duration-150 hover:text-[var(--color-text-primary)] focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-shadow group-hover:opacity-100 group-focus-within:opacity-100"
                :aria-label="formatMessage(messages.editGreeting)"
                @click="requestEdit"
            >
                <PencilIcon class="size-4" />
            </button>
        </div>
        <div
            v-if="variant === 'minimal' && accentRule && showHeading"
            class="home-greeting-rule h-0.5 w-8 rounded-full bg-brand"
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
