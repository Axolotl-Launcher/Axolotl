<script setup lang="ts">
import type { HomeLayout } from '@/store/theme.ts'

/**
 * A vector stand-in for the launcher window. Text is drawn as skeleton bars and
 * every colour comes from the surrounding theme tokens, so wrapping it in a
 * `*-mode` class previews that theme. `--oobe-accent` carries the user's accent
 * in from outside, because a theme class resets `--color-brand` to its default.
 */
defineProps<{
    layout: HomeLayout
}>()

const navIcons = [26, 44, 62, 80, 98]
</script>

<template>
    <svg
        class="block h-auto w-full text-[var(--oobe-accent,var(--color-brand))]"
        viewBox="0 0 240 150"
        fill="none"
        aria-hidden="true"
    >
        <rect width="240" height="150" rx="10" fill="var(--surface-2)" />
        <circle cx="13" cy="9" r="3.5" fill="currentColor" />
        <circle cx="212" cy="9" r="2.5" fill="var(--surface-5)" />
        <circle cx="222" cy="9" r="2.5" fill="var(--surface-5)" />
        <circle cx="232" cy="9" r="2.5" fill="var(--surface-5)" />
        <rect
            v-for="(y, index) in navIcons"
            :key="y"
            x="7"
            :y="y"
            width="12"
            height="12"
            rx="3.5"
            :fill="index === 0 ? 'currentColor' : 'var(--surface-4)'"
        />
        <path d="M26 30 a12 12 0 0 1 12 -12 h202 v132 h-214 z" fill="var(--surface-1)" />

        <template v-if="layout === 'standard'">
            <rect x="38" y="28" width="72" height="7" rx="3.5" fill="var(--surface-5)" />
            <rect x="38" y="39" width="44" height="5" rx="2.5" fill="var(--surface-4)" />
            <rect x="38" y="52" width="118" height="44" rx="7" fill="var(--surface-3)" />
            <rect x="45" y="59" width="30" height="30" rx="5" fill="var(--surface-4)" />
            <rect x="82" y="59" width="30" height="30" rx="5" fill="var(--surface-4)" />
            <rect x="119" y="59" width="30" height="30" rx="5" fill="currentColor" opacity="0.8" />
            <rect x="162" y="52" width="68" height="44" rx="7" fill="var(--surface-3)" />
            <rect x="169" y="60" width="44" height="5" rx="2.5" fill="var(--surface-5)" />
            <rect x="169" y="70" width="54" height="4" rx="2" fill="var(--surface-4)" />
            <rect x="169" y="78" width="38" height="4" rx="2" fill="var(--surface-4)" />
            <rect x="38" y="102" width="62" height="38" rx="7" fill="var(--surface-3)" />
            <rect x="106" y="102" width="62" height="38" rx="7" fill="var(--surface-3)" />
            <rect x="174" y="102" width="56" height="38" rx="7" fill="var(--surface-3)" />
            <rect x="45" y="110" width="32" height="5" rx="2.5" fill="var(--surface-5)" />
            <rect x="113" y="110" width="40" height="5" rx="2.5" fill="var(--surface-5)" />
            <rect x="181" y="110" width="28" height="5" rx="2.5" fill="var(--surface-5)" />
            <path
                d="M45 132 l10 -8 l8 5 l12 -10"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
            />
        </template>

        <template v-else>
            <rect x="97" y="40" width="72" height="8" rx="4" fill="var(--surface-5)" />
            <rect x="111" y="52" width="44" height="5" rx="2.5" fill="var(--surface-4)" />
            <rect x="52" y="70" width="162" height="46" rx="8" fill="var(--surface-3)" />
            <rect x="60" y="78" width="30" height="30" rx="6" fill="var(--surface-4)" />
            <rect x="98" y="84" width="58" height="7" rx="3.5" fill="var(--surface-5)" />
            <rect x="98" y="96" width="38" height="5" rx="2.5" fill="var(--surface-4)" />
            <rect x="166" y="83" width="40" height="20" rx="6" fill="currentColor" />
            <path d="M182 88 l8 5 l-8 5 z" fill="var(--color-accent-contrast)" />
        </template>
    </svg>
</template>
