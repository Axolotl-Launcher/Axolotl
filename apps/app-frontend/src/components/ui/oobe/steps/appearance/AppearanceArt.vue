<script setup lang="ts">
import { useTheming } from '@/store/state'

import LauncherSketch from '../../parts/LauncherSketch.vue'

const themeStore = useTheming()
</script>

<template>
    <div class="flex items-center justify-center [--oobe-accent:var(--color-brand)]">
        <div class="relative aspect-[4/5] w-[78%] max-w-[22rem]">
            <span
                class="oobe-appearance-window oobe-appearance-back light-mode absolute left-0 top-[14%] block w-[78%] overflow-hidden rounded-[var(--radius-md)]"
            >
                <LauncherSketch
                    :layout="themeStore.homeLayout === 'minimal' ? 'standard' : 'minimal'"
                />
            </span>
            <span
                class="oobe-appearance-window oobe-appearance-front dark-mode absolute bottom-[14%] right-0 block w-[78%] overflow-hidden rounded-[var(--radius-md)]"
            >
                <LauncherSketch :layout="themeStore.homeLayout" />
            </span>
            <svg
                class="oobe-appearance-toggle absolute left-[40%] top-[2%] size-[22%]"
                viewBox="0 0 64 64"
                fill="none"
            >
                <circle cx="32" cy="32" r="30" fill="var(--surface-4)" />
                <path d="M32 6 a26 26 0 0 1 0 52 z" fill="var(--color-brand)" />
                <circle cx="32" cy="32" r="26" stroke="var(--surface-5)" stroke-width="2" />
            </svg>
            <span
                class="oobe-appearance-dot oobe-appearance-dot-a absolute bottom-[6%] left-[8%] size-4 rounded-full bg-brand"
            />
            <span
                class="oobe-appearance-dot oobe-appearance-dot-b absolute right-[4%] top-[10%] size-3 rounded-full bg-surface-5"
            />
        </div>
    </div>
</template>

<style scoped lang="scss">
.oobe-appearance-window {
    box-shadow: var(--shadow-floating);
    transition: transform 500ms cubic-bezier(0.22, 1, 0.36, 1);
}

.oobe-appearance-back {
    transform: rotate(-7deg);
    animation: oobe-appearance-float-back 7s ease-in-out infinite;
}

.oobe-appearance-front {
    transform: rotate(5deg);
    animation: oobe-appearance-float-front 7s ease-in-out infinite;
}

.oobe-appearance-toggle {
    animation: oobe-appearance-spin 12s linear infinite;
}

.oobe-appearance-dot {
    animation: oobe-appearance-bob 5s ease-in-out infinite;
}

.oobe-appearance-dot-b {
    animation-delay: -2.5s;
}

@keyframes oobe-appearance-float-back {
    0%,
    100% {
        transform: rotate(-7deg) translateY(0);
    }
    50% {
        transform: rotate(-6deg) translateY(-6px);
    }
}

@keyframes oobe-appearance-float-front {
    0%,
    100% {
        transform: rotate(5deg) translateY(0);
    }
    50% {
        transform: rotate(4deg) translateY(-8px);
    }
}

@keyframes oobe-appearance-spin {
    to {
        transform: rotate(360deg);
    }
}

@keyframes oobe-appearance-bob {
    0%,
    100% {
        transform: translateY(0);
    }
    50% {
        transform: translateY(-10px);
    }
}

@media (prefers-reduced-motion: reduce) {
    .oobe-appearance-back,
    .oobe-appearance-front,
    .oobe-appearance-toggle,
    .oobe-appearance-dot {
        animation: none;
    }
}
</style>
