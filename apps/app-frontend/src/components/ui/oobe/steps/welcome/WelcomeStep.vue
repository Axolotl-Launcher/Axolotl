<script setup lang="ts">
import { RightArrowIcon } from '@modrinth/assets'
import axolotlVisual from '@modrinth/assets/branding/axolotl.png'
import { Button, defineMessages, type MessageDescriptor, useVIntl } from '@modrinth/ui'
import { onMounted, ref } from 'vue'

import { useOobeStep } from '../../context'

defineProps<{
    title: MessageDescriptor
}>()

const { formatMessage } = useVIntl()
const step = useOobeStep()
const action = ref<HTMLElement | null>(null)

const messages = defineMessages({
    ready: {
        id: 'app.oobe.welcome.ready',
        defaultMessage: 'Everything is set. Click anywhere to open the launcher.',
    },
    start: { id: 'app.oobe.welcome.start', defaultMessage: 'Get started' },
})

const sparks = [
    { left: '18%', top: '24%', size: 10, delay: 0, shape: 'round' },
    { left: '28%', top: '70%', size: 14, delay: 0.6, shape: 'square' },
    { left: '76%', top: '22%', size: 12, delay: 0.3, shape: 'square' },
    { left: '82%', top: '64%', size: 9, delay: 0.9, shape: 'round' },
    { left: '64%', top: '82%', size: 7, delay: 1.2, shape: 'round' },
    { left: '38%', top: '14%', size: 6, delay: 1.5, shape: 'round' },
] as const

onMounted(() => {
    action.value?.querySelector('button')?.focus()
})
</script>

<template>
    <div
        class="oobe-welcome relative flex h-full w-full cursor-pointer flex-col items-center justify-center gap-8 text-center"
        @click="step.next()"
    >
        <span
            v-for="(spark, index) in sparks"
            :key="index"
            aria-hidden="true"
            class="oobe-welcome-spark pointer-events-none absolute bg-brand"
            :class="spark.shape === 'round' ? 'rounded-full' : 'rounded-[3px]'"
            :style="{
                left: spark.left,
                top: spark.top,
                width: `${spark.size}px`,
                height: `${spark.size}px`,
                animationDelay: `${spark.delay}s`,
            }"
        />
        <div class="oobe-welcome-logo relative size-40">
            <span class="oobe-welcome-ring absolute inset-[-28%] rounded-full" aria-hidden="true" />
            <img :src="axolotlVisual" alt="" class="relative size-full object-contain" />
        </div>
        <div class="oobe-welcome-copy flex flex-col items-center gap-3">
            <h1
                class="m-0 text-[3.5rem] font-extrabold leading-none text-[var(--color-text-primary)]"
            >
                {{ formatMessage(title) }}
            </h1>
            <p class="m-0 text-lg text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.ready) }}
            </p>
        </div>
        <div ref="action" class="oobe-welcome-action">
            <Button type="colored" color="brand" size="xl" @click.stop="step.next()">
                {{ formatMessage(messages.start) }}
                <RightArrowIcon />
            </Button>
        </div>
    </div>
</template>

<style scoped lang="scss">
.oobe-welcome-logo {
    animation: oobe-welcome-logo 900ms cubic-bezier(0.16, 1, 0.3, 1) both;
}

.oobe-welcome-ring {
    background: radial-gradient(
        circle,
        color-mix(in srgb, var(--color-brand) 35%, transparent) 0%,
        transparent 68%
    );
    animation: oobe-welcome-breathe 4s ease-in-out infinite;
}

.oobe-welcome-copy {
    animation: oobe-welcome-rise 700ms 250ms cubic-bezier(0.16, 1, 0.3, 1) both;
}

.oobe-welcome-action {
    animation: oobe-welcome-rise 700ms 450ms cubic-bezier(0.16, 1, 0.3, 1) both;
}

.oobe-welcome-spark {
    opacity: 0;
    animation: oobe-welcome-spark 5s ease-in-out infinite;
}

@keyframes oobe-welcome-logo {
    from {
        opacity: 0;
        transform: scale(0.6) rotate(-8deg);
    }
}

@keyframes oobe-welcome-rise {
    from {
        opacity: 0;
        transform: translateY(18px);
    }
}

@keyframes oobe-welcome-breathe {
    0%,
    100% {
        transform: scale(0.92);
        opacity: 0.7;
    }
    50% {
        transform: scale(1.06);
        opacity: 1;
    }
}

@keyframes oobe-welcome-spark {
    0% {
        opacity: 0;
        transform: translateY(12px) rotate(0deg) scale(0.6);
    }
    30%,
    70% {
        opacity: 0.55;
    }
    100% {
        opacity: 0;
        transform: translateY(-28px) rotate(90deg) scale(1);
    }
}

@media (prefers-reduced-motion: reduce) {
    .oobe-welcome-logo,
    .oobe-welcome-ring,
    .oobe-welcome-copy,
    .oobe-welcome-action {
        animation: none;
    }

    .oobe-welcome-spark {
        display: none;
    }
}
</style>
