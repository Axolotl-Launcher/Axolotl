<script setup lang="ts">
import ProgressBar from '@modrinth/ui/src/components/base/ProgressBar.vue'
import { injectPopupNotificationManager } from '@modrinth/ui/src/providers/popup-notifications.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'

import { usePreview } from '../../../../lib/state/index'
import axolotlLogo from './splash/axolotl.png'
import cubeBackground from './splash/cube.png'

/**
 * The launcher starting again after `restart_app`: the app's splash screen
 * (apps/app-frontend/src/components/ui/SplashScreen.vue) over the new window
 * while it initializes, which then opens on the default page. Rendered by the
 * shell's app-overlay host, so it outlives the settings page it replaces.
 * The new process starts without the old one's toasts.
 */
const emit = defineEmits<{ close: [] }>()

const router = useRouter()
const { clock, settings } = usePreview()
const notifications = injectNotificationManager()
const popups = injectPopupNotificationManager()

const doneLoading = ref(false)
const loadingProgress = ref(0)

const MIN_DISPLAY_MS = 500
/** Time the relaunched launcher spends initializing its state and first page. */
const STARTUP_MS = 1100

let mountedAt = 0
const cancels: Array<() => void> = []

function fakeLoadingIncrease() {
    if (loadingProgress.value < 95) {
        cancels.push(
            clock.after(5, () => {
                loadingProgress.value += 2
                fakeLoadingIncrease()
            }),
        )
    }
}

onMounted(() => {
    mountedAt = Date.now()
    notifications.clearAllNotifications()
    popups.clearAllNotifications()
    // The new window starts on the default page, behind the splash.
    void router.replace(settings.default_page === 'Library' ? '/library' : '/')
    fakeLoadingIncrease()
    cancels.push(
        clock.after(STARTUP_MS, () => {
            const delay = Math.max(0, MIN_DISPLAY_MS - (Date.now() - mountedAt))
            cancels.push(
                clock.after(delay, () => {
                    doneLoading.value = true
                }),
            )
        }),
    )
})

onBeforeUnmount(() => {
    for (const cancel of cancels) cancel()
})
</script>

<template>
    <Transition name="splash-fade" @after-leave="emit('close')">
        <div v-if="!doneLoading" class="dark fixed inset-0 z-[10000]">
            <div
                class="absolute z-[9998] flex h-full w-full flex-col items-center justify-center gap-4"
            >
                <img class="app-logo" :src="axolotlLogo" alt="Axolotl Launcher" />
                <ProgressBar
                    class="max-w-xs"
                    :progress="Math.min(loadingProgress, 100)"
                    :max="100"
                />
            </div>
            <div class="gradient-bg"></div>
            <div class="cube-bg" :style="{ backgroundImage: `url(${cubeBackground})` }"></div>
            <div class="absolute left-0 top-0 z-[9995] h-full w-full bg-bg"></div>
        </div>
    </Transition>
</template>

<style scoped>
.splash-fade-leave-active {
    transition: opacity 0.3s ease-in-out;
}

.splash-fade-leave-to {
    opacity: 0;
}

/* `min(18rem, 45vh)` and `min(18rem, 45vw)` of the 1280 × 800 window. */
.app-logo {
    height: 18rem;
    width: 18rem;
    object-fit: contain;
}

.gradient-bg {
    position: absolute;
    height: 800px;
    width: 1280px;
    background:
        linear-gradient(180deg, rgba(255, 77, 157, 0.24) 0%, rgba(48, 16, 40, 0.56) 97.29%),
        linear-gradient(0deg, rgba(22, 18, 28, 0.68), rgba(22, 18, 28, 0.68));
    z-index: 9997;
}

.cube-bg {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 2304px;
    height: 1440px;
    opacity: 0.8;
    background-color: var(--surface-1);
    background-position: center;
    background-repeat: no-repeat;
    background-size: contain;
    z-index: 9996;
}
</style>
