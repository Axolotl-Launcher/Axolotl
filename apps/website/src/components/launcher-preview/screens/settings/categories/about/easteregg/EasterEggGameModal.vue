<script setup lang="ts">
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { ref } from 'vue'

import PreviewModal from '../../../../../overlays/PreviewModal.vue'
import { messages } from '../../../messages'

/**
 * The hidden mini game a secret code opens on the About page
 * (apps/app-frontend/src/components/ui/easteregg/EasterEggGameModal.vue): the
 * app's own self-contained game page in a frame, loaded only while the modal
 * is open.
 */
const { formatMessage } = useVIntl()

const modal = ref<InstanceType<typeof PreviewModal> | null>(null)
const gameVisible = ref(false)

const gameUrl = '/launcher-preview/easteregg/game.html'

function show() {
    gameVisible.value = true
    modal.value?.show()
}

function onHide() {
    gameVisible.value = false
}

defineExpose({ show })
</script>

<template>
    <PreviewModal
        ref="modal"
        :header="formatMessage(messages.appSettingsAboutEastereggGameTitle)"
        width="min(832px, calc(1280px - 2rem))"
        max-width="832px"
        noblur
        :on-hide="onHide"
    >
        <div class="flex flex-col items-center py-2">
            <iframe
                v-if="gameVisible"
                :src="gameUrl"
                title="Mini Game"
                class="h-[600px] w-[800px] max-w-full rounded-xl border-none bg-black"
            />
        </div>
    </PreviewModal>
</template>
