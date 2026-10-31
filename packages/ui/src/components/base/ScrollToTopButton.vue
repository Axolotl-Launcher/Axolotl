<script setup lang="ts">
import { ChevronUpIcon } from '@modrinth/assets'
import { onBeforeUnmount, onMounted, useId } from 'vue'

import { defineMessages, useVIntl } from '../../composables/i18n'
import { useScrollToTopVisibility } from '../../composables/scroll-to-top-visibility'
import Button from './buttons/Button.vue'

const { formatMessage } = useVIntl()
const messages = defineMessages({
    tooltip: {
        id: 'ui.scroll-to-top.tooltip',
        defaultMessage: 'Scroll to top',
    },
})

const componentId = useId()
let scrollContainer: Element | null = null
const { visible, update } = useScrollToTopVisibility(() => scrollContainer)

function scrollToTop() {
    scrollContainer?.scrollTo({ top: 0, behavior: 'smooth' })
}

onMounted(() => {
    scrollContainer = document.querySelector('.app-viewport')
    if (scrollContainer) {
        scrollContainer.addEventListener('scroll', update, { passive: true })
        update()
    }
})

onBeforeUnmount(() => {
    scrollContainer?.removeEventListener('scroll', update)
})
</script>

<template>
    <Transition name="scroll-to-top">
        <div v-if="visible" :data-scroll-to-top="componentId" class="scroll-to-top-wrapper">
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
    position: fixed;
    right: calc(var(--right-bar-width, 0px) + 1.5rem);
    bottom: calc(max(4rem, 4dvh) + var(--floating-action-bar-clearance, 0px));
    z-index: 60;
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
