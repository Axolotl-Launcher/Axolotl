<script setup lang="ts">
import './styles/window.scss'

import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { provideModrinthClient } from '@modrinth/ui/src/providers/api-client.ts'
import { provideFilePicker } from '@modrinth/ui/src/providers/file-picker.ts'
import { provideModalBehavior } from '@modrinth/ui/src/providers/modal-behavior.ts'
import { providePopupNotificationManager } from '@modrinth/ui/src/providers/popup-notifications.ts'
import { provideNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'

import { createPreviewFilePicker } from './lib/file-picker'
import { providePreviewI18n } from './lib/i18n'
import { shellMessages } from './lib/messages'
import { createPreviewModrinthClient } from './lib/modrinth-client'
import { createPreviewRouter, providePreviewRouter } from './lib/router'
import { previewScreens } from './lib/screens'
import { createPreviewState, providePreviewState, setCurrentPreview } from './lib/state'
import { providePreviewWindow, WINDOW_HEIGHT, WINDOW_WIDTH } from './lib/window'
import PreviewShell from './shell/PreviewShell.vue'

/**
 * The launcher window: providers for the in-memory launcher, the 1280×800 window
 * scaled to the hero, and the macOS window states driven by the titlebar
 * controls (minimized, full screen, close).
 */
const emit = defineEmits<{ ready: []; close: [] }>()

const frame = ref<HTMLElement | null>(null)
const windowEl = ref<HTMLElement | null>(null)
const layerEl = ref<HTMLElement | null>(null)
const scale = ref(1)
const offset = ref({ x: 0, y: 0 })
const windowState = ref<'open' | 'minimized'>('open')
const fullScreen = ref(false)
/** Only window-state changes animate the transform; resizing the page must not lag. */
const animatingState = ref(false)
let animationTimer: ReturnType<typeof setTimeout> | undefined

function setWindowState(next: 'open' | 'minimized') {
    animatingState.value = true
    windowState.value = next
    clearTimeout(animationTimer)
    animationTimer = setTimeout(() => {
        animatingState.value = false
    }, 360)
}

const windowTransform = computed(() => {
    const s = scale.value
    if (windowState.value === 'open') {
        return `translate(${offset.value.x}px, ${offset.value.y}px) scale(${s})`
    }
    // Shrink towards the bottom centre of the frame, where the reopen control appears.
    const width = WINDOW_WIDTH * s
    const height = WINDOW_HEIGHT * s
    return `translate(${offset.value.x + width * 0.4}px, ${offset.value.y + height * 0.85}px) scale(${s * 0.2})`
})

providePreviewI18n()
const { formatMessage } = useVIntl()
const readyTasks = new Set<Promise<void>>()
providePreviewWindow({
    root: windowEl,
    layer: layerEl,
    scale,
    fullScreen,
    deferReady() {
        let complete!: () => void
        const task = new Promise<void>((resolve) => {
            complete = resolve
        })
        readyTasks.add(task)
        return () => {
            complete()
            readyTasks.delete(task)
        }
    },
    toLocal(clientX, clientY) {
        const rect = windowEl.value?.getBoundingClientRect()
        if (!rect) return { x: WINDOW_WIDTH / 2, y: WINDOW_HEIGHT / 2 }
        return {
            x: (clientX - rect.left) / scale.value,
            y: (clientY - rect.top) / scale.value,
        }
    },
})

const state = createPreviewState()
providePreviewState(state)
setCurrentPreview(state)
provideNotificationManager(state.notifications)
provideFilePicker(createPreviewFilePicker())
provideModrinthClient(createPreviewModrinthClient())
providePopupNotificationManager(state.popups)
provideModalBehavior({ noblur: computed(() => !state.settings.advanced_rendering) })

const router = createPreviewRouter(previewScreens)
providePreviewRouter(router)

let resizeObserver: ResizeObserver | null = null

function updateScale() {
    if (!frame.value) return
    const width = frame.value.clientWidth
    const height = frame.value.clientHeight
    if (fullScreen.value) {
        const fit = Math.min(width / WINDOW_WIDTH, height / WINDOW_HEIGHT)
        scale.value = fit
        offset.value = {
            x: (width - WINDOW_WIDTH * fit) / 2,
            y: (height - WINDOW_HEIGHT * fit) / 2,
        }
    } else {
        scale.value = width / WINDOW_WIDTH
        offset.value = { x: 0, y: 0 }
    }
}

async function toggleFullScreen() {
    if (document.fullscreenElement) {
        await document.exitFullscreen().catch(() => undefined)
        return
    }
    await frame.value?.requestFullscreen?.().catch(() => undefined)
}

function onFullScreenChange() {
    fullScreen.value = document.fullscreenElement === frame.value
    void nextTick(updateScale)
}

async function closeWindow() {
    if (document.fullscreenElement === frame.value) {
        await document.exitFullscreen().catch(() => undefined)
    }
    emit('close')
}

/** Resolves once the images inside the window have loaded, or after `timeoutMs`. */
function imagesSettled(root: HTMLElement, timeoutMs: number) {
    const pending = Array.from(root.querySelectorAll('img'))
        .filter((image) => !image.complete)
        .map(
            (image) =>
                new Promise<void>((resolve) => {
                    image.addEventListener('load', () => resolve(), { once: true })
                    image.addEventListener('error', () => resolve(), { once: true })
                }),
        )
    return Promise.race([
        Promise.all(pending),
        new Promise<void>((resolve) => setTimeout(resolve, timeoutMs)),
    ])
}

onMounted(async () => {
    updateScale()
    resizeObserver = new ResizeObserver(updateScale)
    if (frame.value) resizeObserver.observe(frame.value)
    document.addEventListener('fullscreenchange', onFullScreenChange)

    await router.push(state.settings.default_page === 'Library' ? '/library' : '/')
    await router.isReady()
    await nextTick()
    // Let observed panel sizes settle before collecting the home widgets' images.
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))
    await nextTick()
    while (readyTasks.size) await Promise.all([...readyTasks])
    if (windowEl.value) await imagesSettled(windowEl.value, 1500)
    emit('ready')
})

onBeforeUnmount(() => {
    clearTimeout(animationTimer)
    resizeObserver?.disconnect()
    document.removeEventListener('fullscreenchange', onFullScreenChange)
    if (document.fullscreenElement === frame.value) void document.exitFullscreen()
    state.dispose()
    setCurrentPreview(null)
})
</script>

<template>
    <div
        ref="frame"
        class="launcher-preview-frame absolute inset-0"
        :class="{ 'bg-black': fullScreen }"
    >
        <div
            ref="windowEl"
            class="launcher-preview-window absolute left-0 top-0 h-[800px] w-[1280px] origin-top-left overflow-hidden"
            :class="[
                state.theme.className.value,
                state.theme.accentClassName.value,
                fullScreen ? 'rounded-none' : 'rounded-[10px]',
                {
                    'launcher-preview-window--hidden': windowState !== 'open',
                    'launcher-preview-window--animating': animatingState,
                },
            ]"
            :style="{ transform: windowTransform }"
            :inert="windowState !== 'open' || undefined"
        >
            <PreviewShell
                @minimize="setWindowState('minimized')"
                @zoom="toggleFullScreen"
                @quit="closeWindow"
            />
            <div ref="layerEl" class="launcher-preview-layer" />
        </div>
        <Transition name="launcher-preview-restore">
            <button
                v-if="windowState !== 'open'"
                type="button"
                class="absolute bottom-[6%] left-1/2 flex -translate-x-1/2 flex-col items-center gap-2 rounded-2xl border-0 bg-transparent p-2 text-sm font-semibold text-[var(--color-text-primary)] transition-transform hover:scale-105 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[var(--color-brand)]"
                :aria-label="formatMessage(shellMessages.windowReopen)"
                @click="setWindowState('open')"
            >
                <img
                    src="/axolotl.png"
                    alt=""
                    width="64"
                    height="64"
                    class="size-16 rounded-2xl shadow-lg"
                />
                <span class="rounded-full bg-black/50 px-3 py-1 text-xs text-white">
                    {{ formatMessage(shellMessages.windowReopen) }}
                </span>
            </button>
        </Transition>
    </div>
</template>

<style scoped>
.launcher-preview-window--animating {
    transition:
        opacity 300ms ease,
        transform 340ms cubic-bezier(0.22, 1, 0.36, 1);
}

.launcher-preview-window--hidden {
    opacity: 0;
    pointer-events: none;
}

.launcher-preview-restore-enter-active,
.launcher-preview-restore-leave-active {
    transition:
        opacity 200ms ease,
        transform 200ms ease;
}

.launcher-preview-restore-enter-from,
.launcher-preview-restore-leave-to {
    opacity: 0;
}
</style>
