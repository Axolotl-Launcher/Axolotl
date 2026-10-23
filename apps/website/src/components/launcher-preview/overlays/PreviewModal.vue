<template>
    <Teleport :to="layer" :disabled="!layer">
        <div v-if="open" class="modal-root">
            <div
                :class="[
                    'modal-overlay',
                    {
                        shown: visible,
                        noblur: effectiveNoblur,
                    },
                    computedFade,
                ]"
                @click="() => (closeOnClickOutside && closable ? hide() : {})"
            />
            <div class="modal-container" :class="{ shown: visible }">
                <div
                    ref="modalBodyRef"
                    role="dialog"
                    aria-modal="true"
                    :aria-labelledby="headerId"
                    tabindex="-1"
                    class="modal-body flex flex-col rounded-2xl border border-solid border-surface-5 bg-surface-3"
                    :class="{ 'modal-body-fill': fillContent }"
                    v-bind="$attrs"
                    @keydown="handleKeyDown"
                >
                    <div
                        v-if="!hideHeader"
                        data-modal-header
                        class="grid max-w-full grid-cols-[auto_min-content] items-center gap-4 border-0 border-b-[1px] border-solid border-surface-5 p-6"
                    >
                        <div class="flex min-w-0 items-center gap-3 text-wrap break-words">
                            <slot name="title">
                                <span
                                    v-if="header"
                                    :id="headerId"
                                    class="text-2xl font-semibold text-[var(--color-text-primary)]"
                                >
                                    {{ header }}
                                </span>
                            </slot>
                        </div>
                        <Button
                            v-if="closable"
                            v-tooltip="closeLabel"
                            circular
                            icon-only
                            :aria-label="closeLabel"
                            :disabled="disableClose"
                            @click="hide"
                            ><XIcon aria-hidden="true" />
                        </Button>
                    </div>

                    <Button
                        v-if="mergeHeader && closable"
                        v-tooltip="closeLabel"
                        circular
                        icon-only
                        class="!absolute right-4 top-4 z-10"
                        :aria-label="closeLabel"
                        :disabled="disableClose"
                        @click="hide"
                        ><XIcon aria-hidden="true" />
                    </Button>

                    <div v-if="scrollable" class="relative flex min-h-0 flex-1 flex-col">
                        <Transition
                            enter-active-class="transition-all duration-200 ease-out"
                            enter-from-class="opacity-0 max-h-0"
                            enter-to-class="opacity-100 max-h-6"
                            leave-active-class="transition-all duration-200 ease-in"
                            leave-from-class="opacity-100 max-h-6"
                            leave-to-class="opacity-0 max-h-0"
                        >
                            <div
                                v-if="showTopFade"
                                class="pointer-events-none absolute left-0 right-0 top-0 z-10 h-6 bg-gradient-to-b from-bg-raised to-transparent"
                            />
                        </Transition>

                        <div
                            ref="scrollContainer"
                            data-modal-content
                            :class="[
                                'min-h-0 flex-1',
                                noPadding ? '' : 'overflow-y-auto p-6 !pb-6',
                                { 'pt-12': mergeHeader && closable && !noPadding },
                            ]"
                            :style="noPadding ? {} : { maxHeight: maxContentHeight }"
                            @scroll="checkScrollState"
                        >
                            <slot />
                        </div>

                        <Transition
                            enter-active-class="transition-all duration-200 ease-out"
                            enter-from-class="opacity-0 max-h-0"
                            enter-to-class="opacity-100 max-h-6"
                            leave-active-class="transition-all duration-200 ease-in"
                            leave-from-class="opacity-100 max-h-6"
                            leave-to-class="opacity-0 max-h-0"
                        >
                            <div
                                v-if="showBottomFade"
                                class="pointer-events-none absolute bottom-0 left-0 right-0 z-10 h-6 bg-gradient-to-t from-bg-raised to-transparent"
                            />
                        </Transition>
                    </div>

                    <div
                        v-else
                        data-modal-content
                        :class="[
                            'min-h-0',
                            noPadding ? '' : 'overflow-y-auto p-6',
                            { 'pt-12': mergeHeader && closable && !noPadding },
                        ]"
                    >
                        <slot />
                    </div>

                    <div
                        v-if="$slots.actions"
                        :class="{
                            'border-0 border-t border-solid border-surface-5 pt-4': actionsDivider,
                        }"
                        class="p-4"
                    >
                        <slot name="actions" />
                    </div>
                </div>
            </div>
        </div>
    </Teleport>
</template>

<script setup lang="ts">
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { useModalStack } from '@modrinth/ui/src/composables/modal-stack.ts'
import { useScrollIndicator } from '@modrinth/ui/src/composables/scroll-indicator.ts'
import { injectModalBehavior } from '@modrinth/ui/src/providers/modal-behavior.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, nextTick, onUnmounted, ref } from 'vue'

import { injectPreviewWindow, WINDOW_HEIGHT, WINDOW_WIDTH } from '../lib/window'

/**
 * The shared `NewModal`, rendered inside the launcher window instead of the
 * page body: its overlay covers the window rather than the website, it leaves
 * the page scrollable, and it measures its entrance origin in window space.
 */
const { formatMessage } = useVIntl()
const { layer, toLocal } = injectPreviewWindow()
const modalBehavior = injectModalBehavior(null)
const {
    push: pushModal,
    pop: popModal,
    isTopmost: isTopmostModal,
    stackSize: modalStackSize,
} = useModalStack()

const props = withDefaults(
    defineProps<{
        noblur?: boolean
        closable?: boolean
        fade?: 'standard' | 'warning' | 'danger'
        closeOnEsc?: boolean
        closeOnClickOutside?: boolean
        header?: string
        hideHeader?: boolean
        onHide?: () => void
        beforeHide?: () => boolean
        onShow?: () => void
        mergeHeader?: boolean
        scrollable?: boolean
        maxContentHeight?: string
        noPadding?: boolean
        maxWidth?: string
        width?: string
        fillContent?: boolean
        disableClose?: boolean
        actionsDivider?: boolean
        onAfterHide?: () => void
    }>(),
    {
        noblur: undefined,
        closable: true,
        fade: 'standard',
        closeOnClickOutside: false,
        closeOnEsc: true,
        header: undefined,
        hideHeader: false,
        onHide: () => {},
        beforeHide: undefined,
        onShow: () => {},
        mergeHeader: false,
        scrollable: false,
        // The app's `70vh`, measured against the 800px launcher window.
        maxContentHeight: '560px',
        noPadding: false,
        maxWidth: undefined,
        width: undefined,
        fillContent: false,
        disableClose: false,
        actionsDivider: false,
        onAfterHide: undefined,
    },
)

const effectiveNoblur = computed(() => props.noblur ?? modalBehavior?.noblur.value ?? false)
const computedFade = computed(() => props.fade)

const modalId = `preview-modal-${Math.random().toString(36).slice(2, 9)}`
const headerId = `${modalId}-header`
const closeLabel = computed(() => formatMessage(commonMessages.closeButton))

const open = ref(false)
const visible = ref(false)
const stackDepth = ref(0)
const modalBodyRef = ref<HTMLElement | null>(null)
let previousFocusEl: Element | null = null
let phase: 'closed' | 'opening' | 'open' | 'closing' | 'disposed' = 'closed'
let transitionGeneration = 0
let showTimer: ReturnType<typeof setTimeout> | undefined
let hideTimer: ReturnType<typeof setTimeout> | undefined
let resolveHide: ((completed: boolean) => void) | undefined

function cancelTransitionTimers() {
    clearTimeout(showTimer)
    clearTimeout(hideTimer)
    showTimer = undefined
    hideTimer = undefined
    resolveHide?.(false)
    resolveHide = undefined
}

const scrollContainer = ref<HTMLElement | null>(null)
const { showTopFade, showBottomFade, checkScrollState } = useScrollIndicator(scrollContainer)

const FOCUSABLE_SELECTOR =
    'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'

function getFocusableElements(): HTMLElement[] {
    if (!modalBodyRef.value) return []
    return Array.from(modalBodyRef.value.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR))
}

const mouseX = ref(WINDOW_WIDTH / 2)
const mouseY = ref(WINDOW_HEIGHT / 2)

function updateMousePosition(event: { clientX: number; clientY: number }) {
    const point = toLocal(event.clientX, event.clientY)
    mouseX.value = point.x
    mouseY.value = point.y
}

function resetMousePosition() {
    mouseX.value = WINDOW_WIDTH / 2
    mouseY.value = WINDOW_HEIGHT / 2
}

function show(event?: MouseEvent) {
    if (phase === 'disposed' || phase === 'opening' || phase === 'open') return
    cancelTransitionTimers()
    const generation = ++transitionGeneration
    phase = 'opening'
    const wasEmpty = modalStackSize() === 0
    stackDepth.value = modalStackSize()
    open.value = true
    previousFocusEl = document.activeElement
    pushModal()

    window.addEventListener('keydown', handleWindowKeyDown)
    window.addEventListener('mousedown', updateMousePosition)
    if (event) updateMousePosition(event)
    else resetMousePosition()
    showTimer = setTimeout(() => {
        showTimer = undefined
        if (generation !== transitionGeneration || phase !== 'opening') return
        phase = 'open'
        visible.value = true
        nextTick(() => {
            if (generation !== transitionGeneration || phase !== 'open') return
            const focusable = getFocusableElements()
            if (focusable.length > 0) focusable[0].focus({ preventScroll: true })
            else modalBodyRef.value?.focus({ preventScroll: true })
        })
    }, 50)
    if (wasEmpty) modalBehavior?.onShow?.()
    if (generation === transitionGeneration) props.onShow?.()
}

async function hide() {
    if (phase !== 'opening' && phase !== 'open') return
    if (props.disableClose) return
    if (props.beforeHide?.() === false) return
    cancelTransitionTimers()
    const generation = ++transitionGeneration
    phase = 'closing'
    resetMousePosition()
    visible.value = false
    popModal()
    if (modalStackSize() === 0) modalBehavior?.onHide?.()
    window.removeEventListener('keydown', handleWindowKeyDown)
    window.removeEventListener('mousedown', updateMousePosition)
    if (previousFocusEl instanceof HTMLElement && previousFocusEl.isConnected) {
        previousFocusEl.focus({ preventScroll: true })
    }
    previousFocusEl = null
    const completion = new Promise<boolean>((resolve) => {
        resolveHide = resolve
        hideTimer = setTimeout(() => {
            hideTimer = undefined
            resolveHide = undefined
            resolve(true)
        }, 300)
    })
    props.onHide?.()
    const completed = await completion
    if (!completed || generation !== transitionGeneration || phase !== 'closing') return
    phase = 'closed'
    open.value = false
    await nextTick()
    if (generation !== transitionGeneration) return
    props.onAfterHide?.()
}

async function scrollToBottom(behavior: ScrollBehavior = 'smooth') {
    await nextTick()
    if (!scrollContainer.value) return
    scrollContainer.value.scrollTo({ top: scrollContainer.value.scrollHeight, behavior })
    requestAnimationFrame(checkScrollState)
}

defineExpose({ show, hide, checkScrollState, scrollToBottom })

const MODAL_STACK_BASE_Z = 100
const stackZBase = computed(() => MODAL_STACK_BASE_Z + stackDepth.value * 10)
const stackOverlayZ = computed(() => stackZBase.value + 19)
const stackContainerZ = computed(() => stackZBase.value + 21)
const resolvedMaxWidth = computed(() => props.maxWidth ?? '60rem')
const resolvedWidth = computed(() => props.width ?? 'fit-content')
const mouseXOffset = computed(() => `${(mouseX.value - WINDOW_WIDTH / 2) / 16}px`)
const mouseYOffset = computed(() => `${(mouseY.value - WINDOW_HEIGHT / 2) / 16}px`)

onUnmounted(() => {
    const wasActive = phase === 'opening' || phase === 'open'
    phase = 'disposed'
    transitionGeneration++
    cancelTransitionTimers()
    popModal()
    window.removeEventListener('keydown', handleWindowKeyDown)
    window.removeEventListener('mousedown', updateMousePosition)
    if (wasActive && modalStackSize() === 0) modalBehavior?.onHide?.()
})

function handleWindowKeyDown(event: KeyboardEvent) {
    if (event.defaultPrevented) return
    if (props.closeOnEsc && event.key === 'Escape' && props.closable) {
        setTimeout(() => {
            if (event.defaultPrevented || !isTopmostModal() || props.disableClose) return
            if (phase !== 'opening' && phase !== 'open') return
            event.preventDefault()
            hide()
        }, 0)
    }
}

function handleKeyDown(event: KeyboardEvent) {
    if (event.key !== 'Tab') return
    const focusable = getFocusableElements()
    if (focusable.length === 0) return
    const first = focusable[0]
    const last = focusable[focusable.length - 1]
    if (event.shiftKey && document.activeElement === first) {
        event.preventDefault()
        last.focus()
    } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault()
        first.focus()
    }
}

defineOptions({
    inheritAttrs: false,
})
</script>

<style lang="scss" scoped>
.modal-body-fill {
    > [data-modal-header] {
        flex: 0 0 auto;
    }

    > [data-modal-content] {
        display: flex;
        min-height: 0;
        flex: 1 1 0%;
        flex-direction: column;
        overflow: hidden;
    }

    > [data-modal-content] > * {
        min-height: 0;
    }
}

.modal-overlay {
    position: fixed;
    inset: -5rem;
    z-index: v-bind(stackOverlayZ);
    opacity: 0;
    visibility: hidden;
    pointer-events: none;
    transition: all 0.2s ease-out;
    border-radius: 180px;

    &.standard {
        background: linear-gradient(
            to bottom,
            color-mix(in srgb, var(--color-brand-shadow) 10%, rgba(0, 0, 0, 0.52)) 0%,
            color-mix(in srgb, var(--color-brand-shadow) 2%, rgba(0, 0, 0, 0.95)) 100%
        );
    }

    &.warning {
        background: linear-gradient(
            to bottom,
            rgba(48, 38, 29, 0.52) 0%,
            rgba(26, 20, 14, 0.95) 100%
        );
    }

    &.danger {
        background: linear-gradient(
            to bottom,
            rgba(43, 18, 26, 0.52) 0%,
            rgba(49, 10, 15, 0.95) 100%
        );
    }

    @media (prefers-reduced-motion) {
        transition: none !important;
    }

    &.shown {
        opacity: 1;
        visibility: visible;
        pointer-events: auto;
        backdrop-filter: blur(5px);
    }

    &.noblur {
        backdrop-filter: none;
        filter: none;
    }
}

.modal-container {
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    display: flex;
    justify-content: center;
    align-items: center;
    z-index: v-bind(stackContainerZ);
    visibility: hidden;
    pointer-events: none;
    transform: translate(v-bind(mouseXOffset), v-bind(mouseYOffset));
    transition: none;

    &.shown {
        visibility: visible;
        transform: translate(0, 0);
        transition: all 0.2s ease-out;

        > .modal-body {
            opacity: 1;
            visibility: visible;
            scale: 1;
        }
    }

    > .modal-body {
        position: fixed;
        box-shadow: 4px 4px 26px 10px rgba(0, 0, 0, 0.08);
        max-height: calc(100% - 2 * var(--gap-lg));
        max-width: min(v-bind(resolvedMaxWidth), calc(100% - 2 * var(--gap-lg)));
        overflow-y: hidden;
        overflow-x: hidden;
        width: v-bind(resolvedWidth);
        pointer-events: auto;
        scale: 0.97;
        visibility: hidden;
        opacity: 0;
        transition: all 0.2s ease-in-out;

        &:focus {
            outline: none;
        }

        @media (prefers-reduced-motion) {
            transition: none !important;
        }
    }
}
</style>
