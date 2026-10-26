<script setup lang="ts">
import { ChevronUpIcon } from '@modrinth/assets'
import { onBeforeUnmount, onMounted, ref } from 'vue'

import { defineMessages, useVIntl } from '../../composables/i18n'
import Button from './buttons/Button.vue'

const { formatMessage } = useVIntl()
const messages = defineMessages({
    tooltip: {
        id: 'ui.scroll-to-top.tooltip',
        defaultMessage: 'Scroll to top',
    },
})

const visible = ref(false)
let scrollContainer: Element | null = null

function update() {
    visible.value = (scrollContainer?.scrollTop ?? 0) > 300
}

function scrollToTop() {
    scrollContainer?.scrollTo({ top: 0, behavior: 'smooth' })
}

onMounted(() => {
    scrollContainer = document.querySelector('.app-viewport')
    if (scrollContainer) {
        scrollContainer.addEventListener('scroll', update, { passive: true })
        // Read the position with the next frame: reading it now would force a layout
        // of the whole page inside the task that mounts it.
        requestAnimationFrame(update)
    }
})

onBeforeUnmount(() => {
    scrollContainer?.removeEventListener('scroll', update)
})
</script>

<template>
    <Transition name="scroll-to-top">
        <div v-if="visible" class="scroll-to-top-wrapper">
            <Button
                v-tooltip="formatMessage(messages.tooltip)"
                type="colored"
                color="brand"
                size="xl"
                circular
                icon-only
                class="scroll-to-top-btn"
                :aria-label="formatMessage(messages.tooltip)"
                @click="scrollToTop"
                ><ChevronUpIcon aria-hidden="true" />
            </Button>
        </div>
    </Transition>
</template>

<style scoped>
.scroll-to-top-btn {
    @apply shadow-lg transition-all duration-200 hover:brightness-110 hover:shadow-xl active:scale-95;
}

.scroll-to-top-wrapper {
    @apply fixed bottom-10 left-24 z-50;
}

.scroll-to-top-enter-active,
.scroll-to-top-leave-active {
    transition:
        opacity 0.24s ease,
        transform 0.24s ease;
}

.scroll-to-top-enter-from,
.scroll-to-top-leave-to {
    opacity: 0;
    transform: translateY(10px);
}
</style>
