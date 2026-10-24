<script setup lang="ts">
import { type Component, defineAsyncComponent, onBeforeUnmount, onMounted, ref } from 'vue'

defineProps<{
    /** Accessible description of the launcher poster shown before the preview mounts. */
    label: string
}>()
const emit = defineEmits<{ close: [] }>()

/**
 * Starts the poster download from `<head>`, before the body is parsed, using the
 * same theme resolution as the site's `theme-init` script and the language the
 * i18n plugin will pick (the `locale` cookie, else Chinese). Populate the native
 * image as it is parsed so decoding does not wait for CSS background discovery.
 * The CSS poster remains the no-script fallback, with the same aspect ratio.
 */
const POSTER_PRELOAD = `(function(){try{var t=localStorage.getItem('axolotl-theme');var r=t;if(!r||r==='system'){r=(window.matchMedia&&window.matchMedia('(prefers-color-scheme: light)').matches)?'light':'dark'}var c=document.cookie.match(/(?:^|; )locale=([^;]*)/);var g=c&&decodeURIComponent(c[1])==='en-US'?'en':'zh';document.documentElement.setAttribute('data-launcher-poster',g);var l=document.createElement('link');l.rel='preload';l.as='image';l.href='/launcher-preview/poster-'+(r==='light'?'light':r==='oled'?'oled':'dark')+'-'+g+'.webp';l.setAttribute('fetchpriority','high');document.head.appendChild(l);document.documentElement.setAttribute('data-launcher-poster-src',l.href);var o;function p(){var n=document.querySelector('[data-launcher-poster-image]');if(n){n.src=l.href;if(o)o.disconnect()}}o=new MutationObserver(p);o.observe(document.documentElement,{childList:true,subtree:true});p()}catch(e){}})()`

useHead({
    script: [{ key: 'launcher-preview-poster', innerHTML: POSTER_PRELOAD }],
})

const host = ref<HTMLElement | null>(null)
const posterSrc = import.meta.client
    ? (document.documentElement.getAttribute('data-launcher-poster-src') ?? undefined)
    : undefined
const mounted = ref(false)
const ready = ref(false)
const failed = ref(false)

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
let disposed = false

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
    if (disposed || mounted.value || failed.value) return
    cancelIdle()
    observer?.disconnect()
    mounted.value = true
}

function loadThenMount() {
    loadPreview()
        .then(() => {
            if (!disposed) whenIdle(mountNow)
        })
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
    disposed = true
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
            class="launcher-preview-poster absolute inset-0 rounded-[10px] bg-cover bg-center"
            :class="{ 'launcher-preview-poster--hidden': ready }"
            @pointerdown="mountNow"
            @focusin="mountNow"
        >
            <img
                data-launcher-poster-image
                data-allow-mismatch="attribute"
                :src="posterSrc"
                alt=""
                width="1280"
                height="800"
                decoding="async"
                fetchpriority="high"
                draggable="false"
                class="h-full w-full rounded-[10px] object-cover [&:not([src])]:invisible"
            />
        </div>
        <div
            v-if="mounted && !failed"
            class="launcher-preview-stage launcher-preview-surface absolute inset-0 rounded-[10px]"
            :class="{ 'launcher-preview-stage--ready': ready }"
        >
            <LauncherPreview @ready="ready = true" @close="emit('close')" />
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

html[data-launcher-poster-src] .launcher-preview-poster {
    background-image: none;
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
