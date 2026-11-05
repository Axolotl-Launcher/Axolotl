<script setup lang="ts">
import { CheckIcon, InfoIcon } from '@modrinth/assets'
import { defineMessages, type MessageDescriptor, useVIntl } from '@modrinth/ui'
import { ref } from 'vue'

import { update } from '@/helpers/settings.ts'
import { useTheming } from '@/store/state'
import type { ColorTheme, HomeLayout } from '@/store/theme.ts'

import { useOobeStep } from '../../context'
import LauncherSketch from '../../parts/LauncherSketch.vue'

const { formatMessage } = useVIntl()
const themeStore = useTheming()
const step = useOobeStep()

const messages = defineMessages({
    homeTitle: { id: 'app.oobe.appearance.home.title', defaultMessage: 'Home page' },
    standard: { id: 'app.oobe.appearance.home.standard', defaultMessage: 'Information' },
    standardDetail: {
        id: 'app.oobe.appearance.home.standard-detail',
        defaultMessage: 'A greeting and widgets for your instances, news and more.',
    },
    minimal: { id: 'app.oobe.appearance.home.minimal', defaultMessage: 'Minimal' },
    minimalDetail: {
        id: 'app.oobe.appearance.home.minimal-detail',
        defaultMessage: 'One instance and a Play button, nothing else.',
    },
    themeTitle: { id: 'app.oobe.appearance.theme.title', defaultMessage: 'Color theme' },
    dark: { id: 'app.oobe.appearance.theme.dark', defaultMessage: 'Dark' },
    light: { id: 'app.oobe.appearance.theme.light', defaultMessage: 'Light' },
    oled: { id: 'app.oobe.appearance.theme.oled', defaultMessage: 'OLED' },
    system: { id: 'app.oobe.appearance.theme.system', defaultMessage: 'Match system' },
    later: {
        id: 'app.oobe.appearance.later',
        defaultMessage:
            'You can change these any time in Settings: the color theme under Appearance, the home page under Home & navigation.',
    },
    saveFailed: {
        id: 'app.oobe.appearance.save-failed',
        defaultMessage: 'This choice could not be saved. It still applies until you restart.',
    },
})

const layouts: { value: HomeLayout; label: MessageDescriptor; detail: MessageDescriptor }[] = [
    { value: 'standard', label: messages.standard, detail: messages.standardDetail },
    { value: 'minimal', label: messages.minimal, detail: messages.minimalDetail },
]

const themes: { value: ColorTheme; label: MessageDescriptor }[] = [
    { value: 'dark', label: messages.dark },
    { value: 'light', label: messages.light },
    { value: 'oled', label: messages.oled },
    { value: 'system', label: messages.system },
]

const failed = ref(false)
/** Saves run in the order they were made, and the step stays busy until the last one settles. */
let saves: Promise<void> = Promise.resolve()
let pending = 0

function save(patch: { home_layout?: HomeLayout; theme?: ColorTheme }) {
    failed.value = false
    pending += 1
    step.setBusy(true)
    saves = saves.then(async () => {
        try {
            await update(patch)
        } catch (error) {
            console.warn('Failed to save the OOBE appearance choice', error)
            failed.value = true
        } finally {
            pending -= 1
            if (pending === 0) step.setBusy(false)
        }
    })
}

function chooseLayout(value: HomeLayout) {
    if (themeStore.homeLayout === value) return
    themeStore.homeLayout = value
    save({ home_layout: value })
}

function chooseTheme(value: ColorTheme) {
    if (themeStore.selectedTheme === value) return
    themeStore.setThemeState(value)
    save({ theme: value })
}
</script>

<template>
    <div class="flex flex-col gap-4 [--oobe-accent:var(--color-brand)]">
        <section class="flex flex-col gap-2">
            <h2 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.homeTitle) }}
            </h2>
            <div
                role="radiogroup"
                :aria-label="formatMessage(messages.homeTitle)"
                class="grid grid-cols-2 gap-3"
            >
                <button
                    v-for="layout in layouts"
                    :key="layout.value"
                    type="button"
                    role="radio"
                    :aria-checked="themeStore.homeLayout === layout.value"
                    class="oobe-tile relative flex cursor-pointer flex-col gap-2 rounded-[var(--radius-lg)] border-2 border-solid p-2.5 text-left transition-[border-color,background-color] duration-150"
                    :class="
                        themeStore.homeLayout === layout.value
                            ? 'border-brand bg-brand-highlight'
                            : 'border-surface-4 bg-surface-3 hover:border-surface-5'
                    "
                    @click="chooseLayout(layout.value)"
                >
                    <span class="overflow-hidden rounded-[var(--radius-md)] shadow-md">
                        <LauncherSketch :layout="layout.value" />
                    </span>
                    <span class="flex flex-col gap-0.5 px-1 pb-0.5">
                        <span class="font-semibold text-[var(--color-text-primary)]">
                            {{ formatMessage(layout.label) }}
                        </span>
                        <span class="text-sm leading-snug text-[var(--color-text-tertiary)]">
                            {{ formatMessage(layout.detail) }}
                        </span>
                    </span>
                    <CheckIcon
                        v-if="themeStore.homeLayout === layout.value"
                        class="oobe-tile-check absolute right-5 top-5 size-6 rounded-full bg-brand p-1 text-[var(--color-accent-contrast)]"
                    />
                </button>
            </div>
        </section>

        <section class="flex flex-col gap-2">
            <h2 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.themeTitle) }}
            </h2>
            <div
                role="radiogroup"
                :aria-label="formatMessage(messages.themeTitle)"
                class="grid grid-cols-4 gap-3"
            >
                <button
                    v-for="theme in themes"
                    :key="theme.value"
                    type="button"
                    role="radio"
                    :aria-checked="themeStore.selectedTheme === theme.value"
                    class="oobe-tile relative flex cursor-pointer flex-col gap-2 rounded-[var(--radius-lg)] border-2 border-solid p-2 text-left transition-[border-color,background-color] duration-150"
                    :class="
                        themeStore.selectedTheme === theme.value
                            ? 'border-brand bg-brand-highlight'
                            : 'border-surface-4 bg-surface-3 hover:border-surface-5'
                    "
                    @click="chooseTheme(theme.value)"
                >
                    <span class="relative overflow-hidden rounded-[var(--radius-sm)]">
                        <span
                            v-if="theme.value === 'system'"
                            class="grid [&>*]:col-start-1 [&>*]:row-start-1"
                        >
                            <span class="light-mode block">
                                <LauncherSketch :layout="themeStore.homeLayout" />
                            </span>
                            <span class="oobe-system-half dark-mode block">
                                <LauncherSketch :layout="themeStore.homeLayout" />
                            </span>
                        </span>
                        <span v-else :class="`${theme.value}-mode`" class="block">
                            <LauncherSketch :layout="themeStore.homeLayout" />
                        </span>
                    </span>
                    <span class="flex items-center gap-1.5 px-1 pb-0.5">
                        <span
                            class="truncate text-sm font-semibold text-[var(--color-text-primary)]"
                        >
                            {{ formatMessage(theme.label) }}
                        </span>
                        <CheckIcon
                            v-if="themeStore.selectedTheme === theme.value"
                            class="ml-auto size-4 shrink-0 text-brand"
                        />
                    </span>
                </button>
            </div>
        </section>

        <p class="m-0 flex items-center gap-2 text-sm text-[var(--color-text-tertiary)]">
            <InfoIcon class="size-4 shrink-0" />
            {{ formatMessage(failed ? messages.saveFailed : messages.later) }}
        </p>
    </div>
</template>

<style scoped lang="scss">
.oobe-tile:focus-visible {
    outline: none;
    box-shadow: 0 0 0 3px var(--color-focus-ring);
}

.oobe-system-half {
    clip-path: polygon(62% 0, 100% 0, 100% 100%, 38% 100%);
}

.oobe-tile-check {
    animation: oobe-tile-check 260ms cubic-bezier(0.34, 1.56, 0.64, 1);
}

@keyframes oobe-tile-check {
    from {
        scale: 0.5;
        opacity: 0;
    }
}

@media (prefers-reduced-motion: reduce) {
    .oobe-tile-check {
        animation: none;
    }
}
</style>
