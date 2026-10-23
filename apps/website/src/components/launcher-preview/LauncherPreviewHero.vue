<script setup lang="ts">
import { type Component, defineAsyncComponent, onBeforeUnmount, onMounted, ref } from 'vue'

defineProps<{
    /** Accessible description of the launcher poster shown before the preview mounts. */
    label: string
}>()

/**
 * Starts the poster download from `<head>`, before the body is parsed, using the
 * same theme resolution as the site's `theme-init` script and the language the
 * i18n plugin will pick (the `locale` cookie, else Chinese). The poster is a CSS
 * background so that only the variant matching the visitor's theme and language
 * is fetched; `data-launcher-poster` carries the language to the stylesheet.
 */
const POSTER_PRELOAD = `(function(){try{var t=localStorage.getItem('axolotl-theme');var r=t;if(!r||r==='system'){r=(window.matchMedia&&window.matchMedia('(prefers-color-scheme: light)').matches)?'light':'dark'}var c=document.cookie.match(/(?:^|; )locale=([^;]*)/);var g=c&&decodeURIComponent(c[1])==='en-US'?'en':'zh';document.documentElement.setAttribute('data-launcher-poster',g);var l=document.createElement('link');l.rel='preload';l.as='image';l.href='/launcher-preview/poster-'+(r==='light'?'light':r==='oled'?'oled':'dark')+'-'+g+'.webp';l.setAttribute('fetchpriority','high');document.head.appendChild(l)}catch(e){}})()`

useHead({
    script: [{ key: 'launcher-preview-poster', innerHTML: POSTER_PRELOAD }],
})

const host = ref<HTMLElement | null>(null)
const mounted = ref(false)
const ready = ref(false)
const failed = ref(false)
/** Bumped when the launcher is quit and opened again, so it starts from a fresh state. */
const launchCount = ref(0)

function relaunch() {
    ready.value = false
    launchCount.value++
}

let previewModule: Promise<Component> | null = null
function loadPreview() {
    // The preview only runs in the browser, so the server bundle leaves it out.
    previewModule ??= import.meta.client
        ? import('./LauncherPreview.vue').then((module) => module.default)
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
    if ('requestIdleCallback' in window) {
        idleHandle = window.requestIdleCallback(callback, { timeout: 1500 })
    } else {
        idleHandle = window.setTimeout(callback, 200)
    }
}

function cancelIdle() {
    if (idleHandle === null) return
    if ('cancelIdleCallback' in window) window.cancelIdleCallback(idleHandle)
    else window.clearTimeout(idleHandle)
    idleHandle = null
}

function mountNow() {
    if (mounted.value || failed.value) return
    cancelIdle()
    observer?.disconnect()
    mounted.value = true
}

function loadThenMount() {
    loadPreview()
        .then(() => whenIdle(mountNow))
        .catch(() => {
            failed.value = true
        })
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
    // Wait for the document's own load so the preview never competes with the
    // first paint of the page, then start once the hero is near the viewport.
    if (document.readyState === 'complete') {
        observeHero()
    } else {
        loadListener = () => observeHero()
        window.addEventListener('load', loadListener, { once: true })
    }
})

onBeforeUnmount(() => {
    observer?.disconnect()
    cancelIdle()
    if (loadListener) window.removeEventListener('load', loadListener)
})
</script>

<template>
    <div ref="host" class="launcher-preview-hero relative w-full">
        <div
            role="img"
            :aria-label="label"
            :aria-hidden="ready ? 'true' : undefined"
            class="launcher-preview-poster launcher-preview-surface absolute inset-0 rounded-[10px] bg-cover bg-center"
            :class="{ 'launcher-preview-poster--hidden': ready }"
            @pointerdown="mountNow"
            @focusin="mountNow"
        />
        <div
            v-if="mounted && !failed"
            class="launcher-preview-stage launcher-preview-surface absolute inset-0 rounded-[10px]"
            :class="{ 'launcher-preview-stage--ready': ready }"
        >
            <LauncherPreview :key="launchCount" @ready="ready = true" @relaunch="relaunch" />
        </div>
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

.launcher-preview-poster {
    background-image: url('/launcher-preview/poster-dark-zh.webp');
    transition:
        opacity 320ms ease,
        visibility 0s linear 320ms;
}

html.light-mode .launcher-preview-poster {
    background-image: url('/launcher-preview/poster-light-zh.webp');
}

html.oled-mode .launcher-preview-poster {
    background-image: url('/launcher-preview/poster-oled-zh.webp');
}

html[data-launcher-poster='en'] .launcher-preview-poster {
    background-image: url('/launcher-preview/poster-dark-en.webp');
}

html.light-mode[data-launcher-poster='en'] .launcher-preview-poster {
    background-image: url('/launcher-preview/poster-light-en.webp');
}

html.oled-mode[data-launcher-poster='en'] .launcher-preview-poster {
    background-image: url('/launcher-preview/poster-oled-en.webp');
}

.launcher-preview-poster--hidden {
    opacity: 0;
    visibility: hidden;
}

.launcher-preview-stage {
    opacity: 0;
    transition: opacity 320ms ease;
}

.launcher-preview-stage--ready {
    opacity: 1;
}
</style>
