<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectPopupNotificationManager } from '@modrinth/ui/src/providers/popup-notifications.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'

import { usePreview } from '../../../../lib/state/index'
import { injectPreviewWindow } from '../../../../lib/window'
import LauncherSplash from '../../../../shared/LauncherSplash.vue'

/** The restart overlay outlives Settings while the configured first page becomes ready. */
const emit = defineEmits<{ close: [] }>()
const router = useRouter()
const { settings } = usePreview()
const { waitUntilReady } = injectPreviewWindow()
const notifications = injectNotificationManager()
const popups = injectPopupNotificationManager()
const { formatMessage } = useVIntl()
const messages = defineMessages({
    loading: { id: 'axolotl-preview.loading', defaultMessage: 'Loading Axolotl Launcher' },
    failed: { id: 'axolotl-preview.failed', defaultMessage: 'The launcher could not load.' },
    retry: { id: 'axolotl-preview.retry', defaultMessage: 'Retry' },
})

const doneLoading = ref(false)
const loadingProgress = ref(0)
const failed = ref(false)
let active = false
let disposed = false

async function restart() {
    if (active || disposed) return
    active = true
    failed.value = false
    loadingProgress.value = 0
    notifications.clearAllNotifications()
    popups.clearAllNotifications()
    try {
        await router.replace(settings.default_page === 'Library' ? '/library' : '/')
        if (disposed) return
        await waitUntilReady((progress) => {
            if (!disposed) loadingProgress.value = progress
        })
        if (disposed) return
        loadingProgress.value = 100
        doneLoading.value = true
    } catch {
        if (!disposed) failed.value = true
    } finally {
        active = false
    }
}

onMounted(restart)
onBeforeUnmount(() => {
    disposed = true
})
</script>

<template>
    <Transition name="splash-fade" @after-leave="emit('close')">
        <div v-if="!doneLoading" class="fixed inset-0 z-[10000]">
            <LauncherSplash
                :progress="loadingProgress"
                :loading-label="formatMessage(messages.loading)"
                :failed="failed"
                :failure-label="formatMessage(messages.failed)"
                :retry-label="formatMessage(messages.retry)"
                @retry="restart"
            />
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

@media (prefers-reduced-motion: reduce) {
    .splash-fade-leave-active {
        transition-duration: 0.01ms;
    }
}
</style>
