<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui'
import {
    type Component,
    defineAsyncComponent,
    nextTick,
    onBeforeUnmount,
    onMounted,
    ref,
} from 'vue'

import LauncherSplash from './shared/LauncherSplash.vue'

defineProps<{ label: string }>()
const emit = defineEmits<{ close: [] }>()
const { formatMessage } = useVIntl()
const messages = defineMessages({
    loading: { id: 'axolotl-preview.loading', defaultMessage: 'Loading Axolotl Launcher' },
    failed: { id: 'axolotl-preview.failed', defaultMessage: 'The launcher could not load.' },
    retry: { id: 'axolotl-preview.retry', defaultMessage: 'Try again' },
})

const host = ref<HTMLElement | null>(null)
const mounted = ref(false)
const prepared = ref(false)
const ready = ref(false)
const failed = ref(false)
const progress = ref(0)
const launchCount = ref(0)
let disposed = false
let previewModule: Promise<Component> | null = null

function loadPreview() {
    previewModule ??= import.meta.client
        ? import('./LauncherPreview.vue')
              .then((module) => module.default)
              .catch((error) => {
                  previewModule = null
                  throw error
              })
        : Promise.reject(new Error('The launcher preview runs in the browser'))
    return previewModule
}

const LauncherPreview = defineAsyncComponent({
    loader: loadPreview,
    onError(_error, _retry, fail) {
        failed.value = true
        fail()
    },
})

let observer: IntersectionObserver | null = null
let idleHandle: number | null = null
let loadListener: (() => void) | null = null

function whenIdle(callback: () => void) {
    if ('requestIdleCallback' in window)
        idleHandle = window.requestIdleCallback(callback, { timeout: 1500 })
    else idleHandle = window.setTimeout(callback, 200)
}

function cancelIdle() {
    if (idleHandle === null) return
    if ('cancelIdleCallback' in window) window.cancelIdleCallback(idleHandle)
    else window.clearTimeout(idleHandle)
    idleHandle = null
}

function mountNow() {
    if (disposed || mounted.value || failed.value) return
    cancelIdle()
    observer?.disconnect()
    mounted.value = true
}

function loadThenMount() {
    loadPreview()
        .then(() => {
            if (disposed) return
            progress.value = 25
            whenIdle(mountNow)
        })
        .catch(() => {
            if (!disposed) failed.value = true
        })
}

async function finishLoading() {
    progress.value = 100
    await nextTick()
    if (disposed) return
    prepared.value = true
}

async function retry() {
    failed.value = false
    mounted.value = false
    prepared.value = false
    ready.value = false
    progress.value = 0
    launchCount.value++
    await nextTick()
    loadThenMount()
}

function observeHero() {
    if (!host.value) return
    if (!('IntersectionObserver' in window)) {
        whenIdle(loadThenMount)
        return
    }
    observer = new IntersectionObserver(
        (entries) => {
            if (!entries.some((entry) => entry.isIntersecting)) return
            observer?.disconnect()
            observer = null
            whenIdle(loadThenMount)
        },
        { rootMargin: '200px' },
    )
    observer.observe(host.value)
}

onMounted(() => {
    if (document.readyState === 'complete') observeHero()
    else {
        loadListener = observeHero
        window.addEventListener('load', loadListener, { once: true })
    }
})

onBeforeUnmount(() => {
    disposed = true
    observer?.disconnect()
    cancelIdle()
    if (loadListener) window.removeEventListener('load', loadListener)
})
</script>

<template>
    <div
        ref="host"
        class="launcher-preview-hero relative w-full rounded-[10px]"
        :aria-label="label"
        :aria-busy="!ready"
    >
        <div
            v-if="mounted && !failed"
            class="launcher-preview-stage launcher-preview-surface absolute inset-0 rounded-[10px]"
            :class="{ 'launcher-preview-stage--ready': ready }"
            :inert="!ready || undefined"
            :aria-hidden="!ready || undefined"
        >
            <LauncherPreview
                :key="launchCount"
                @ready="finishLoading"
                @progress="progress = $event"
                @failed="failed = true"
                @close="emit('close')"
            />
        </div>
        <Transition name="splash-fade" @after-leave="ready = true">
            <LauncherSplash
                v-if="!prepared"
                class="z-10"
                :progress="progress"
                :loading-label="formatMessage(messages.loading)"
                :failure-label="formatMessage(messages.failed)"
                :retry-label="formatMessage(messages.retry)"
                :failed="failed"
                @retry="retry"
            />
        </Transition>
    </div>
</template>

<style scoped lang="scss">
.launcher-preview-hero {
    aspect-ratio: 1280 / 800;
}

.launcher-preview-surface {
    box-shadow:
        0 0 0 0.5px rgb(0 0 0 / 50%),
        0 22px 70px 4px rgb(0 0 0 / 56%);
}

html.light-mode .launcher-preview-surface {
    box-shadow:
        0 0 0 0.5px rgb(0 0 0 / 18%),
        0 22px 70px 4px rgb(0 0 0 / 22%);
}

.splash-fade-leave-active {
    transition: opacity 300ms ease-in-out;
}

.splash-fade-leave-to {
    opacity: 0;
}

@media (prefers-reduced-motion: reduce) {
    .splash-fade-leave-active {
        transition-duration: 100ms;
    }
}
</style>
