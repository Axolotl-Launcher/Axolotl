<script setup lang="ts">
import { useModalStack } from '@modrinth/ui/src/composables/modal-stack.ts'
import { computed, onUnmounted, ref, watch } from 'vue'

import { usePreviewShell } from '../lib/shell'
import { injectPreviewWindow } from '../lib/window'

/**
 * The shared `FloatingActionBar`, positioned inside the launcher window between
 * the navigation rail and the sidebar instead of across the website viewport.
 */
const props = defineProps<{
    shown: boolean
    ariaLabel?: string
    hideWhenModalOpen?: boolean
    position?: 'bottom' | 'top'
    allowOverflow?: boolean
}>()

const { layer } = injectPreviewWindow()
const shell = usePreviewShell()
const { stackCount } = useModalStack()

const toolbarEl = ref<HTMLElement | null>(null)
const compact = ref(false)

const shown = computed(() => props.shown && (!props.hideWhenModalOpen || stackCount.value === 0))
const barStyle = computed(() => ({
    zIndex: 100 + stackCount.value * 10 + 7,
    left: 'var(--left-bar-width, 4rem)',
    right: shell.sidebarVisible.value ? '300px' : '0px',
}))

function checkCompact() {
    const el = toolbarEl.value
    if (!el?.parentElement) return
    const clone = el.cloneNode(true) as HTMLElement
    clone.classList.remove('bar-compact')
    clone.style.position = 'absolute'
    clone.style.visibility = 'hidden'
    clone.style.pointerEvents = 'none'
    clone.style.width = `${el.offsetWidth}px`
    el.parentElement.appendChild(clone)
    compact.value = clone.offsetHeight > 70
    clone.remove()
}

let observer: ResizeObserver | null = null
watch(
    toolbarEl,
    (el) => {
        observer?.disconnect()
        if (!el?.parentElement) return
        observer = new ResizeObserver(checkCompact)
        observer.observe(el.parentElement)
        checkCompact()
    },
    { immediate: true },
)

onUnmounted(() => observer?.disconnect())
</script>

<template>
    <Teleport :to="layer" :disabled="!layer">
        <Transition name="floating-action-bar" appear>
            <div
                v-if="shown"
                class="floating-action-bar pointer-events-none fixed p-4 drop-shadow-2xl"
                :class="position === 'top' ? 'top-12' : 'bottom-0'"
                :style="barStyle"
                aria-live="polite"
            >
                <div
                    ref="toolbarEl"
                    role="toolbar"
                    :aria-label="ariaLabel"
                    class="pointer-events-auto relative mx-auto flex max-w-[768px] items-center gap-1.5 rounded-[var(--radius-xl)] border border-solid border-surface-5 bg-surface-3 px-3 py-2.5 shadow-[0px_1px_3px_0px_rgba(0,0,0,0.3),0px_6px_10px_0px_rgba(0,0,0,0.15)]"
                    :class="{
                        'overflow-visible': allowOverflow,
                        'overflow-clip': !allowOverflow,
                        'bar-compact': compact,
                    }"
                >
                    <slot />
                </div>
            </div>
        </Transition>
    </Teleport>
</template>

<style scoped>
.floating-action-bar {
    transition:
        bottom 0.25s ease-in-out,
        top 0.25s ease-in-out,
        right 0.32s cubic-bezier(0.22, 1, 0.36, 1);
}

.floating-action-bar-enter-active {
    transition:
        transform 0.25s cubic-bezier(0.15, 1.4, 0.64, 0.96),
        opacity 0.25s cubic-bezier(0.15, 1.4, 0.64, 0.96);
}

.floating-action-bar-leave-active {
    transition:
        transform 0.25s ease,
        opacity 0.25s ease;
}

.floating-action-bar-enter-from {
    transform: scale(0.5) translateY(-10rem);
    opacity: 0;
}

.floating-action-bar-leave-to {
    transform: scale(0.96) translateY(-0.25rem);
    opacity: 0;
}
</style>

<style>
.launcher-preview-window .bar-compact .bar-label {
    display: none;
}

.launcher-preview-window .bar-compact .cq-show-icon {
    display: block;
}
</style>
